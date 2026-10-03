use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::{Emitter, Manager, State};

use crate::models::{DownloadRecord, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::services::download_queue::notify_records_changed;
use crate::utils::AppError;
use crate::AppContext;
use crate::QueueContext;

/// 一轮检查的结果。用枚举而非 `&'static str`，是为了让「只有成功才推进游标」
/// 这个判断由编译器兜底 —— 写成魔法字符串的话，拼错会静默反转行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckStatus {
    Success,
    Failed,
}

impl CheckStatus {
    /// 持久化到 `Subscription::last_check_status` 的取值。
    fn as_str(self) -> &'static str {
        match self {
            CheckStatus::Success => "success",
            CheckStatus::Failed => "failed",
        }
    }
}

/// 单个订阅一轮检查完成后需要落库的结果。
///
/// 只携带订阅 id 与要更新的字段，不携带订阅快照 —— 收尾时按 id 在事务内增量应用，
/// 因此不会覆盖并发写者（健康检查、批量导入、单个订阅命令）对同一订阅其它字段的修改。
pub(crate) struct SubCheckOutcome {
    pub sub_id: String,
    pub checked_at: String,
    pub status: CheckStatus,
    pub error: Option<String>,
}

/// 按 id 增量应用检查结果到 `subscriptions.json`。
///
/// 「边跑边收集、收尾一次写入」模式的关键一步：长耗时阶段不持任何 storage 锁，
/// 收尾时也只按 id 改字段，而不是用陈旧快照整表覆盖。
///
/// 时间字段的写回规则不同，不要合并：`last_checked_at` 每次尝试都写（供显示），
/// `last_successful_check_at`（日期游标）**只在成功时**推进 —— 失败时若也推进，
/// 失败窗口内上传的视频会被 `--dateafter` 永久排除。
pub(crate) fn apply_check_outcomes(
    data_dir: &Path,
    outcomes: &[SubCheckOutcome],
) -> Result<(), AppError> {
    StorageService::update_subscriptions(data_dir, |subs| {
        for outcome in outcomes {
            if let Some(sub) = subs.iter_mut().find(|s| s.id == outcome.sub_id) {
                sub.last_checked_at = Some(outcome.checked_at.clone());
                if outcome.status == CheckStatus::Success {
                    sub.last_successful_check_at = Some(outcome.checked_at.clone());
                }
                sub.last_check_status = Some(outcome.status.as_str().to_string());
                sub.last_check_error = outcome.error.clone();
            }
        }
        Ok(())
    })
}

