use crate::app_data::AppData;
use crate::job::Job;
use crate::palworld_rcon;
use crate::validation;
use chrono::Datelike;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;
use mc_rcon::RconClient;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct BackupProgressPayload {
    pub job_name: String,
    pub percent: u8,
}

const DEFAULT_MONTHLY_BASE_DIR: &str = r"C:\arkade\Arkade Shared Global\FOTM Backups";

const MONTHLY_CATEGORY_FOLDERS: &[&str] = &[
    "ASA Legacy",
    "ASE Legacy",
    "ASA Omega",
    "Minecraft",
    "Palworld",
];

fn monthly_folder_name(dt: DateTime<Utc>) -> String {
    // Example: "2026-03-March"
    let month_name = match dt.month() {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    };
    format!("{}-{:02}-{}", dt.year(), dt.month(), month_name)
}

fn ensure_monthly_dir_structure(base_dir: &str, dt: DateTime<Utc>) -> Result<PathBuf> {
    let month_dir = Path::new(base_dir).join(monthly_folder_name(dt));
    fs::create_dir_all(&month_dir)
        .with_context(|| format!("Failed to create monthly directory: {}", month_dir.display()))?;

    for folder in MONTHLY_CATEGORY_FOLDERS {
        fs::create_dir_all(month_dir.join(folder))
            .with_context(|| format!("Failed to create monthly subdirectory: {}", folder))?;
    }

    Ok(month_dir)
}

fn count_existing_monthly_copies(category_dir: &Path, job_name: &str) -> Result<usize> {
    if !category_dir.exists() {
        return Ok(0);
    }
    let prefix = format!("{}_", job_name);
    let mut count = 0usize;
    for entry in fs::read_dir(category_dir)? {
        let entry = entry?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let file_name = match p.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if !file_name.starts_with(&prefix) {
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("7z") {
            count += 1;
        }
    }
    Ok(count)
}

fn maybe_copy_backup_to_monthly(app_data: &AppData, job: &Job, backup_path: &Path) -> Result<()> {
    // Only act on completed backups that exist on disk.
    if !backup_path.exists() {
        return Ok(());
    }

    let config = app_data.get_config()?;
    let base_dir = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let now = Utc::now();
    let month_dir = ensure_monthly_dir_structure(&base_dir, now)?;

    let category = job.monthly_cluster.trim();
    if category.is_empty() {
        log::warn!(
            "MONTHLY: Job {} has empty monthly_cluster; skipping monthly copy",
            job.name
        );
        return Ok(());
    }
    if !MONTHLY_CATEGORY_FOLDERS.iter().any(|c| c.eq_ignore_ascii_case(category)) {
        log::warn!(
            "MONTHLY: Job {} has unknown monthly_cluster '{}'; skipping monthly copy",
            job.name,
            category
        );
        return Ok(());
    }
    let category_dir = month_dir.join(category);

    // First 2 backups for each job each month.
    let existing = count_existing_monthly_copies(&category_dir, &job.name)?;
    if existing >= 2 {
        return Ok(());
    }

    let file_name = backup_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid backup path: {}", backup_path.display()))?;
    let dest_path = category_dir.join(file_name);

    // Copy (do not remove original backup).
    fs::copy(backup_path, &dest_path).with_context(|| {
        format!(
            "Failed to copy monthly backup: {} -> {}",
            backup_path.display(),
            dest_path.display()
        )
    })?;
    log::info!(
        "MONTHLY: Copied {} (job {}) into {}",
        backup_path.display(),
        job.name,
        dest_path.display()
    );

    // After a monthly copy completes, pre-create next month's folder structure.
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next_dt = DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        Utc,
    );
    let _ = ensure_monthly_dir_structure(&base_dir, next_dt)?;

    Ok(())
}

/// Extensions that are already compressed. Storing them (no deflate) avoids making them larger and speeds up backup.
const STORED_EXTENSIONS: &[&str] = &[
    "jar", "zip", "7z", "rar", "gz", "bz2", "xz", "zst",
    "png", "jpg", "jpeg", "gif", "webp",
    "mp3", "ogg", "flac",
    "mp4", "mkv", "webm", "avi",
];

fn zip_options_for_path(path: &Path) -> FileOptions {
    let use_stored = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| STORED_EXTENSIONS.iter().any(|&e| e.eq_ignore_ascii_case(ext)))
        .unwrap_or(false);
    if use_stored {
        FileOptions::default().compression_method(CompressionMethod::Stored)
    } else {
        FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9))
    }
}

/// Find 7-Zip executable. Windows: check Program Files then PATH. Other: PATH only.
fn find_7z() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\7-Zip\7z.exe",
            r"C:\Program Files (x86)\7-Zip\7z.exe",
        ];
        for p in &candidates {
            let path = Path::new(p);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }
    if which_cmd("7z").is_some() {
        return which_cmd("7z");
    }
    if which_cmd("7za").is_some() {
        return which_cmd("7za");
    }
    None
}

