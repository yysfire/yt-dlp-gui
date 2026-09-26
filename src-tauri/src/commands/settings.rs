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
    mut settings: AppSettings,
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<AppSettings, String> {
    // scheduler_paused 由托盘菜单拥有，前端设置对话框并不编辑它，但保存时会把整个
    // settings 回传（带着打开对话框那一刻的陈旧值）。用缓存里的权威值覆盖入参，
    // 否则「托盘暂停 → 打开设置 → 保存」会把暂停悄悄反转。
    // 先读缓存并释放锁、再落盘：save_settings 内部会拿全局写锁，两步不重叠以避免锁序环。
    let authoritative_paused = state
        .settings
        .lock()
        .map(|cached| cached.scheduler_paused)
        .unwrap_or(settings.scheduler_paused);
    settings.scheduler_paused = authoritative_paused;

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

/// Returns the current application state (last check time).
///
/// 已下载数不再由本命令返回：它由前端从下载记录派生（口径 = `status == "completed"`
/// 的记录条数），因此这里是纯读，不再有「读时修正并写回」的行为。
#[tauri::command]
pub async fn get_app_state(
    state: State<'_, AppContext>,
) -> Result<AppState, String> {
    StorageService::load_state(&state.data_dir).map_err(|e| e.to_string())
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
