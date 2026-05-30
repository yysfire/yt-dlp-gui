use std::path::PathBuf;

use chrono::Utc;
use tauri::{Emitter, Manager, State};

use crate::models::{DownloadRecord, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::services::download_queue::{DownloadQueue, QueueState};
use crate::utils::AppError;
use crate::AppContext;
use crate::QueueContext;

/// Core logic for checking a single subscription for new videos and downloading them.
/// Used by both the command layer and the scheduler.
pub(crate) async fn check_and_download(
    sub: &Subscription,
    yt_dlp_path: &str,
    proxy: &Option<String>,
    cookie_file: &Option<String>,
    download_dir: &PathBuf,
    data_dir: &PathBuf,
    last_check_time: &Option<String>,
    existing_records: &[DownloadRecord],
    app_handle: &tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, AppError> {
    // Determine the date cutoff for checking
    let since = match last_check_time {
        Some(t) => {
            // Convert ISO 8601 to YYYYMMDD
            let parsed = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.fZ")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%SZ"))
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(&t[..10], "%Y-%m-%d")
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                });
            match parsed {
                Ok(dt) => dt.format("%Y%m%d").to_string(),
                Err(_) => "19700101".to_string(),
            }
        }
        None => "19700101".to_string(),
    };

    // Check for new videos
    log::info!("check_and_download: since={}, url={}", since, sub.url);
    let videos = YtDlpService::check_new_videos(yt_dlp_path, proxy, cookie_file, &sub.url, &since)?;

    let mut new_records: Vec<DownloadRecord> = Vec::new();
    // Track seen video IDs and URLs to avoid duplicates.
    // Failed records are not excluded — they can be retried.
    let mut seen_ids: std::collections::HashSet<String> = existing_records
        .iter()
        .filter(|r| r.status != "failed")
        .filter(|r| !r.video_id.is_empty())
        .map(|r| r.video_id.clone())
        .collect();
    let mut seen_urls: std::collections::HashSet<String> = existing_records
        .iter()
        .filter(|r| r.status != "failed")
        .map(|r| r.video_url.clone())
        .collect();

    for video in videos {
        // Dedup: prefer video_id, fall back to video_url
        let vid = video.id.clone().unwrap_or_default();
        if !vid.is_empty() {
            if !seen_ids.insert(vid.clone()) {
                continue;
            }
        } else if !seen_urls.insert(video.url.clone()) {
            continue;
        }

        // Create a "downloading" record
        let mut record =
            DownloadRecord::new(sub.id.clone(), video.title.clone(), video.url.clone(), video.id.clone().unwrap_or_default());

        // Save the record immediately so the frontend sees "downloading"
        let mut all_records = StorageService::load_download_records(data_dir)?;
        all_records.push(record.clone());
        StorageService::save_download_records(data_dir, &all_records)?;

        // Emit event so frontend can refresh immediately
        let _ = app_handle.emit("records-changed", ());

        // Attempt download
        let quality = sub.quality_preset.clone();
        match YtDlpService::download_video(
            yt_dlp_path, proxy, cookie_file, &video.url, &quality, download_dir,
        ) {
            Ok(result) => {
                record.status = "completed".to_string();
                record.file_path = result.file_path;
                record.file_size = result.file_size;
                record.downloaded_at = Utc::now().to_rfc3339();

                // Send desktop notification if enabled
                if let Some(ctx) = app_handle.try_state::<crate::AppContext>() {
                    if let Ok(settings) = ctx.settings.lock() {
                        if settings.notifications_enabled {
                            let _ = app_handle.emit(
                                "download-complete",
                                serde_json::json!({
                                    "title": &record.video_title,
                                    "channel": &sub.channel_name,
                                }),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Download failed for {}: {}", video.title, e);
                record.status = "failed".to_string();
                record.error_message = Some(e.to_string());
                record.downloaded_at = Utc::now().to_rfc3339();
            }
        }

        // Update the record in storage
        all_records = StorageService::load_download_records(data_dir)?;
        if let Some(existing) = all_records.iter_mut().find(|r| r.id == record.id) {
            *existing = record.clone();
        }
        StorageService::save_download_records(data_dir, &all_records)?;

        // Emit event for status update
        let _ = app_handle.emit("records-changed", ());

        new_records.push(record);
    }

    Ok(new_records)
}

/// Checks a single subscription for new videos and downloads them.
#[tauri::command]
pub async fn check_subscription(
    id: String,
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, String> {
    let subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;
    let sub = subs
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
        .map_err(|e| e.to_string())?;

    if sub.paused {
        return Ok(Vec::new());
    }

    // Clone settings values and drop the MutexGuard before awaiting
    let (yt_dlp_path, proxy, cookie_file, download_dir) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            settings.yt_dlp_path.clone(),
            Some(settings.proxy_url.clone()),
            Some(settings.cookie_file.clone()),
            PathBuf::from(&settings.download_dir),
        )
    };

    let app_state = StorageService::load_state(&state.data_dir).map_err(|e| e.to_string())?;
    let records =
        StorageService::load_download_records(&state.data_dir).map_err(|e| e.to_string())?;

    let new_records = check_and_download(
        sub,
        &yt_dlp_path,
        &proxy,
        &cookie_file,
        &download_dir,
        &state.data_dir,
        &app_state.last_check_time,
        &records,
        &app_handle,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(new_records)
}

/// Checks all non-paused subscriptions for new videos and downloads them.
#[tauri::command]
pub async fn check_all_subscriptions(
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, String> {
    let subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;

    // Clone settings values and drop the MutexGuard before awaiting
    let (yt_dlp_path, proxy, cookie_file, download_dir) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            settings.yt_dlp_path.clone(),
            Some(settings.proxy_url.clone()),
            Some(settings.cookie_file.clone()),
            PathBuf::from(&settings.download_dir),
        )
    };

    let app_state = StorageService::load_state(&state.data_dir).map_err(|e| e.to_string())?;
    let records =
        StorageService::load_download_records(&state.data_dir).map_err(|e| e.to_string())?;

    let mut all_new: Vec<DownloadRecord> = Vec::new();
    let mut total_completed: u32 = 0;

    log::info!(
        "check_all: {} subscriptions, cookie={}, proxy={}, ytdlp={}",
        subs.len(),
        cookie_file.as_deref().unwrap_or("none"),
        proxy.as_deref().unwrap_or("none"),
        yt_dlp_path,
    );

    for sub in &subs {
        if sub.paused {
            continue;
        }

        match check_and_download(
            sub,
            &yt_dlp_path,
            &proxy,
            &cookie_file,
            &download_dir,
            &state.data_dir,
            &app_state.last_check_time,
            &records,
            &app_handle,
        )
        .await
        {
            Ok(new_records) => {
                total_completed +=
                    new_records.iter().filter(|r| r.status == "completed").count() as u32;
                all_new.extend(new_records);
            }
            Err(e) => {
                return Err(format!(
                    "Error checking {}: {}",
                    sub.channel_name,
                    e
                ));
            }
        }
    }

    // Update application state
    let mut updated_state = app_state.clone();
    updated_state.last_check_time = Some(Utc::now().to_rfc3339());
    updated_state.total_downloads += total_completed;
    StorageService::save_state(&state.data_dir, &updated_state)
        .map_err(|e| e.to_string())?;

    Ok(all_new)
}

/// Returns download records, optionally filtered by subscription ID.
#[tauri::command]
pub async fn get_download_records(
    subscription_id: Option<String>,
    state: State<'_, AppContext>,
) -> Result<Vec<DownloadRecord>, String> {
    let records = StorageService::load_download_records(&state.data_dir)
        .map_err(|e| e.to_string())?;

    match subscription_id {
        Some(sid) => Ok(records.into_iter().filter(|r| r.subscription_id == sid).collect()),
        None => Ok(records),
    }
}

/// Returns all download records without filtering.
#[tauri::command]
pub async fn get_all_download_records(
    state: State<'_, AppContext>,
) -> Result<Vec<DownloadRecord>, String> {
    StorageService::load_download_records(&state.data_dir).map_err(|e| e.to_string())
}

/// Returns the current in-memory download queue tasks.

/// Check for new videos and enqueue downloads instead of downloading directly.
/// Used by the queue-driven flow.
pub(crate) async fn check_and_enqueue(
    sub: &Subscription,
    yt_dlp_path: &str,
    proxy: &Option<String>,
    cookie_file: &Option<String>,
    _download_dir: &PathBuf,
    data_dir: &PathBuf,
    last_check_time: &Option<String>,
    existing_records: &[DownloadRecord],
    queue: &DownloadQueue,
) -> Result<usize, AppError> {
    // Determine the date cutoff for checking
    let since = match last_check_time {
        Some(t) => {
            let parsed = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.fZ")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%SZ"))
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(&t[..10], "%Y-%m-%d")
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                });
            match parsed {
                Ok(dt) => dt.format("%Y%m%d").to_string(),
                Err(_) => "19700101".to_string(),
            }
        }
        None => "19700101".to_string(),
    };

    let videos = YtDlpService::check_new_videos(yt_dlp_path, proxy, cookie_file, &sub.url, &since)?;

    // Dedup: prefer video_id, fall back to video_url. Failed records can retry.
    let mut seen_ids: std::collections::HashSet<String> = existing_records
        .iter()
        .filter(|r| r.status != "failed")
        .filter(|r| !r.video_id.is_empty())
        .map(|r| r.video_id.clone())
        .collect();
    let mut seen_urls: std::collections::HashSet<String> = existing_records
        .iter()
        .filter(|r| r.status != "failed")
        .map(|r| r.video_url.clone())
        .collect();

    let quality = sub.quality_preset.clone();
    let mut count = 0;

    for video in videos {
        let vid = video.id.clone().unwrap_or_default();
        if !vid.is_empty() {
            if !seen_ids.insert(vid.clone()) {
                continue;
            }
        } else if !seen_urls.insert(video.url.clone()) {
            continue;
        }

        // Create and save a "downloading" record
        let record = DownloadRecord::new(
            sub.id.clone(),
            video.title.clone(),
            video.url.clone(),
            vid.clone(),
        );

        let mut all_records = StorageService::load_download_records(data_dir)?;
        all_records.push(record);
        StorageService::save_download_records(data_dir, &all_records)?;

        // Enqueue to download queue
        queue.enqueue_from_video(
            sub,
            video.title.clone(),
            video.url.clone(),
            vid.clone(),
            quality.clone(),
            data_dir,
        )?;

        count += 1;
    }

    Ok(count)
}
#[tauri::command]
pub async fn get_download_queue(
    queue_ctx: State<'_, QueueContext>,
) -> Result<Vec<crate::services::download_queue::DownloadTask>, String> {
    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => Ok(q.get_tasks()),
        None => Ok(Vec::new()),
    }
}

