use crate::job::Job;
use anyhow::{Context, Result};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Returns true if the job has RCON settings (Minecraft save-off/save-on flow).
pub(crate) fn job_has_rcon(job: &Job) -> bool {
    job.job_type == "minecraft"
        && job.rcon_host.as_ref().map(|h| !h.trim().is_empty()).unwrap_or(false)
        && job.rcon_port.map(|p| p > 0).unwrap_or(false)
        && job.rcon_password.as_ref().map(|p| !p.is_empty()).unwrap_or(false)
}

/// Run RCON commands in a blocking task. Used for save-off, save-all flush, save-on and SaveWorld.
/// Every network step has a timeout, so an unresponsive server returns an error instead of
/// blocking the backup (and with it the scheduler) forever.
/// All commands are attempted; if any of them fails, an error naming the failed commands is returned.
pub(crate) fn run_rcon_commands(host: String, port: u16, password: String, commands: Vec<&'static str>) -> Result<()> {
    let mut client = connect_and_log_in(&host, port, &password)?;
    let mut failures = Vec::new();
    for cmd in commands {
        if let Err(e) = client.send_command(cmd) {
            log::warn!("RCON command '{}' failed: {:#}", cmd, e);
            failures.push(format!("'{}': {:#}", cmd, e));
        }
    }
    if !failures.is_empty() {
        anyhow::bail!("RCON command(s) failed: {}", failures.join("; "));
    }
    Ok(())
}

fn connect_and_log_in(host: &str, port: u16, password: &str) -> Result<RconClient> {
    let addr = format!("{}:{}", host.trim(), port);
    let mut client = RconClient::connect(&addr, DEFAULT_TIMEOUTS)
        .map_err(|e| anyhow::Error::new(RconConnectError(format!("{:#}", e))))?;
    client.log_in(&password).context("RCON login failed")?;
    Ok(client)
}

/// Stop Minecraft from writing the world (`save-off`), then write everything pending to disk
/// (`save-all flush`), checking that the server confirms each step. Copying the world is only
/// safe once both are confirmed; on error the caller's `SaveOnGuard` re-enables saving.
pub(crate) fn pause_minecraft_saving(host: String, port: u16, password: String) -> Result<()> {
    let mut client = connect_and_log_in(&host, port, &password)?;

    let reply = client.send_command("save-off").context("RCON command 'save-off' failed")?;
    log::info!("RCON save-off reply: {}", reply.trim());
    check_save_off_reply(&reply)?;

    let reply = client
        .send_command("save-all flush")
        .context("RCON command 'save-all flush' failed")?;
    log::info!("RCON save-all flush reply: {}", reply.trim());
    check_flush_reply(&reply)
}

/// Saving must be off before the copy. "Already turned off" is accepted (the world is not being
/// written either way) but logged, since something other than this backup turned it off.
fn check_save_off_reply(reply: &str) -> Result<()> {
    let lower = reply.to_ascii_lowercase();
    if lower.contains("saving is now disabled") {
        return Ok(());
    }
    if lower.contains("already turned off") {
        log::warn!(
            "RCON: world saving was ALREADY off before this backup (an earlier save-on may have failed, or another tool turned it off). Continuing; saving will be turned back on afterwards."
        );
        return Ok(());
    }
    anyhow::bail!(
        "Server did not confirm that world saving is off (save-off replied: {:?}). Not copying a world that may still be written to",
        reply.trim()
    )
}

fn check_flush_reply(reply: &str) -> Result<()> {
    if reply.to_ascii_lowercase().contains("saved the game") {
        return Ok(());
    }
    anyhow::bail!(
        "Server did not confirm the world was saved (save-all flush replied: {:?}). Not copying a world that may be half-written",
        reply.trim()
    )
}

/// Nothing accepted the RCON connection (server stopped, or RCON not listening), as opposed to a
/// wrong password or a command that failed on a running server.
#[derive(Debug)]
pub(crate) struct RconConnectError(String);

impl std::fmt::Display for RconConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RCON connect failed: {}", self.0)
    }
}

impl std::error::Error for RconConnectError {}

/// True if `error` (from `run_rcon_commands`) means the server could not be reached at all.
pub(crate) fn is_connect_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<RconConnectError>().is_some()
}

/// Re-enables Minecraft world saving. Arm it *before* sending `save-off`, then call `send()` once
/// the copy is done. If the backup bails out early, panics or is cancelled in between, `Drop`
/// still sends `save-on` (on a background thread) so the server is never left with saving off.
pub(crate) struct SaveOnGuard {
    target: Option<(String, u16, String)>,
}

