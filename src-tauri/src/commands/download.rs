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
                Ok(dt) => Some(dt.format("%Y%m%d").to_string()),
                Err(_) => None,
            }
        }
        None => None,
    };

    // Check for new videos
    log::info!("check_and_download: since={:?}, url={}", since, sub.url);
    let videos = YtDlpService::check_new_videos(yt_dlp_path, proxy, cookie_file, &sub.url, since.as_deref())?;

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

        let quality = sub.quality_preset.clone();

        // Queue path: enqueue_from_video handles record creation and saving
        let use_queue = app_handle.try_state::<QueueContext>().is_some();
        if use_queue {
            let queue_guard = app_handle.state::<QueueContext>();
            let maybe_queue = queue_guard.queue.lock().ok();
            if let Some(guard) = maybe_queue {
                if let Some(ref queue) = *guard {
                    queue.enqueue_from_video(
                        sub,
                        video.title.clone(),
                        video.url.clone(),
                        vid.clone(),
                        quality,
                        data_dir,
                    )?;
                    queue.start_processing(
                        crate::services::download_queue::DownloadContext {
                            yt_dlp_path: yt_dlp_path.to_string(),
                            proxy: proxy.clone(),
                            cookie_file: cookie_file.clone(),
                            download_dir: download_dir.clone(),
                            data_dir: data_dir.clone(),
                        },
                    );
                    continue;
                }
            }
        }

        // Non-queue fallback: create record directly
        let record =
            DownloadRecord::new(sub.id.clone(), video.title.clone(), video.url.clone(), vid.clone());

        // Save the record immediately so the frontend sees "downloading"
        let mut all_records = StorageService::load_download_records(data_dir)?;
        all_records.push(record.clone());
        StorageService::save_download_records(data_dir, &all_records)?;
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
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;
    let sub_idx = subs
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
        .map_err(|e| e.to_string())?;

    if subs[sub_idx].paused {
        return Ok(Vec::new());
    }

    let sub = &subs[sub_idx];

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

    // Update per-subscription data — download_count is managed by queue callback
    let sub = &mut subs[sub_idx];
    sub.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
    sub.last_check_status = Some("success".to_string());
    sub.last_check_error = None;
    StorageService::save_subscriptions(&state.data_dir, &subs).map_err(|e| e.to_string())?;

    // Also update global last_check_time
    if let Ok(mut app_state) = StorageService::load_state(&state.data_dir) {
        app_state.last_check_time = Some(chrono::Utc::now().to_rfc3339());
        let _ = StorageService::save_state(&state.data_dir, &app_state);
    }

    Ok(new_records)
}

/// Checks all non-paused subscriptions for new videos and downloads them.
#[tauri::command]
pub async fn check_all_subscriptions(
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, String> {
    let mut subs = StorageService::load_subscriptions(&state.data_dir)
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
    let mut subs_changed = false;

    log::info!(
        "check_all: {} subscriptions, cookie={}, proxy={}, ytdlp={}",
        subs.len(),
        cookie_file.as_deref().unwrap_or("none"),
        proxy.as_deref().unwrap_or("none"),
        yt_dlp_path,
    );

    for idx in 0..subs.len() {
        if subs[idx].paused {
            continue;
        }

        let sub = &subs[idx];
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
                // download_count is managed by the download queue callback
                let sub = &mut subs[idx];
                sub.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
                sub.last_check_status = Some("success".to_string());
                sub.last_check_error = None;
                subs_changed = true;
                all_new.extend(new_records);
            }
            Err(e) => {
                let sub = &mut subs[idx];
                sub.last_checked_at = Some(chrono::Utc::now().to_rfc3339());
                sub.last_check_status = Some("failed".to_string());
                sub.last_check_error = Some(e.to_string());
                subs_changed = true;
                log::error!(
                    "check_all: error checking {}: {}",
                    sub.channel_name,
                    sub.last_check_error.as_deref().unwrap_or("")
                );
            }
        }
    }

    if subs_changed {
        StorageService::save_subscriptions(&state.data_dir, &subs).map_err(|e| e.to_string())?;
    }

    // Update application state — only last_check_time.
    // total_downloads is managed by the download queue completion callback.
    let mut updated_state = app_state.clone();
    updated_state.last_check_time = Some(Utc::now().to_rfc3339());
    StorageService::save_state(&state.data_dir, &updated_state)
        .map_err(|e| e.to_string())?;

    Ok(all_new)
}

