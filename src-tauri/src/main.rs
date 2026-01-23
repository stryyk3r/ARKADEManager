// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_data;
mod backup;
mod config;
mod job;
mod map;
mod plugins;
mod scheduler;
mod validation;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

use app_data::AppData;
use scheduler::Scheduler;

#[derive(Clone)]
struct AppState {
    app_data: Arc<Mutex<AppData>>,
    scheduler: Arc<Mutex<Scheduler>>,
}

#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> Result<config::Config, String> {
    let app_data = state.app_data.lock().await;
    app_data.get_config().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_theme(
    theme: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut app_data = state.app_data.lock().await;
    let mut config = app_data.get_config().map_err(|e| e.to_string())?;
    config.theme = Some(theme);
    app_data.save_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_jobs(state: tauri::State<'_, AppState>) -> Result<Vec<job::Job>, String> {
    let app_data = state.app_data.lock().await;
    app_data.list_jobs().map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_job(
    job: job::JobInput,
    state: tauri::State<'_, AppState>,
) -> Result<job::Job, String> {
    // Validate job
    validation::validate_job(&job).map_err(|e| e.to_string())?;

    let mut app_data = state.app_data.lock().await;
    let new_job = app_data.add_job(job).map_err(|e| e.to_string())?;

    // Notify scheduler
    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(new_job)
}

#[tauri::command]
async fn update_job(
    job: job::JobInput,
    state: tauri::State<'_, AppState>,
) -> Result<job::Job, String> {
    // Validate job
    validation::validate_job(&job).map_err(|e| e.to_string())?;

    let mut app_data = state.app_data.lock().await;
    let updated_job = app_data.update_job(job).map_err(|e| e.to_string())?;

    // Notify scheduler
    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(updated_job)
}

#[tauri::command]
async fn delete_job(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut app_data = state.app_data.lock().await;
    app_data.delete_job(&id).map_err(|e| e.to_string())?;

    // Notify scheduler
    let mut scheduler = state.scheduler.lock().await;
    scheduler.refresh_jobs(&app_data).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn run_job_now(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let app_data = state.app_data.lock().await;
    let job = app_data
        .get_job(&id)
        .ok_or_else(|| "Job not found".to_string())?;

    let mut scheduler = state.scheduler.lock().await;
    scheduler.enqueue_job(job.clone()).map_err(|e| e.to_string())?;
    
    // Trigger immediate scheduler tick to process the manually enqueued job
    // Use a small delay to avoid winit event loop warnings
    drop(scheduler);
    drop(app_data);
    
    let scheduler_clone = state.scheduler.clone();
    let app_data_clone = state.app_data.clone();
    tauri::async_runtime::spawn(async move {
        // Small delay to let the event loop settle before triggering tick
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
async fn preview_monthly_archive(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyArchivePreview, String> {
    let app_data = state.app_data.lock().await;
    backup::preview_monthly_archive(&app_data).map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_monthly_archive(
    state: tauri::State<'_, AppState>,
) -> Result<backup::MonthlyArchiveResult, String> {
    let app_data = state.app_data.lock().await;
    backup::run_monthly_archive(&app_data).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_status(state: tauri::State<'_, AppState>) -> Result<scheduler::Status, String> {
    let scheduler = state.scheduler.lock().await;
    Ok(scheduler.get_status())
}

#[tauri::command]
async fn get_app_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
async fn check_for_updates() -> Result<serde_json::Value, String> {
    use serde_json::Value;
    
    log::info!("Checking for updates via GitHub API...");
    let current_version = env!("CARGO_PKG_VERSION");
    
    // Check GitHub releases API
    let client = reqwest::Client::new();
    let response = match client
        .get("https://api.github.com/repos/stryyk3r/ARKADEManager/releases/latest")
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Failed to fetch GitHub releases: {}", e);
            return Err(format!("Failed to fetch releases: {}", e));
        }
    };
    
    if !response.status().is_success() {
        log::error!("GitHub API returned error: {}", response.status());
        return Err(format!("GitHub API error: {}", response.status()));
    }
    
    let release: Value = match response.json().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to parse GitHub response: {}", e);
            return Err(format!("Failed to parse response: {}", e));
        }
    };
    
    let latest_version = match release.get("tag_name") {
        Some(tag) => {
            let tag_str = tag.as_str().unwrap_or("");
            // Remove 'v' prefix if present
            tag_str.strip_prefix('v').unwrap_or(tag_str)
        }
        None => {
            log::error!("No tag_name in GitHub response");
            return Ok(serde_json::json!({ "available": false }));
        }
    };
    
    log::info!("Current version: {}, Latest version: {}", current_version, latest_version);
    
    // Compare versions using semver
    let current_ver = match semver::Version::parse(current_version) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse current version: {}", e);
            return Ok(serde_json::json!({ "available": false }));
        }
    };
    
    let latest_ver = match semver::Version::parse(latest_version) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse latest version: {}", e);
            return Ok(serde_json::json!({ "available": false }));
        }
    };
    
    if latest_ver > current_ver {
        log::info!("Update available: {}", latest_version);
        Ok(serde_json::json!({
            "available": true,
            "version": latest_version,
            "body": release.get("body").and_then(|b| b.as_str()).unwrap_or("")
        }))
    } else {
        log::info!("No update available (current: {}, latest: {})", current_version, latest_version);
        Ok(serde_json::json!({
            "available": false
        }))
    }
}

#[tauri::command]
async fn install_update() -> Result<String, String> {
    use serde_json::Value;
    use std::fs;
    use std::io::Write;
    use std::process::Command;
    
    log::info!("Starting update installation...");
    
    // Get latest release info
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/stryyk3r/ARKADEManager/releases/latest")
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release info: {}", e))?;
    
    let release: Value = response.json().await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;
    
    // Find the NSIS installer asset
    let assets = release.get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "No assets found in release".to_string())?;
    
    let installer_url = assets.iter()
        .find_map(|asset| {
            let name = asset.get("browser_download_url")?.as_str()?;
            if name.ends_with("-setup.exe") {
                Some(name)
            } else {
                None
            }
        })
        .ok_or_else(|| "No installer found in release assets".to_string())?;
    
    log::info!("Downloading installer from: {}", installer_url);
    
    // Download installer
    let installer_response = client
        .get(installer_url)
        .header("User-Agent", "ARKADE-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to download installer: {}", e))?;
    
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("ARKADE_Manager_Update.exe");
    
    let mut file = fs::File::create(&installer_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    
    let bytes = installer_response.bytes().await
        .map_err(|e| format!("Failed to read installer data: {}", e))?;
    
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write installer: {}", e))?;
    
    // Close the file handle before launching
    drop(file);
    
    log::info!("Installer downloaded to: {}", installer_path.display());
    
    // Small delay to ensure file is fully written and closed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Launch installer
    Command::new(&installer_path)
        .spawn()
        .map_err(|e| format!("Failed to launch installer: {}", e))?;
    
    log::info!("Installer launched, exiting application...");
    
    // Exit the app so installer can run
    std::process::exit(0);
}

