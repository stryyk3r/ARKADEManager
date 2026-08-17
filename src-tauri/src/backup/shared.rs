use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;
#[derive(Clone, Serialize)]
pub struct BackupProgressPayload {
    pub job_name: String,
    pub percent: u8,
}
/// Extensions that are already compressed. Storing them (no deflate) avoids making them larger and speeds up backup.
const STORED_EXTENSIONS: &[&str] = &[
    "jar", "zip", "7z", "rar", "gz", "bz2", "xz", "zst",
    "png", "jpg", "jpeg", "gif", "webp",
    "mp3", "ogg", "flac",
    "mp4", "mkv", "webm", "avi",
];

pub(crate) fn zip_options_for_path(path: &Path) -> FileOptions {
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
pub(crate) fn find_7z() -> Option<PathBuf> {
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
pub(crate) fn is_disk_full_error(error: &anyhow::Error) -> bool {
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
pub(crate) fn rename_temp_to_backup(temp_path: &Path, backup_path: &Path) -> Result<()> {
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
pub(crate) fn add_single_file_to_zip(
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
pub(crate) fn verify_zip_integrity(zip_path: &Path) -> Result<()> {
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