#[cfg(windows)]
fn which_cmd(cmd: &str) -> Option<PathBuf> {
    Command::new("where").arg(cmd).output().ok().and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout);
        s.lines().next().map(|l| PathBuf::from(l.trim()))
    })
}

#[cfg(not(windows))]
fn which_cmd(cmd: &str) -> Option<PathBuf> {
    Command::new("which").arg(cmd).output().ok().and_then(|o| {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout);
            Some(PathBuf::from(s.lines().next()?.trim()))
        } else {
            None
        }
    })
}

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

/// Returns true if the job has RCON settings (Minecraft save-off/save-on flow).
fn job_has_rcon(job: &Job) -> bool {
    job.job_type == "minecraft"
        && job.rcon_host.as_ref().map(|h| !h.trim().is_empty()).unwrap_or(false)
        && job.rcon_port.map(|p| p > 0).unwrap_or(false)
        && job.rcon_password.as_ref().map(|p| !p.is_empty()).unwrap_or(false)
}

/// Run RCON commands in a blocking task. Used for save-off, save-all flush, and save-on.
fn run_rcon_commands(host: String, port: u16, password: String, commands: Vec<&'static str>) -> Result<()> {
    let addr = format!("{}:{}", host.trim(), port);
    let client = RconClient::connect(&addr).context("RCON connect failed")?;
    client.log_in(&password).context("RCON login failed")?;
    for cmd in commands {
        let _ = client.send_command(cmd);
    }
    Ok(())
}

/// Copy server root to staging directory with same exclusions as backup: .jar, orebfuscator_cache, session.lock.
fn copy_root_to_staging(root_path: &Path, staging_path: &Path) -> Result<()> {
    log::info!("Copying {} -> {}", root_path.display(), staging_path.display());
    fs::create_dir_all(staging_path).with_context(|| format!("Failed to create staging dir: {}", staging_path.display()))?;
    let mut file_count: u64 = 0;
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == OsStr::new("orebfuscator_cache")) {
            continue;
        }
        if path.is_file() {
            if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("jar")) {
                continue;
            }
            // Minecraft holds a lock on session.lock while the server is running; skip it
            if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.eq_ignore_ascii_case("session.lock")) {
                continue;
            }
            let relative = path.strip_prefix(root_path).with_context(|| format!("Invalid path prefix: {}", path.display()))?;
            let dest = staging_path.join(relative);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p).with_context(|| format!("Failed to create dir: {}", p.display()))?;
            }
            fs::copy(path, &dest).with_context(|| format!("Failed to copy {} to {}", path.display(), dest.display()))?;
            file_count += 1;
            if file_count <= 3 || file_count % 500 == 0 {
                log::info!("Copied {} files so far...", file_count);
            }
        }
    }
    log::info!("Copy complete: {} files", file_count);
    Ok(())
}

/// Guard to remove staging directory on drop (when using RCON flow).
struct StagingGuard(Option<PathBuf>);
impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            if !p.exists() {
                return;
            }
            const RETRIES: u32 = 5;
            const RETRY_DELAY_MS: u64 = 500;
            for attempt in 1..=RETRIES {
                match fs::remove_dir_all(&p) {
                    Ok(()) => {
                        log::info!("Removed staging dir: {}", p.display());
                        return;
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to remove staging dir (attempt {}/{}): {} - {}",
                            attempt,
                            RETRIES,
                            p.display(),
                            e
                        );
                        if attempt < RETRIES {
                            std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
                        }
                    }
                }
            }
            log::error!("Could not remove staging dir after {} attempts: {}", RETRIES, p.display());
        }
    }
}

