use crate::server_ini::PalworldRestSettings;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().to_lowercase();
    h == "127.0.0.1" || h == "localhost" || h == "::1"
}

fn format_rest_error(host: &str, port: u16, detail: &str) -> String {
    format!(
        "Palworld REST save failed for http://{host}:{port}/v1/api/save — {detail}. \
         Confirm the Palworld process is running, RESTAPIEnabled=True, RESTAPIPort matches, \
         and AdminPassword is set in PalWorldSettings.ini. If ARKADE Manager is on the same machine, leave API Host blank."
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
            "connection refused (nothing is listening on this host/port — Palworld may be down, or RESTAPIPort is wrong)".to_string()
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

    let connect = timeout(Duration::from_secs(8), TcpStream::connect(&addr));
    let mut stream = match connect.await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => anyhow::bail!(format_rest_error(host, port, &describe_connect_error(&e))),
        Err(_) => anyhow::bail!(format_rest_error(
            host,
            port,
            "timed out connecting after 8s (Palworld not running, REST not listening, or firewall)"
        )),
    };

    timeout(Duration::from_secs(8), stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| anyhow!(format_rest_error(host, port, "timed out sending save request")))?
        .with_context(|| format_rest_error(host, port, "failed writing save request"))?;

    let mut buf = Vec::new();
    match timeout(Duration::from_secs(15), stream.read_to_end(&mut buf)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => anyhow::bail!(format_rest_error(
            host,
            port,
            &format!("connection dropped while waiting for reply ({e})")
        )),
        Err(_) => anyhow::bail!(format_rest_error(
            host,
            port,
            "timed out waiting for save reply (server may still be writing the world)"
        )),
    }

    let response = String::from_utf8_lossy(&buf);
    let status = parse_status_line(&response).unwrap_or(0);
    if status == 401 || status == 403 {
        anyhow::bail!(format_rest_error(
            host,
            port,
            "authentication failed (check AdminPassword in PalWorldSettings.ini)"
        ));
    }
    if !(200..300).contains(&status) {
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").trim();
        anyhow::bail!(format_rest_error(
            host,
            port,
            &format!("HTTP {status} {}", body)
        ));
    }

    log::info!("Palworld REST save succeeded: http://{host}:{port}/v1/api/save");
    Ok(())
}

/// Flush world state via Palworld REST API before backup.
pub async fn send_save(settings: &PalworldRestSettings, host_override: Option<&str>) -> Result<()> {
    if !settings.rest_api_enabled {
        anyhow::bail!(
            "Palworld REST API is disabled. Set RESTAPIEnabled=True in PalWorldSettings.ini before running backups."
        );
    }
    if settings.admin_password.trim().is_empty() {
        anyhow::bail!(
            "Palworld AdminPassword is empty in PalWorldSettings.ini. REST save requires a non-empty admin password."
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
