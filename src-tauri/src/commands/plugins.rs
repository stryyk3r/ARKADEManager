use crate::plugins;
use crate::server_roots;
use crate::state::AppState;

#[tauri::command]
pub async fn get_plugin_server_roots(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let app_data = state.app_data.lock().await;
    let config = app_data.get_config().map_err(|e| e.to_string())?;
    let jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
    let job_roots: Vec<String> = jobs
        .iter()
        .filter(|job| job.job_type == "ark")
        .map(|job| job.root_dir.clone())
        .collect();

    Ok(server_roots::collect_asa_server_roots(&config, &job_roots))
}

#[tauri::command]
pub async fn list_plugin_folders(server_root: String) -> Result<Vec<serde_json::Value>, String> {
    plugins::list_plugin_folders(&server_root)
}

#[tauri::command]
pub async fn toggle_plugin_folder(folder_path: String) -> Result<String, String> {
    plugins::toggle_plugin_folder(&folder_path)
}

#[tauri::command]
pub async fn toggle_plugin_for_all_servers(
    base_folder_name: String,
    target_state_disabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let app_data = state.app_data.lock().await;
    let config = app_data.get_config().map_err(|e| e.to_string())?;
    let jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
    let job_roots: Vec<String> = jobs
        .iter()
        .filter(|job| job.job_type == "ark")
        .map(|job| job.root_dir.clone())
        .collect();
    let roots = server_roots::collect_asa_server_roots(&config, &job_roots);

    plugins::toggle_plugin_for_all_servers(&base_folder_name, target_state_disabled, &roots)
}

#[tauri::command]
pub async fn list_source_plugins(
    source_path: String,
) -> Result<Vec<plugins::SourcePlugin>, String> {
    plugins::list_source_plugins(&source_path)
}

#[tauri::command]
pub async fn discover_plugin_destinations(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<plugins::DestinationServer>, String> {
    let app_data = state.app_data.lock().await;
    let config = app_data.get_config().map_err(|e| e.to_string())?;
    let jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
    let job_roots: Vec<String> = jobs
        .iter()
        .filter(|job| job.job_type == "ark")
        .map(|job| job.root_dir.clone())
        .collect();

    Ok(plugins::discover_plugin_destinations(&config, &job_roots))
}

#[tauri::command]
pub async fn install_plugins(
    source_plugin_paths: Vec<String>,
    destination_plugin_paths: Vec<String>,
) -> Result<plugins::InstallResult, String> {
    log::info!(
        "install_plugins: {} source(s), {} destination(s)",
        source_plugin_paths.len(),
        destination_plugin_paths.len()
    );
    if source_plugin_paths.is_empty() || destination_plugin_paths.is_empty() {
        log::warn!("install_plugins: missing source or destination paths");
    }
    plugins::install_plugins(source_plugin_paths, destination_plugin_paths)
}