impl SaveOnGuard {
    pub(crate) fn arm(host: String, port: u16, password: String) -> Self {
        Self { target: Some((host, port, password)) }
    }

    /// Send `save-on` now and disarm the guard.
    pub(crate) async fn send(mut self) -> Result<()> {
        let (host, port, password) = self.target.take().expect("SaveOnGuard already used");
        tokio::task::spawn_blocking(move || run_rcon_commands(host, port, password, vec!["save-on"]))
            .await
            .context("RCON task failed")?
    }
}

impl Drop for SaveOnGuard {
    fn drop(&mut self) {
        if let Some((host, port, password)) = self.target.take() {
            log::warn!("RCON: backup ended before re-enabling saves; sending save-on now");
            std::thread::spawn(move || {
                if let Err(e) = run_rcon_commands(host.clone(), port, password, vec!["save-on"]) {
                    log::error!(
                        "RCON: FAILED to re-enable saving on {}:{}: {:#}. World saving is still OFF — run `save-on` on the server manually.",
                        host, port, e
                    );
                }
            });
        }
    }
}

const PACKET_TYPE_AUTH: i32 = 3;
const PACKET_TYPE_AUTH_RESPONSE: i32 = 2;
const PACKET_TYPE_EXEC_COMMAND: i32 = 2;
/// Request id the server uses to signal an authentication failure.
const AUTH_FAILED_ID: i32 = -1;
/// id + type + two trailing nul bytes.
const PACKET_HEADER_LEN: i32 = 10;
/// Real responses are at most 4 KiB; this only stops a corrupt length prefix from allocating a huge buffer.
const MAX_PACKET_LEN: i32 = 64 * 1024;

#[derive(Debug, Clone, Copy)]
struct RconTimeouts {
    connect: Duration,
    write: Duration,
    login: Duration,
    /// `save-all flush` and ARK `SaveWorld` only reply once the save is done, so allow plenty of time.
    command: Duration,
}

const DEFAULT_TIMEOUTS: RconTimeouts = RconTimeouts {
    connect: Duration::from_secs(10),
    write: Duration::from_secs(10),
    login: Duration::from_secs(15),
    command: Duration::from_secs(120),
};

struct RconPacket {
    id: i32,
    packet_type: i32,
    payload: String,
}

/// Minimal Source/Minecraft RCON client (https://developer.valvesoftware.com/wiki/Source_RCON_Protocol).
struct RconClient {
    stream: TcpStream,
    next_id: i32,
    timeouts: RconTimeouts,
}

impl RconClient {
    fn connect(addr: &str, timeouts: RconTimeouts) -> Result<Self> {
        let mut last_err = None;
        for socket_addr in addr
            .to_socket_addrs()
            .with_context(|| format!("Could not resolve RCON address {}", addr))?
        {
            match TcpStream::connect_timeout(&socket_addr, timeouts.connect) {
                Ok(stream) => {
                    stream.set_write_timeout(Some(timeouts.write))?;
                    return Ok(Self { stream, next_id: 1, timeouts });
                }
                Err(e) => last_err = Some(e),
            }
        }
        match last_err {
            Some(e) => Err(describe_io_error(e, "connecting", timeouts.connect)).context(format!("Could not connect to {}", addr)),
            None => anyhow::bail!("RCON address {} did not resolve to any IP", addr),
        }
    }

    fn log_in(&mut self, password: &str) -> Result<()> {
        let id = self.send_packet(PACKET_TYPE_AUTH, password)?;
        self.stream.set_read_timeout(Some(self.timeouts.login))?;
        loop {
            let packet = self.read_packet(self.timeouts.login)?;
            if packet.id == AUTH_FAILED_ID {
                anyhow::bail!("RCON password was rejected");
            }
            // Source servers send an empty response-value packet before the auth response; skip it.
            if packet.id == id && packet.packet_type == PACKET_TYPE_AUTH_RESPONSE {
                return Ok(());
            }
        }
    }

    fn send_command(&mut self, command: &str) -> Result<String> {
        let id = self.send_packet(PACKET_TYPE_EXEC_COMMAND, command)?;
        self.stream.set_read_timeout(Some(self.timeouts.command))?;
        loop {
            let packet = self.read_packet(self.timeouts.command)?;
            if packet.id == AUTH_FAILED_ID {
                anyhow::bail!("RCON session is not authenticated");
            }
            // Skip leftovers from earlier requests (e.g. fragments of a previous long reply).
            if packet.id == id {
                return Ok(packet.payload);
            }
        }
    }

