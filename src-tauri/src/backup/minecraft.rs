use crate::app_data::AppData;
use crate::job::Job;
use crate::validation;
use anyhow::{Context, Result};
use chrono::Utc;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use zip::write::ZipWriter;

use super::monthly::maybe_copy_backup_to_monthly;
use super::rcon::{job_has_rcon, pause_minecraft_saving, SaveOnGuard};
use super::retention::cleanup_minecraft_retention;
use super::shared::{
    add_optional_file_to_zip, copy_large_file, find_7z, is_disk_full_error, rename_temp_to_backup,
    same_volume, verify_zip_integrity, zip_options_for_path, BackupProgressPayload, TempFileGuard,
};

/// 7-Zip level 3: Minecraft region files are already zlib-compressed, so higher levels cost far
/// more CPU and RAM (level 7 with 12 threads needed ~2 GB) for very little size reduction.
const SEVEN_ZIP_LEVEL: u32 = 3;

/// Use at most half the cores so the game servers on this machine keep the rest.
fn seven_zip_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(2)
}

/// True if a running Minecraft server holds `world/session.lock`. The server keeps an OS lock on
/// that file while the world is open (Minecraft 1.16+), so reading it fails while the server runs.
/// Used when a job has no RCON settings: without `save-off`, the world may only be copied while
/// the server is stopped.
fn minecraft_world_in_use(world_dir: &Path) -> bool {
    let lock_path = world_dir.join("session.lock");
    let result = fs::File::open(&lock_path).and_then(|mut f| {
        let mut buf = [0u8; 1];
        f.read(&mut buf)
    });
    match result {
        Ok(_) => false,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        // ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33)
        Err(e) if matches!(e.raw_os_error(), Some(32) | Some(33)) => true,
        Err(e) => {
            log::warn!(
                "Could not check {} ({}); assuming the server is running",
                lock_path.display(),
                e
            );
            true
        }
    }
}

/// Spot-check a copied world for signs it was caught mid-write: region files (.mca) are always a
/// whole number of 4 KiB sectors, and level.dat must decompress cleanly (gzip checks its CRC).
/// Returns one message per problem; an empty list means nothing suspicious was found.
fn verify_minecraft_world_copy(dirs: &[PathBuf]) -> Vec<String> {
    let mut problems = Vec::new();
    for dir in dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            let is_region = path.extension().map_or(false, |e| e.eq_ignore_ascii_case("mca"));
            let is_level_dat = path.file_name().map_or(false, |n| n.eq_ignore_ascii_case("level.dat"));
            if is_region {
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if len % 4096 != 0 {
                    problems.push(format!(
                        "{} is {} bytes, not whole 4 KiB sectors (may have been copied mid-write)",
                        path.display(),
                        len
                    ));
                }
            } else if is_level_dat {
                let result = fs::File::open(path).and_then(|f| {
                    let mut decoder = flate2::read::GzDecoder::new(BufReader::new(f));
                    std::io::copy(&mut decoder, &mut std::io::sink())
                });
                match result {
                    Ok(n) if n > 0 => {}
                    Ok(_) => problems.push(format!("{} is empty", path.display())),
                    Err(e) => problems.push(format!("{} is not readable ({})", path.display(), e)),
                }
            }
        }
    }
    problems
}

/// Return the archive size, or the shared "completed with warnings" error (archive kept, run
/// reported as a warning in the UI) when anything about the backup was not clean.
fn finish_with_warnings(file_size: u64, backup_path: &Path, warnings: &[String]) -> Result<u64> {
    if warnings.is_empty() {
        return Ok(file_size);
    }
    Err(anyhow::anyhow!(
        "Backup completed with warnings: {}. Archive was created at: {}",
        warnings.join("; "),
        backup_path.display()
    ))
}

/// Copy one file for the staging copy. `File::open` shares read, write and delete access, so the
/// server can still write or replace the file (player data, mod files) while it is being copied
/// instead of failing with "used by another process".
fn copy_file_shared(source: &Path, dest: &Path) -> std::io::Result<u64> {
    let mut reader = BufReader::with_capacity(1 << 20, fs::File::open(source)?);
    let mut writer = fs::File::create(dest)?;
    std::io::copy(&mut reader, &mut writer)
}
/// Returns true if a file should be skipped when copying/archiving Minecraft backup paths.
fn should_skip_minecraft_backup_file(path: &Path) -> bool {
    if path.components().any(|c| c.as_os_str() == OsStr::new("orebfuscator_cache")) {
        return true;
    }
    if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("jar")) {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| n.eq_ignore_ascii_case("session.lock"))
}