/// Returns download records, optionally filtered by subscription ID.
/// Records are deduplicated by video_id before returning.
#[tauri::command]
pub async fn get_download_records(
    subscription_id: Option<String>,
    state: State<'_, AppContext>,
) -> Result<Vec<DownloadRecord>, String> {
    let records = StorageService::load_download_records(&state.data_dir)
        .map_err(|e| e.to_string())?;
    let deduped = StorageService::deduplicate_vec(records);

    match subscription_id {
        Some(sid) => Ok(deduped.into_iter().filter(|r| r.subscription_id == sid).collect()),
        None => Ok(deduped),
    }
}

/// Returns all download records without filtering.
/// Records are deduplicated by video_id before returning.
#[tauri::command]
pub async fn get_all_download_records(
    state: State<'_, AppContext>,
) -> Result<Vec<DownloadRecord>, String> {
    let records = StorageService::load_download_records(&state.data_dir)
        .map_err(|e| e.to_string())?;
    Ok(StorageService::deduplicate_vec(records))
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
                Ok(dt) => Some(dt.format("%Y%m%d").to_string()),
                Err(_) => None,
            }
        }
        None => None,
    };

    let videos = YtDlpService::check_new_videos(yt_dlp_path, proxy, cookie_file, &sub.url, since.as_deref())?;

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

/// Pauses a running download task by its task ID.
#[tauri::command]
pub async fn pause_download(
    id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => q.pause(&id, &state.data_dir).map_err(|e| e.to_string()),
        None => Err("Download queue not initialized".to_string()),
    }
}

/// Resumes a paused download task by its task ID.
#[tauri::command]
pub async fn resume_download(
    id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => q.resume(&id, &state.data_dir).map_err(|e| e.to_string()),
        None => Err("Download queue not initialized".to_string()),
    }
}

/// Cancels a download task and cleans up partial files.
#[tauri::command]
pub async fn cancel_download(
    id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let ctx = crate::services::download_queue::DownloadContext {
        yt_dlp_path: settings.yt_dlp_path.clone(),
        proxy: Some(settings.proxy_url.clone()),
        cookie_file: Some(settings.cookie_file.clone()),
        download_dir: std::path::PathBuf::from(&settings.download_dir),
        data_dir: state.data_dir.clone(),
    };
    drop(settings);

    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => q.cancel(&id, &ctx).map_err(|e| e.to_string()),
        None => Err("Download queue not initialized".to_string()),
    }
}

/// Pauses a running download task identified by video_url.
#[tauri::command]
pub async fn pause_download_by_url(
    video_url: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => q.pause_by_url(&video_url, &state.data_dir).map_err(|e| e.to_string()),
        None => Err("Download queue not initialized".to_string()),
    }
}