    fn send_packet(&mut self, packet_type: i32, payload: &str) -> Result<i32> {
        let id = self.next_id;
        self.next_id += 1;

        let body_len = PACKET_HEADER_LEN + payload.len() as i32;
        let mut buf = Vec::with_capacity(4 + body_len as usize);
        buf.extend_from_slice(&body_len.to_le_bytes());
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&packet_type.to_le_bytes());
        buf.extend_from_slice(payload.as_bytes());
        buf.extend_from_slice(&[0, 0]);

        // Written in one call: Minecraft disconnects if it reads a partial packet.
        self.stream
            .write_all(&buf)
            .map_err(|e| describe_io_error(e, "sending", self.timeouts.write))?;
        Ok(id)
    }

    fn read_packet(&mut self, timeout: Duration) -> Result<RconPacket> {
        let mut len_bytes = [0u8; 4];
        self.stream
            .read_exact(&mut len_bytes)
            .map_err(|e| describe_io_error(e, "waiting for a reply", timeout))?;
        let len = i32::from_le_bytes(len_bytes);
        if !(PACKET_HEADER_LEN..=MAX_PACKET_LEN).contains(&len) {
            anyhow::bail!("RCON server sent an invalid packet length: {}", len);
        }

        let mut body = vec![0u8; len as usize];
        self.stream
            .read_exact(&mut body)
            .map_err(|e| describe_io_error(e, "waiting for a reply", timeout))?;

        let id = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let packet_type = i32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        let payload = String::from_utf8_lossy(&body[8..body.len() - 2]).into_owned();
        Ok(RconPacket { id, packet_type, payload })
    }
}

