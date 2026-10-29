use crate::app_data::AppData;
use crate::plugins;
use crate::server_roots;
use crate::state::AppState;

/// ARK job roots from the saved jobs (used together with the configured ASA server root).
fn ark_job_roots(app_data: &AppData) -> Result<Vec<String>, String> {
    let jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
    Ok(jobs
        .iter()
        .filter(|job| job.job_type == "ark")
        .map(|job| job.root_dir.clone())
        .collect())
}

/// All known ASA server roots: subfolders of the configured ASA root plus ARK job roots.
async fn asa_server_roots(state: &tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let app_data = state.app_data.lock().await;
    let config = app_data.get_config().map_err(|e| e.to_string())?;
    let job_roots = ark_job_roots(&app_data)?;
    Ok(server_roots::collect_asa_server_roots(&config, &job_roots))
}

#[tauri::command]
pub async fn get_plugin_server_roots(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    asa_server_roots(&state).await
}

#[tauri::command]
pub async fn list_plugin_folders(server_root: String) -> Result<Vec<serde_json::Value>, String> {
    plugins::list_plugin_folders(&server_root)
}

#[tauri::command]
pub async fn toggle_plugin_folder(
    folder_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let roots = asa_server_roots(&state).await?;
    plugins::toggle_plugin_folder(&folder_path, &plugins::allowed_plugin_dir_keys(&roots))
}

#[tauri::command]
pub async fn toggle_plugin_for_all_servers(
    base_folder_name: String,
    target_state_disabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let roots = asa_server_roots(&state).await?;
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
    let job_roots = ark_job_roots(&app_data)?;

    Ok(plugins::discover_plugin_destinations(&config, &job_roots))
}

#[tauri::command]
pub async fn install_plugins(
    source_plugin_paths: Vec<String>,
    destination_plugin_paths: Vec<String>,
    keep_existing_config: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<plugins::InstallResult, String> {
    log::info!(
        "install_plugins: {} source(s), {} destination(s)",
        source_plugin_paths.len(),
        destination_plugin_paths.len()
    );
    if source_plugin_paths.is_empty() || destination_plugin_paths.is_empty() {
        log::warn!("install_plugins: missing source or destination paths");
    }
    let roots = asa_server_roots(&state).await?;
    // Blocking file IO that may wait out locked files, so keep it off the async runtime.
    tokio::task::spawn_blocking(move || {
        plugins::install_plugins(
            source_plugin_paths,
            destination_plugin_paths,
            keep_existing_config.unwrap_or(false),
            &plugins::allowed_plugin_dir_keys(&roots),
        )
    })
    .await
    .map_err(|e| format!("Plugin install task failed: {}", e))?
}