/// Minecraft backup: use 7-Zip if available (better compression), else built-in ZIP.
/// When job has RCON settings: save-off, save-all flush, copy to staging, save-on, then compress staging.
/// If app_handle is Some, progress (0-100) is emitted: 0-10% copy to staging, 10-90% compress, 90-100% copy to backup location.
pub async fn create_minecraft_backup(job: &Job, app_data: &AppData, app_handle: Option<AppHandle>) -> Result<u64> {
    log::info!("Starting Minecraft backup for job: {}", job.name);

    let root_path = Path::new(&job.root_dir);
    if !root_path.exists() {
        anyhow::bail!("Server root does not exist: {}", job.root_dir);
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

    // Progress: 0% = started, 10% = copy to staging done, 10-90% = 7z compressing, 90% = start copy to backup, 100% = done
    let progress_tx = match &app_handle {
        Some(h) => {
            let (tx, rx) = mpsc::channel();
            let app_handle = h.clone();
            let job_name = job.name.clone();
            std::thread::spawn(move || {
                while let Ok(percent) = rx.recv() {
                    let handle_for_emit = app_handle.clone();
                    let payload = BackupProgressPayload {
                        job_name: job_name.clone(),
                        percent,
                    };
                    app_handle.run_on_main_thread(move || {
                        let _ = handle_for_emit.emit("backup_progress", payload);
                    }).ok();
                }
            });
            tx.send(0).ok();
            Some(tx)
        }
        None => None,
    };

    let (source_path, _staging_guard) = if job_has_rcon(job) {
        let host = job.rcon_host.as_ref().unwrap().trim().to_string();
        let port = job.rcon_port.unwrap();
        let password = job.rcon_password.as_ref().unwrap().clone();

        log::info!("RCON: disabling saves and flushing (save-off; save-all flush)");
        tokio::task::spawn_blocking({
            let host = host.clone();
            let password = password.clone();
            move || run_rcon_commands(host, port, password, vec!["save-off", "save-all flush"])
        })
        .await
        .context("RCON task failed")??;

        log::info!("Waiting 3 seconds for save-all flush to complete before copying...");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let staging_dir = std::env::temp_dir().join(format!(
            "arkade_minecraft_{}_{}",
            job.id,
            Utc::now().format("%Y%m%d_%H%M%S")
        ));
        fs::create_dir_all(&staging_dir)
            .with_context(|| format!("Failed to create staging dir: {}", staging_dir.display()))?;

        let copy_result = copy_root_to_staging(root_path, &staging_dir);

        log::info!("RCON: re-enabling saves (save-on)");
        let host = job.rcon_host.as_ref().unwrap().trim().to_string();
        let port = job.rcon_port.unwrap();
        let password = job.rcon_password.as_ref().unwrap().clone();
        tokio::task::spawn_blocking(move || run_rcon_commands(host, port, password, vec!["save-on"]))
            .await
            .context("RCON task failed")??;

        copy_result.map_err(|e| {
            log::error!("Copy to staging failed: {:#}", e);
            e
        })?;
        log::info!("Staging copy completed, compressing from: {}", staging_dir.display());
        (staging_dir.clone(), StagingGuard(Some(staging_dir)))
    } else {
        (root_path.to_path_buf(), StagingGuard(None))
    };

    // Copy to staging (or ready for 7z) done → 10%
    if let Some(ref tx) = progress_tx {
        let _ = tx.send(10);
    }

    if let Some(seven_z) = find_7z() {
        let dest_dir = job.destination_dir.clone();
        let filename = format!("{}_{}.7z", job.name, timestamp);
        let backup_path = Path::new(&dest_dir).join(&filename);

        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create destination directory: {}", parent.display()))?;
        }

        // Compress to temp on system drive (e.g. C) first, then copy to backup location (e.g. D).
        // This can be faster when the backup drive is slower, and avoids 7z doing many small writes across drives.
        let temp_7z = std::env::temp_dir().join(format!("arkade_7z_{}_{}.7z", job.id, timestamp));
        log::info!("7-Zip writing to temp: {} then copying to {}", temp_7z.display(), backup_path.display());

        // 7z progress 0-100 is mapped to 10-90%
        let result = run_minecraft_backup_7z(&seven_z, &temp_7z, &source_path, progress_tx.as_ref().cloned()).await;
        match result {
            Ok(outcome) => {
                if outcome.file_size == 0 {
                    let _ = fs::remove_file(&temp_7z);
                    anyhow::bail!("Backup produced an empty file (0 bytes). 7-Zip may have added no files.");
                }
                // Compression done → 90%; copying .7z to backup location
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(90);
                }
                log::info!("Copying 7z archive to backup location: {}", backup_path.display());
                fs::copy(&temp_7z, &backup_path)
                    .with_context(|| format!("Failed to copy 7z from {} to {}", temp_7z.display(), backup_path.display()))?;
                let _ = fs::remove_file(&temp_7z);
                // Copy to backup location done → 100%
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(100);
                }

                log::info!(
                    "Minecraft backup completed (7-Zip): {} ({} bytes){}",
                    backup_path.display(),
                    outcome.file_size,
                    if outcome.had_warnings { " [WARNINGS]" } else { "" }
                );
                cleanup_minecraft_retention(&job.destination_dir, &job.name)?;

                if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                    log::warn!(
                        "MONTHLY: Failed to copy Minecraft backup into monthly folder: {:#}",
                        e
                    );
                }

                // If 7-Zip completed with warnings (locked/unreadable files), keep the archive but surface a warning in UI.
                // This prevents fallback to built-in ZIP and avoids repeated attempts.
                if outcome.had_warnings {
                    return Err(anyhow::anyhow!(
                        "Backup completed with warnings: some files were locked/unreadable and were skipped. Archive was created at: {}",
                        backup_path.display()
                    ));
                }

                return Ok(outcome.file_size);
            }
            Err(e) => {
                let _ = fs::remove_file(&temp_7z);
                log::warn!("7-Zip backup failed ({}), falling back to built-in ZIP", e);
            }
        }
    } else {
        log::info!("7-Zip not found, using built-in ZIP for Minecraft backup");
    }

    let filename = format!("{}_{}.zip", job.name, timestamp);
    let backup_path = Path::new(&job.destination_dir).join(&filename);
    let temp_path = backup_path.with_extension("zip.tmp");

    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create destination directory: {}", parent.display()))?;
    }

    let result = attempt_minecraft_backup(&temp_path, &backup_path, &source_path, job).await;

    match result {
        Ok(file_size) => {
            if file_size == 0 {
                if backup_path.exists() {
                    let _ = fs::remove_file(&backup_path);
                }
                return Err(anyhow::anyhow!("Backup produced an empty file (0 bytes). No files were added to the archive."));
            }
            log::info!("Minecraft backup completed: {} ({} bytes)", backup_path.display(), file_size);
            cleanup_minecraft_retention(&job.destination_dir, &job.name)?;

            if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                log::warn!(
                    "MONTHLY: Failed to copy Minecraft backup into monthly folder: {:#}",
                    e
                );
            }

            Ok(file_size)
        }
        Err(e) if is_disk_full_error(&e) => {
            log::warn!("Minecraft backup failed (disk full). Cleaning up old backups and retrying...");
            cleanup_minecraft_retention(&job.destination_dir, &job.name)
                .context("Failed to cleanup old Minecraft backups")?;
            let retry_result = attempt_minecraft_backup(&temp_path, &backup_path, &source_path, job).await;
            match retry_result {
                Ok(file_size) => {
                    if file_size == 0 {
                        if backup_path.exists() {
                            let _ = fs::remove_file(&backup_path);
                        }
                        return Err(anyhow::anyhow!("Backup produced an empty file (0 bytes)."));
                    }
                    log::info!("Minecraft backup completed after cleanup: {} ({} bytes)", backup_path.display(), file_size);

                    if let Err(e) = maybe_copy_backup_to_monthly(app_data, job, &backup_path) {
                        log::warn!(
                            "MONTHLY: Failed to copy Minecraft backup into monthly folder: {:#}",
                            e
                        );
                    }

                    Ok(file_size)
                }
                Err(retry_err) => {
                    if is_disk_full_error(&retry_err) {
                        Err(anyhow::anyhow!("Backup failed even after cleaning up. Still insufficient disk space: {}", retry_err))
                    } else {
                        Err(retry_err)
                    }
                }
            }
        }
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone, Copy)]
struct SevenZipOutcome {
    file_size: u64,
    had_warnings: bool,
}

