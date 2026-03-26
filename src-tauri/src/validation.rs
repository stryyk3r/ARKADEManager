use crate::job::JobInput;
use crate::map::Map;
use anyhow::{Context, Result};
use std::path::Path;

pub fn validate_job(job: &JobInput) -> Result<()> {
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
        // Minecraft with RCON: require host, port, password for save-off/save-on flow
        let has_host = job.rcon_host.as_ref().map(|h| !h.trim().is_empty()).unwrap_or(false);
        let has_port = job.rcon_port.map(|p| p > 0).unwrap_or(false);
        let has_password = job.rcon_password.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
        if !has_host {
            anyhow::bail!("Minecraft backup requires RCON host (IP or hostname)");
        }
        if !has_port {
            anyhow::bail!("Minecraft backup requires RCON port (e.g. 25575)");
        }
        if !has_password {
            anyhow::bail!("Minecraft backup requires RCON password");
        }
        return Ok(());
    }

    // ARK: validate map and derived paths
    Map::from_str(&job.map)
        .with_context(|| format!("Invalid map: {}", job.map))?;

    let map = Map::from_str(&job.map).unwrap();
    let config_dir = derive_config_dir(&job.root_dir);
    let saves_dir = derive_saves_dir(&job.root_dir, map.folder_name());
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_derive_paths_different_maps() {
        let root = r"C:\test\server";
        let maps = vec!["TheIsland", "Ragnarok", "Forglar"];
        
        for map in maps {
            let saves = derive_saves_dir(root, map);
            assert!(saves.to_string_lossy().ends_with(map));
        }
    }
}