#[tauri::command]
async fn read_logs(lines: Option<usize>) -> Result<String, String> {
    app_data::read_logs(lines.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_source_plugins(source_path: String) -> Result<Vec<plugins::SourcePlugin>, String> {
    plugins::list_source_plugins(&source_path)
}

#[tauri::command]
async fn discover_plugin_destinations() -> Result<Vec<plugins::DestinationServer>, String> {
    plugins::discover_destinations()
}

#[tauri::command]
async fn install_plugins(
    source_plugin_paths: Vec<String>,
    destination_plugin_paths: Vec<String>,
) -> Result<plugins::InstallResult, String> {
    plugins::install_plugins(source_plugin_paths, destination_plugin_paths)
}

fn main() {
    // Initialize logger with file output
    let app_data = AppData::new().unwrap_or_else(|e| {
        eprintln!("Failed to initialize app data: {}", e);
        std::process::exit(1);
    });
    let log_file = app_data.get_logs_dir().join("arkade_manager.log");
    
    simplelog::WriteLogger::init(
        log::LevelFilter::Info,
        simplelog::Config::default(),
        std::fs::File::create(&log_file).unwrap_or_else(|e| {
            eprintln!("Failed to create log file: {}", e);
            std::process::exit(1);
        }),
    )
    .unwrap_or(());
    
    log::info!("ARKADE Manager starting up");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data = Arc::new(Mutex::new(AppData::new()?));
            let scheduler = Arc::new(Mutex::new(Scheduler::new(app.handle().clone())));
            let app_handle = app.handle().clone();

            // Start scheduler
            {
                let scheduler_clone = scheduler.clone();
                let app_data_clone = app_data.clone();
                let _app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                    loop {
                        interval.tick().await;
                        let mut sched = scheduler_clone.lock().await;
                        let app_data_guard = app_data_clone.lock().await;
                        if let Err(e) = sched.tick(&app_data_guard) {
                            log::error!("Scheduler tick error: {}", e);
                        }
                        // Clear current_job if backup completed (check by seeing if job was updated)
                        // This is handled by the backup task, but we can also check here
                    }
                });
            }

            app.manage(AppState {
                app_data,
                scheduler,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_theme,
            list_jobs,
            add_job,
            update_job,
            delete_job,
            run_job_now,
            preview_monthly_archive,
            run_monthly_archive,
            get_status,
            read_logs,
            list_source_plugins,
            discover_plugin_destinations,
            install_plugins,
            get_app_version,
            check_for_updates,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

