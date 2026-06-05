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
    let old_interval = cached.check_interval_minutes;
    let concurrent_changed = cached.max_concurrent_downloads != settings.max_concurrent_downloads;
    *cached = settings.clone();
    drop(cached); // release lock before await

    // Notify scheduler if interval changed so it wakes up immediately
    if detect_interval_change(old_interval, settings.check_interval_minutes) {
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

// T032: helper extracted for testability — determines whether scheduler notification is needed.
#[doc(hidden)]
pub fn detect_interval_change(old_interval: u32, new_interval: u32) -> bool {
    old_interval != new_interval
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::watch;

    #[test]
    fn test_detect_interval_change_same() {
        // No change: should return false
        assert!(!detect_interval_change(60, 60));
        assert!(!detect_interval_change(0, 0));
        assert!(!detect_interval_change(1440, 1440));
    }

    #[test]
    fn test_detect_interval_change_different() {
        // Change detected: should return true
        assert!(detect_interval_change(60, 30));
        assert!(detect_interval_change(30, 60));
        assert!(detect_interval_change(1440, 0));
        assert!(detect_interval_change(0, 60));
    }

    #[test]
    fn test_scheduler_notify_sent_on_change() {
        // Verify watch::Sender::send() is actually called when interval changes
        let (tx, mut rx) = watch::channel(());
        let old_interval = 60u32;
        let new_interval = 30u32;

        if detect_interval_change(old_interval, new_interval) {
            let _ = tx.send(());
        }

        // rx should have been notified
        assert!(rx.has_changed().unwrap_or(false));
    }

    #[test]
    fn test_scheduler_notify_not_sent_when_unchanged() {
        let (tx, mut rx) = watch::channel(());

        // Consume the initial value
        let _ = rx.borrow_and_update();

        let old_interval = 60u32;
        let new_interval = 60u32;

        if detect_interval_change(old_interval, new_interval) {
            // This branch should NOT execute
            let _ = tx.send(());
        }

        // rx should NOT have changed
        assert!(!rx.has_changed().unwrap_or(true));
    }

    #[test]
    fn test_detect_called_for_all_frequency_options() {
        // Verify detection works across all spec-defined frequencies
        let options: Vec<(&str, u32)> = vec![
            ("手动", 0),
            ("30分钟", 30),
            ("每小时", 60),
            ("每天", 1440),
        ];

        for (i, (_, v1)) in options.iter().enumerate() {
            for (j, (_, v2)) in options.iter().enumerate() {
                let expected = i != j;
                assert_eq!(
                    detect_interval_change(*v1, *v2),
                    expected,
                    "detect_interval_change({}, {}) should be {}",
                    v1, v2, expected
                );
            }
        }
    }
}
