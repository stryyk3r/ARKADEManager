use crate::app_data::AppData;
use crate::job::Job;
use crate::palworld_rest;
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
use super::retention::cleanup_old_backups;
use super::shared::{
    add_single_file_to_zip, is_disk_full_error, rename_temp_to_backup, verify_zip_integrity,
};
pub async fn create_palworld_backup(job: &Job, app_data: &AppData) -> Result<u64> {
    log::info!("Starting Palworld backup for job: {}", job.name);

    let config_dir = validation::derive_palworld_config_dir(&job.root_dir);
    let rest_settings = server_ini::read_palworld_rest_settings(&config_dir)?;
    let host_override = job.rcon_host.as_deref(); // optional REST API host; defaults to 127.0.0.1

    palworld_rest::send_save(&rest_settings, host_override).await?;

    log::info!("Waiting 3 seconds for Palworld save to flush to disk...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let world_dir = validation::discover_palworld_world_dir(&job.root_dir)?;
    let config_dir = validation::derive_palworld_config_dir(&job.root_dir);

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.zip", job.name, timestamp);
    let backup_path = Path::new(&job.destination_dir).join(&filename);
    let temp_path = backup_path.with_extension("zip.tmp");

    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create destination directory: {}", parent.display())
        })?;
    }

    let result = attempt_palworld_backup(&temp_path, &backup_path, &world_dir, &config_dir).await;

    match result {
        Ok(file_size) => {
            log::info!(
                "Palworld backup completed: {} ({} bytes)",
                backup_path.display(),
                file_size
            );

            if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                log::warn!(
                    "MONTHLY: Failed to copy Palworld backup into monthly folder: {:#}",
                    e
                );
            }

            cleanup_old_backups(&job.destination_dir, job.retention_days)?;
            Ok(file_size)
        }
        Err(e) if is_disk_full_error(&e) => {
            log::warn!(
                "Palworld backup failed due to insufficient disk space. Cleaning up old backups and retrying..."
            );

            cleanup_old_backups(&job.destination_dir, job.retention_days)
                .context("Failed to cleanup old Palworld backups")?;

            log::info!("Retrying Palworld backup after cleanup...");

            let retry_result =
                attempt_palworld_backup(&temp_path, &backup_path, &world_dir, &config_dir).await;

            match retry_result {
                Ok(file_size) => {
                    log::info!(
                        "Palworld backup completed after cleanup: {} ({} bytes)",
                        backup_path.display(),
                        file_size
                    );

                    if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                        log::warn!(
                            "MONTHLY: Failed to copy Palworld backup into monthly folder: {:#}",
                            e
                        );
                    }

                    cleanup_old_backups(&job.destination_dir, job.retention_days)?;
                    Ok(file_size)
                }
                Err(retry_err) => {
                    if is_disk_full_error(&retry_err) {
                        Err(anyhow::anyhow!(
                            "Palworld backup failed even after cleaning up old backups. Still insufficient disk space: {}",
                            retry_err
                        ))
                    } else {
                        Err(retry_err)
                    }
                }
            }
        }
        Err(e) => Err(e),
    }
}

async fn attempt_palworld_backup(
    temp_path: &Path,
    backup_path: &Path,
    world_dir: &Path,
    config_dir: &Path,
) -> Result<u64> {
    let file = fs::File::create(temp_path)
        .with_context(|| format!("Failed to create backup file: {}", temp_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(5));

    add_palworld_files_to_zip(&mut zip, world_dir, config_dir, &options)?;

    let file = zip.finish().context("Failed to finalize Palworld ZIP file")?;
    file.sync_all().context("Failed to sync Palworld ZIP file to disk")?;
    drop(file);

    #[cfg(target_os = "windows")]
    std::thread::sleep(std::time::Duration::from_millis(100));

    let file_size = fs::metadata(temp_path)
        .context("Failed to get Palworld backup file metadata")?
        .len();

    rename_temp_to_backup(temp_path, backup_path)?;
    verify_zip_integrity(backup_path)?;

    Ok(file_size)
}

fn add_palworld_files_to_zip(
    zip: &mut ZipWriter<fs::File>,
    world_dir: &Path,
    config_dir: &Path,
    options: &FileOptions,
) -> Result<()> {
    let level_sav = world_dir.join("Level.sav");
    let level_meta = world_dir.join("LevelMeta.sav");
    let players_dir = world_dir.join("Players");
    let settings_ini = config_dir.join("PalWorldSettings.ini");

    add_single_file_to_zip(zip, &level_sav, "SaveGames/Level.sav", options)?;
    if level_meta.exists() {
        add_single_file_to_zip(zip, &level_meta, "SaveGames/LevelMeta.sav", options)?;
    }

    if players_dir.exists() {
        for entry in WalkDir::new(&players_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let relative = path
                    .strip_prefix(&players_dir)
                    .with_context(|| format!("Invalid Players path: {}", path.display()))?;
                let zip_path = format!(
                    "SaveGames/Players/{}",
                    relative.to_string_lossy().replace('\\', "/")
                );
                add_single_file_to_zip(zip, path, &zip_path, options)?;
            }
        }
    } else {
        log::warn!(
            "Players folder not found at {}; skipping player saves",
            players_dir.display()
        );
    }

    add_single_file_to_zip(zip, &settings_ini, "INI Settings/PalWorldSettings.ini", options)?;

    Ok(())
}
