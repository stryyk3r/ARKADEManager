use crate::job::JobInput;
use crate::map::{self, MapDefinition};
use crate::server_ini;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const MAX_JOB_NAME_LEN: usize = 100;

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The job name becomes part of every backup file name (`{name}_YYYYMMDD_HHMMSS.zip`), so it must
/// be a valid Windows file name and must not contain path separators that would place backups
/// outside the destination folder.
pub fn validate_job_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("Job name is required");
    }
    if name != name.trim() {
        anyhow::bail!("Job name cannot start or end with spaces");
    }
    if name.ends_with('.') {
        anyhow::bail!("Job name cannot end with a period");
    }
    if name.chars().count() > MAX_JOB_NAME_LEN {
        anyhow::bail!("Job name must be at most {} characters", MAX_JOB_NAME_LEN);
    }
    if let Some(c) = name
        .chars()
        .find(|c| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control())
    {
        anyhow::bail!(
            "Job name cannot contain '{}' (not allowed: < > : \" / \\ | ? * or control characters)",
            c.escape_default()
        );
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if WINDOWS_RESERVED_NAMES.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        anyhow::bail!("Job name cannot be the reserved Windows name '{}'", stem);
    }
    Ok(())
}

/// A host must be a plain IP address (IPv6 optionally in brackets) or DNS hostname. This rejects
/// spaces, line breaks, ports, paths and other characters that would otherwise be pasted straight
/// into the RCON address or the Palworld HTTP request.
pub fn validate_host(host: &str) -> Result<()> {
    let host = host.trim();
    if host.is_empty() {
        anyhow::bail!("Host is empty");
    }
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if unbracketed.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    let valid_hostname = host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !valid_hostname {
        anyhow::bail!(
            "Invalid host '{}': enter an IP address or hostname only (no port, spaces or path)",
            host.escape_default()
        );
    }
    Ok(())
}

pub fn validate_job(job: &JobInput, maps: &[MapDefinition]) -> Result<()> {
    validate_job_name(&job.name)?;

    // Validate root_dir exists
    if !Path::new(&job.root_dir).exists() {
        anyhow::bail!("Server root directory does not exist: {}", job.root_dir);
    }

    // Validate destination_dir exists
    if !Path::new(&job.destination_dir).exists() {
        anyhow::bail!("Destination directory does not exist: {}", job.destination_dir);
    }

    let cluster = job.monthly_cluster.trim();
    if cluster.is_empty() {
        anyhow::bail!("Monthly cluster is required");
    }
    if !matches!(
        cluster,
        "ASA Legacy" | "ASE Legacy" | "ASA Omega" | "Minecraft" | "Palworld"
    ) {
        anyhow::bail!(
            "Invalid monthly cluster: {} (expected ASA Legacy, ASE Legacy, ASA Omega, Minecraft, or Palworld)",
            cluster
        );
    }

    if job.job_type == "minecraft" {
        validate_interval_retention(job)?;
        validate_rcon_fields(job)?;
        resolve_minecraft_backup_dirs(&job.root_dir)?;
        return Ok(());
    }

    if job.job_type == "palworld" {
        validate_interval_retention(job)?;
        // Optional REST API host override; blank means 127.0.0.1.
        if let Some(host) = job.rcon_host.as_deref().filter(|h| !h.trim().is_empty()) {
            validate_host(host).context("Palworld API host")?;
        }
        validate_palworld_rest_settings(&job.root_dir)?;

        let world_dir = discover_palworld_world_dir(&job.root_dir)?;
        if !world_dir.join("Players").exists() {
            anyhow::bail!(
                "Players folder not found in Palworld world directory: {}",
                world_dir.display()
            );
        }
        if !world_dir.join("Level.sav").exists() {
            anyhow::bail!(
                "Level.sav not found in Palworld world directory: {}",
                world_dir.display()
            );
        }
        return Ok(());
    }

    // ARK: validate map and derived paths
    let map = map::resolve_map(maps, &job.map)
        .with_context(|| format!("Invalid map: {}", job.map))?;

    let config_dir = derive_config_dir(&job.root_dir);
    let saves_dir = derive_saves_dir(&job.root_dir, &map.folder_name);
    let plugins_dir = derive_plugins_dir(&job.root_dir);

    // If including server files, config dir must exist and contain required files
    if job.include_server_files {
        if !config_dir.exists() {
            anyhow::bail!(
                "Server config directory does not exist: {}",
                config_dir.display()
            );
        }
        let game_ini = config_dir.join("Game.ini");
        let game_user_settings_ini = config_dir.join("GameUserSettings.ini");
        if !game_ini.exists() {
            anyhow::bail!("Game.ini not found in: {}", config_dir.display());
        }
        if !game_user_settings_ini.exists() {
            anyhow::bail!(
                "GameUserSettings.ini not found in: {}",
                config_dir.display()
            );
        }
    }

    // If including saves/map, saves dir must exist
    if job.include_saves || job.include_map {
        if !saves_dir.exists() {
            anyhow::bail!("Saves directory does not exist: {}", saves_dir.display());
        }
    }

    // If including plugin configs, plugins dir must exist
    if job.include_plugin_configs {
        if !plugins_dir.exists() {
            anyhow::bail!("Plugins directory does not exist: {}", plugins_dir.display());
        }
    }

    validate_interval_retention(job)?;

    Ok(())
}

