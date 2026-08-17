use crate::job::Job;
use anyhow::{Context, Result};
use mc_rcon::RconClient;
/// Returns true if the job has RCON settings (Minecraft save-off/save-on flow).
pub(crate) fn job_has_rcon(job: &Job) -> bool {
    job.job_type == "minecraft"
        && job.rcon_host.as_ref().map(|h| !h.trim().is_empty()).unwrap_or(false)
        && job.rcon_port.map(|p| p > 0).unwrap_or(false)
        && job.rcon_password.as_ref().map(|p| !p.is_empty()).unwrap_or(false)
}

/// Run RCON commands in a blocking task. Used for save-off, save-all flush, and save-on.
pub(crate) fn run_rcon_commands(host: String, port: u16, password: String, commands: Vec<&'static str>) -> Result<()> {
    let addr = format!("{}:{}", host.trim(), port);
    let client = RconClient::connect(&addr).context("RCON connect failed")?;
    client.log_in(&password).context("RCON login failed")?;
    for cmd in commands {
        let _ = client.send_command(cmd);
    }
    Ok(())
}