/// Returns the runtime queue state (active/waiting counts).

/// Recovers download state on application restart.
/// Marks "downloading" and "paused" records as "failed" since the download process
/// was terminated when the application exited.
pub fn recover_state(data_dir: &PathBuf) -> Result<(), AppError> {
    let mut records = StorageService::load_download_records(data_dir)?;
    let mut changed = false;
    for record in records.iter_mut() {
        if record.status == "downloading" || record.status == "paused" {
            record.status = "failed".to_string();
            record.error_message = Some("Application restarted".to_string());
            changed = true;
        }
    }
    if changed {
        StorageService::save_download_records(data_dir, &records)?;
        log::info!("Recovered {} download records to failed state after restart", 
            records.iter().filter(|r| r.error_message.as_deref() == Some("Application restarted")).count());
    }
    Ok(())
}
#[tauri::command]
pub async fn get_queue_state(
    queue_ctx: State<'_, QueueContext>,
) -> Result<QueueState, String> {
    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => Ok(q.get_state()),
        None => Ok(QueueState {
            active_count: 0,
            waiting_count: 0,
            max_concurrent: 1,
        }),
    }
}

/// Manually triggers a full check of all subscriptions (same as check_all_subscriptions).
/// This is the user-facing "Check All" action.
#[tauri::command]
pub async fn manual_check_all(
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, String> {
    check_all_subscriptions(state, app_handle).await
}
