use crate::backup;
use crate::state::AppState;

#[tauri::command]
pub async fn open_backup_location(path: String) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Path is empty".to_string());
    }
    // Explorer *runs* a file it is given (e.g. an .exe), so only ever pass it an existing folder.
    // Requiring an absolute path also rules out Explorer switches such as `/select,...`.
    let as_path = std::path::Path::new(path);
    if !as_path.is_absolute() || !as_path.is_dir() {
        return Err(format!("Not an existing folder: {}", path));
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
    // Only planning needs the app-data lock; the copying runs after it is released, on a blocking
    // thread, so the UI and scheduler aren't frozen during multi-GB copies to the shared folder.
    let plan = {
        let app_data = state.app_data.lock().await;
        backup::plan_monthly_archive(&app_data).map_err(|e| e.to_string())?
    };
    tokio::task::spawn_blocking(move || backup::execute_monthly_archive(plan))
        .await
        .map_err(|e| format!("Monthly archive task failed: {}", e))?
        .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub async fn get_monthly_status(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyStatusResult, String> {
    let app_data = state.app_data.lock().await;
    backup::get_monthly_status(&app_data).map_err(|e| e.to_string())
}
