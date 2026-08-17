use crate::app_data::AppData;
use crate::job::Job;
use crate::server_ini;
use crate::validation;
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

use super::monthly::maybe_copy_backup_to_monthly;
use super::rcon::run_rcon_commands;
use super::retention::cleanup_old_backups;
use super::shared::{is_disk_full_error, rename_temp_to_backup, verify_zip_integrity};
pub async fn create_backup(job: &Job, app_data: &AppData) -> Result<u64> {
    log::info!("Starting backup for job: {}", job.name);

    let config_dir = validation::derive_config_dir(&job.root_dir);
    match server_ini::read_ark_rcon_settings(&config_dir) {
        Ok(rcon) if rcon.rcon_enabled => {
            if let (Some(port), Some(password)) = (rcon.rcon_port, rcon.admin_password) {
                log::info!("RCON: sending SaveWorld before ARK backup on 127.0.0.1:{}", port);
                let password_clone = password.clone();
                tokio::task::spawn_blocking(move || {
                    run_rcon_commands(
                        "127.0.0.1".to_string(),
                        port,
                        password_clone,
                        vec!["SaveWorld"],
                    )
                })
                .await
                .context("ARK SaveWorld RCON task failed")??;
                log::info!("Waiting 3 seconds for ARK save to flush to disk...");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            } else {
                log::warn!(
                    "RCON is enabled in GameUserSettings.ini but RCONPort or ServerAdminPassword is missing; skipping SaveWorld"
                );
            }
        }
        Ok(_) => {
            log::warn!(
                "RCON is disabled in GameUserSettings.ini; proceeding with ARK backup without SaveWorld"
            );
        }
        Err(e) => {
            log::warn!(
                "Could not read ARK RCON settings from GameUserSettings.ini ({}); proceeding without SaveWorld",
                e
            );
        }
    }

    let maps = app_data.get_config()?.ark_maps();
    let map = job
        .resolve_map(&maps)
        .ok_or_else(|| anyhow::anyhow!("Invalid map: {}", job.map))?;

    // Derive paths
    let saves_dir = validation::derive_saves_dir(&job.root_dir, &map.folder_name);
    let config_dir = validation::derive_config_dir(&job.root_dir);
    let plugins_dir = validation::derive_plugins_dir(&job.root_dir);

    // Create backup filename with timestamp
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.zip", job.name, timestamp);
    let backup_path = Path::new(&job.destination_dir).join(&filename);
    let temp_path = backup_path.with_extension("zip.tmp");

    // Ensure destination directory exists
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create destination directory: {}", parent.display()))?;
    }

    // Attempt backup with retry on disk full
    let result = attempt_backup(
        &temp_path,
        &backup_path,
        &saves_dir,
        &config_dir,
        &plugins_dir,
        job,
        &map,
    ).await;

    match result {
        Ok(file_size) => {
            log::info!("Backup completed: {} ({} bytes)", backup_path.display(), file_size);

            if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                log::warn!("MONTHLY: Failed to copy ARK backup into monthly folder: {:#}", e);
            }

            // Cleanup old backups after successful backup
            cleanup_old_backups(&job.destination_dir, job.retention_days)?;
            Ok(file_size)
        }
        Err(e) if is_disk_full_error(&e) => {
            log::warn!("Backup failed due to insufficient disk space. Cleaning up old backups and retrying...");
            
            // Cleanup old backups to free space
            cleanup_old_backups(&job.destination_dir, job.retention_days)
                .context("Failed to cleanup old backups")?;
            
            log::info!("Retrying backup after cleanup...");
            
            // Retry the backup
            let retry_result = attempt_backup(
                &temp_path,
                &backup_path,
                &saves_dir,
                &config_dir,
                &plugins_dir,
                job,
                &map,
            ).await;
            
            match retry_result {
                Ok(file_size) => {
                    log::info!("Backup completed after cleanup: {} ({} bytes)", backup_path.display(), file_size);
                    Ok(file_size)
                }
                Err(retry_err) => {
                    if is_disk_full_error(&retry_err) {
                        Err(anyhow::anyhow!("Backup failed even after cleaning up old backups. Still insufficient disk space: {}", retry_err))
                    } else {
                        Err(retry_err)
                    }
                }
            }
        }
        Err(e) => Err(e),
    }
}

