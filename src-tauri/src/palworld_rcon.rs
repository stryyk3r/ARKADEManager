use anyhow::{Context, Result};
use mc_rcon::RconClient;
use std::io;
use std::net::ToSocketAddrs;

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().to_lowercase();
    h == "127.0.0.1" || h == "localhost" || h == "::1"
}

fn format_connect_error(addr: &str, err: &io::Error) -> String {
    let detail = match err.kind() {
        io::ErrorKind::ConnectionRefused => {
            "Connection refused. Confirm RCONEnabled=True in PalWorldSettings.ini, the server is running, and the RCON port is open in the firewall."
        }
        io::ErrorKind::TimedOut => {
            "Connection timed out. Check that this machine can reach the RCON port (firewall/security group)."
        }
        io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkUnreachable => {
            "Network unreachable. Verify the RCON host address is correct."
        }
        _ => "Check the RCON host, port, and network path.",
    };

    format!(
        "Palworld RCON connect failed: {} ({detail}). {hint} Use the AdminPassword from PalWorldSettings.ini as the RCON password. If ARKADE Manager runs on the same machine as the server, try 127.0.0.1 instead of the public IP.",
        addr,
        detail = err,
        hint = detail
    )
}

fn connect_rcon(host: &str, port: u16) -> Result<RconClient> {
    let host = host.trim();
    let primary_addr = format!("{host}:{port}");

    match RconClient::connect(&primary_addr) {
        Ok(client) => return Ok(client),
        Err(primary_err) => {
            if is_loopback_host(host) {
                return Err(anyhow::anyhow!(format_connect_error(&primary_addr, &primary_err)));
            }

            let fallback_addr = format!("127.0.0.1:{port}");
            log::warn!(
                "Palworld RCON connect to {} failed ({}); retrying {}",
                primary_addr,
                primary_err,
                fallback_addr
            );

            match RconClient::connect(&fallback_addr) {
                Ok(client) => {
                    log::info!(
                        "Palworld RCON connected via {} (configured host {} was unreachable from this machine)",
                        fallback_addr,
                        host
                    );
                    Ok(client)
                }
                Err(_) => Err(anyhow::anyhow!(format_connect_error(
                    &primary_addr,
                    &primary_err
                ))),
            }
        }
    }
}

/// Authenticate and send a Palworld RCON command using Source RCON (same protocol as Minecraft).
pub fn send_command(host: &str, port: u16, password: &str, command: &str) -> Result<String> {
    let addr = format!("{}:{}", host.trim(), port);
    // Validate address resolves early for clearer errors.
    addr.to_socket_addrs()
        .with_context(|| format!("Invalid Palworld RCON address: {addr}"))?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve Palworld RCON address: {addr}"))?;

    let client = connect_rcon(host, port)?;

    client.log_in(password).map_err(|e| {
        anyhow::anyhow!(
            "Palworld RCON authentication failed for {}: {}. Verify the RCON password matches AdminPassword in PalWorldSettings.ini.",
            addr,
            e
        )
    })?;

    client
        .send_command(command)
        .map_err(|e| anyhow::anyhow!("Palworld RCON command '{command}' failed on {addr}: {e}"))
}

/// Send the Save command to flush world state to disk before backup.
pub fn send_save(host: &str, port: u16, password: &str) -> Result<()> {
    log::info!(
        "Palworld RCON: sending Save command to {}:{}",
        host.trim(),
        port
    );
    let response = send_command(host, port, password, "Save")?;
    log::info!("Palworld RCON Save response: {}", response.trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_detected() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("213.239.211.202"));
    }
}