/// Cancels a download task identified by video_url.
#[tauri::command]
pub async fn cancel_download_by_url(
    video_url: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let ctx = crate::services::download_queue::DownloadContext {
        yt_dlp_path: settings.yt_dlp_path.clone(),
        proxy: Some(settings.proxy_url.clone()),
        cookie_file: Some(settings.cookie_file.clone()),
        download_dir: std::path::PathBuf::from(&settings.download_dir),
        data_dir: state.data_dir.clone(),
    };
    drop(settings);

    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(q) => q.cancel_by_url(&video_url, &ctx).map_err(|e| e.to_string()),
        None => Err("Download queue not initialized".to_string()),
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

/// 分页获取订阅频道的视频列表。
///
/// 若订阅 health_status 为 Dead，返回错误。
#[tauri::command]
pub async fn get_channel_videos(
    subscription_id: String,
    page: u32,
    page_size: u32,
    state: State<'_, AppContext>,
) -> Result<crate::models::VideoListResult, String> {
    let subs = StorageService::load_subscriptions(&state.data_dir)
        .map_err(|e| e.to_string())?;
    let sub = subs
        .iter()
        .find(|s| s.id == subscription_id)
        .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", subscription_id)))
        .map_err(|e| e.to_string())?;

    // 健康状态为 Dead 时拒接请求
    if sub.health_status == Some(crate::models::health::HealthStatus::Dead) {
        return Err("此频道已失效，无法获取视频列表".to_string());
    }

    // 克隆配置值并释放锁
    let (yt_dlp_path, proxy, cookie_file) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            settings.yt_dlp_path.clone(),
            Some(settings.proxy_url.clone()),
            Some(settings.cookie_file.clone()),
        )
    };

    let start = (page.saturating_sub(1)) * page_size + 1;
    // 多取一条用于判断 has_more
    let end = page * page_size + 1;

    let videos = YtDlpService::get_channel_videos_paginated(
        &yt_dlp_path,
        &proxy,
        &cookie_file,
        &sub.url,
        start,
        end,
    )
    .map_err(|e| e.to_string())?;

    let fetched_count = videos.len();
    let has_more = fetched_count > page_size as usize;

    // 截断多余的那一条（用于判断 has_more 的）
    let display_videos = if has_more {
        videos.into_iter().take(page_size as usize).collect()
    } else {
        videos
    };

    Ok(crate::models::VideoListResult {
        videos: display_videos,
        // total 从 yt-dlp 的 playlist_count 可能得不到，这里用 fetched_count 近似
        // 当 has_more 为 true 时至少有 page_size * page + 1 条
        total: if has_more {
            (page * page_size) as usize + 1 // 至少还有
        } else {
            ((page.saturating_sub(1)) * page_size) as usize + fetched_count
        },
        page,
        page_size,
        has_more,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_record(
        sub_id: &str,
        title: &str,
        url: &str,
        vid: &str,
        status: &str,
    ) -> DownloadRecord {
        let mut r = DownloadRecord::new(
            sub_id.to_string(),
            title.to_string(),
            url.to_string(),
            vid.to_string(),
        );
        r.status = status.to_string();
        r
    }

    // ── recover_state tests (T031) ─────────────────────────────────

    #[test]
    fn test_recover_state_marks_downloading_as_failed() {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let records = vec![
            make_record("sub-1", "Video A", "https://youtube.com/watch?v=a", "vid-a", "downloading"),
            make_record("sub-1", "Video B", "https://youtube.com/watch?v=b", "vid-b", "completed"),
        ];
        StorageService::save_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered.len(), 2);
        // Video A was "downloading" → should be "failed"
        assert_eq!(recovered[0].status, "failed");
        assert_eq!(recovered[0].error_message, Some("Application restarted".to_string()));
        // Video B was "completed" → should stay "completed"
        assert_eq!(recovered[1].status, "completed");
        assert_eq!(recovered[1].error_message, None);
    }

    #[test]
    fn test_recover_state_marks_paused_as_failed() {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let records = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=c", "vid-c", "paused"),
        ];
        StorageService::save_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered[0].status, "failed");
        assert_eq!(recovered[0].error_message, Some("Application restarted".to_string()));
    }

    #[test]
    fn test_recover_state_leaves_completed_and_failed_unchanged() {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let records = vec![
            make_record("sub-1", "Video A", "https://youtube.com/watch?v=a", "vid-a", "completed"),
            make_record("sub-1", "Video B", "https://youtube.com/watch?v=b", "vid-b", "failed"),
            make_record("sub-1", "Video C", "https://youtube.com/watch?v=c", "vid-c", "cancelled"),
        ];
        StorageService::save_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered[0].status, "completed");
        assert_eq!(recovered[1].status, "failed");
        assert_eq!(recovered[2].status, "cancelled");
    }

    #[test]
    fn test_recover_state_no_changes_when_nothing_to_recover() {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let records = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=a", "vid-a", "completed"),
        ];
        StorageService::save_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered[0].status, "completed");
    }

    // ── Dedup logic tests (T014) ───────────────────────────────────

    /// Helper that mimics the dedup logic in check_and_download:
    /// builds seen_ids and seen_urls HashSet from existing records,
    /// skipping "failed" records to allow retry.
    fn should_skip(
        existing: &[DownloadRecord],
        video_id: &str,
        video_url: &str,
    ) -> bool {
        let seen_ids: std::collections::HashSet<String> = existing
            .iter()
            .filter(|r| r.status != "failed")
            .filter(|r| !r.video_id.is_empty())
            .map(|r| r.video_id.clone())
            .collect();
        let seen_urls: std::collections::HashSet<String> = existing
            .iter()
            .filter(|r| r.status != "failed")
            .map(|r| r.video_url.clone())
            .collect();

        if !video_id.is_empty() {
            seen_ids.contains(video_id)
        } else {
            seen_urls.contains(video_url)
        }
    }

    #[test]
    fn test_dedup_skips_existing_completed_video_id() {
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "completed"),
        ];
        assert!(should_skip(&existing, "abc", "https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn test_dedup_skips_existing_completed_video_url_fallback() {
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "", "completed"),
        ];
        assert!(should_skip(&existing, "", "https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn test_dedup_allows_retry_for_failed_record() {
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "failed"),
        ];
        // Failed records are excluded from seen_ids/seen_urls, so this should return false
        assert!(!should_skip(&existing, "abc", "https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn test_dedup_allows_new_video() {
        let existing = vec![
            make_record("sub-1", "Video A", "https://youtube.com/watch?v=abc", "abc", "completed"),
        ];
        // Different video_id
        assert!(!should_skip(&existing, "xyz", "https://youtube.com/watch?v=xyz"));
    }

    #[test]
    fn test_dedup_skips_cancelled_record() {
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "cancelled"),
        ];
        // Cancelled records are in seen_ids (not "failed"), so should be skipped
        assert!(should_skip(&existing, "abc", "https://youtube.com/watch?v=abc"));
    }

    // ── get_channel_videos tests (T038) ─────────────────────────────

    /// 计算 yt-dlp --playlist-start 和 --playlist-end 的辅助函数
    fn calc_playlist_range(page: u32, page_size: u32) -> (u32, u32) {
        let start = (page.saturating_sub(1)) * page_size + 1;
        // 多取一条用于判断 has_more
        let end = page * page_size + 1;
        (start, end)
    }

    /// 根据实际取到的条目数和请求的 endpoint 判断 has_more
    fn determine_has_more(fetched_count: usize, page_size: u32) -> bool {
        fetched_count > page_size as usize
    }

    #[test]
    fn test_pagination_calc_page1_size10() {
        let (start, end) = calc_playlist_range(1, 10);
        assert_eq!(start, 1);
        assert_eq!(end, 11); // page*size+1 = 10+1
    }

    #[test]
    fn test_pagination_calc_page2_size10() {
        let (start, end) = calc_playlist_range(2, 10);
        assert_eq!(start, 11);
        assert_eq!(end, 21);
    }

    #[test]
    fn test_pagination_calc_page3_size10() {
        let (start, end) = calc_playlist_range(3, 10);
        assert_eq!(start, 21);
        assert_eq!(end, 31);
    }

    #[test]
    fn test_pagination_calc_page1_size5() {
        let (start, end) = calc_playlist_range(1, 5);
        assert_eq!(start, 1);
        assert_eq!(end, 6);
    }

    #[test]
    fn test_pagination_calc_page1_size1() {
        let (start, end) = calc_playlist_range(1, 1);
        assert_eq!(start, 1);
        assert_eq!(end, 2);
    }

    #[test]
    fn test_has_more_true_when_extra_video_returned() {
        // 请求了 page_size=10, end=11，实际返回 11 条 → has_more = true
        assert!(determine_has_more(11, 10));
    }

    #[test]
    fn test_has_more_false_when_exact_or_less() {
        // 返回 10 条或更少 → has_more = false
        assert!(!determine_has_more(10, 10));
        assert!(!determine_has_more(5, 10));
        assert!(!determine_has_more(0, 10));
    }

    #[test]
    fn test_video_info_parsing_from_ytdlp_json() {
        use crate::models::VideoInfo;

        // yt-dlp --flat-playlist --dump-json 的输出行格式
        let json = r#"{"id":"dQw4w9WgXcQ","title":"Test Video","url":"https://youtube.com/watch?v=dQw4w9WgXcQ","duration":"03:21","upload_date":"20250528","thumbnail":"https://example.com/thumb.jpg"}"#;

        let video: VideoInfo = serde_json::from_str(json)
            .expect("should parse yt-dlp flat-playlist JSON");

        assert_eq!(video.id, "dQw4w9WgXcQ");
        assert_eq!(video.title, "Test Video");
        assert_eq!(video.url, "https://youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(video.duration, Some(201.0));
        assert_eq!(video.upload_date, Some("20250528".to_string()));
        assert_eq!(video.thumbnail, Some("https://example.com/thumb.jpg".to_string()));
    }

    #[test]
    fn test_video_info_parsing_minimal_fields() {
        use crate::models::VideoInfo;

        // 最少字段（只有 id, title, url）
        let json = r#"{"id":"abc123","title":"Minimal Video","url":"https://example.com/watch?v=abc123"}"#;

        let video: VideoInfo = serde_json::from_str(json)
            .expect("should parse minimal JSON");

        assert_eq!(video.id, "abc123");
        assert_eq!(video.title, "Minimal Video");
        assert_eq!(video.url, "https://example.com/watch?v=abc123");
        assert!(video.duration.is_none());
        assert!(video.upload_date.is_none());
        assert!(video.thumbnail.is_none());
    }

    #[test]
    fn test_video_list_result_construction_empty_channel() {
        use crate::models::{VideoInfo, VideoListResult};

        let result = VideoListResult {
            videos: vec![],
            total: 0,
            page: 1,
            page_size: 10,
            has_more: false,
        };

        assert!(result.videos.is_empty());
        assert_eq!(result.total, 0);
        assert_eq!(result.page, 1);
        assert_eq!(result.page_size, 10);
        assert!(!result.has_more);
    }

    #[test]
    fn test_video_list_result_construction_with_videos() {
        use crate::models::{VideoInfo, VideoListResult};

        let result = VideoListResult {
            videos: vec![
                VideoInfo {
                    id: "v1".to_string(),
                    title: "Video 1".to_string(),
                    url: "https://example.com/v1".to_string(),
                    duration: Some(600.0),
                    upload_date: Some("20250601".to_string()),
                    epoch: None,
                    thumbnail: None,
                },
            ],
            total: 50,
            page: 1,
            page_size: 10,
            has_more: true,
        };

        assert_eq!(result.videos.len(), 1);
        assert_eq!(result.total, 50);
        assert_eq!(result.page, 1);
        assert!(result.has_more);
    }

    #[test]
    fn test_video_list_result_last_page_no_more() {
        use crate::models::{VideoInfo, VideoListResult};

        let result = VideoListResult {
            videos: vec![
                VideoInfo {
                    id: "v50".to_string(),
                    title: "Last Video".to_string(),
                    url: "https://example.com/last".to_string(),
                    duration: Some(300.0),
                    upload_date: Some("20250610".to_string()),
                    epoch: None,
                    thumbnail: None,
                },
            ],
            total: 50,
            page: 5,
            page_size: 10,
            has_more: false,
        };

        assert_eq!(result.videos.len(), 1);
        assert!(!result.has_more);
        assert_eq!(result.page, 5);
    }
}