/// Copy only Minecraft backup directories (world, config, optional DiscordIntegration-Data) to staging.
fn copy_minecraft_paths_to_staging(
    root_path: &Path,
    staging_path: &Path,
    backup_dirs: &[PathBuf],
) -> Result<()> {
    log::info!(
        "Copying Minecraft backup paths from {} -> {}",
        root_path.display(),
        staging_path.display()
    );
    fs::create_dir_all(staging_path)
        .with_context(|| format!("Failed to create staging dir: {}", staging_path.display()))?;

    let mut file_count: u64 = 0;
    for backup_dir in backup_dirs {
        let dir_name = backup_dir
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid backup directory: {}", backup_dir.display()))?;
        let staging_subdir = staging_path.join(dir_name);

        for entry in WalkDir::new(backup_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() || should_skip_minecraft_backup_file(path) {
                continue;
            }

            let relative = path
                .strip_prefix(backup_dir)
                .with_context(|| format!("Invalid path prefix: {}", path.display()))?;
            let dest = staging_subdir.join(relative);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)
                    .with_context(|| format!("Failed to create dir: {}", p.display()))?;
            }
            copy_file_shared(path, &dest).with_context(|| {
                format!("Failed to copy {} to {}", path.display(), dest.display())
            })?;
            file_count += 1;
            if file_count <= 3 || file_count % 500 == 0 {
                log::info!("Copied {} files so far...", file_count);
            }
        }
    }

    log::info!("Minecraft staging copy complete: {} files", file_count);
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