fn validate_rcon_fields(job: &JobInput) -> Result<()> {
    let has_host = job
        .rcon_host
        .as_ref()
        .map(|h| !h.trim().is_empty())
        .unwrap_or(false);
    let has_port = job.rcon_port.map(|p| p > 0).unwrap_or(false);
    let has_password = job
        .rcon_password
        .as_ref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    if !has_host {
        anyhow::bail!("Backup requires RCON host (IP or hostname)");
    }
    validate_host(job.rcon_host.as_deref().unwrap_or_default()).context("RCON host")?;
    if !has_port {
        anyhow::bail!("Backup requires RCON port (e.g. 25575)");
    }
    if !has_password {
        anyhow::bail!("Backup requires RCON password");
    }
    Ok(())
}

fn validate_palworld_rest_settings(root_dir: &str) -> Result<()> {
    let config_dir = derive_palworld_config_dir(root_dir);
    let settings_ini = config_dir.join("PalWorldSettings.ini");
    if !settings_ini.exists() {
        anyhow::bail!(
            "PalWorldSettings.ini not found in: {}",
            config_dir.display()
        );
    }

    let settings = server_ini::read_palworld_rest_settings(&config_dir)?;
    if !settings.rest_api_enabled {
        anyhow::bail!(
            "Palworld REST API is disabled in PalWorldSettings.ini (RESTAPIEnabled=False). Enable REST API before creating a Palworld backup job."
        );
    }
    if settings.rest_api_port == 0 {
        anyhow::bail!("Palworld RESTAPIPort is invalid in PalWorldSettings.ini");
    }
    if settings.admin_password.trim().is_empty() {
        anyhow::bail!(
            "Palworld AdminPassword is empty in PalWorldSettings.ini. REST save requires a non-empty admin password."
        );
    }
    Ok(())
}

fn validate_interval_retention(job: &JobInput) -> Result<()> {
    if job.interval_value == 0 {
        anyhow::bail!("Interval value must be greater than 0");
    }
    if !matches!(job.interval_unit.as_str(), "minutes" | "hours" | "days") {
        anyhow::bail!("Invalid interval unit: {}", job.interval_unit);
    }
    if job.retention_days == 0 {
        anyhow::bail!("Retention days must be greater than 0");
    }
    Ok(())
}

pub fn derive_config_dir(root_dir: &str) -> std::path::PathBuf {
    Path::new(root_dir)
        .join("ShooterGame")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
}

pub fn derive_saves_dir(root_dir: &str, map_folder: &str) -> std::path::PathBuf {
    Path::new(root_dir)
        .join("ShooterGame")
        .join("Saved")
        .join("SavedArks")
        .join(map_folder)
}

pub fn derive_plugins_dir(root_dir: &str) -> std::path::PathBuf {
    Path::new(root_dir)
        .join("ShooterGame")
        .join("Binaries")
        .join("Win64")
        .join("ArkApi")
        .join("Plugins")
}

/// Standard Minecraft server directories always included in backups.
pub const MINECRAFT_REQUIRED_BACKUP_DIRS: &[&str] = &["world", "config"];

/// Optional Minecraft directories included only when present (e.g. Cobblemon Discord integration).
pub const MINECRAFT_OPTIONAL_BACKUP_DIRS: &[&str] = &["DiscordIntegration-Data"];

/// Resolve absolute paths for Minecraft backup directories under the server root.
pub fn resolve_minecraft_backup_dirs(root_dir: &str) -> Result<Vec<PathBuf>> {
    let root = Path::new(root_dir);
    let mut dirs = Vec::new();

    for name in MINECRAFT_REQUIRED_BACKUP_DIRS {
        let path = root.join(name);
        if !path.is_dir() {
            anyhow::bail!(
                "Minecraft backup requires directory: {}",
                path.display()
            );
        }
        dirs.push(path);
    }

    for name in MINECRAFT_OPTIONAL_BACKUP_DIRS {
        let path = root.join(name);
        if path.is_dir() {
            dirs.push(path);
        }
    }

    Ok(dirs)
}