/// 对所有非暂停订阅执行一轮检查并落库，返回本轮新产生的下载记录。
///
/// - 每轮自行从磁盘重读 `AppSettings`：yt-dlp 路径 / 代理 / Cookie / 下载目录的改动
///   下一轮即生效，调用方不需要（也不应该）传设置快照。
/// - 基础设施错误（快照读取、收尾写回）向上传播，由调用方决定处置：周期任务（scheduler）
///   与托盘吞掉并记日志，命令层（`check_all_subscriptions`）转成前端契约的 `String`。
///   单订阅失败仍只记日志并记入 `failed` outcome，不中断整轮。
/// - 不判断 `scheduler_paused`：暂停是「自动调度」的策略，托盘手动触发的检查不应被它拦下。
///
/// 由 `services::scheduler` 的自动检查、`services::tray` 的「检查全部」以及
/// 命令层 `check_all_subscriptions`（薄委托）共同调用。
pub(crate) async fn run_check_round(
    data_dir: &Path,
    app_handle: &tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, AppError> {
    let settings = StorageService::load_settings(data_dir);
    let yt_dlp_path = settings.yt_dlp_path.clone();
    let proxy = Some(settings.proxy_url.clone());
    let cookie_file = Some(settings.cookie_file.clone());
    let download_dir = PathBuf::from(&settings.download_dir);

    // 只读快照（长耗时阶段不持任何 storage 锁）
    let subs = StorageService::load_subscriptions(data_dir)?;
    let records = StorageService::load_download_records(data_dir)?;
    let data_dir_buf = data_dir.to_path_buf();

    log::info!(
        "check_round: {} subscriptions, cookie={}, proxy={}, ytdlp={}",
        subs.len(),
        cookie_file.as_deref().unwrap_or("none"),
        proxy.as_deref().unwrap_or("none"),
        yt_dlp_path,
    );

    let mut all_new: Vec<DownloadRecord> = Vec::new();
    let mut outcomes: Vec<SubCheckOutcome> = Vec::new();

    for sub in subs.iter().filter(|s| !s.paused) {
        let checked_at = Utc::now().to_rfc3339();
        match check_and_download(
            sub,
            &yt_dlp_path,
            &proxy,
            &cookie_file,
            &download_dir,
            &data_dir_buf,
            &records,
            app_handle,
        )
        .await
        {
            Ok(new_records) => {
                all_new.extend(new_records);
                outcomes.push(SubCheckOutcome {
                    sub_id: sub.id.clone(),
                    checked_at,
                    status: CheckStatus::Success,
                    error: None,
                });
            }
            Err(e) => {
                log::error!("check_round: error checking {}: {}", sub.channel_name, e);
                outcomes.push(SubCheckOutcome {
                    sub_id: sub.id.clone(),
                    checked_at,
                    status: CheckStatus::Failed,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    // 收尾：按 id 增量写回，不再整表覆盖（否则会 clobber
    // 并发写者对同一订阅其它字段的修改）
    if !outcomes.is_empty() {
        apply_check_outcomes(data_dir, &outcomes)?;
    }

    // 只更新 last_check_time
    StorageService::update_state(data_dir, |state| {
        state.last_check_time = Some(Utc::now().to_rfc3339());
        Ok(())
    })?;

    // 通知前端刷新
    let _ = app_handle.emit("scheduler-check-complete", ());

    Ok(all_new)
}

/// 把订阅的**检查游标**（`last_successful_check_at`，ISO 8601）换算成 yt-dlp
/// `--dateafter` 的下界（`YYYYMMDD`）。
///
/// 返回 `None` 表示**不设日期下界**，即该订阅从未成功检查过（含刚加入订阅）。
/// 这是有意为之：新订阅应当能抓到「加入之前就已上传」的视频（典型场景是订阅
/// 单个视频 URL）。因此下界必须取自**订阅自己的**游标，不能用全局的
/// `state.last_check_time`：全局游标会被其它订阅的检查推进，导致新订阅一加入
/// 就带着一个晚于目标视频的日期下界，从而永远抓不到它。
///
/// 也**不能**用 `last_checked_at`（每次尝试都写）—— 检查失败时它会早于真实进度，
/// 于是失败窗口内上传的视频会被静默跳过。游标只在成功时推进，见
/// [`apply_check_outcomes`]。
///
/// 无法解析的时间戳按「不设下界」处理（宁可多抓，不可漏抓）。
fn date_lower_bound(cursor: &Option<String>) -> Option<String> {
    let t = cursor.as_deref()?;
    // 前 10 个字符即 `YYYY-MM-DD`。应用自己写的格式是
    // `Utc::now().to_rfc3339()` → `2026-09-30T13:12:37.659045467+00:00`，
    // 尾部的 `+00:00` / `Z` 与时间部分对 `--dateafter` 都无意义，故只取日期部分。
    // 必须用 get 而不是切片：该字符串来自磁盘上的 subscriptions.json，
    // 短串或非 ASCII 内容用 &t[..10] 会直接 panic（旧实现在此处就会崩）。
    let date = t.get(..10)?;
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    Some(parsed.format("%Y%m%d").to_string())
}

/// Core logic for checking a single subscription for new videos and downloading them.
/// Used by both the command layer and the scheduler.
pub(crate) async fn check_and_download(
    sub: &Subscription,
    yt_dlp_path: &str,
    proxy: &Option<String>,
    cookie_file: &Option<String>,
    download_dir: &PathBuf,
    data_dir: &PathBuf,
    existing_records: &[DownloadRecord],
    app_handle: &tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, AppError> {
    // 日期下界取自本条订阅自己的游标（只在成功时推进）：从未成功过时为 None（不过滤日期）
    let since = date_lower_bound(&sub.last_successful_check_at);

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
        let mut record =
            DownloadRecord::new(sub.id.clone(), video.title.clone(), video.url.clone(), vid.clone());
        record.quality = quality.clone();

        // Save the record immediately so the frontend sees "downloading"
        StorageService::update_download_records(data_dir, |all_records| {
            all_records.push(record.clone());
            Ok(())
        })?;
        notify_records_changed(app_handle);

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
    // 只读快照：定位并克隆订阅，之后不再持有整表快照
    let sub = {
        let subs = StorageService::load_subscriptions(&state.data_dir)
            .map_err(|e| e.to_string())?;
        let sub = subs
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::NotFound(format!("Subscription not found: {}", id)))
            .map_err(|e| e.to_string())?
            .clone();
        if sub.paused {
            return Ok(Vec::new());
        }
        sub
    };

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

    let records =
        StorageService::load_download_records(&state.data_dir).map_err(|e| e.to_string())?;

    // 长耗时阶段：不持任何 storage 锁
    let new_records = check_and_download(
        &sub,
        &yt_dlp_path,
        &proxy,
        &cookie_file,
        &download_dir,
        &state.data_dir,
        &records,
        &app_handle,
    )
    .await
    .map_err(|e| e.to_string())?;

    // 收尾：按 id 增量写回。保持原语义 —— 上面失败时 `?` 已提前返回，
    // 既不写 last_check_status 也不写 last_check_time。
    apply_check_outcomes(
        &state.data_dir,
        &[SubCheckOutcome {
            sub_id: sub.id.clone(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            status: CheckStatus::Success,
            error: None,
        }],
    )
    .map_err(|e| e.to_string())?;

    // 只更新 last_check_time
    StorageService::update_state(&state.data_dir, |app_state| {
        app_state.last_check_time = Some(chrono::Utc::now().to_rfc3339());
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    // 一次检查完成（与 run_check_round / check_all_subscriptions 同一语义：检查已完成、
    // last_check_time 已落盘）。语义是「一次检查完成」，不是「调度器轮次」。
    let _ = app_handle.emit("scheduler-check-complete", ());

    Ok(new_records)
}

/// Checks all non-paused subscriptions for new videos and downloads them.
///
/// 与后台自动调度（`services::scheduler`）、托盘「检查全部」（`services::tray`）共用
/// [`run_check_round`]（含设置来源与收尾逻辑），此处只负责把错误转成前端契约要求的
/// `String`。
///
/// 注意：设置来源由 `run_check_round` 从磁盘 `settings.json` 读取（而非
/// `AppContext.settings` 内存缓存）。这修掉了「外部修改 `settings.json` 后本命令仍看到旧值」
/// 的偏差；正常路径下两条写路径都是「先落盘、再更新缓存」，因此两者一致。
#[tauri::command]
pub async fn check_all_subscriptions(
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<DownloadRecord>, String> {
    run_check_round(&state.data_dir, &app_handle)
        .await
        .map_err(|e| e.to_string())
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

/// 按**订阅当前**的画质 preset 重下某条记录（升级与「重新下载」共用）。
///
/// 不区分「升级」与「重新下载」—— 二者行为完全一致，差别只在 UI 展示理由。
/// 后端据 `record_id` 反查记录 → 反查其订阅 → 取订阅**当前的** `quality_preset`
/// （前端不传画质，避免两侧各持一份真相）→ 走统一入队收口（含幂等守卫与 upsert 重置）。
#[tauri::command]
pub async fn redownload_video(
    record_id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<DownloadRecord, String> {
    // 只读定位：记录与它归属订阅的当前画质（长耗时阶段不持任何锁）
    let (record, quality) = {
        let records = StorageService::load_download_records(&state.data_dir)
            .map_err(|e| e.to_string())?;
        let record = records
            .iter()
            .find(|r| r.id == record_id)
            .ok_or_else(|| AppError::NotFound(format!("下载记录不存在: {}", record_id)))
            .map_err(|e| e.to_string())?
            .clone();

        let subs = StorageService::load_subscriptions(&state.data_dir)
            .map_err(|e| e.to_string())?;
        let sub = subs
            .iter()
            .find(|s| s.id == record.subscription_id)
            .ok_or_else(|| {
                AppError::NotFound(format!("订阅不存在: {}", record.subscription_id))
            })
            .map_err(|e| e.to_string())?;

        (record, sub.quality_preset.clone())
    };

    // 克隆下载参数并释放设置锁（enqueue 会做文件 I/O）
    let ctx = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        crate::services::download_queue::DownloadContext {
            yt_dlp_path: settings.yt_dlp_path.clone(),
            proxy: Some(settings.proxy_url.clone()),
            cookie_file: Some(settings.cookie_file.clone()),
            download_dir: std::path::PathBuf::from(&settings.download_dir),
            data_dir: state.data_dir.clone(),
        }
    };

    let guard = queue_ctx.queue.lock().map_err(|e| e.to_string())?;
    let Some(queue) = guard.as_ref() else {
        return Err("Download queue not initialized".to_string());
    };

    let updated = queue
        .enqueue_record(
            &state.data_dir,
            &record.subscription_id,
            &record.video_id,
            &record.video_url,
            &record.video_title,
            &quality,
        )
        .map_err(|e| e.to_string())?;

    // 确保队列驱动在运行（与检查路径同一约定）
    queue.start_processing(ctx);

    Ok(updated)
}

/// Recovers download state on application restart.
/// Marks "downloading", "paused" and "retrying" records as "failed" since the download
/// process was terminated when the application exited.
///
/// 置成 `failed`（而非保留 `retrying`）后，下次检查因 `seen_ids` 排除 `failed`
/// 会自动重新入队，并由入队的 upsert 复用同一条记录 —— 不需要任何新机制。
pub fn recover_state(data_dir: &PathBuf) -> Result<(), AppError> {
    let recovered = StorageService::update_download_records(data_dir, |records| {
        let mut recovered = 0usize;
        for record in records.iter_mut() {
            if record.status == "downloading"
                || record.status == "paused"
                || record.status == "retrying"
            {
                record.status = "failed".to_string();
                record.error_message = Some("Application restarted".to_string());
                recovered += 1;
            }
        }
        Ok(recovered)
    })?;

    if recovered > 0 {
        log::info!(
            "Recovered {} download records to failed state after restart",
            recovered
        );
    }
    Ok(())
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
        StorageService::seed_download_records(tmp.path(), &records)
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
        StorageService::seed_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered[0].status, "failed");
        assert_eq!(recovered[0].error_message, Some("Application restarted".to_string()));
    }

    #[test]
    fn test_recover_state_marks_retrying_as_failed() {
        // 应用退出时正在退避等待的记录：必须置 failed（不主动恢复重试）。
        // 置 failed 后下次检查会自动重新入队并由 upsert 复用同一条记录。
        let tmp = TempDir::new().expect("failed to create temp dir");
        let records = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=r", "vid-r", "retrying"),
        ];
        StorageService::seed_download_records(tmp.path(), &records)
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
        StorageService::seed_download_records(tmp.path(), &records)
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
        StorageService::seed_download_records(tmp.path(), &records)
            .expect("save should succeed");

        recover_state(&tmp.path().to_path_buf()).expect("recover should succeed");

        let recovered = StorageService::load_download_records(tmp.path())
            .expect("load should succeed");
        assert_eq!(recovered[0].status, "completed");
    }

    // ── 日期下界换算 + 游标推进：取自订阅自己的 last_successful_check_at ──

    /// 造一条带指定游标的订阅，用于 apply_check_outcomes 的落库断言。
    fn make_sub_with_cursor(cursor: Option<&str>) -> Subscription {
        let mut sub = Subscription::new(
            "https://youtube.com/@test".to_string(),
            "youtube".to_string(),
            "Test Channel".to_string(),
            String::new(),
        );
        sub.last_successful_check_at = cursor.map(str::to_string);
        sub
    }

    #[test]
    fn test_check_status_as_str_matches_persisted_values() {
        // 这两个字面量是 subscriptions.json 的对外取值，前端据此显示成功/失败
        assert_eq!(CheckStatus::Success.as_str(), "success");
        assert_eq!(CheckStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn test_apply_check_outcomes_success_advances_cursor() {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let sub = make_sub_with_cursor(None);
        StorageService::seed_subscriptions(tmp.path(), std::slice::from_ref(&sub))
            .expect("seed should succeed");

        apply_check_outcomes(
            tmp.path(),
            &[SubCheckOutcome {
                sub_id: sub.id.clone(),
                checked_at: "2026-09-30T13:12:37.659045467+00:00".to_string(),
                status: CheckStatus::Success,
                error: None,
            }],
        )
        .expect("apply should succeed");

        let applied = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed")
            .remove(0);
        assert_eq!(
            applied.last_checked_at.as_deref(),
            Some("2026-09-30T13:12:37.659045467+00:00")
        );
        assert_eq!(
            applied.last_successful_check_at.as_deref(),
            Some("2026-09-30T13:12:37.659045467+00:00")
        );
        assert_eq!(applied.last_check_status.as_deref(), Some("success"));
        assert_eq!(applied.last_check_error, None);
    }

    #[test]
    fn test_apply_check_outcomes_failure_keeps_cursor() {
        // 失败只更新「尝试」时间与状态。游标必须留在上次成功的位置，否则
        // 失败窗口内上传的视频会被 --dateafter 永久排除，且没有任何记录能反映这次遗漏。
        let tmp = TempDir::new().expect("failed to create temp dir");
        let sub = make_sub_with_cursor(Some("2026-09-01T00:00:00+00:00"));
        StorageService::seed_subscriptions(tmp.path(), std::slice::from_ref(&sub))
            .expect("seed should succeed");

        apply_check_outcomes(
            tmp.path(),
            &[SubCheckOutcome {
                sub_id: sub.id.clone(),
                checked_at: "2026-09-30T13:12:37.659045467+00:00".to_string(),
                status: CheckStatus::Failed,
                error: Some("boom".to_string()),
            }],
        )
        .expect("apply should succeed");

        let applied = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed")
            .remove(0);
        assert_eq!(
            applied.last_checked_at.as_deref(),
            Some("2026-09-30T13:12:37.659045467+00:00")
        );
        assert_eq!(
            applied.last_successful_check_at.as_deref(),
            Some("2026-09-01T00:00:00+00:00")
        );
        assert_eq!(applied.last_check_status.as_deref(), Some("failed"));
        assert_eq!(applied.last_check_error.as_deref(), Some("boom"));
    }

    #[test]
    fn test_apply_check_outcomes_failure_on_never_succeeded_sub_keeps_cursor_none() {
        // 从未成功过的订阅连续失败：游标保持 None，下次检查仍不设下界
        let tmp = TempDir::new().expect("failed to create temp dir");
        let sub = make_sub_with_cursor(None);
        StorageService::seed_subscriptions(tmp.path(), std::slice::from_ref(&sub))
            .expect("seed should succeed");

        apply_check_outcomes(
            tmp.path(),
            &[SubCheckOutcome {
                sub_id: sub.id.clone(),
                checked_at: "2026-09-30T13:12:37.659045467+00:00".to_string(),
                status: CheckStatus::Failed,
                error: Some("boom".to_string()),
            }],
        )
        .expect("apply should succeed");

        let applied = StorageService::load_subscriptions(tmp.path())
            .expect("load should succeed")
            .remove(0);
        assert_eq!(applied.last_successful_check_at, None);
        assert_eq!(date_lower_bound(&applied.last_successful_check_at), None);
    }

    #[test]
    fn test_date_lower_bound_none_means_no_filter() {
        // 没有游标就不设下界 —— 这正是「订阅单个视频 URL」能抓到该视频的前提。
        // 若改用全局 state.last_check_time，新订阅一加入就会带上下界，永远抓不到。
        assert_eq!(date_lower_bound(&None), None);
    }

    #[test]
    fn test_date_lower_bound_parses_app_rfc3339_format() {
        // 应用实际写入的格式：Utc::now().to_rfc3339()，尾部是 +00:00 而非 Z
        assert_eq!(
            date_lower_bound(&Some("2026-09-30T13:12:37.659045467+00:00".to_string())),
            Some("20260930".to_string())
        );
    }

    #[test]
    fn test_date_lower_bound_parses_zulu_and_date_only() {
        assert_eq!(
            date_lower_bound(&Some("2026-06-02T12:00:00Z".to_string())),
            Some("20260602".to_string())
        );
        assert_eq!(
            date_lower_bound(&Some("2026-06-02".to_string())),
            Some("20260602".to_string())
        );
    }

    #[test]
    fn test_date_lower_bound_invalid_falls_back_to_no_filter() {
        // 宁可多抓，不可漏抓：解析不了就不设下界
        assert_eq!(date_lower_bound(&Some("not-a-timestamp".to_string())), None);
        assert_eq!(date_lower_bound(&Some("2026-13-45T00:00:00Z".to_string())), None);
    }

    #[test]
    fn test_date_lower_bound_never_panics_on_short_or_non_ascii_input() {
        // last_checked_at 来自磁盘，可能是被手改过的脏数据。
        // 旧实现用 &t[..10] 切字节，这两种输入都会 panic。
        assert_eq!(date_lower_bound(&Some("abc".to_string())), None);
        assert_eq!(date_lower_bound(&Some("2026年09月30日".to_string())), None);
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

    #[test]
    fn test_dedup_skips_deleted_record_no_auto_redownload() {
        // 「已删除」的记录**留在** seen 集合内 → 检查不会自动重下（有意设计：
        // 否则用户删文件腾空间会被检查无限填满）。
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "deleted"),
        ];
        assert!(should_skip(&existing, "abc", "https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn test_dedup_skips_missing_file_record_no_auto_redownload() {
        // 「文件缺失」不落库（记录状态仍是 completed），因此它天然留在 seen 集合内，
        // 检查不会自动重下 —— 重下只由用户在 UI 手动触发。
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "completed"),
        ];
        assert!(should_skip(&existing, "abc", "https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn test_dedup_skips_retrying_record() {
        // 「重试中」也必须留在 seen 集合内：否则一次检查会给正在重试的视频
        // 再排一个下载任务，与就地重试的并发语义冲突。
        let existing = vec![
            make_record("sub-1", "Video", "https://youtube.com/watch?v=abc", "abc", "retrying"),
        ];
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
        use crate::models::VideoListResult;

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
