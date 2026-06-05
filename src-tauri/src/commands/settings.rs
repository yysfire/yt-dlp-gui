use tauri::{Manager, State};

use crate::models::{AppSettings, AppState};
use crate::services::settings_validator;
use crate::services::StorageService;
use crate::AppContext;
use crate::QueueContext;

/// Returns the current application settings.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppContext>,
) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

/// Updates application settings and persists them.
#[tauri::command]
pub async fn update_settings(
    settings: AppSettings,
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<AppSettings, String> {
    // Persist to disk
    StorageService::save_settings(&state.data_dir, &settings)
        .map_err(|e| e.to_string())?;

    // Update runtime cache
    let mut cached = state.settings.lock().map_err(|e| e.to_string())?;
    let interval_changed = cached.check_interval_minutes != settings.check_interval_minutes;
    let concurrent_changed = cached.max_concurrent_downloads != settings.max_concurrent_downloads;
    *cached = settings.clone();
    drop(cached); // release lock before await

    // Notify scheduler if interval changed so it wakes up immediately
    if interval_changed {
        let _ = state.scheduler_notify.send(());
    }

    // T029: Notify download queue if concurrent limit changed (FR-010)
    if concurrent_changed {
        if let Some(queue_state) = app_handle.try_state::<QueueContext>() {
            if let Ok(guard) = queue_state.queue.lock() {
                if let Some(ref queue) = *guard {
                    queue.update_max_concurrent(settings.max_concurrent_downloads);
                }
            }
        }
    }

    log::info!("Settings updated");
    Ok(settings)
}

/// Returns the current application state (last check time, total downloads).
#[tauri::command]
pub async fn get_app_state(
    state: State<'_, AppContext>,
) -> Result<AppState, String> {
    let mut app_state = StorageService::load_state(&state.data_dir).map_err(|e| e.to_string())?;
    // Recompute from actual records in case state.json is stale
    if let Ok(records) = StorageService::load_download_records(&state.data_dir) {
        let actual = records.iter().filter(|r| r.status == "completed").count() as u32;
        if app_state.total_downloads != actual {
            app_state.total_downloads = actual;
            let _ = StorageService::save_state(&state.data_dir, &app_state);
        }
    }
    Ok(app_state)
}

/// The background scheduler is managed by lib.rs setup. This command exists
/// for API compatibility and logs that the scheduler is already running.
#[tauri::command]
pub async fn start_scheduler(
    _app_handle: tauri::AppHandle,
    _state: State<'_, AppContext>,
) -> Result<(), String> {
    log::info!("start_scheduler called — scheduler is managed by lib.rs setup");
    Ok(())
}

/// Stops the background scheduler.
#[tauri::command]
pub async fn stop_scheduler(
    _state: State<'_, AppContext>,
) -> Result<(), String> {
    // In the MVP, the scheduler handle is managed via a leaked Box.
    // A production implementation would store the handle in AppContext.
    log::info!("Scheduler stop requested (no-op in MVP)");
    Ok(())
}

/// Validates a download directory path for existence and writability.
#[tauri::command]
pub async fn validate_download_path(
    path: String,
) -> Result<settings_validator::PathValidateResult, String> {
    Ok(settings_validator::validate_download_path(&path))
}

/// Validates a proxy URL format (supports http, https, socks5, socks5h).
#[tauri::command]
pub async fn validate_proxy_url(
    url: String,
) -> Result<settings_validator::ProxyValidateResult, String> {
    Ok(settings_validator::validate_proxy_url(&url))
}
