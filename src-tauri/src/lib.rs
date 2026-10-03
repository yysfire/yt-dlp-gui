use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;
use tokio::sync::watch;

mod models;
mod utils;
mod services;
mod commands;

use crate::services::StorageService;
use crate::services::download_queue::DownloadQueue;
use crate::services::tray::TrayService;
use crate::commands::import_export::ImportContext;

/// Application-wide context shared across all command handlers.
pub struct AppContext {
    /// Path to the app data directory (~/.yt-dlp-sub-gui/)
    pub data_dir: PathBuf,
    /// Runtime settings cache, protected by a mutex for interior mutability.
    pub settings: Mutex<models::settings::AppSettings>,
    /// Watch channel to notify the scheduler when settings (interval) change.
    pub scheduler_notify: watch::Sender<()>,
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
            let _ = StorageService::update_state(&data_dir, |_| Ok(()));

            let (scheduler_tx, scheduler_rx) = watch::channel(());
            let max_concurrent = settings.max_concurrent_downloads;
            let data_dir_clone = data_dir.clone();
            let data_dir_fs_sync = data_dir.clone();
            let settings_clone = settings.clone();

            let ctx = AppContext {
                data_dir,
                settings: Mutex::new(settings),
                scheduler_notify: scheduler_tx,
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

            // Initialize import cancellation context
            let import_ctx = ImportContext {
                cancel_flag: Mutex::new(None),
            };
            app.manage(import_ctx);

            // ── System Tray Setup (spec 005-system-tray-icon) ──────────
            let tray_service = match TrayService::init(&app_handle, data_dir_fs_sync.clone()) {
                Ok(svc) => {
                    log::info!("Tray service initialized, supported={}", svc.is_supported());
                    std::sync::Arc::new(svc)
                }
                Err(e) => {
                    log::error!("Tray service initialization failed: {}", e);
                    return Err(e);
                }
            };

            // Register menu event handler on the tray icon
            if let Some(ref tray) = tray_service.tray_icon {
                let tray_svc = tray_service.clone();
                let app_handle_clone = app_handle.clone();
                tray.on_menu_event(move |_app, event| {
                    tray_svc.handle_menu_event(&app_handle_clone, event);
                });
            }

            // Start minimized to tray if configured
            if settings_clone.start_in_tray {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            // Register window event handlers for close-to-tray and minimize-to-tray
            if let Some(window) = app_handle.get_webview_window("main") {
                let tray_clone = tray_service.clone();
                let data_dir_events = data_dir_fs_sync.clone();
                let app_handle_events = app_handle.clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            let settings = StorageService::load_settings(&data_dir_events);
                            if settings.close_to_tray && tray_clone.is_supported() {
                                api.prevent_close();
                                tray_clone.hide_window(&app_handle_events);
                            }
                        }
                        _ => {}
                    }
                });
            }

            app.manage(tray_service);

            // ── 后台周期任务（实现见 services::scheduler）────────────────
            // 自动检查订阅；循环内部每轮从磁盘重读设置，并遵循 scheduler_paused
            services::scheduler::spawn_scheduler(
                data_dir_clone,
                app_handle.clone(),
                scheduler_rx,
            );

            // 周期性文件状态同步（每 5 分钟）
            services::scheduler::spawn_file_sync(data_dir_fs_sync, app_handle.clone());

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
            commands::download::get_download_queue,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::cancel_download,
            commands::download::pause_download_by_url,
            commands::download::cancel_download_by_url,
            commands::download::redownload_video,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_app_state,
            commands::settings::validate_download_path,
            commands::settings::validate_proxy_url,
            commands::import_export::export_subscriptions_json,
            commands::import_export::export_subscriptions_opml,
            commands::import_export::batch_import_subscriptions,
            commands::import_export::batch_import_preview,
            commands::import_export::batch_import_execute,
            commands::import_export::cancel_import,
            commands::file_manager::open_in_folder,
            commands::file_manager::check_file_existence,
            commands::file_manager::delete_file,
            commands::file_manager::sync_file_states,
            commands::health::check_all_health,
            commands::health::check_selected_health,
            commands::download::get_channel_videos,
            commands::subscription::get_channel_info,
            commands::subscription::batch_delete_subscriptions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
