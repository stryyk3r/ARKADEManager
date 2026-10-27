use crate::app_data;

#[tauri::command]
pub async fn read_logs(lines: Option<usize>, file: Option<String>) -> Result<String, String> {
    app_data::read_logs(lines.unwrap_or(100), file.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_log_files() -> Result<Vec<app_data::LogFileInfo>, String> {
    app_data::list_log_files().map_err(|e| e.to_string())
}

/// Open the logs directory in the system file manager.
#[tauri::command]
pub async fn open_logs_folder() -> Result<(), String> {
    let app_data = app_data::AppData::new().map_err(|e| e.to_string())?;
    let path = app_data.get_logs_dir().to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
