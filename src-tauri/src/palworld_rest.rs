use crate::server_ini::PalworldRestSettings;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().to_lowercase();
    h == "127.0.0.1" || h == "localhost" || h == "::1"
}

fn format_rest_error(host: &str, port: u16, detail: &str) -> String {
    format!(
        "Palworld REST save failed for http://{host}:{port}/v1/api/save — {detail}. \
         Confirm RESTAPIEnabled=True and AdminPassword are set in PalWorldSettings.ini. \
         If ARKADE Manager runs on the same machine as the server, use 127.0.0.1 as the API host."
    )
}

fn describe_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "timed out after 15s (server not running, RESTAPIEnabled=False until restart, or wrong port)"
            .to_string()
    } else if err.is_connect() {
        format!("connection refused ({err})")
    } else {
        err.to_string()
    }
}

async fn post_save(host: &str, port: u16, password: &str) -> Result<()> {
    let host = host.trim();
    let url = format!("http://{host}:{port}/v1/api/save");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("Failed to build HTTP client for Palworld REST save")?;

    let response = client
        .post(&url)
        .basic_auth("admin", Some(password))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow!(format_rest_error(host, port, &describe_reqwest_error(&e))))?;

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        anyhow::bail!(format_rest_error(
            host,
            port,
            "authentication failed (check AdminPassword in PalWorldSettings.ini)"
        ));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(format_rest_error(
            host,
            port,
            &format!("HTTP {} {}", status.as_u16(), body.trim())
        ));
    }

    log::info!("Palworld REST save succeeded: {}", url);
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

    log::info!(
        "Palworld REST: sending save to {}:{}",
        primary_host, port
    );

    match post_save(primary_host, port, password).await {
        Ok(()) => Ok(()),
        Err(primary_err) if !is_loopback_host(primary_host) => {
            log::warn!(
                "Palworld REST save to {}:{} failed ({}); retrying 127.0.0.1",
                primary_host,
                port,
                primary_err
            );
            post_save("127.0.0.1", port, password).await
        }
        Err(e) => Err(e),
    }
}
