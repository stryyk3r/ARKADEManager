use crate::app_data::AppData;
use crate::job::Job;
use crate::server_ini;
use crate::validation;
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

use super::monthly::maybe_copy_backup_to_monthly;
use super::rcon::{is_connect_error, run_rcon_commands};
use super::retention::cleanup_old_backups;
use super::shared::{
    add_optional_file_to_zip, add_single_file_to_zip, is_disk_full_error, rename_temp_to_backup,
    verify_zip_integrity, TempFileGuard,
};
pub async fn create_backup(job: &Job, app_data: &AppData) -> Result<u64> {
    log::info!("Starting backup for job: {}", job.name);

    let config_dir = validation::derive_config_dir(&job.root_dir);
    match server_ini::read_ark_rcon_settings(&config_dir) {
        Ok(rcon) if rcon.rcon_enabled => {
            if let (Some(port), Some(password)) = (rcon.rcon_port, rcon.admin_password) {
                log::info!("RCON: sending SaveWorld before ARK backup on 127.0.0.1:{}", port);
                let password_clone = password.clone();
                let save_result = tokio::task::spawn_blocking(move || {
                    run_rcon_commands(
                        "127.0.0.1".to_string(),
                        port,
                        password_clone,
                        vec!["SaveWorld"],
                    )
                })
                .await
                .context("ARK SaveWorld RCON task failed")?;
                match save_result {
                    Ok(()) => {
                        log::info!("Waiting 3 seconds for ARK save to flush to disk...");
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    }
                    // Nothing listening: the server is stopped, so the files on disk are already
                    // final and safe to back up (this is the cleanest time to take a backup).
                    Err(e) if is_connect_error(&e) => {
                        log::warn!(
                            "RCON: could not connect to 127.0.0.1:{} ({:#}); the server appears to be stopped. Backing up the saved files without SaveWorld.",
                            port,
                            e
                        );
                    }
                    // Server is running but rejected the password or the save failed: don't back up
                    // a world that may be mid-save.
                    Err(e) => return Err(e.context("ARK SaveWorld before backup failed")),
                }
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
            cleanup_old_backups(&job.destination_dir, &job.name, job.retention_days)?;
            Ok(file_size)
        }
        Err(e) if is_disk_full_error(&e) => {
            log::warn!("Backup failed due to insufficient disk space. Cleaning up old backups and retrying...");
            
            // Cleanup old backups to free space
            cleanup_old_backups(&job.destination_dir, &job.name, job.retention_days)
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
    let _temp_guard = TempFileGuard::new(temp_path);
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
        anyhow::bail!("Saves directory does not exist: {}", saves_dir.display());
    }

    // Expected map file name: all maps have _WP suffix in the .ark file name
    let expected_map_file = &map.map_file_name;
    let mut map_added = false;

    for entry in WalkDir::new(saves_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        // Put all files in "SavedArks" folder in ZIP
        let zip_path = format!("SavedArks/{}", file_name);

        match ext {
            // The map save is the core of the backup: it must be included or the backup fails.
            "ark" if include_map && file_name == expected_map_file => {
                add_single_file_to_zip(zip, path, &zip_path, options)?;
                map_added = true;
            }
            // Individual player/tribe files are skipped with a warning if locked.
            "arkprofile" | "arktribe" if include_saves => {
                add_optional_file_to_zip(zip, path, &zip_path, options)?;
            }
            _ => {}
        }
    }

    if include_map && !map_added {
        anyhow::bail!(
            "Map save file {} was not found in {}. Check the map's file name in Settings.",
            expected_map_file,
            saves_dir.display()
        );
    }

    Ok(())
}

fn add_ini_files_to_zip(
    zip: &mut ZipWriter<fs::File>,
    config_dir: &Path,
    options: &FileOptions,
) -> Result<()> {
    add_single_file_to_zip(zip, &config_dir.join("Game.ini"), "INI Settings/Game.ini", options)?;
    add_single_file_to_zip(
        zip,
        &config_dir.join("GameUserSettings.ini"),
        "INI Settings/GameUserSettings.ini",
        options,
    )
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
            add_optional_file_to_zip(zip, path, &zip_path, options)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MapDefinition;

    fn island() -> MapDefinition {
        MapDefinition {
            id: "TheIsland".into(),
            display_name: "The Island".into(),
            folder_name: "TheIsland_WP".into(),
            map_file_name: "TheIsland_WP.ark".into(),
        }
    }

    fn zip_entries(path: &Path) -> Vec<String> {
        let archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        archive.file_names().map(String::from).collect()
    }

    #[test]
    fn saves_include_map_and_profiles() {
        let temp = tempfile::tempdir().unwrap();
        let saves = temp.path().join("saves");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("TheIsland_WP.ark"), b"map").unwrap();
        fs::write(saves.join("0123.arkprofile"), b"profile").unwrap();
        fs::write(saves.join("TheIsland_WP_old.ark"), b"older map backup").unwrap();

        let zip_path = temp.path().join("out.zip");
        let mut zip = ZipWriter::new(fs::File::create(&zip_path).unwrap());
        add_saves_to_zip(&mut zip, &saves, true, true, &island(), &FileOptions::default()).unwrap();
        zip.finish().unwrap();

        let mut entries = zip_entries(&zip_path);
        entries.sort();
        assert_eq!(entries, vec!["SavedArks/0123.arkprofile", "SavedArks/TheIsland_WP.ark"]);
    }

    #[test]
    fn missing_map_file_fails_backup() {
        let temp = tempfile::tempdir().unwrap();
        let saves = temp.path().join("saves");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("0123.arkprofile"), b"profile").unwrap();

        let mut zip = ZipWriter::new(fs::File::create(temp.path().join("out.zip")).unwrap());
        let err = add_saves_to_zip(&mut zip, &saves, true, true, &island(), &FileOptions::default()).unwrap_err();
        assert!(err.to_string().contains("TheIsland_WP.ark was not found"), "{}", err);
    }
}