fn describe_io_error(e: io::Error, action: &str, timeout: Duration) -> anyhow::Error {
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => anyhow::anyhow!(
            "RCON server did not respond within {}s while {}",
            timeout.as_secs(),
            action
        ),
        _ => anyhow::Error::new(e).context(format!("RCON error while {}", action)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    const TEST_TIMEOUTS: RconTimeouts = RconTimeouts {
        connect: Duration::from_secs(2),
        write: Duration::from_secs(2),
        login: Duration::from_millis(300),
        command: Duration::from_millis(300),
    };

    fn read_request(stream: &mut TcpStream) -> (i32, i32, String) {
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).unwrap();
        let mut body = vec![0u8; i32::from_le_bytes(len_bytes) as usize];
        stream.read_exact(&mut body).unwrap();
        let id = i32::from_le_bytes(body[0..4].try_into().unwrap());
        let packet_type = i32::from_le_bytes(body[4..8].try_into().unwrap());
        (id, packet_type, String::from_utf8_lossy(&body[8..body.len() - 2]).into_owned())
    }

    fn write_reply(stream: &mut TcpStream, id: i32, packet_type: i32, payload: &str) {
        let len = PACKET_HEADER_LEN + payload.len() as i32;
        let mut buf = len.to_le_bytes().to_vec();
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&packet_type.to_le_bytes());
        buf.extend_from_slice(payload.as_bytes());
        buf.extend_from_slice(&[0, 0]);
        stream.write_all(&buf).unwrap();
    }

    /// Start a fake RCON server on a random local port; `handler` runs for the first connection.
    fn fake_server(handler: impl FnOnce(TcpStream) + Send + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handler(stream);
        });
        addr
    }

    #[test]
    fn logs_in_and_runs_command() {
        let addr = fake_server(|mut s| {
            let (id, packet_type, password) = read_request(&mut s);
            assert_eq!(packet_type, PACKET_TYPE_AUTH);
            assert_eq!(password, "secret");
            // Source-style: empty response value first, then the auth response.
            write_reply(&mut s, id, 0, "");
            write_reply(&mut s, id, PACKET_TYPE_AUTH_RESPONSE, "");
            let (id, _, command) = read_request(&mut s);
            assert_eq!(command, "save-all flush");
            write_reply(&mut s, id, 0, "Saved the game");
        });

        let mut client = RconClient::connect(&addr, TEST_TIMEOUTS).unwrap();
        client.log_in("secret").unwrap();
        assert_eq!(client.send_command("save-all flush").unwrap(), "Saved the game");
    }

    #[test]
    fn rejects_bad_password() {
        let addr = fake_server(|mut s| {
            read_request(&mut s);
            write_reply(&mut s, AUTH_FAILED_ID, PACKET_TYPE_AUTH_RESPONSE, "");
        });

        let mut client = RconClient::connect(&addr, TEST_TIMEOUTS).unwrap();
        let err = client.log_in("wrong").unwrap_err();
        assert!(err.to_string().contains("rejected"));
    }

    /// Accept the login, then hand each received command to `on_command`.
    fn fake_logged_in_server(on_command: impl Fn(&mut TcpStream, i32, String) + Send + 'static) -> (String, u16) {
        let addr = fake_server(move |mut s| {
            let (id, _, _) = read_request(&mut s);
            write_reply(&mut s, id, PACKET_TYPE_AUTH_RESPONSE, "");
            let mut len_bytes = [0u8; 4];
            while s.peek(&mut len_bytes).map(|n| n > 0).unwrap_or(false) {
                let (id, _, command) = read_request(&mut s);
                on_command(&mut s, id, command);
            }
        });
        let (host, port) = addr.rsplit_once(':').unwrap();
        (host.to_string(), port.parse().unwrap())
    }

    #[test]
    fn run_rcon_commands_reports_failed_command() {
        let (host, port) = fake_logged_in_server(|s, id, command| {
            if command == "save-off" {
                write_reply(s, id, 0, "Automatic saving is now disabled");
            } else {
                // Drop the connection instead of answering the second command.
                s.shutdown(std::net::Shutdown::Both).unwrap();
            }
        });

        let err = run_rcon_commands(host, port, "pw".into(), vec!["save-off", "save-all flush"]).unwrap_err();
        assert!(err.to_string().contains("save-all flush"), "{}", err);
        assert!(!err.to_string().contains("'save-off'"), "{}", err);
    }

    #[test]
    fn save_off_reply_must_confirm_saving_is_off() {
        assert!(check_save_off_reply("Automatic saving is now disabled").is_ok());
        assert!(check_save_off_reply("Saving is already turned off").is_ok());
        assert!(check_save_off_reply("").is_err());
        assert!(check_save_off_reply("Unknown or incomplete command").is_err());
    }

    #[test]
    fn flush_reply_must_confirm_save() {
        assert!(check_flush_reply("Saving the game (this may take a moment!)Saved the game").is_ok());
        assert!(check_flush_reply("Saving the game (this may take a moment!)").is_err());
        assert!(check_flush_reply("").is_err());
    }

    #[test]
    fn pause_minecraft_saving_stops_when_save_off_is_not_confirmed() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (host, port) = fake_logged_in_server(move |s, id, command| {
            tx.send(command).unwrap();
            write_reply(s, id, 0, "Unknown or incomplete command");
        });

        let err = pause_minecraft_saving(host, port, "pw".into()).unwrap_err();
        assert!(err.to_string().contains("did not confirm"), "{}", err);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "save-off");
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "save-all flush should not be sent");
    }

    #[test]
    fn pause_minecraft_saving_accepts_confirmed_replies() {
        let (host, port) = fake_logged_in_server(|s, id, command| {
            let reply = match command.as_str() {
                "save-off" => "Automatic saving is now disabled",
                _ => "Saving the game (this may take a moment!)Saved the game",
            };
            write_reply(s, id, 0, reply);
        });

        pause_minecraft_saving(host, port, "pw".into()).unwrap();
    }

    #[test]
    fn connect_failure_is_distinguished_from_other_errors() {
        // Grab a free port, then close the listener so nothing is listening there.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let err = run_rcon_commands("127.0.0.1".into(), port, "pw".into(), vec!["SaveWorld"]).unwrap_err();
        assert!(is_connect_error(&err), "{:#}", err);

        let addr = fake_server(|mut s| {
            read_request(&mut s);
            write_reply(&mut s, AUTH_FAILED_ID, PACKET_TYPE_AUTH_RESPONSE, "");
        });
        let (host, port) = addr.rsplit_once(':').unwrap();
        let err = run_rcon_commands(host.into(), port.parse().unwrap(), "wrong".into(), vec!["SaveWorld"]).unwrap_err();
        assert!(!is_connect_error(&err), "{:#}", err);
    }

    #[test]
    fn save_on_guard_sends_save_on_when_dropped() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (host, port) = fake_logged_in_server(move |s, id, command| {
            write_reply(s, id, 0, "");
            tx.send(command).unwrap();
        });

        drop(SaveOnGuard::arm(host, port, "pw".into()));

        let command = rx.recv_timeout(Duration::from_secs(5)).expect("save-on was not sent");
        assert_eq!(command, "save-on");
    }

    #[test]
    fn times_out_when_server_never_replies() {
        let addr = fake_server(|mut s| {
            read_request(&mut s);
            // Hold the connection open without replying.
            thread::sleep(Duration::from_secs(2));
        });

        let mut client = RconClient::connect(&addr, TEST_TIMEOUTS).unwrap();
        let err = client.log_in("secret").unwrap_err();
        assert!(err.to_string().contains("did not respond"), "{}", err);
    }
}
