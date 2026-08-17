use crate::server_ini::PalworldRestSettings;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const WRITE_TIMEOUT: Duration = Duration::from_secs(8);
/// Large Palworld worlds can take a while to flush; also cover slow replies.
const READ_TIMEOUT: Duration = Duration::from_secs(90);

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().to_lowercase();
    h == "127.0.0.1" || h == "localhost" || h == "::1"
}

fn format_rest_error(host: &str, port: u16, detail: &str) -> String {
    format!(
        "Palworld REST save failed for http://{host}:{port}/v1/api/save — {detail}. \
         Confirm the Palworld process is running, and that Pal/Saved/Config/WindowsServer/PalWorldSettings.ini \
         has RESTAPIEnabled=True, a matching RESTAPIPort, and a non-empty AdminPassword (then restart the server). \
         If ARKADE Manager is on the same machine, leave API Host blank."
    )
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if i + 1 < input.len() {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < input.len() {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

fn basic_auth_header(password: &str) -> String {
    format!("Basic {}", encode_base64(format!("admin:{password}").as_bytes()))
}

fn describe_connect_error(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::ConnectionRefused => {
            "connection refused (nothing is listening on this host/port — Palworld may be down, RESTAPIEnabled=False, or RESTAPIPort is wrong)".to_string()
        }
        std::io::ErrorKind::TimedOut => {
            "timed out connecting (firewall, wrong host, or REST not bound to this address)".to_string()
        }
        _ => format!("could not connect ({err})"),
    }
}

fn parse_status_line(response: &str) -> Option<u16> {
    let first = response.lines().next()?.trim();
    let mut parts = first.split_whitespace();
    let _version = parts.next()?;
    parts.next()?.parse().ok()
}

fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    for line in headers.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case(name) {
                return Some(value.trim());
            }
        }
    }
    None
}

/// Returns Some(complete_response) once headers (+ body) are fully received.
fn try_complete_http_response(buf: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(buf);
    let header_end = text.find("\r\n\r\n").or_else(|| text.find("\n\n"))?;
    let (headers, body_sep_len) = if text[header_end..].starts_with("\r\n\r\n") {
        (&text[..header_end], 4)
    } else {
        (&text[..header_end], 2)
    };
    let body_start = header_end + body_sep_len;
    let body = &text[body_start..];

    if header_value(headers, "Transfer-Encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        // Treat a terminating zero chunk as complete.
        if body.contains("\r\n0\r\n\r\n") || body.ends_with("0\r\n\r\n") || body.trim_end().ends_with("0") {
            return Some(text.into_owned());
        }
        // Empty chunked responses sometimes just have headers; if no body bytes expected yet keep reading
        // unless Content-Length is also present (handled below).
    }

    if let Some(len_str) = header_value(headers, "Content-Length") {
        let len: usize = len_str.parse().ok()?;
        if body.as_bytes().len() >= len {
            return Some(text.into_owned());
        }
        return None;
    }

    // No Content-Length / not clearly chunked: headers alone are enough for empty POST replies
    // (Palworld often returns 200 with an empty or tiny JSON body and keeps the socket open).
    Some(text.into_owned())
}

async fn read_http_response(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];

    let deadline = tokio::time::Instant::now() + READ_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            // If we already got a usable status line, accept it rather than failing a successful save.
            if let Some(response) = try_complete_http_response(&buf).or_else(|| {
                let text = String::from_utf8_lossy(&buf);
                if parse_status_line(&text).is_some() {
                    Some(text.into_owned())
                } else {
                    None
                }
            }) {
                return Ok(response);
            }
            return Err(
                "timed out waiting for save reply (server may still be writing the world)".to_string(),
            );
        }

        match timeout(remaining, stream.read(&mut tmp)).await {
            Ok(Ok(0)) => {
                if buf.is_empty() {
                    return Err("connection closed with no reply".to_string());
                }
                return Ok(String::from_utf8_lossy(&buf).into_owned());
            }
            Ok(Ok(n)) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(response) = try_complete_http_response(&buf) {
                    return Ok(response);
                }
            }
            Ok(Err(e)) => {
                if !buf.is_empty() {
                    if let Some(response) = try_complete_http_response(&buf).or_else(|| {
                        let text = String::from_utf8_lossy(&buf);
                        parse_status_line(&text).map(|_| text.into_owned())
                    }) {
                        return Ok(response);
                    }
                }
                return Err(format!("connection dropped while waiting for reply ({e})"));
            }
            Err(_) => {
                if let Some(response) = try_complete_http_response(&buf).or_else(|| {
                    let text = String::from_utf8_lossy(&buf);
                    parse_status_line(&text).map(|_| text.into_owned())
                }) {
                    return Ok(response);
                }
                return Err(
                    "timed out waiting for save reply (server may still be writing the world)".to_string(),
                );
            }
        }
    }
}