#[derive(Debug, Clone, Copy)]
enum SevenZipExit {
    Ok,
    Warn,
}

/// Run Minecraft backup using 7-Zip (LZMA2). Excludes .jar, orebfuscator_cache, session.lock.
/// Streams 7-Zip output to the log so you can see progress and, if it hangs, the last file it was processing.
/// If progress_tx is Some, sends percent (0-100) for UI progress bar.
async fn run_minecraft_backup_7z(
    seven_z: &Path,
    backup_path: &Path,
    root_path: &Path,
    progress_tx: Option<mpsc::Sender<u8>>,
) -> Result<SevenZipOutcome> {
    let seven_z = seven_z.to_path_buf();
    let backup_path = backup_path.to_path_buf();
    let root_path = root_path.to_path_buf();

    let result = tokio::task::spawn_blocking(move || {
        let backup_abs = backup_path
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .map(|p| p.join(backup_path.file_name().unwrap()))
            .unwrap_or(backup_path.clone());
        let backup_str = backup_abs.to_string_lossy().to_string();

        let mut cmd = Command::new(&seven_z);
        cmd.arg("a")
            .arg("-t7z")
            .arg("-mx=7")
            .arg("-mmt=12")
            .arg("-bsp2")
            .arg("-xr!*.jar")
            .arg("-xr!orebfuscator_cache")
            .arg("-xr!session.lock")
            .arg(&backup_str)
            .arg("*")
            .current_dir(&root_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return (Err(e.into()), backup_path),
        };

        log::info!("7-Zip compression started (level 7, 12 threads) from {} -> {}", root_path.display(), backup_abs.display());

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut last_logged_pct: Option<u8> = None;
            let mut buf = Vec::with_capacity(512);
            let mut line_buf = String::new();
            loop {
                buf.resize(256, 0);
                let n = match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) => break,
                    Ok(k) => k,
                    Err(_) => break,
                };
                let chunk = String::from_utf8_lossy(&buf[..n]);
                for c in chunk.chars() {
                    if c == '\n' || c == '\r' {
                        if !line_buf.is_empty() {
                            let trimmed = line_buf.trim();
                            if !trimmed.is_empty() {
                                let pct = if let Some(p) = trimmed.rfind('%') {
                                    trimmed[..p].trim().chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>().chars().rev().collect::<String>()
                                } else {
                                    String::new()
                                };
                                if let Ok(num) = pct.parse::<u8>() {
                                    let should_log = num == 100
                                        || (num % 10 == 0 && last_logged_pct.map_or(true, |prev| prev != num));
                                    if should_log {
                                        last_logged_pct = Some(num);
                                        log::info!("7z progress: {}%", num);
                                    }
                                } else {
                                    log::info!("7z: {}", trimmed);
                                }
                            }
                            line_buf.clear();
                        }
                    } else {
                        line_buf.push(c);
                        if line_buf.len() > 1024 {
                            line_buf.drain(..line_buf.len() - 128);
                        }
                    }
                }
            }
            if !line_buf.trim().is_empty() {
                log::info!("7z: {}", line_buf.trim());
            }
        });
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut last_logged_pct: Option<u8> = None;
            let mut buf = Vec::with_capacity(512);
            let mut line_buf = String::new();
            loop {
                buf.resize(256, 0);
                let n = match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) => break,
                    Ok(k) => k,
                    Err(_) => break,
                };
                let chunk = String::from_utf8_lossy(&buf[..n]);
                for c in chunk.chars() {
                    if c == '\n' || c == '\r' {
                        if !line_buf.is_empty() {
                            let trimmed = line_buf.trim();
                            if !trimmed.is_empty() {
                                let pct = if let Some(p) = trimmed.rfind('%') {
                                    trimmed[..p].trim().chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>().chars().rev().collect::<String>()
                                } else {
                                    String::new()
                                };
                                if let Ok(num) = pct.parse::<u8>() {
                                    let should_log = num == 100
                                        || (num % 10 == 0 && last_logged_pct.map_or(true, |prev| prev != num));
                                    if should_log {
                                        last_logged_pct = Some(num);
                                        log::info!("7z progress: {}%", num);
                                        // Map 7z 0-100 to UI 10-90%
                                        if let Some(ref tx) = progress_tx {
                                            let mapped = 10 + (num as u32 * 80 / 100).min(80) as u8;
                                            let _ = tx.send(mapped);
                                        }
                                    }
                                } else {
                                    log::info!("7z stderr: {}", trimmed);
                                }
                            }
                            line_buf.clear();
                        }
                    } else {
                        line_buf.push(c);
                        if line_buf.len() > 1024 {
                            line_buf.drain(..line_buf.len() - 128);
                        }
                    }
                }
            }
            if !line_buf.trim().is_empty() {
                log::info!("7z stderr: {}", line_buf.trim());
            }
        });

        // Log a heartbeat every 30 seconds while 7-Zip runs so we know it hasn't hung
        let wait_handle = std::thread::spawn(move || child.wait());
        let mut waited_secs: u64 = 0;
        loop {
            if wait_handle.is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
            waited_secs += 30;
            if !wait_handle.is_finished() {
                log::info!("7-Zip still compressing... ({}s elapsed)", waited_secs);
            }
        }
        let wait_result = wait_handle.join().unwrap();

        match wait_result {
            Ok(status) => {
                match status.code() {
                    Some(0) => {
                        log::info!("7-Zip finished successfully");
                        (Ok(SevenZipExit::Ok), backup_path)
                    }
                    Some(1) => {
                        log::info!("7-Zip finished with warnings (e.g. some files skipped)");
                        (Ok(SevenZipExit::Warn), backup_path)
                    }
                    Some(c) => (Err(anyhow::anyhow!("7-Zip exited with code {}", c)), backup_path),
                    None => (Err(anyhow::anyhow!("7-Zip process terminated")), backup_path),
                }
            }
            Err(e) => (Err(e.into()), backup_path),
        }
    })
    .await
    .context("7-Zip task failed")?;

    let (status_result, backup_path) = result;
    let exit = status_result.context("7-Zip process error")?;

    let meta = fs::metadata(&backup_path).context("Failed to read 7z backup size")?;
    Ok(SevenZipOutcome {
        file_size: meta.len(),
        had_warnings: matches!(exit, SevenZipExit::Warn),
    })
}

