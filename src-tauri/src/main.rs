// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_data;
mod backup;
mod commands;
mod config;
mod data_lookup;
mod job;
mod map;
mod palworld_rest;
mod plugins;
mod scheduler;
mod server_ini;
mod server_roots;
mod state;
mod validation;

use std::sync::Arc;

use chrono::Utc;
use tauri::Manager;
use tokio::sync::Mutex;

use app_data::AppData;
use scheduler::Scheduler;
use state::AppState;

// Custom filter to suppress winit warnings
struct WinitFilter {
    inner: Box<dyn log::Log>,
}

impl log::Log for WinitFilter {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if metadata.level() == log::Level::Warn {
            let target = metadata.target();
            if target.contains("winit") || target.contains("tauri_wry") {
                return false;
            }
        }
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if record.level() == log::Level::Warn {
            let msg = format!("{}", record.args());
            if msg.contains("NewEvents emitted without explicit RedrawEventsCleared")
                || msg.contains("RedrawEventsCleared emitted without explicit MainEventsCleared")
                || record.target().contains("winit")
                || record.target().contains("tauri_wry")
            {
                return;
            }
        }
        self.inner.log(record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

fn main() {
    let app_data = AppData::new().unwrap_or_else(|e| {
        eprintln!("Failed to initialize app data: {}", e);
        std::process::exit(1);
    });
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let log_file_name = format!("arkade_manager_{}.log", timestamp);
    let log_file = app_data.get_logs_dir().join(&log_file_name);
    let _ = app_data::CURRENT_LOG_FILE.set(log_file_name);

    let log_config = simplelog::ConfigBuilder::new()
        // Error is the least verbose level, so time and module path are written on every line
        // (the Logs tab parses both).
        .set_time_level(log::LevelFilter::Error)
        .set_time_format_rfc3339()
        .set_thread_level(log::LevelFilter::Off)
        .set_target_level(log::LevelFilter::Error)
        .set_location_level(log::LevelFilter::Off)
        .build();

    let file = std::fs::File::create(&log_file).unwrap_or_else(|e| {
        eprintln!("Failed to create log file: {}", e);
        std::process::exit(1);
    });

    let base_logger = simplelog::WriteLogger::new(log::LevelFilter::Info, log_config, file);

    let filtered_logger = WinitFilter {
        inner: Box::new(base_logger),
    };

    log::set_boxed_logger(Box::new(filtered_logger))
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap_or(());

    log::info!("ARKADE Manager starting up");
    log::info!("Log file: {}", log_file.display());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data = Arc::new(Mutex::new(AppData::new()?));
            let scheduler = Arc::new(Mutex::new(Scheduler::new(app.handle().clone())));

            // Registered before the first tick so backup tasks can always reach the shared state.
            app.manage(AppState {
                app_data: app_data.clone(),
                scheduler: scheduler.clone(),
            });

            {
                let scheduler_clone = scheduler.clone();
                let app_data_clone = app_data.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(30));
                    loop {
                        interval.tick().await;
                        // Lock order: app_data, then scheduler (see AppState).
                        let app_data_guard = app_data_clone.lock().await;
                        let mut sched = scheduler_clone.lock().await;
                        if let Err(e) = sched.tick(&app_data_guard) {
                            log::error!("Scheduler tick error: {}", e);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_plugin_server_roots,
            commands::list_plugin_folders,
            commands::toggle_plugin_folder,
            commands::toggle_plugin_for_all_servers,
            commands::get_config,
            commands::set_theme,
            commands::get_ark_maps,
            commands::save_ark_maps,
            commands::reset_ark_maps,
            commands::save_server_roots,
            commands::save_monthly_destination,
            commands::get_app_paths,
            commands::inspect_directory,
            commands::check_destination,
            commands::list_jobs,
            commands::add_job,
            commands::update_job,
            commands::delete_job,
            commands::run_job_now,
            commands::open_backup_location,
            commands::preview_monthly_archive,
            commands::run_monthly_archive,
            commands::get_monthly_status,
            commands::get_status,
            commands::read_logs,
            commands::list_log_files,
            commands::open_logs_folder,
            commands::lookup_data_files,
            commands::delete_data_files,
            commands::open_external_url,
            commands::list_source_plugins,
            commands::discover_plugin_destinations,
            commands::install_plugins,
            commands::get_app_version,
            commands::check_for_updates,
            commands::install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