async fn post_save(host: &str, port: u16, password: &str) -> Result<()> {
    let host = host.trim();
    let addr = format!("{host}:{port}");
    let auth = basic_auth_header(password);
    let request = format!(
        "POST /v1/api/save HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Authorization: {auth}\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         Content-Length: 0\r\n\
         \r\n"
    );

    let connect = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr));
    let mut stream = match connect.await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => anyhow::bail!(format_rest_error(host, port, &describe_connect_error(&e))),
        Err(_) => anyhow::bail!(format_rest_error(
            host,
            port,
            "timed out connecting after 8s (Palworld not running, REST not listening, or firewall)"
        )),
    };

    timeout(WRITE_TIMEOUT, stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| anyhow!(format_rest_error(host, port, "timed out sending save request")))?
        .with_context(|| format_rest_error(host, port, "failed writing save request"))?;

    let _ = stream.flush().await;

    let response = match read_http_response(&mut stream).await {
        Ok(response) => response,
        Err(detail) => anyhow::bail!(format_rest_error(host, port, &detail)),
    };

    let status = parse_status_line(&response).unwrap_or(0);
    if status == 401 || status == 403 {
        anyhow::bail!(format_rest_error(
            host,
            port,
            "authentication failed (check AdminPassword in Pal/Saved/Config/WindowsServer/PalWorldSettings.ini)"
        ));
    }
    if status == 0 {
        anyhow::bail!(format_rest_error(
            host,
            port,
            &format!("unreadable HTTP reply: {}", response.chars().take(200).collect::<String>())
        ));
    }
    if !(200..300).contains(&status) {
        let body = response
            .split("\r\n\r\n")
            .nth(1)
            .or_else(|| response.split("\n\n").nth(1))
            .unwrap_or("")
            .trim();
        anyhow::bail!(format_rest_error(
            host,
            port,
            &format!("HTTP {status} {}", body)
        ));
    }

    log::info!("Palworld REST save succeeded: http://{host}:{port}/v1/api/save (HTTP {status})");
    Ok(())
}

/// Flush world state via Palworld REST API before backup.
pub async fn send_save(settings: &PalworldRestSettings, host_override: Option<&str>) -> Result<()> {
    if !settings.rest_api_enabled {
        anyhow::bail!(
            "Palworld REST API is disabled in Pal/Saved/Config/WindowsServer/PalWorldSettings.ini \
             (RESTAPIEnabled=False). Set RESTAPIEnabled=True, set AdminPassword, then restart the server."
        );
    }
    if settings.admin_password.trim().is_empty() {
        anyhow::bail!(
            "Palworld AdminPassword is empty in Pal/Saved/Config/WindowsServer/PalWorldSettings.ini. \
             REST save requires a non-empty admin password, then restart the server."
        );
    }

    let port = settings.rest_api_port;
    let password = settings.admin_password.trim();
    let primary_host = host_override
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .unwrap_or("127.0.0.1");

    log::info!("Palworld REST: sending save to {primary_host}:{port}");

    match post_save(primary_host, port, password).await {
        Ok(()) => Ok(()),
        Err(primary_err) if !is_loopback_host(primary_host) => {
            log::warn!(
                "Palworld REST save to {primary_host}:{port} failed ({primary_err}); retrying 127.0.0.1"
            );
            post_save("127.0.0.1", port, password).await
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_when_headers_and_content_length_present() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        let response = try_complete_http_response(raw).unwrap();
        assert!(response.contains("200 OK"));
    }

    #[test]
    fn completes_empty_body_without_content_length() {
        // Palworld often answers save with headers only / keep-alive.
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
        let response = try_complete_http_response(raw).unwrap();
        assert_eq!(parse_status_line(&response), Some(200));
    }

    #[test]
    fn waits_for_content_length_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n";
        assert!(try_complete_http_response(raw).is_none());
    }
}
