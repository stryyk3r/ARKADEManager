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

/// Windows ERROR_HANDLE_DISK_FULL and ERROR_DISK_FULL.
#[cfg(windows)]
const WINDOWS_DISK_FULL_CODES: &[i32] = &[39, 112];

/// Check if an error is due to insufficient disk space. Looks for an actual I/O error in the
/// chain (zip errors expose theirs via `source()`) rather than matching message text, which used to
/// misfire on paths/timestamps containing "112".
pub(crate) fn is_disk_full_error(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(is_disk_full_io_error)
}

fn is_disk_full_io_error(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::StorageFull {
        return true;
    }
    #[cfg(windows)]
    if e.raw_os_error().is_some_and(|code| WINDOWS_DISK_FULL_CODES.contains(&code)) {
        return true;
    }
    false
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
/// Deletes a temporary archive when dropped, if it still exists. Create one before writing a temp
/// file: after a successful rename the file is gone and this does nothing; on an error or early
/// return the partial file is removed instead of being left in the backup folder.
pub(crate) struct TempFileGuard(PathBuf);

impl TempFileGuard {
    pub(crate) fn new(path: &Path) -> Self {
        Self(path.to_path_buf())
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            match fs::remove_file(&self.0) {
                Ok(()) => log::info!("Removed incomplete temp file: {}", self.0.display()),
                Err(e) => log::warn!("Could not remove incomplete temp file {}: {}", self.0.display(), e),
            }
        }
    }
}

const READ_RETRY_ATTEMPTS: u32 = 3;
const READ_RETRY_DELAY_MS: u64 = 1000;

/// Read a whole file, retrying briefly when it can't be opened or read (e.g. antivirus or the
/// game server holding it for a moment). A missing file is not retried.
fn read_file_with_retry(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut attempt = 1;
    loop {
        let result = fs::File::open(path).and_then(|mut file| {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).map(|_| buffer)
        });
        match result {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound && attempt < READ_RETRY_ATTEMPTS => {
                log::warn!(
                    "Could not read {} (attempt {}/{}): {}. Retrying...",
                    path.display(),
                    attempt,
                    READ_RETRY_ATTEMPTS,
                    e
                );
                std::thread::sleep(std::time::Duration::from_millis(READ_RETRY_DELAY_MS));
                attempt += 1;
            }
            other => return other,
        }
    }
}

fn write_zip_entry(
    zip: &mut ZipWriter<fs::File>,
    zip_path: &str,
    buffer: &[u8],
    options: &FileOptions,
) -> Result<()> {
    // A "closed"/"finished" writer means an earlier error (disk full, I/O) already finalized the ZIP.
    // The original error is kept as the source (not flattened to text) so is_disk_full_error can see it.
    zip.start_file(zip_path.to_string(), *options).map_err(|e| {
        let err_msg = e.to_string();
        let context = if err_msg.contains("closed") || err_msg.contains("finished") {
            format!("ZIP writer was closed while starting {} entry", zip_path)
        } else {
            format!("Failed to start {} entry in ZIP", zip_path)
        };
        anyhow::Error::new(e).context(context)
    })?;
    zip.write_all(buffer).map_err(|e| {
        let err_msg = e.to_string();
        let context = if err_msg.contains("closed") || err_msg.contains("finished") {
            format!("ZIP writer was closed while adding {}", zip_path)
        } else {
            format!("Failed to write {} to ZIP", zip_path)
        };
        anyhow::Error::new(e).context(context)
    })
}

/// Add a file the backup cannot be complete without. If it is missing or still unreadable after
/// a few retries, the backup fails instead of silently producing an archive without it.
pub(crate) fn add_single_file_to_zip(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    zip_path: &str,
    options: &FileOptions,
) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Required backup file not found: {}", path.display());
    }

    let buffer = read_file_with_retry(path)
        .with_context(|| format!("Could not read required backup file (it may be locked): {}", path.display()))?;
    write_zip_entry(zip, zip_path, &buffer, options)
}

/// Add a file the backup can do without. If it can't be read it is skipped with a warning.
/// Returns whether the file was added.
pub(crate) fn add_optional_file_to_zip(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    zip_path: &str,
    options: &FileOptions,
) -> Result<bool> {
    match fs::File::open(path).and_then(|mut file| {
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map(|_| buffer)
    }) {
        Ok(buffer) => {
            write_zip_entry(zip, zip_path, &buffer, options)?;
            Ok(true)
        }
        Err(e) => {
            log::warn!("Failed to read {} (may be locked by server): {}. Skipping.", path.display(), e);
            Ok(false)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn new_zip(dir: &Path) -> ZipWriter<fs::File> {
        ZipWriter::new(fs::File::create(dir.join("test.zip")).unwrap())
    }

    #[test]
    fn disk_full_detected_from_io_error_in_chain() {
        let io_full = || std::io::Error::from(std::io::ErrorKind::StorageFull);
        assert!(is_disk_full_error(&anyhow::Error::new(io_full())));
        assert!(is_disk_full_error(&anyhow::Error::new(io_full()).context("Failed to write ZIP")));
        let zip_err = zip::result::ZipError::Io(io_full());
        assert!(is_disk_full_error(&anyhow::Error::new(zip_err).context("Failed to finalize ZIP file")));
        #[cfg(windows)]
        assert!(is_disk_full_error(&anyhow::Error::new(std::io::Error::from_raw_os_error(112))));
    }

    #[test]
    fn disk_full_not_guessed_from_message_text() {
        // These used to match the old text heuristics.
        assert!(!is_disk_full_error(&anyhow::anyhow!(
            r"Failed to open D:\Backups\Island_20261112_112233.zip"
        )));
        assert!(!is_disk_full_error(&anyhow::anyhow!("insufficient permissions")));
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(!is_disk_full_error(&anyhow::Error::new(denied).context("disk full? no")));
    }

    #[test]
    fn required_file_missing_fails() {
        let temp = tempfile::tempdir().unwrap();
        let mut zip = new_zip(temp.path());
        let err = add_single_file_to_zip(&mut zip, &temp.path().join("Level.sav"), "Level.sav", &FileOptions::default())
            .unwrap_err();
        assert!(err.to_string().contains("not found"), "{}", err);
    }

    #[test]
    fn optional_file_unreadable_is_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let mut zip = new_zip(temp.path());
        let added = add_optional_file_to_zip(&mut zip, &temp.path().join("missing.arkprofile"), "x", &FileOptions::default())
            .unwrap();
        assert!(!added);
    }

    #[cfg(windows)]
    #[test]
    fn required_file_locked_fails_after_retries() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Level.sav");
        fs::write(&path, b"world").unwrap();
        // share_mode(0): exclusive lock, like a server holding the file mid-write.
        let _lock = fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();

        let mut zip = new_zip(temp.path());
        let err = add_single_file_to_zip(&mut zip, &path, "Level.sav", &FileOptions::default()).unwrap_err();
        assert!(err.to_string().contains("Could not read required backup file"), "{}", err);
    }
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