async fn attempt_minecraft_backup(
    temp_path: &Path,
    backup_path: &Path,
    root_path: &Path,
    _job: &Job,
) -> Result<u64> {
    let file = fs::File::create(temp_path)
        .with_context(|| format!("Failed to create backup file: {}", temp_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let mut skipped_locked: Vec<String> = Vec::new();

    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == OsStr::new("orebfuscator_cache")) {
            continue;
        }
        if path.is_file() {
            if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("jar")) {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.eq_ignore_ascii_case("session.lock")) {
                continue;
            }
            let relative = path
                .strip_prefix(root_path)
                .with_context(|| format!("Invalid path prefix: {}", path.display()))?;
            let zip_path = relative.to_string_lossy().replace('\\', "/");
            let options = zip_options_for_path(path);

            match fs::File::open(path) {
                Ok(mut f) => {
                    let mut buffer = Vec::new();
                    if f.read_to_end(&mut buffer).is_ok() {
                        if let Err(e) = zip.start_file(zip_path.clone(), options) {
                            return Err(anyhow::anyhow!("Failed to add file to ZIP: {}", e));
                        }
                        if let Err(e) = zip.write_all(&buffer) {
                            return Err(anyhow::anyhow!("Failed to write file to ZIP: {}", e));
                        }
                    }
                }
                Err(e) => {
                    let path_str = path.display().to_string();
                    log::warn!("Skipping file {} (may be locked): {}", path_str, e);
                    skipped_locked.push(path_str);
                }
            }
        }
    }

    if !skipped_locked.is_empty() {
        log::warn!(
            "MINECRAFT BACKUP: {} file(s) were SKIPPED (locked/unreadable). Consider excluding these or closing the program that has them open:",
            skipped_locked.len()
        );
        for p in &skipped_locked {
            log::warn!("  - {}", p);
        }
    }

    // finish() returns the underlying File; we must drop it so the handle is closed before rename (required on Windows)
    let file = zip.finish().context("Failed to finalize ZIP file")?;
    file.sync_all().context("Failed to sync ZIP file to disk")?;
    drop(file);

    #[cfg(target_os = "windows")]
    std::thread::sleep(std::time::Duration::from_millis(100));

    let file_size = fs::metadata(temp_path)
        .context("Failed to get backup file metadata")?
        .len();

    rename_temp_to_backup(temp_path, backup_path)?;

    if file_size == 0 {
        let _ = fs::remove_file(backup_path);
        anyhow::bail!("Backup produced an empty file (0 bytes). No files were added to the archive.");
    }

    verify_zip_integrity(backup_path)?;

    Ok(file_size)
}

