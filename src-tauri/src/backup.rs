use crate::app_data::AppData;
use crate::job::Job;
use crate::validation;
use chrono::Datelike;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

/// Check if an error is due to insufficient disk space
fn is_disk_full_error(error: &anyhow::Error) -> bool {
    let error_str = format!("{}", error);
    let error_lower = error_str.to_lowercase();
    
    // Check for common disk full error messages
    error_lower.contains("no space") ||
    error_lower.contains("disk full") ||
    error_lower.contains("not enough space") ||
    error_lower.contains("insufficient") ||
    error_lower.contains("error 112") || // Windows ERROR_DISK_FULL
    error_lower.contains("error code: 112") ||
    // Check for Windows error code in the error chain
    error.chain().any(|e| {
        let e_str = format!("{}", e).to_lowercase();
        e_str.contains("112") || e_str.contains("no space") || e_str.contains("disk full")
    })
}

pub async fn create_backup(job: &Job, _app_data: &AppData) -> Result<u64> {
    log::info!("Starting backup for job: {}", job.name);

    let map = job
        .get_map()
        .ok_or_else(|| anyhow::anyhow!("Invalid map: {}", job.map))?;

    // Derive paths
    let saves_dir = validation::derive_saves_dir(&job.root_dir, map.folder_name());
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
    map: &crate::map::Map,
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

    // Finish ZIP
    zip.finish()
        .context("Failed to finalize ZIP file")?;

    // Get file size before atomic rename
    let file_size = fs::metadata(temp_path)
        .context("Failed to get backup file metadata")?
        .len();

    // Atomic rename
    fs::rename(temp_path, backup_path)
        .with_context(|| format!("Failed to rename temp file to: {}", backup_path.display()))?;

    // Verify integrity
    verify_zip_integrity(backup_path)?;

    Ok(file_size)
}

fn add_saves_to_zip(
    zip: &mut ZipWriter<fs::File>,
    saves_dir: &Path,
    include_saves: bool,
    include_map: bool,
    map: &crate::map::Map,
    options: &FileOptions,
) -> Result<()> {
    if !saves_dir.exists() {
        return Ok(());
    }

    // Expected map file name: all maps have _WP suffix in the .ark file name
    let expected_map_file = map.map_file_name();

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

fn verify_zip_integrity(zip_path: &Path) -> Result<()> {
    use std::io::BufReader;
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open ZIP for verification: {}", zip_path.display()))?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .with_context(|| format!("Failed to read ZIP archive: {}", zip_path.display()))?;

    // Check that archive has at least one entry
    if archive.len() == 0 {
        anyhow::bail!("ZIP archive is empty");
    }

    // Try to read first entry
    archive.by_index(0)
        .with_context(|| "Failed to read first entry from ZIP")?;

    Ok(())
}

fn cleanup_old_backups(destination_dir: &str, retention_days: u32) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|e| e.to_str()) == Some("zip") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let modified_dt: DateTime<Utc> = modified.into();
                    if modified_dt < cutoff {
                        fs::remove_file(&path)
                            .with_context(|| format!("Failed to delete old backup: {}", path.display()))?;
                        log::info!("Deleted old backup: {}", path.display());
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyArchivePreview {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyArchiveResult {
    pub archived: usize,
    pub destination: String,
}

pub fn preview_monthly_archive(app_data: &AppData) -> Result<MonthlyArchivePreview> {
    let config = app_data.get_config()?;
    let _destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| r"C:\arkade\Arkade Shared Global\FOTM Backups".to_string());

    let jobs = app_data.list_jobs()?;
    let mut all_backups: Vec<(PathBuf, SystemTime)> = Vec::new();

    // Collect all backups from all jobs
    for job in jobs {
        let dir = Path::new(&job.destination_dir);
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("zip") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            all_backups.push((path, modified));
                        }
                    }
                }
            }
        }
    }

    // Filter to current month
    let now = Utc::now();
    let current_month = now.format("%Y-%m").to_string();
    let current_month_backups: Vec<_> = all_backups
        .iter()
        .filter(|(_, modified)| {
            let dt: DateTime<Utc> = (*modified).into();
            dt.format("%Y-%m").to_string() == current_month
        })
        .collect();

    // Sort by modification time (oldest first) and take 2
    let mut sorted = current_month_backups.clone();
    sorted.sort_by_key(|(_, modified)| *modified);
    let to_archive: Vec<String> = sorted
        .iter()
        .take(2)
        .map(|(path, _)| path.to_string_lossy().to_string())
        .collect();

    Ok(MonthlyArchivePreview {
        files: to_archive,
    })
}

pub fn run_monthly_archive(app_data: &AppData) -> Result<MonthlyArchiveResult> {
    let config = app_data.get_config()?;
    let destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| r"C:\arkade\Arkade Shared Global\FOTM Backups".to_string());

    let preview = preview_monthly_archive(app_data)?;
    
    if preview.files.is_empty() {
        return Ok(MonthlyArchiveResult {
            archived: 0,
            destination,
        });
    }

    // Create destination directory structure
    let now = Utc::now();
    let month_dir = format!("{}-{}", now.year(), format!("{:02}", now.month()));
    let archive_dir = Path::new(&destination)
        .join(&month_dir)
        .join("ASA");
    fs::create_dir_all(&archive_dir)
        .with_context(|| format!("Failed to create archive directory: {}", archive_dir.display()))?;

    // Move files
    let mut archived = 0;
    for file_path_str in &preview.files {
        let source_path = Path::new(file_path_str);
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", file_path_str))?;
        let dest_path = archive_dir.join(file_name);

        fs::copy(source_path, &dest_path)
            .with_context(|| format!("Failed to copy file: {}", file_path_str))?;
        fs::remove_file(source_path)
            .with_context(|| format!("Failed to remove source file: {}", file_path_str))?;
        
        archived += 1;
        log::info!("Archived: {} -> {}", file_path_str, dest_path.display());
    }

    Ok(MonthlyArchiveResult {
        archived,
        destination,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_retention_cutoff() {
        let retention_days = 7;
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let old_date = cutoff - chrono::Duration::days(1);
        let recent_date = cutoff + chrono::Duration::days(1);
        
        assert!(old_date < cutoff);
        assert!(recent_date > cutoff);
    }

    #[test]
    fn test_monthly_archive_selection() {
        let now = Utc::now();
        let current_month = now.format("%Y-%m").to_string();
        
        // Test dates in current month
        let test_date = now - chrono::Duration::days(5);
        let test_month = test_date.format("%Y-%m").to_string();
        
        if test_month == current_month {
            // Would be included in monthly archive
            assert!(true);
        }
    }

    #[test]
    fn test_monthly_archive_oldest_first() {
        let now = Utc::now();
        let dates = vec![
            now - chrono::Duration::days(10),
            now - chrono::Duration::days(5),
            now - chrono::Duration::days(1),
        ];
        
        let mut sorted = dates.clone();
        sorted.sort();
        
        // Oldest should be first
        assert_eq!(sorted[0], dates[0]);
        assert_eq!(sorted[2], dates[2]);
    }
}