pub fn derive_palworld_config_dir(root_dir: &str) -> PathBuf {
    Path::new(root_dir)
        .join("Pal")
        .join("Saved")
        .join("Config")
        .join("WindowsServer")
}

pub fn derive_palworld_savegames_dir(root_dir: &str) -> PathBuf {
    Path::new(root_dir)
        .join("Pal")
        .join("Saved")
        .join("SaveGames")
        .join("0")
}

/// Find the single world directory under SaveGames/0 that contains Level.sav.
pub fn discover_palworld_world_dir(root_dir: &str) -> Result<PathBuf> {
    let savegames = derive_palworld_savegames_dir(root_dir);
    if !savegames.exists() {
        anyhow::bail!(
            "Palworld savegames directory does not exist: {}",
            savegames.display()
        );
    }

    let mut matches = Vec::new();
    for entry in std::fs::read_dir(&savegames)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("Level.sav").exists() {
            matches.push(path);
        }
    }

    match matches.len() {
        0 => anyhow::bail!(
            "No Palworld world directory found containing Level.sav under {}",
            savegames.display()
        ),
        1 => Ok(matches.remove(0)),
        n => {
            let names: Vec<String> = matches
                .iter()
                .filter_map(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .collect();
            anyhow::bail!(
                "Multiple Palworld world directories found ({}): {}. Ensure only one world exists.",
                n,
                names.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_derive_paths() {
        let root = r"C:\arkservers\asaservers\omega-forglar";
        let config = derive_config_dir(root);
        assert!(config.to_string_lossy().contains("WindowsServer"));
        assert!(config.to_string_lossy().contains("Config"));
        
        let saves = derive_saves_dir(root, "Forglar");
        assert!(saves.to_string_lossy().contains("Forglar"));
        assert!(saves.to_string_lossy().contains("SavedArks"));
        
        let plugins = derive_plugins_dir(root);
        assert!(plugins.to_string_lossy().contains("Plugins"));
        assert!(plugins.to_string_lossy().contains("ArkApi"));
    }

    #[test]
    fn test_validate_job_name() {
        for ok in ["Island", "Omega Forglar PVE", "Server_PVE-2", "Cobblemon (main)"] {
            assert!(validate_job_name(ok).is_ok(), "{}", ok);
        }
        for bad in [
            "", "   ", " Island", "Island ", "Island.", r"..\..\Windows\x", "a/b", "C:x",
            "what?", "bad\"name", "tab\tname", "CON", "nul", "com1.txt",
        ] {
            assert!(validate_job_name(bad).is_err(), "{:?}", bad);
        }
        assert!(validate_job_name(&"x".repeat(101)).is_err());
    }

    #[test]
    fn test_validate_host() {
        for ok in ["127.0.0.1", "localhost", "pal.example.com", "10.0.0.5", "::1", "[::1]", "game-server-01"] {
            assert!(validate_host(ok).is_ok(), "{}", ok);
        }
        for bad in [
            "", "  ", "127.0.0.1:8212", "http://host", "host/path", "bad host",
            "host\r\nX-Injected: 1", "-host", "host-", "a..b",
        ] {
            assert!(validate_host(bad).is_err(), "{:?}", bad);
        }
    }

    #[test]
    fn test_derive_paths_different_maps() {
        let root = r"C:\test\server";
        let maps = vec!["TheIsland", "Ragnarok", "Forglar"];
        
        for map in maps {
            let saves = derive_saves_dir(root, map);
            assert!(saves.to_string_lossy().ends_with(map));
        }
    }

    #[test]
    fn test_resolve_minecraft_backup_dirs_required_only() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("world")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();

        let dirs = resolve_minecraft_backup_dirs(root.to_str().unwrap()).unwrap();
        assert_eq!(dirs.len(), 2);
        assert!(dirs.iter().any(|p| p.ends_with("world")));
        assert!(dirs.iter().any(|p| p.ends_with("config")));
    }

    #[test]
    fn test_resolve_minecraft_backup_dirs_includes_discord_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("world")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("DiscordIntegration-Data")).unwrap();

        let dirs = resolve_minecraft_backup_dirs(root.to_str().unwrap()).unwrap();
        assert_eq!(dirs.len(), 3);
        assert!(dirs.iter().any(|p| p.ends_with("DiscordIntegration-Data")));
    }

    #[test]
    fn test_resolve_minecraft_backup_dirs_missing_world_errors() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("config")).unwrap();

        let err = resolve_minecraft_backup_dirs(root.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("world"));
    }
}

