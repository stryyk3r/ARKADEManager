use crate::job;
use crate::state::AppState;
use crate::validation;

#[tauri::command]
pub async fn list_jobs(state: tauri::State<'_, AppState>) -> Result<Vec<job::Job>, String> {
    let app_data = state.app_data.lock().await;
    app_data.list_jobs().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_job(
    job: job::JobInput,
    state: tauri::State<'_, AppState>,
) -> Result<job::Job, String> {
    let maps = {
        let app_data = state.app_data.lock().await;
        app_data.get_config().map_err(|e| e.to_string())?.ark_maps()
    };
    validation::validate_job(&job, &maps).map_err(|e| e.to_string())?;

    let mut app_data = state.app_data.lock().await;
    let new_job = app_data.add_job(job).map_err(|e| e.to_string())?;

    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(new_job)
}

#[tauri::command]
pub async fn update_job(
    job: job::JobInput,
    state: tauri::State<'_, AppState>,
) -> Result<job::Job, String> {
    let maps = {
        let app_data = state.app_data.lock().await;
        app_data.get_config().map_err(|e| e.to_string())?.ark_maps()
    };
    validation::validate_job(&job, &maps).map_err(|e| e.to_string())?;

    let mut app_data = state.app_data.lock().await;
    let updated_job = app_data.update_job(job).map_err(|e| e.to_string())?;

    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(updated_job)
}

#[tauri::command]
pub async fn delete_job(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut app_data = state.app_data.lock().await;
    app_data.delete_job(&id).map_err(|e| e.to_string())?;

    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn run_job_now(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let app_data = state.app_data.lock().await;
    let job = app_data
        .get_job(&id)
        .ok_or_else(|| "Job not found".to_string())?;

    let mut scheduler = state.scheduler.lock().await;
    scheduler.enqueue_job(job.clone()).map_err(|e| e.to_string())?;

    drop(scheduler);
    drop(app_data);

    let scheduler_clone = state.scheduler.clone();
    let app_data_clone = state.app_data.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let mut sched = scheduler_clone.lock().await;
        let app_data_guard = app_data_clone.lock().await;
        if let Err(e) = sched.tick(&app_data_guard) {
            log::error!("Scheduler tick error after manual enqueue: {}", e);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn get_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::scheduler::Status, String> {
    let scheduler = state.scheduler.lock().await;
    Ok(scheduler.get_status())
}