/// Minecraft backup: selective directories (world, config, optional DiscordIntegration-Data).
/// Uses 7-Zip if available (better compression), else built-in ZIP.
/// When job has RCON settings: save-off, save-all flush, copy to staging, save-on, then compress staging.
/// If app_handle is Some, progress (0-100) is emitted: 0-10% copy to staging, 10-90% compress, 90-100% copy to backup location.
pub async fn create_minecraft_backup(job: &Job, app_data: &AppData, app_handle: Option<AppHandle>) -> Result<u64> {
    log::info!("Starting Minecraft backup for job: {}", job.name);

    let root_path = Path::new(&job.root_dir);
    if !root_path.exists() {
        anyhow::bail!("Server root does not exist: {}", job.root_dir);
    }

    let backup_dirs = validation::resolve_minecraft_backup_dirs(&job.root_dir)?;
    let dir_names: Vec<String> = backup_dirs
        .iter()
        .filter_map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .collect();
    log::info!(
        "Minecraft backup includes: {}",
        dir_names.join(", ")
    );

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

        // Armed before save-off: from here on, every exit path re-enables saving.
        let save_on_guard = SaveOnGuard::arm(host.clone(), port, password.clone());

        log::info!("RCON: disabling saves and flushing (save-off; save-all flush)");
        let copy_result = async {
            tokio::task::spawn_blocking(move || pause_minecraft_saving(host, port, password))
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
            let staging_guard = StagingGuard(Some(staging_dir.clone()));

            copy_minecraft_paths_to_staging(root_path, &staging_dir, &backup_dirs)?;
            anyhow::Ok((staging_dir, staging_guard))
        }
        .await;

        log::info!("RCON: re-enabling saves (save-on)");
        let save_on_result = save_on_guard.send().await;

        // A failed save-on is the more urgent problem: the live server is not saving the world.
        if let Err(e) = save_on_result {
            log::error!(
                "RCON: FAILED to re-enable saving (save-on): {:#}. World saving is still OFF — run `save-on` on the server manually.",
                e
            );
            return Err(e.context(
                "Failed to re-enable world saving (save-on) after backup copy. World saving is still OFF on the server; run `save-on` manually",
            ));
        }

        let (staging_dir, staging_guard) = copy_result.map_err(|e| {
            log::error!("Copy to staging failed: {:#}", e);
            e
        })?;
        log::info!("Staging copy completed, compressing from: {}", staging_dir.display());
        (staging_dir, staging_guard)
    } else {
        // No RCON: saving can't be paused, so the world may only be copied while the server is stopped.
        let world_dir = backup_dirs
            .iter()
            .find(|d| d.file_name() == Some(OsStr::new("world")))
            .cloned()
            .unwrap_or_else(|| root_path.join("world"));
        if minecraft_world_in_use(&world_dir) {
            anyhow::bail!(
                "The Minecraft server is running, but this job has no RCON settings, so world saving can't be paused for the backup. Copying a world while the server writes to it can corrupt the backup and make the server's own saves fail. Edit the job and add RCON host, port and password (enable-rcon in server.properties), or run the backup while the server is stopped"
            );
        }
        log::info!("No RCON settings; the server is stopped (world not locked), backing up the world folders directly");
        (root_path.to_path_buf(), StagingGuard(None))
    };

    // Copy to staging (or ready for 7z) done → 10%
    if let Some(ref tx) = progress_tx {
        let _ = tx.send(10);
    }

    let use_staging_layout = job_has_rcon(job);
    let archive_items: Vec<String> = if use_staging_layout {
        vec!["*".to_string()]
    } else {
        dir_names.clone()
    };

    // With RCON, archive the staging copy taken while saving was paused — not the live folders,
    // which the server has been writing to again since save-on.
    let archived_dirs: Vec<PathBuf> = if use_staging_layout {
        dir_names
            .iter()
            .map(|name| source_path.join(name))
            .filter(|dir| dir.is_dir())
            .collect()
    } else {
        backup_dirs.clone()
    };

    let mut warnings: Vec<String> = Vec::new();
    let problems = verify_minecraft_world_copy(&archived_dirs);
    if !problems.is_empty() {
        log::warn!("MINECRAFT BACKUP: the copied world looks damaged in {} place(s):", problems.len());
        for p in &problems {
            log::warn!("  - {}", p);
        }
        warnings.push(format!(
            "{} world file(s) look damaged (see the log; the live world may already be corrupted, or it was written to during the copy)",
            problems.len()
        ));
    }

    if let Some(seven_z) = find_7z() {
        let dest_dir = job.destination_dir.clone();
        let filename = format!("{}_{}.7z", job.name, timestamp);
        let backup_path = Path::new(&dest_dir).join(&filename);
        // Written under a .tmp name and renamed when complete, so an interrupted run never
        // leaves a truncated .7z that retention and the monthly archive treat as a backup.
        let dest_temp = backup_path.with_extension("7z.tmp");
        let _dest_temp_guard = TempFileGuard::new(&dest_temp);

        let dest_parent = backup_path.parent().unwrap_or(Path::new(&dest_dir));
        fs::create_dir_all(dest_parent)
            .with_context(|| format!("Failed to create destination directory: {}", dest_parent.display()))?;

        // When the backup folder is on another drive, compress to temp on the system drive first and
        // copy the finished archive across once (avoids 7z doing many small writes to a slower
        // drive). On the same drive, write straight into the backup folder and skip that copy.
        let write_direct = same_volume(&std::env::temp_dir(), dest_parent);
        let temp_7z = if write_direct {
            dest_temp.clone()
        } else {
            std::env::temp_dir().join(format!("arkade_7z_{}_{}.7z", job.id, timestamp))
        };
        let _temp_7z_guard = TempFileGuard::new(&temp_7z);
        // 7z adds to an existing archive, so never let it find a leftover file.
        let _ = fs::remove_file(&temp_7z);
        if write_direct {
            log::info!("7-Zip writing directly to: {}", temp_7z.display());
        } else {
            log::info!("7-Zip writing to temp: {} then copying to {}", temp_7z.display(), backup_path.display());
        }

        // 7z progress 0-100 is mapped to 10-90%
        let result = run_minecraft_backup_7z(
            &seven_z,
            &temp_7z,
            &source_path,
            &archive_items,
            progress_tx.as_ref().cloned(),
        )
        .await;
        match result {
            Ok(outcome) => {
                if outcome.file_size == 0 {
                    let _ = fs::remove_file(&temp_7z);
                    anyhow::bail!("Backup produced an empty file (0 bytes). 7-Zip may have added no files.");
                }
                // Compression done → 90%; moving .7z into the backup location
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(90);
                }
                if !write_direct {
                    log::info!("Copying 7z archive to backup location: {}", backup_path.display());
                    copy_large_file(&temp_7z, &dest_temp)
                        .with_context(|| format!("Failed to copy 7z from {} to {}", temp_7z.display(), dest_temp.display()))?;
                }
                rename_temp_to_backup(&dest_temp, &backup_path)?;
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
                    warnings.push("some files were locked/unreadable and were skipped".to_string());
                }

                return finish_with_warnings(outcome.file_size, &backup_path, &warnings);
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

    let zip_source_dirs = archived_dirs;

    let result = attempt_minecraft_backup(&temp_path, &backup_path, &zip_source_dirs).await;

    match result {
        Ok((file_size, skipped)) => {
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

            if skipped > 0 {
                warnings.push(format!("{} file(s) were locked/unreadable and were skipped", skipped));
            }
            finish_with_warnings(file_size, &backup_path, &warnings)
        }
        Err(e) if is_disk_full_error(&e) => {
            log::warn!("Minecraft backup failed (disk full). Cleaning up old backups and retrying...");
            cleanup_minecraft_retention(&job.destination_dir, &job.name)
                .context("Failed to cleanup old Minecraft backups")?;
            let retry_result = attempt_minecraft_backup(&temp_path, &backup_path, &zip_source_dirs).await;
            match retry_result {
                Ok((file_size, skipped)) => {
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

                    if skipped > 0 {
                        warnings.push(format!("{} file(s) were locked/unreadable and were skipped", skipped));
                    }
                    finish_with_warnings(file_size, &backup_path, &warnings)
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

/// Run Minecraft backup using 7-Zip (LZMA2) for selective server directories.
/// Streams 7-Zip output to the log so you can see progress and, if it hangs, the last file it was processing.
/// If progress_tx is Some, sends percent (0-100) for UI progress bar.
async fn run_minecraft_backup_7z(
    seven_z: &Path,
    backup_path: &Path,
    root_path: &Path,
    archive_items: &[String],
    progress_tx: Option<mpsc::Sender<u8>>,
) -> Result<SevenZipOutcome> {
    let seven_z = seven_z.to_path_buf();
    let backup_path = backup_path.to_path_buf();
    let root_path = root_path.to_path_buf();
    let archive_items = archive_items.to_vec();

    let result = tokio::task::spawn_blocking(move || {
        let backup_abs = backup_path
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .map(|p| p.join(backup_path.file_name().unwrap()))
            .unwrap_or(backup_path.clone());
        let backup_str = backup_abs.to_string_lossy().to_string();

        let threads = seven_zip_threads();
        let mut cmd = Command::new(&seven_z);
        cmd.arg("a")
            .arg("-t7z")
            .arg(format!("-mx={}", SEVEN_ZIP_LEVEL))
            .arg(format!("-mmt={}", threads))
            .arg("-bsp2")
            .arg("-xr!*.jar")
            .arg("-xr!orebfuscator_cache")
            .arg("-xr!session.lock")
            .arg(&backup_str);
        for item in &archive_items {
            cmd.arg(item);
        }
        cmd.current_dir(&root_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            // Below-normal priority: the game servers get the CPU first; 7-Zip uses what is left.
            cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return (Err(e.into()), backup_path),
        };

        log::info!(
            "7-Zip compression started (level {}, {} threads, below-normal priority) from {} -> {} (items: {})",
            SEVEN_ZIP_LEVEL,
            threads,
            root_path.display(),
            backup_abs.display(),
            archive_items.join(", ")
        );

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

/// Returns (archive size, number of files skipped because they could not be read).
async fn attempt_minecraft_backup(
    temp_path: &Path,
    backup_path: &Path,
    backup_dirs: &[PathBuf],
) -> Result<(u64, usize)> {
    let _temp_guard = TempFileGuard::new(temp_path);
    let file = fs::File::create(temp_path)
        .with_context(|| format!("Failed to create backup file: {}", temp_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let mut skipped_locked: Vec<String> = Vec::new();

    for backup_dir in backup_dirs {
        let dir_name = backup_dir
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid backup directory: {}", backup_dir.display()))?;

        for entry in WalkDir::new(backup_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() || should_skip_minecraft_backup_file(path) {
                continue;
            }

            let relative = path
                .strip_prefix(backup_dir)
                .with_context(|| format!("Invalid path prefix: {}", path.display()))?;
            let zip_path = format!(
                "{}/{}",
                dir_name.to_string_lossy(),
                relative.to_string_lossy().replace('\\', "/")
            );
            let options = zip_options_for_path(path);

            if !add_optional_file_to_zip(&mut zip, path, &zip_path, &options)? {
                skipped_locked.push(path.display().to_string());
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

    Ok((file_size, skipped_locked.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_gzip(path: &Path, data: &[u8]) {
        let mut enc = flate2::write::GzEncoder::new(fs::File::create(path).unwrap(), flate2::Compression::default());
        enc.write_all(data).unwrap();
        enc.finish().unwrap();
    }

    #[test]
    fn world_copy_check_passes_clean_world() {
        let temp = tempfile::tempdir().unwrap();
        let world = temp.path().join("world");
        fs::create_dir_all(world.join("region")).unwrap();
        write_gzip(&world.join("level.dat"), b"nbt data");
        fs::write(world.join("region").join("r.0.0.mca"), vec![0u8; 8192]).unwrap();
        fs::write(world.join("region").join("r.1.0.mca"), b"").unwrap();

        assert!(verify_minecraft_world_copy(&[world]).is_empty());
    }

    #[test]
    fn world_copy_check_flags_torn_region_and_bad_level_dat() {
        let temp = tempfile::tempdir().unwrap();
        let world = temp.path().join("world");
        fs::create_dir_all(world.join("region")).unwrap();
        // Truncated gzip: the decoder fails instead of reaching the CRC.
        let mut level = Vec::new();
        {
            let mut enc = flate2::write::GzEncoder::new(&mut level, flate2::Compression::default());
            enc.write_all(&[7u8; 4096]).unwrap();
        }
        fs::write(world.join("level.dat"), &level[..level.len() / 2]).unwrap();
        fs::write(world.join("region").join("r.0.0.mca"), vec![0u8; 8192 + 100]).unwrap();

        let problems = verify_minecraft_world_copy(&[world]);
        assert_eq!(problems.len(), 2, "{:?}", problems);
        assert!(problems.iter().any(|p| p.contains("r.0.0.mca")));
        assert!(problems.iter().any(|p| p.contains("level.dat")));
    }

    #[test]
    fn world_not_in_use_without_lock() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!minecraft_world_in_use(temp.path()));
        fs::write(temp.path().join("session.lock"), "\u{2603}").unwrap();
        assert!(!minecraft_world_in_use(temp.path()));
    }

    #[cfg(windows)]
    #[test]
    fn world_in_use_while_session_lock_is_held() {
        let temp = tempfile::tempdir().unwrap();
        let lock_path = temp.path().join("session.lock");
        fs::write(&lock_path, "\u{2603}").unwrap();
        // Like a running server: an exclusive OS lock over the whole file.
        let held = fs::OpenOptions::new().write(true).open(&lock_path).unwrap();
        held.try_lock().unwrap();

        assert!(minecraft_world_in_use(temp.path()));
        drop(held);
        assert!(!minecraft_world_in_use(temp.path()));
    }

    #[test]
    fn shared_copy_works_while_file_is_open_for_writing() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("player.dat");
        fs::write(&source, b"player").unwrap();
        let _writer = fs::OpenOptions::new().append(true).open(&source).unwrap();

        let dest = temp.path().join("copy.dat");
        copy_file_shared(&source, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"player");
    }

    #[test]
    fn warnings_keep_the_ui_prefix() {
        assert_eq!(finish_with_warnings(5, Path::new("a.7z"), &[]).unwrap(), 5);
        let err = finish_with_warnings(5, Path::new("a.7z"), &["x".into()]).unwrap_err();
        assert!(err.to_string().starts_with("Backup completed with warnings: x."), "{}", err);
    }
}
