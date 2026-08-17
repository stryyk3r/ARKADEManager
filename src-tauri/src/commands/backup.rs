use crate::backup;
use crate::state::AppState;

#[tauri::command]
pub async fn open_backup_location(path: String) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Path is empty".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn preview_monthly_archive(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyArchivePreview, String> {
    let app_data = state.app_data.lock().await;
    backup::preview_monthly_archive(&app_data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_monthly_archive(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyArchiveResult, String> {
    let app_data = state.app_data.lock().await;
    backup::run_monthly_archive(&app_data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_monthly_status(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyStatusResult, String> {
    let app_data = state.app_data.lock().await;
    backup::get_monthly_status(&app_data).map_err(|e| e.to_string())
}
