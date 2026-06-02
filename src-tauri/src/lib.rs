use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Emitter, Manager};

mod models;
mod utils;
mod services;
mod commands;

use crate::services::StorageService;
use crate::services::download_queue::DownloadQueue;

/// Application-wide context shared across all command handlers.
pub struct AppContext {
    /// Path to the app data directory (~/.yt-dlp-sub-gui/)
    pub data_dir: PathBuf,
    /// Runtime settings cache, protected by a mutex for interior mutability.
    pub settings: Mutex<models::settings::AppSettings>,
}

/// Holds the global download queue instance.
pub struct QueueContext {
    pub queue: Mutex<Option<DownloadQueue>>,
}

/// Initializes the Tauri application, registers all commands and plugins.
pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Resolve the app data directory
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");

            // Ensure the data directory exists
            std::fs::create_dir_all(&data_dir)
                .expect("failed to create app data directory");

            // Load or create default settings
            let settings = StorageService::load_settings(&data_dir);

            // Initialize default state if missing
            let _ = StorageService::save_state(
                &data_dir,
                &StorageService::load_state(&data_dir).unwrap_or_default(),
            );

            let interval_mins = settings.check_interval_minutes;
            let max_concurrent = settings.max_concurrent_downloads;
            let data_dir_clone = data_dir.clone();
            let settings_clone = settings.clone();

            let ctx = AppContext {
                data_dir,
                settings: Mutex::new(settings),
            };

            app.manage(ctx);

            // Recover download state from previous session
            let _ = commands::download::recover_state(&data_dir_clone);

            // Deduplicate download records from previous sessions
            let _ = StorageService::deduplicate_records(&data_dir_clone);

            // Initialize the global download queue
            let app_handle = app.handle().clone();
            let queue = DownloadQueue::new(app_handle.clone(), max_concurrent);
            let queue_ctx = QueueContext {
                queue: Mutex::new(Some(queue)),
            };
            app.manage(queue_ctx);

            // Start the background scheduler in a tokio task
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(
                    tokio::time::Duration::from_secs((interval_mins as u64) * 60),
                );
                // Skip first immediate tick
                interval.tick().await;

                loop {
                    interval.tick().await;
                    log::info!("Scheduler: checking subscriptions...");

                    let subs = StorageService::load_subscriptions(&data_dir_clone)
                        .unwrap_or_default();
                    let records = StorageService::load_download_records(&data_dir_clone)
                        .unwrap_or_default();
                    let app_state = StorageService::load_state(&data_dir_clone)
                        .unwrap_or_default();

                    let yt_dlp_path = settings_clone.yt_dlp_path.clone();
                    let proxy = Some(settings_clone.proxy_url.clone());
                    let cookie_file = Some(settings_clone.cookie_file.clone());
                    let download_dir =
                        std::path::PathBuf::from(&settings_clone.download_dir);

                    let mut total_completed: u32 = 0;

                    for sub in &subs {
                        if sub.paused {
                            continue;
                        }

                        match commands::download::check_and_download(
                            sub,
                            &yt_dlp_path,
                            &proxy,
                            &cookie_file,
                            &download_dir,
                            &data_dir_clone,
                            &app_state.last_check_time,
                            &records,
                            &app_handle,
                        )
                        .await
                        {
                            Ok(new_records) => {
                                total_completed += new_records
                                    .iter()
                                    .filter(|r| r.status == "completed")
                                    .count() as u32;
                            }
                            Err(e) => {
                                log::error!(
                                    "Scheduler: error checking {}: {}",
                                    sub.channel_name,
                                    e
                                );
                            }
                        }
                    }

                    // Update application state
                    let mut updated_state = app_state;
                    updated_state.last_check_time =
                        Some(chrono::Utc::now().to_rfc3339());
                    updated_state.total_downloads += total_completed;
                    let _ = StorageService::save_state(
                        &data_dir_clone,
                        &updated_state,
                    );

                    // Emit an event to notify the frontend to refresh
                    let _ = app_handle.emit("scheduler-check-complete", ());
                }
            });

            log::info!("Application initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::subscription::add_subscription,
            commands::subscription::delete_subscription,
            commands::subscription::get_subscriptions,
            commands::subscription::toggle_subscription_pause,
            commands::subscription::update_subscription_quality,
            commands::subscription::update_subscription_group,
            commands::download::check_subscription,
            commands::download::check_all_subscriptions,
            commands::download::get_download_records,
            commands::download::get_all_download_records,
            commands::download::manual_check_all,
            commands::download::get_download_queue,
            commands::download::get_queue_state,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::cancel_download,
            commands::download::pause_download_by_url,
            commands::download::cancel_download_by_url,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_app_state,
            commands::settings::start_scheduler,
            commands::settings::stop_scheduler,
            commands::import_export::export_subscriptions_json,
            commands::import_export::export_subscriptions_opml,
            commands::import_export::batch_import_subscriptions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