async fn attempt_backup(
    temp_path: &Path,
    backup_path: &Path,
    saves_dir: &Path,
    config_dir: &Path,
    plugins_dir: &Path,
    job: &Job,
    map: &crate::map::MapDefinition,
) -> Result<u64> {
    // Create ZIP file
    let file = fs::File::create(temp_path)
        .with_context(|| format!("Failed to create backup file: {}", temp_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(5));

    // Add saves if enabled
    if job.include_saves || job.include_map {
        if let Err(e) = add_saves_to_zip(&mut zip, saves_dir, job.include_saves, job.include_map, map, &options) {
            log::error!("Error adding saves to ZIP: {}", e);
            return Err(e);
        }
    }

    // Add server INI files if enabled
    if job.include_server_files {
        if let Err(e) = add_ini_files_to_zip(&mut zip, config_dir, &options) {
            log::error!("Error adding INI files to ZIP: {}", e);
            return Err(e);
        }
    }

    // Add plugin configs if enabled
    if job.include_plugin_configs {
        if let Err(e) = add_plugin_configs_to_zip(&mut zip, plugins_dir, &options) {
            log::error!("Error adding plugin configs to ZIP: {}", e);
            return Err(e);
        }
    }

    // finish() returns the underlying File; we must drop it so the handle is closed before rename (required on Windows)
    let file = zip.finish().context("Failed to finalize ZIP file")?;
    file.sync_all().context("Failed to sync ZIP file to disk")?;
    drop(file);

    #[cfg(target_os = "windows")]
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Get file size before rename
    let file_size = fs::metadata(temp_path)
        .context("Failed to get backup file metadata")?
        .len();

    rename_temp_to_backup(temp_path, backup_path)?;

    // Verify integrity
    verify_zip_integrity(backup_path)?;

    Ok(file_size)
}

fn add_saves_to_zip(
    zip: &mut ZipWriter<fs::File>,
    saves_dir: &Path,
    include_saves: bool,
    include_map: bool,
    map: &crate::map::MapDefinition,
    options: &FileOptions,
) -> Result<()> {
    if !saves_dir.exists() {
        return Ok(());
    }

    // Expected map file name: all maps have _WP suffix in the .ark file name
    let expected_map_file = &map.map_file_name;

    for entry in WalkDir::new(saves_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            let should_include = match ext {
                "ark" => {
                    // For map files, only include exact match: "{MapName}_WP.ark"
                    if include_map {
                        file_name == expected_map_file
                    } else {
                        false
                    }
                },
                "arkprofile" | "arktribe" => include_saves,
                _ => false,
            };

            if should_include {
                // Put all files in "SavedArks" folder in ZIP
                let zip_path = format!("SavedArks/{}", file_name);

                // Try to add file to ZIP, skip if locked or inaccessible
                // Check if file can be opened and read before adding to ZIP
                match fs::File::open(path) {
                    Ok(mut file) => {
                        let mut buffer = Vec::new();
                        match file.read_to_end(&mut buffer) {
                            Ok(_) => {
                                // File is readable, now try to add to ZIP
                                // Only start file entry if we successfully read the file
                                match zip.start_file(zip_path.clone(), *options) {
                                    Ok(_) => {
                                        // Write the file data - if this fails, we need to handle it carefully
                                        // The ZIP writer is now in a state where it expects data
                                        if let Err(e) = zip.write_all(&buffer) {
                                            let err_msg = format!("{}", e);
                                            // If ZIP was closed, we can't continue
                                            if err_msg.contains("closed") || err_msg.contains("finished") {
                                                log::error!("ZIP writer was closed while writing file {}. This may indicate the ZIP was finalized prematurely or the underlying file was closed. Backup cannot continue.", path.display());
                                                return Err(anyhow::anyhow!("ZIP writer was closed: {}", e));
                                            }
                                            // Other write errors - could be disk full, permission issues, etc.
                                            log::error!("Failed to write file {} to ZIP: {}. This may indicate disk full, permission issues, or other I/O problems. Backup cannot continue.", path.display(), e);
                                            return Err(anyhow::anyhow!("Failed to write file to ZIP: {}", e));
                                        }
                                        // File successfully added
                                    }
                                    Err(e) => {
                                        // Check if ZIP was closed (this shouldn't happen normally)
                                        let err_msg = format!("{}", e);
                                        if err_msg.contains("closed") || err_msg.contains("finished") || err_msg.contains("already closed") {
                                            log::error!("ZIP writer was closed unexpectedly while starting file entry for {}. This indicates the ZIP was finalized prematurely, possibly due to a previous error (disk full, I/O error, etc.). Cannot continue backup.", path.display());
                                            return Err(anyhow::anyhow!("ZIP writer was closed: {}. This may indicate a previous error (disk full, I/O error) caused the ZIP to be finalized prematurely.", e));
                                        }
                                        // Other errors starting file entry - might be recoverable, but log as error
                                        log::error!("Failed to start file entry for {} in ZIP: {}. This may indicate ZIP corruption or I/O issues.", path.display(), e);
                                        return Err(anyhow::anyhow!("Failed to start file entry in ZIP: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to read file {} (may be locked by server): {}. Skipping.", path.display(), e);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to open file {} (may be locked by server): {}. Skipping.", path.display(), e);
                        continue;
                    }
                }
            }
        }
    }

    Ok(())
}

fn add_ini_files_to_zip(
    zip: &mut ZipWriter<fs::File>,
    config_dir: &Path,
    options: &FileOptions,
) -> Result<()> {
    let game_ini = config_dir.join("Game.ini");
    let game_user_settings_ini = config_dir.join("GameUserSettings.ini");

    if game_ini.exists() {
        // Check if file can be opened before adding to ZIP
        match fs::File::open(&game_ini) {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                match file.read_to_end(&mut buffer) {
                    Ok(_) => {
                        match zip.start_file("INI Settings/Game.ini", *options) {
                            Ok(_) => {
                                if let Err(e) = zip.write_all(&buffer) {
                                    let err_msg = format!("{}", e);
                                    if err_msg.contains("closed") || err_msg.contains("finished") {
                                        return Err(anyhow::anyhow!("ZIP writer was closed while adding Game.ini: {}", e));
                                    }
                                    return Err(anyhow::anyhow!("Failed to write Game.ini to ZIP: {}", e));
                                }
                            }
                            Err(e) => {
                                let err_msg = format!("{}", e);
                                if err_msg.contains("closed") || err_msg.contains("finished") {
                                    return Err(anyhow::anyhow!("ZIP writer was closed while starting Game.ini entry: {}", e));
                                }
                                return Err(anyhow::anyhow!("Failed to start Game.ini entry in ZIP: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to read Game.ini: {}. Skipping.", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to open Game.ini: {}. Skipping.", e);
            }
        }
    }

    if game_user_settings_ini.exists() {
        // Check if file can be opened before adding to ZIP
        match fs::File::open(&game_user_settings_ini) {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                match file.read_to_end(&mut buffer) {
                    Ok(_) => {
                        match zip.start_file("INI Settings/GameUserSettings.ini", *options) {
                            Ok(_) => {
                                if let Err(e) = zip.write_all(&buffer) {
                                    let err_msg = format!("{}", e);
                                    if err_msg.contains("closed") || err_msg.contains("finished") {
                                        return Err(anyhow::anyhow!("ZIP writer was closed while adding GameUserSettings.ini: {}", e));
                                    }
                                    return Err(anyhow::anyhow!("Failed to write GameUserSettings.ini to ZIP: {}", e));
                                }
                            }
                            Err(e) => {
                                let err_msg = format!("{}", e);
                                if err_msg.contains("closed") || err_msg.contains("finished") {
                                    return Err(anyhow::anyhow!("ZIP writer was closed while starting GameUserSettings.ini entry: {}", e));
                                }
                                return Err(anyhow::anyhow!("Failed to start GameUserSettings.ini entry in ZIP: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to read GameUserSettings.ini: {}. Skipping.", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to open GameUserSettings.ini: {}. Skipping.", e);
            }
        }
    }

    Ok(())
}

fn add_plugin_configs_to_zip(
    zip: &mut ZipWriter<fs::File>,
    plugins_dir: &Path,
    options: &FileOptions,
) -> Result<()> {
    if !plugins_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(plugins_dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("config.json") {
            // Get the plugin folder name (parent directory of config.json)
            let plugin_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            
            // Put in "Plugin" folder with plugin name: Plugin/{plugin_name}/config.json
            let zip_path = format!("Plugin/{}/config.json", plugin_name);

            // Check if file can be opened before adding to ZIP
            match fs::File::open(path) {
                Ok(mut file) => {
                    let mut buffer = Vec::new();
                    match file.read_to_end(&mut buffer) {
                        Ok(_) => {
                            match zip.start_file(zip_path, *options) {
                                Ok(_) => {
                                    if let Err(e) = zip.write_all(&buffer) {
                                        let err_msg = format!("{}", e);
                                        if err_msg.contains("closed") || err_msg.contains("finished") {
                                            return Err(anyhow::anyhow!("ZIP writer was closed while adding config.json: {}", e));
                                        }
                                        return Err(anyhow::anyhow!("Failed to write config.json to ZIP: {}", e));
                                    }
                                }
                                Err(e) => {
                                    let err_msg = format!("{}", e);
                                    if err_msg.contains("closed") || err_msg.contains("finished") {
                                        return Err(anyhow::anyhow!("ZIP writer was closed while starting config.json entry: {}", e));
                                    }
                                    return Err(anyhow::anyhow!("Failed to start config.json entry in ZIP: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to read config.json {}: {}. Skipping.", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to open config.json {}: {}. Skipping.", path.display(), e);
                }
            }
        }
    }

    Ok(())
}
