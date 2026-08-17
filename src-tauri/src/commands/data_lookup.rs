use crate::data_lookup;
use crate::state::AppState;

#[tauri::command]
pub async fn lookup_data_files(
    lookup_type: String,
    identifier: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<data_lookup::DataLookupMatch>, String> {
    let app_data = state.app_data.lock().await;
    data_lookup::lookup_data_files(&app_data, &lookup_type, &identifier)
}

#[tauri::command]
pub async fn delete_data_files(
    file_paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<data_lookup::DeleteFileResult>, String> {
    let app_data = state.app_data.lock().await;
    Ok(data_lookup::delete_data_files(&app_data, &file_paths))
}
