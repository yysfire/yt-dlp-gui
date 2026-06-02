use tauri::State;

use crate::models::{AppSettings, AppState};
use crate::services::StorageService;
use crate::AppContext;

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
) -> Result<AppSettings, String> {
    // Persist to disk
    StorageService::save_settings(&state.data_dir, &settings)
        .map_err(|e| e.to_string())?;

    // Update runtime cache
    let mut cached = state.settings.lock().map_err(|e| e.to_string())?;
    *cached = settings.clone();

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

/// Starts the background scheduler that periodically checks all subscriptions.
#[tauri::command]
pub async fn start_scheduler(
    app_handle: tauri::AppHandle,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let (interval_mins, data_dir, settings_clone) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            settings.check_interval_minutes,
            state.data_dir.clone(),
            settings.clone(),
        )
    };

    log::info!(
        "Starting scheduler with interval {} minutes",
        interval_mins
    );

    // Spawn a background tokio task that periodically checks all subscriptions
    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs((interval_mins as u64) * 60),
        );
        // Skip the first immediate tick
        interval.tick().await;

        loop {
            interval.tick().await;
            log::info!("Scheduler: checking subscriptions...");

            let subs = StorageService::load_subscriptions(&data_dir).unwrap_or_default();
            let records =
                StorageService::load_download_records(&data_dir).unwrap_or_default();
            let app_state = StorageService::load_state(&data_dir).unwrap_or_default();

            let yt_dlp_path = settings_clone.yt_dlp_path.clone();
            let proxy = Some(settings_clone.proxy_url.clone());
            let cookie_file = Some(settings_clone.cookie_file.clone());
            let download_dir = std::path::PathBuf::from(&settings_clone.download_dir);

            let mut total_completed: u32 = 0;

            for sub in &subs {
                if sub.paused {
                    continue;
                }

                match crate::commands::download::check_and_download(
                    sub,
                    &yt_dlp_path,
                    &proxy,
                    &cookie_file,
                    &download_dir,
                    &data_dir,
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

            // Update state
            let mut updated_state = app_state;
            updated_state.last_check_time =
                Some(chrono::Utc::now().to_rfc3339());
            updated_state.total_downloads += total_completed;
            let _ = StorageService::save_state(&data_dir, &updated_state);
        }
    });

    // Leak the handle to keep the scheduler alive (improved in T05)
    log::info!("Scheduler started successfully");
    let _ = Box::leak(Box::new(handle));

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