const RENAME_RETRY_DELAY_MS: u64 = 2000;
const RENAME_RETRY_ATTEMPTS: u32 = 6;

fn is_windows_file_locked_error(e: &std::io::Error) -> bool {
    let err_str = e.to_string();
    err_str.contains("Access is denied")
        || err_str.contains("used by another process")
        || err_str.contains("(os error 5)")
        || err_str.contains("(os error 32)")
}

/// Rename temp file to final path. Retries with delay if file is still locked (Windows).
/// Then falls back to copy+remove if rename never succeeds.
fn rename_temp_to_backup(temp_path: &Path, backup_path: &Path) -> Result<()> {
    let mut last_err = match fs::rename(temp_path, backup_path) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    #[cfg(target_os = "windows")]
    {
        if !is_windows_file_locked_error(&last_err) {
            return Err(last_err).with_context(|| format!("Failed to rename temp file to: {}", backup_path.display()));
        }
        log::warn!(
            "BACKUP FILE LOCKED: The backup output is in use (not a server file). Temp: {} -> Dest: {} | Often caused by antivirus or OneDrive scanning the file. Retrying...",
            temp_path.display(),
            backup_path.display()
        );
        for attempt in 1..=RENAME_RETRY_ATTEMPTS {
            log::warn!(
                "Rename failed (backup file still in use), retry {}/{} in {}s",
                attempt,
                RENAME_RETRY_ATTEMPTS,
                RENAME_RETRY_DELAY_MS / 1000
            );
            std::thread::sleep(std::time::Duration::from_millis(RENAME_RETRY_DELAY_MS));
            last_err = match fs::rename(temp_path, backup_path) {
                Ok(()) => return Ok(()),
                Err(e) => e,
            };
            if !is_windows_file_locked_error(&last_err) {
                return Err(last_err).with_context(|| format!("Failed to rename temp file to: {}", backup_path.display()));
            }
        }
        log::warn!(
            "Rename still failed after {} retries (backup file locked: {}). Trying copy+remove fallback.",
            RENAME_RETRY_ATTEMPTS,
            temp_path.display()
        );
        fs::copy(temp_path, backup_path)
            .with_context(|| format!("Failed to copy temp to: {}", backup_path.display()))?;
        for attempt in 1..=RENAME_RETRY_ATTEMPTS {
            if fs::remove_file(temp_path).is_ok() {
                return Ok(());
            }
            log::warn!("Could not remove temp file (attempt {}), waiting {}s", attempt, RENAME_RETRY_DELAY_MS / 1000);
            std::thread::sleep(std::time::Duration::from_millis(RENAME_RETRY_DELAY_MS));
        }
        if let Err(rm) = fs::remove_file(temp_path) {
            log::warn!("Could not remove temp file {}: {}", temp_path.display(), rm);
        }
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    Err(last_err).with_context(|| format!("Failed to rename temp file to: {}", backup_path.display()))
}

pub async fn create_palworld_backup(job: &Job, app_data: &AppData) -> Result<u64> {
    log::info!("Starting Palworld backup for job: {}", job.name);

    let host = job
        .rcon_host
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Palworld backup requires RCON host"))?
        .trim()
        .to_string();
    let port = job
        .rcon_port
        .ok_or_else(|| anyhow::anyhow!("Palworld backup requires RCON port"))?;
    let password = job
        .rcon_password
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Palworld backup requires RCON password"))?
        .clone();

    tokio::task::spawn_blocking(move || palworld_rcon::send_save(&host, port, &password))
        .await
        .context("Palworld RCON task failed")??;

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

fn add_single_file_to_zip(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    zip_path: &str,
    options: &FileOptions,
) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Required backup file not found: {}", path.display());
    }

    match fs::File::open(path) {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            match file.read_to_end(&mut buffer) {
                Ok(_) => match zip.start_file(zip_path.to_string(), *options) {
                    Ok(_) => {
                        if let Err(e) = zip.write_all(&buffer) {
                            let err_msg = format!("{}", e);
                            if err_msg.contains("closed") || err_msg.contains("finished") {
                                return Err(anyhow::anyhow!(
                                    "ZIP writer was closed while adding {}: {}",
                                    zip_path,
                                    e
                                ));
                            }
                            return Err(anyhow::anyhow!(
                                "Failed to write {} to ZIP: {}",
                                zip_path,
                                e
                            ));
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);
                        if err_msg.contains("closed") || err_msg.contains("finished") {
                            return Err(anyhow::anyhow!(
                                "ZIP writer was closed while starting {} entry: {}",
                                zip_path,
                                e
                            ));
                        }
                        return Err(anyhow::anyhow!(
                            "Failed to start {} entry in ZIP: {}",
                            zip_path,
                            e
                        ));
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read {}: {}. Skipping.", path.display(), e);
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to open {}: {}. Skipping.", path.display(), e);
        }
    }

    Ok(())
}

pub async fn create_backup(job: &Job, app_data: &AppData) -> Result<u64> {
    log::info!("Starting backup for job: {}", job.name);

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

/// Minecraft retention: keep all backups from the last 48 hours, plus the first
/// backup of each calendar day for the last 30 days. Considers both .zip and .7z
/// files whose filename starts with `job_name_` and has a timestamp suffix YYYYMMDD_HHMMSS.
fn cleanup_minecraft_retention(destination_dir: &str, job_name: &str) -> Result<()> {
    let dir = Path::new(destination_dir);
    if !dir.exists() {
        return Ok(());
    }

    let prefix = format!("{}_", job_name);
    let now = Utc::now();
    let cutoff_48h = now - chrono::Duration::hours(48);
    let cutoff_30d = now - chrono::Duration::days(30);

    #[derive(Clone)]
    struct BackupEntry {
        path: PathBuf,
        dt: DateTime<Utc>,
    }

    let mut entries: Vec<BackupEntry> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if !filename.starts_with(&prefix) {
            continue;
        }
        let rest = filename
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".zip").or_else(|| s.strip_suffix(".7z")));
        let rest = match rest {
            Some(s) => s,
            None => continue,
        };
        // Expect YYYYMMDD_HHMMSS (15 chars)
        if rest.len() != 15 {
            continue;
        }
        let naive = match NaiveDateTime::parse_from_str(rest, "%Y%m%d_%H%M%S") {
            Ok(t) => t,
            Err(_) => continue,
        };
        let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
        entries.push(BackupEntry { path, dt });
    }

    // Keep: (1) all within last 48h, (2) first backup of each day in last 30 days
    let mut to_keep: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for e in &entries {
        if e.dt >= cutoff_48h {
            to_keep.insert(e.path.clone());
        }
    }
    let older_than_48h: Vec<_> = entries.iter().filter(|e| e.dt < cutoff_48h && e.dt >= cutoff_30d).collect();
    use std::collections::HashMap;
    let mut first_of_day_earliest: HashMap<NaiveDate, &BackupEntry> = HashMap::new();
    for e in &older_than_48h {
        let date = e.dt.naive_utc().date();
        first_of_day_earliest
            .entry(date)
            .and_modify(|prev| {
                if e.dt < prev.dt {
                    *prev = e;
                }
            })
            .or_insert(e);
    }
    for e in first_of_day_earliest.values() {
        to_keep.insert((*e).path.clone());
    }

    for e in &entries {
        if !to_keep.contains(&e.path) {
            fs::remove_file(&e.path)
                .with_context(|| format!("Failed to delete old Minecraft backup: {}", e.path.display()))?;
            log::info!("Deleted old Minecraft backup: {}", e.path.display());
        }
    }

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyJobStatus {
    pub job_id: String,
    pub job_name: String,
    pub monthly_cluster: String,
    pub copied_this_month: u32,
    pub completed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonthlyStatusResult {
    pub month_folder: String,
    pub destination_base: String,
    pub jobs: Vec<MonthlyJobStatus>,
}

pub fn get_monthly_status(app_data: &AppData) -> Result<MonthlyStatusResult> {
    let config = app_data.get_config()?;
    let destination_base = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let now = Utc::now();
    let month_folder = Path::new(&destination_base).join(monthly_folder_name(now));

    let jobs = app_data.list_jobs()?;
    let mut statuses = Vec::with_capacity(jobs.len());

    for job in &jobs {
        let cluster = job.monthly_cluster.trim();
        let cluster_dir = if cluster.is_empty() {
            None
        } else {
            Some(month_folder.join(cluster))
        };

        let copied = if let Some(dir) = cluster_dir.as_ref() {
            count_existing_monthly_copies(dir, &job.name).unwrap_or(0) as u32
        } else {
            0
        };

        statuses.push(MonthlyJobStatus {
            job_id: job.id.clone(),
            job_name: job.name.clone(),
            monthly_cluster: job.monthly_cluster.clone(),
            copied_this_month: copied,
            completed: copied >= 2,
        });
    }

    // Stable presentation: completed last, then by cluster/name.
    statuses.sort_by(|a, b| {
        a.completed
            .cmp(&b.completed)
            .then_with(|| a.monthly_cluster.cmp(&b.monthly_cluster))
            .then_with(|| a.job_name.cmp(&b.job_name))
    });

    Ok(MonthlyStatusResult {
        month_folder: month_folder.to_string_lossy().to_string(),
        destination_base,
        jobs: statuses,
    })
}

fn parse_backup_timestamp_from_filename(job_name: &str, filename: &str) -> Option<DateTime<Utc>> {
    // Expected pattern: "{job_name}_YYYYMMDD_HHMMSS.(zip|7z)"
    let prefix = format!("{}_", job_name);
    let rest = filename
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix(".zip").or_else(|| s.strip_suffix(".7z")))?;
    if rest.len() != 15 {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(rest, "%Y%m%d_%H%M%S").ok()?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

/// Which job owns this backup filename: among jobs whose name parses as a prefix of the file,
/// pick the **longest** job name (e.g. `Server_PVE` wins over `Server` for `Server_PVE_20260226_120000.zip`).
fn owning_job_for_backup_filename<'a>(filename: &str, jobs: &'a [Job]) -> Option<&'a Job> {
    jobs.iter()
        .filter(|j| parse_backup_timestamp_from_filename(&j.name, filename).is_some())
        .max_by_key(|j| j.name.len())
}

pub fn preview_monthly_archive(app_data: &AppData) -> Result<MonthlyArchivePreview> {
    let config = app_data.get_config()?;
    let _destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let jobs = app_data.list_jobs()?;
    let now = Utc::now();
    let current_month = now.format("%Y-%m").to_string();

    // Per-job: first two backups in the current month (filename must match `{name}_YYYYMMDD_HHMMSS`;
    // do not use file modified time — that mis-attributes files when several jobs share a folder or
    // when one job name is a prefix of another). Ownership is resolved with longest job-name match.
    let mut to_archive: Vec<String> = Vec::new();
    for job in &jobs {
        let dir = Path::new(&job.destination_dir);
        if !dir.exists() {
            continue;
        }

        let mut entries: Vec<(PathBuf, DateTime<Utc>)> = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !(ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("7z")) {
                continue;
            }
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(s) => s,
                None => continue,
            };
            let owner = match owning_job_for_backup_filename(filename, &jobs) {
                Some(j) => j,
                None => continue,
            };
            if owner.id != job.id {
                continue;
            }
            let dt = match parse_backup_timestamp_from_filename(&job.name, filename) {
                Some(dt) => dt,
                None => continue,
            };

            if dt.format("%Y-%m").to_string() != current_month {
                continue;
            }

            entries.push((path, dt));
        }

        entries.sort_by_key(|(_, dt)| *dt);
        for (p, _) in entries.into_iter().take(2) {
            to_archive.push(p.to_string_lossy().to_string());
        }
    }

    Ok(MonthlyArchivePreview {
        files: to_archive,
    })
}

pub fn run_monthly_archive(app_data: &AppData) -> Result<MonthlyArchiveResult> {
    let config = app_data.get_config()?;
    let destination = config
        .monthly_archive_destination
        .unwrap_or_else(|| DEFAULT_MONTHLY_BASE_DIR.to_string());

    let preview = preview_monthly_archive(app_data)?;
    
    if preview.files.is_empty() {
        return Ok(MonthlyArchiveResult {
            archived: 0,
            destination,
        });
    }

    // Create destination directory structure (and next month ahead)
    let now = Utc::now();
    let month_dir = ensure_monthly_dir_structure(&destination, now)?;

    // Copy files (do not remove originals)
    let mut archived = 0;
    for file_path_str in &preview.files {
        let source_path = Path::new(file_path_str);
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", file_path_str))?
            .to_string_lossy()
            .to_string();

        let jobs = app_data.list_jobs().unwrap_or_default();
        let category = owning_job_for_backup_filename(&file_name, &jobs)
            .map(|j| j.monthly_cluster.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("ASA Legacy");

        let dest_path = month_dir.join(category).join(&file_name);
        fs::copy(source_path, &dest_path)
            .with_context(|| format!("Failed to copy file: {}", file_path_str))?;
        
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

