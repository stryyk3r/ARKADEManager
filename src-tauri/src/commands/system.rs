use std::path::{Path, PathBuf};

use crate::backup;
use crate::state::AppState;

/// Most child folder names returned by `inspect_directory`.
const MAX_CHILD_DIRS: usize = 200;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppPaths {
    pub config_path: String,
    pub logs_dir: String,
    pub monthly_destination: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectoryInfo {
    pub exists: bool,
    /// Names of the immediate subfolders, sorted case-insensitively.
    pub child_dirs: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DestinationCheck {
    pub exists: bool,
    /// Whether a file could be created in the folder (or, if it does not exist yet, in the
    /// closest existing parent it would be created under).
    pub writable: Option<bool>,
    pub free_bytes: Option<u64>,
    /// Drive or share the folder is on, e.g. `D:`.
    pub volume: Option<String>,
    pub same_volume_as_monthly: Option<bool>,
}

#[tauri::command]
pub async fn get_app_paths(state: tauri::State<'_, AppState>) -> Result<AppPaths, String> {
    let app_data = state.app_data.lock().await;
    let config = app_data.get_config().map_err(|e| e.to_string())?;
    Ok(AppPaths {
        config_path: app_data.get_config_path().to_string_lossy().to_string(),
        logs_dir: app_data.get_logs_dir().to_string_lossy().to_string(),
        monthly_destination: backup::monthly_destination(&config),
    })
}

/// Read-only look at a folder: whether it exists and which subfolders it has.
#[tauri::command]
pub async fn inspect_directory(path: String) -> Result<DirectoryInfo, String> {
    let path = path.trim();
    if path.is_empty() || !Path::new(path).is_dir() {
        return Ok(DirectoryInfo { exists: false, child_dirs: Vec::new() });
    }
    let mut child_dirs: Vec<String> = crate::server_roots::list_child_directories(path)
        .iter()
        .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();
    child_dirs.truncate(MAX_CHILD_DIRS);
    Ok(DirectoryInfo { exists: true, child_dirs })
}

/// Checks for a backup destination chosen in the Add Job wizard.
#[tauri::command]
pub async fn check_destination(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<DestinationCheck, String> {
    let monthly = {
        let app_data = state.app_data.lock().await;
        let config = app_data.get_config().map_err(|e| e.to_string())?;
        backup::monthly_destination(&config)
    };
    let path = path.trim();
    if path.is_empty() || !Path::new(path).is_absolute() {
        return Err("Enter a full folder path".to_string());
    }
    let target = Path::new(path);
    let existing = nearest_existing_dir(target);
    let volume = existing.as_deref().and_then(volume_of);
    let monthly_volume = nearest_existing_dir(Path::new(&monthly)).as_deref().and_then(volume_of);

    Ok(DestinationCheck {
        exists: target.is_dir(),
        writable: existing.as_deref().map(can_create_file_in),
        free_bytes: existing.as_deref().and_then(free_space),
        same_volume_as_monthly: match (&volume, &monthly_volume) {
            (Some(a), Some(b)) => Some(a.eq_ignore_ascii_case(b)),
            _ => None,
        },
        volume,
    })
}

fn nearest_existing_dir(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.is_dir()).map(Path::to_path_buf)
}

fn volume_of(path: &Path) -> Option<String> {
    match std::fs::canonicalize(path).ok()?.components().next()? {
        std::path::Component::Prefix(prefix) => {
            let text = prefix.as_os_str().to_string_lossy().to_string();
            // Canonical paths are verbatim (`\\?\D:`); show the plain drive.
            Some(text.trim_start_matches(r"\\?\").to_string())
        }
        _ => None,
    }
}

/// Create and remove a uniquely named empty file.
fn can_create_file_in(dir: &Path) -> bool {
    let probe = dir.join(format!(".arkade-write-test-{}", uuid::Uuid::new_v4()));
    let created = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok();
    if created {
        let _ = std::fs::remove_file(&probe);
    }
    created
}

#[cfg(windows)]
fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut available: u64 = 0;
    // SAFETY: `wide` is NUL-terminated and outlives the call; the out pointer is valid.
    let ok = unsafe {
        GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, std::ptr::null_mut(), std::ptr::null_mut())
    };
    (ok != 0).then_some(available)
}

#[cfg(not(windows))]
fn free_space(_dir: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_existing_dir_walks_up() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("a").join("b");
        assert_eq!(nearest_existing_dir(&missing).unwrap(), temp.path());
    }

    #[test]
    fn write_probe_leaves_no_file() {
        let temp = tempfile::tempdir().unwrap();
        assert!(can_create_file_in(temp.path()));
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}
