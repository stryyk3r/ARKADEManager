use crate::job::JobInput;
use crate::map::{self, MapDefinition};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn validate_job(job: &JobInput, maps: &[MapDefinition]) -> Result<()> {
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
        return Ok(());
    }

    if job.job_type == "palworld" {
        validate_interval_retention(job)?;
        validate_rcon_fields(job)?;

        let config_dir = derive_palworld_config_dir(&job.root_dir);
        let settings_ini = config_dir.join("PalWorldSettings.ini");
        if !settings_ini.exists() {
            anyhow::bail!(
                "PalWorldSettings.ini not found in: {}",
                config_dir.display()
            );
        }

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
    if !has_port {
        anyhow::bail!("Backup requires RCON port (e.g. 25575)");
    }
    if !has_password {
        anyhow::bail!("Backup requires RCON password");
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

