use crate::config;
use crate::map;
use crate::state::AppState;

#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<config::Config, String> {
    let app_data = state.app_data.lock().await;
    app_data.get_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_theme(theme: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut app_data = state.app_data.lock().await;
    let mut config = app_data.get_config().map_err(|e| e.to_string())?;
    config.theme = Some(theme);
    app_data.save_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ark_maps(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<map::MapDefinition>, String> {
    let app_data = state.app_data.lock().await;
    Ok(app_data.get_config().map_err(|e| e.to_string())?.ark_maps())
}

#[tauri::command]
pub async fn save_ark_maps(
    maps: Vec<map::MapDefinition>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<map::MapDefinition>, String> {
    map::validate_maps(&maps)?;

    let mut app_data = state.app_data.lock().await;
    let mut config = app_data.get_config().map_err(|e| e.to_string())?;
    let old_maps = config.ark_maps();
    let renames = map::find_map_id_renames(&old_maps, &maps);
    config.ark_maps = Some(maps);
    app_data.save_config(&config).map_err(|e| e.to_string())?;

    if !renames.is_empty() {
        let mut jobs = app_data.list_jobs().map_err(|e| e.to_string())?;
        let mut changed = false;
        for job in jobs.iter_mut() {
            if let Some(new_id) = renames.get(&job.map) {
                log::info!(
                    "Migrating job \"{}\" map id {} -> {}",
                    job.name,
                    job.map,
                    new_id
                );
                job.map = new_id.clone();
                changed = true;
            }
        }
        if changed {
            app_data.save_jobs(&jobs).map_err(|e| e.to_string())?;
        }
    }

    Ok(config.ark_maps())
}

#[tauri::command]
pub async fn reset_ark_maps(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<map::MapDefinition>, String> {
    let defaults = map::default_ark_maps();
    let mut app_data = state.app_data.lock().await;
    let mut config = app_data.get_config().map_err(|e| e.to_string())?;
    config.ark_maps = Some(defaults.clone());
    app_data.save_config(&config).map_err(|e| e.to_string())?;
    Ok(defaults)
}

#[tauri::command]
pub async fn save_server_roots(
    asa_server_root: Option<String>,
    minecraft_server_root: Option<String>,
    palworld_server_root: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<config::Config, String> {
    let mut app_data = state.app_data.lock().await;
    let mut config = app_data.get_config().map_err(|e| e.to_string())?;
    config.asa_server_root = asa_server_root
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    config.minecraft_server_root = minecraft_server_root
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    config.palworld_server_root = palworld_server_root
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    app_data.save_config(&config).map_err(|e| e.to_string())?;
    Ok(config)
}
