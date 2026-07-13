use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const READ_TIMEOUT_SECS: u64 = 8;
const WRITE_TIMEOUT_SECS: u64 = 8;

/// Build a Palworld RCON packet: size (LE), id (LE), type (LE), body (ASCII), 2-byte null terminator.
fn create_packet(packet_type: i32, id: i32, body: &str) -> Vec<u8> {
    let body_bytes = body.as_bytes();
    let size = body_bytes.len() + 14;
    let mut buffer = vec![0u8; size];

    let size_field = (size - 4) as i32;
    buffer[0..4].copy_from_slice(&size_field.to_le_bytes());
    buffer[4..8].copy_from_slice(&id.to_le_bytes());
    buffer[8..12].copy_from_slice(&packet_type.to_le_bytes());
    buffer[12..12 + body_bytes.len()].copy_from_slice(body_bytes);
    buffer
}

fn read_response(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buf = [0u8; 8192];
    let n = stream
        .read(&mut buf)
        .context("Failed to read Palworld RCON response")?;
    Ok(buf[..n].to_vec())
}

/// Authenticate and send a Palworld RCON command.
pub fn send_command(host: &str, port: u16, password: &str, command: &str) -> Result<String> {
    let addr = format!("{}:{}", host.trim(), port);
    let mut stream = TcpStream::connect(&addr)
        .with_context(|| format!("Palworld RCON connect failed: {}", addr))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
        .context("Failed to set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(WRITE_TIMEOUT_SECS)))
        .context("Failed to set write timeout")?;

    let auth_packet = create_packet(3, 1, password);
    stream
        .write_all(&auth_packet)
        .context("Failed to send Palworld RCON auth packet")?;

    let _ = read_response(&mut stream).context("Failed to read Palworld RCON auth response")?;

    let cmd_packet = create_packet(2, 2, command);
    stream
        .write_all(&cmd_packet)
        .context("Failed to send Palworld RCON command packet")?;

    let response = read_response(&mut stream).context("Failed to read Palworld RCON command response")?;
    Ok(String::from_utf8_lossy(&response).to_string())
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
