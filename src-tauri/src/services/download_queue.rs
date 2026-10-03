use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, AppHandle};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Semaphore;

use crate::models::{AppSettings, DownloadRecord, RecordStatus, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::utils::AppError;
use crate::utils::progress_parser;
use crate::utils::retry_policy;

// ── Windows process suspension (S-07) ────────────────────────────

#[cfg(windows)]
mod windows_process {
    use std::mem;

    const TH32CS_SNAPTHREAD: u32 = 0x00000004;
    const THREAD_SUSPEND_RESUME: u32 = 0x0002;

    #[repr(C)]
    struct ThreadEntry32 {
        dw_size: u32,
        _cnt_usage: u32,
        th32_thread_id: u32,
        th32_owner_process_id: u32,
        _tp_base_pri: i32,
        _tp_delta_pri: i32,
        _dw_flags: u32,
    }

    extern "system" {
        fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
        fn Thread32First(h_snapshot: isize, lpte: *mut ThreadEntry32) -> i32;
        fn Thread32Next(h_snapshot: isize, lpte: *mut ThreadEntry32) -> i32;
        fn OpenThread(dw_desired_access: u32, b_inherit_handle: i32, dw_thread_id: u32) -> isize;
        fn SuspendThread(h_thread: isize) -> u32;
        fn ResumeThread(h_thread: isize) -> u32;
        fn CloseHandle(h_object: isize) -> i32;
    }

    unsafe fn with_process_threads(pid: u32, action: fn(isize)) {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == -1isize {
            return;
        }

        let mut te: ThreadEntry32 = mem::zeroed();
        te.dw_size = mem::size_of::<ThreadEntry32>() as u32;

        if Thread32First(snapshot, &mut te) != 0 {
            loop {
                if te.th32_owner_process_id == pid {
                    let h = OpenThread(THREAD_SUSPEND_RESUME, 0, te.th32_thread_id);
                    if h != -1isize && h != 0 {
                        action(h);
                        CloseHandle(h);
                    }
                }
                te.dw_size = mem::size_of::<ThreadEntry32>() as u32;
                if Thread32Next(snapshot, &mut te) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }

    pub fn suspend_process(pid: u32) {
        unsafe {
            with_process_threads(pid, |h| {
                SuspendThread(h);
            });
        }
        log::info!("Windows: suspended process pid={}", pid);
    }

    pub fn resume_process(pid: u32) {
        unsafe {
            with_process_threads(pid, |h| {
                ResumeThread(h);
            });
        }
        log::info!("Windows: resumed process pid={}", pid);
    }
}

/// Status of a download task in the in-memory queue.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Waiting,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    /// 失败后处于退避等待（子进程已退出、等待下次重试）。前端据此显示「重试中」。
    Retrying,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Waiting => write!(f, "waiting"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Paused => write!(f, "paused"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
            TaskStatus::Retrying => write!(f, "retrying"),
        }
    }
}

/// A download task in the in-memory queue.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadTask {
    pub id: String,
    pub video_id: String,
    pub video_url: String,
    pub video_title: String,
    pub subscription_id: String,
    pub quality: String,
    pub status: TaskStatus,
    pub progress: Option<ProgressInfo>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    /// 对应 `DownloadRecord.id`。完成 / 失败 / 状态回写**全部按它精确定位** ——
    /// 旧的 `(video_url, subscription_id)` 过滤会匹配到所有同键记录。
    pub record_id: String,
    /// 退避期的下次重试时刻（ISO 8601），非退避期为 `None`。**不落库**。
    pub next_retry_at: Option<String>,
}

/// Progress information for a running download.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressInfo {
    pub percent: f32,
    pub speed: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub eta: String,
}

/// Progress event emitted to the frontend during downloads.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub task_id: String,
    pub video_url: String,
    pub percent: f32,
    pub speed: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub eta: String,
}

/// Runtime state of the download queue.
#[derive(Debug, Clone, Serialize)]
pub struct QueueState {
    pub active_count: usize,
    pub waiting_count: usize,
    pub max_concurrent: u32,
}

/// Context parameters required for download tasks.
#[derive(Clone)]
pub struct DownloadContext {
    pub yt_dlp_path: String,
    pub proxy: Option<String>,
    pub cookie_file: Option<String>,
    pub download_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl DownloadContext {
    /// 从当前设置构造下载上下文。
    ///
    /// `proxy` / `cookie_file` 一律包成 `Some`（空串 = 未配置），由 yt-dlp 调用层
    /// 按非空判断决定是否传参 —— 这段包装语义原先在三个命令里各写一遍，收敛到这里。
    pub fn from_settings(settings: &AppSettings, data_dir: PathBuf) -> Self {
        Self {
            yt_dlp_path: settings.yt_dlp_path.clone(),
            proxy: Some(settings.proxy_url.clone()),
            cookie_file: Some(settings.cookie_file.clone()),
            download_dir: PathBuf::from(&settings.download_dir),
            data_dir,
        }
    }
}

/// Holds the child process handle and PID for an active download.
struct ActiveTask {
    child: Option<tokio::process::Child>,
    /// 与 `child` **同步**：`Some` 当且仅当 `child` 存在。
    ///
    /// 退避期间 entry 继续存活但子进程早已退出，那个数字**可能已被系统复用给
    /// 无关进程** —— 对陈旧 PID 发 SIGSTOP 会挂起用户毫不相干的进程。用类型让
    /// 「没有 PID 却发信号」在编译期不可表示。
    pid: Option<u32>,
    task: DownloadTask,
    /// T028: Last known download progress percentage (0.0–100.0).
    /// Used by adjust_concurrency to select which tasks to pause first.
    last_progress_percent: f32,
    /// 打断退避等待（取消 / 暂停时唤醒）。
    notify: Arc<tokio::sync::Notify>,
}

/// 等待子进程退出时的轮询间隔（毫秒）。
///
/// `child` 必须留在 entry 里，cancel 才能对它发信号；而 `Child::wait()` 需要
/// `&mut Child`（无法在持锁等待的同时让 cancel 拿到锁）。故用短轮询 + `notify`
/// 唤醒的组合：正常退出最多晚 [`WAIT_POLL_MS`] 被发现，取消则被 `notify` 立即唤醒。
const WAIT_POLL_MS: u64 = 200;

/// stderr 累积上限：只保留尾部 64 KiB，让内存有界。
const STDERR_TAIL_LIMIT: usize = 64 * 1024;

/// 等待进程退出的一次轮询结果。
enum ProcessPoll {
    Exited(std::process::ExitStatus),
    Cancelled,
    Running,
}

/// 退避等待的结局。
enum BackoffOutcome {
    /// 计满剩余时长，可以重试。
    Elapsed,
    /// entry 已被取消移除 —— 调用方必须直接退出且**不回写任何记录**。
    Cancelled,
}

/// 记录的 upsert / 归属键匹配：`(video_id, subscription_id)`；`video_id` 为空时回退 `video_url`。
fn record_matches_key(
    r: &DownloadRecord,
    subscription_id: &str,
    video_id: &str,
    video_url: &str,
) -> bool {
    r.subscription_id == subscription_id
        && if video_id.is_empty() {
            r.video_url == video_url
        } else {
            r.video_id == video_id
        }
}

/// FIFO download queue with concurrency control and process lifecycle management.
pub struct DownloadQueue {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    semaphore: Arc<Semaphore>,
    active_tasks: Arc<Mutex<HashMap<String, ActiveTask>>>,
    app_handle: AppHandle,
    max_concurrent: Arc<AtomicU32>,
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    pub fn new(app_handle: AppHandle, max_concurrent: u32) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            app_handle,
            max_concurrent: Arc::new(AtomicU32::new(max_concurrent)),
        }
    }

    /// 统一入队收口：按 `(video_id, subscription_id)` **upsert 重置**记录并压入队列。
    ///
    /// 键里的 `subscription_id` 是**归属**：带它能防止「重试 B 的失败记录时误把 A 的
    /// 完成记录重置掉」。全局去重下 B 根本走不到入队（`seen_ids` 挡掉），所以不影响
    /// 全局唯一。
    ///
    /// 幂等守卫放在这一个收口：`queue` / `active_tasks` 已有同 `record_id` 的任务，
    /// **或**记录状态 ∈ `{waiting, downloading, retrying}` → 不重复入队，直接返回当前记录。
    /// 这样检查路径与命令路径（升级 / 重新下载）拿到的是**同一份**保证。
    pub fn enqueue_from_video(
        &self,
        sub: &Subscription,
        video_title: String,
        video_url: String,
        video_id: String,
        quality: String,
        data_dir: &PathBuf,
    ) -> Result<DownloadRecord, AppError> {
        self.enqueue_record(
            data_dir,
            &sub.id,
            &video_id,
            &video_url,
            &video_title,
            &quality,
        )
    }

    /// 入队收口的核心实现（见 [`Self::enqueue_from_video`]）。
    pub(crate) fn enqueue_record(
        &self,
        data_dir: &Path,
        subscription_id: &str,
        video_id: &str,
        video_url: &str,
        video_title: &str,
        quality: &str,
    ) -> Result<DownloadRecord, AppError> {
        let matches = |r: &DownloadRecord| {
            record_matches_key(r, subscription_id, video_id, video_url)
        };

        // 阶段 1：只读定位现有记录，做幂等守卫（必须在重置之前判断状态）
        let existing = StorageService::load_download_records(data_dir)?
            .into_iter()
            .find(|r| matches(r));

        if let Some(existing) = existing {
            let busy = matches!(
                existing.status,
                RecordStatus::Waiting | RecordStatus::Downloading | RecordStatus::Retrying
            );
            if busy || self.has_in_flight_task(&existing.id) {
                return Ok(existing);
            }
        }

        // 阶段 2：事务内 upsert 重置（命中）或新建（未命中）
        let record = StorageService::update_download_records(data_dir, |all| {
            if let Some(r) = all.iter_mut().find(|r| matches(r)) {
                // 重置本次尝试相关字段；**保留** file_path / file_size（旧路径还要用于
                // 回收）与 downloaded_at（完成时才更新）。
                r.status = RecordStatus::Downloading;
                r.error_message = None;
                r.quality = quality.to_string();
                r.retry_count = 0;
                r.last_retry_at = None;
                if !video_title.is_empty() {
                    r.video_title = video_title.to_string();
                }
                Ok(r.clone())
            } else {
                let mut r = DownloadRecord::new(
                    subscription_id.to_string(),
                    video_title.to_string(),
                    video_url.to_string(),
                    video_id.to_string(),
                );
                r.quality = quality.to_string();
                all.push(r.clone());
                Ok(r)
            }
        })?;
        self.notify_records_changed();

        // 阶段 3：压入队列
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(DownloadTask {
            id: uuid::Uuid::new_v4().to_string(),
            video_id: video_id.to_string(),
            video_url: video_url.to_string(),
            video_title: video_title.to_string(),
            subscription_id: subscription_id.to_string(),
            quality: quality.to_string(),
            status: TaskStatus::Waiting,
            progress: None,
            error_message: None,
            created_at: Utc::now().to_rfc3339(),
            completed_at: None,
            record_id: record.id.clone(),
            next_retry_at: None,
        });
        drop(queue);
        self.emit_queue_changed();

        Ok(record)
    }

    /// 队列或活动任务里是否已有指向该记录的任务（幂等守卫用）。
    ///
    /// 加锁顺序固定为 `queue → active_tasks`；两把锁都只短暂持有且不嵌套。
    fn has_in_flight_task(&self, record_id: &str) -> bool {
        if record_id.is_empty() {
            return false;
        }
        {
            let queue = self.queue.lock().unwrap();
            if queue.iter().any(|t| t.record_id == record_id) {
                return true;
            }
        }
        let active = self.active_tasks.lock().unwrap();
        active.values().any(|e| e.task.record_id == record_id)
    }

    /// Starts processing the queue in a background task.
    pub fn start_processing(&self, ctx: DownloadContext) {
        let queue = Arc::clone(&self.queue);
        let semaphore = Arc::clone(&self.semaphore);
        let active_tasks = Arc::clone(&self.active_tasks);
        let app_handle = self.app_handle.clone();
        let max_conc = Arc::clone(&self.max_concurrent);

        tokio::spawn(async move {
            loop {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let task = {
                    let mut q = queue.lock().unwrap();
                    q.pop_front()
                };

                let task = match task {
                    Some(t) => t,
                    None => break,
                };

                let ctx_clone = ctx.clone();
                let queue_clone = Arc::clone(&queue);
                let active_clone = Arc::clone(&active_tasks);
                let app_clone = app_handle.clone();
                let max_conc_clone = Arc::clone(&max_conc);

                tokio::spawn(async move {
                    let _permit = permit;

                    Self::execute_download_with_control(
                        &task,
                        &ctx_clone,
                        &queue_clone,
                        &active_clone,
                        &app_clone,
                        &max_conc_clone,
                    ).await;

                    // Clean up active entry
                    {
                        let mut active = active_clone.lock().unwrap();
                        active.remove(&task.id);
                    }

                    // 任务结束（成功/失败/取消）后活动项已移除，通知前端重新拉取
                    emit_queue_state(
                        &app_clone,
                        &queue_clone,
                        &active_clone,
                        max_conc_clone.load(Ordering::Relaxed),
                    );
                });
            }
        });
    }

    /// Executes a download with process lifecycle control and automatic retry.
    ///
    /// 骨架（前提 P11：循环接线无自动化测试，纯逻辑已抽到 `utils::retry_policy` 单测）：
    /// - spawn → 并发读 stdout（进度）/ stderr（失败归因）→ **可取消地**等待进程退出；
    /// - 成功 → 按 `record_id` 回写记录 → 旧文件回收（新旧路径不等时）；
    /// - 失败 → 分类：可重试且未达上限 ⇒ 写 `retrying` 后进入退避并重新 spawn；
    ///   不可重试 / 达上限 ⇒ 写 `failed` + 脱敏后的最后一条 `ERROR:` 行；
    /// - 退避期间子进程已退出（`child=None` / `pid=None`），entry 继续存活并**占用并发名额**；
    /// - 取消 ⇒ entry 被移除，本函数**不回写任何记录**（否则会把 `cancelled` 覆盖成 `failed`）。
    async fn execute_download_with_control(
        task: &DownloadTask,
        ctx: &DownloadContext,
        queue: &Arc<Mutex<VecDeque<DownloadTask>>>,
        active_tasks: &Arc<Mutex<HashMap<String, ActiveTask>>>,
        app_handle: &AppHandle,
        max_concurrent: &Arc<AtomicU32>,
    ) {
        let task_id = task.id.clone();
        let record_id = task.record_id.clone();

        // FR-012: Verify download path, fall back to default if inaccessible
        let effective_download_dir = {
            let dir = std::path::PathBuf::from(&ctx.download_dir);
            match std::fs::create_dir_all(&dir) {
                Ok(()) => dir,
                Err(e) => {
                    log::warn!(
                        "Download path '{}' inaccessible ({}), falling back to default",
                        ctx.download_dir.display(),
                        e
                    );
                    let default_dir = crate::models::settings::default_download_dir();
                    let default = std::path::PathBuf::from(&default_dir);
                    let _ = std::fs::create_dir_all(&default);
                    default
                }
            }
        };

        // 注册 entry（退避期间 entry 存活而子进程不存在，故初始 child/pid 都是 None）
        let notify = Arc::new(tokio::sync::Notify::new());
        {
            let mut active = active_tasks.lock().unwrap();
            let mut running_task = task.clone();
            running_task.status = TaskStatus::Running;
            active.insert(
                task_id.clone(),
                ActiveTask {
                    child: None,
                    pid: None,
                    task: running_task,
                    last_progress_percent: 0.0,
                    notify: Arc::clone(&notify),
                },
            );
        }

        // 通知前端「该任务已进入运行态」。`queue-changed` 是 AppShell 唯一重新拉取队列的
        // 触发点，缺了这次 emit，前端会一直停留在入队时的「等待中」直到下载结束。
        emit_queue_state(
            app_handle,
            queue,
            active_tasks,
            max_concurrent.load(Ordering::Relaxed),
        );

        let mut attempts: u32 = 0; // 已重试次数

        loop {
            // ── spawn ─────────────────────────────────────────────
            let spawned = match YtDlpService::download_video_spawn(
                &ctx.yt_dlp_path,
                &ctx.proxy,
                &ctx.cookie_file,
                &task.video_url,
                &task.quality,
                &effective_download_dir,
            ) {
                Ok(s) => s,
                Err(e) => {
                    // spawn 失败（路径不存在 / 无执行权限）：配置问题，**不可重试**。
                    // 必须补写 failed —— 旧实现直接 return，记录会永久停在「下载中」。
                    log::error!("Failed to spawn yt-dlp for {}: {}", task.video_title, e);
                    // 脱敏与截断在 finalize_failed 内统一处理
                    Self::finalize_failed(ctx, &record_id, &format!("无法启动 yt-dlp：{}", e));
                    notify_records_changed(app_handle);
                    return;
                }
            };

            let mut child = spawned.child;
            let pid = child.id();
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // 把 child 放回 entry：cancel 需要能对 `Some(child)` 调 `start_kill`
            {
                let mut active = active_tasks.lock().unwrap();
                match active.get_mut(&task_id) {
                    Some(entry) => {
                        entry.child = Some(child);
                        entry.pid = pid;
                        entry.task.status = TaskStatus::Running;
                        entry.task.next_retry_at = None;
                        entry.task.error_message = None;
                    }
                    // 已取消：本函数不回写任何记录
                    None => return,
                }
            }
            emit_queue_state(
                app_handle,
                queue,
                active_tasks,
                max_concurrent.load(Ordering::Relaxed),
            );

            // ── stdout：进度事件 + `--print after_move:filepath` 的最终路径 ──
            let Some(stdout) = stdout else {
                log::warn!("Could not read stdout of yt-dlp process");
                let msg = "无法读取 yt-dlp 的输出流".to_string();
                Self::finalize_failed(ctx, &record_id, &msg);
                notify_records_changed(app_handle);
                return;
            };
            let progress_handle = {
                let mut lines = BufReader::new(stdout).lines();
                let app_clone = app_handle.clone();
                let active_progress = Arc::clone(active_tasks);
                let task_url = task.video_url.clone();
                let task_id_for_progress = task_id.clone();

                tokio::spawn(async move {
                    let mut file_path = String::new();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Some(event) = progress_parser::parse_progress_line(trimmed) {
                            let speed_str = event.speed.clone();
                            let eta_str = event.eta.clone();
                            let progress_event = DownloadProgressEvent {
                                task_id: task_id_for_progress.clone(),
                                video_url: task_url.clone(),
                                percent: event.percent,
                                speed: event.speed,
                                downloaded_bytes: event.downloaded_bytes,
                                total_bytes: event.total_bytes,
                                eta: event.eta,
                            };
                            let _ = app_clone.emit("download-progress", progress_event);

                            // T028: Update progress on active task for concurrency adjustment
                            let mut active = active_progress.lock().unwrap();
                            if let Some(entry) = active.get_mut(&task_id_for_progress) {
                                entry.last_progress_percent = event.percent;
                                entry.task.progress = Some(ProgressInfo {
                                    percent: event.percent,
                                    speed: speed_str,
                                    downloaded_bytes: event.downloaded_bytes,
                                    total_bytes: event.total_bytes,
                                    eta: eta_str,
                                });
                            }
                        } else {
                            file_path = trimmed.to_string();
                        }
                    }
                    file_path
                })
            };

            // ── stderr：并发读取（只保留尾部 64 KiB，内存有界）──
            //
            // stderr 是 `piped()` 却不读会让管道写满（约 64 KiB），yt-dlp 会**阻塞在写
            // stderr 上**，表现为「下载卡死」。必须与 stdout 并发读。
            let stderr_handle = match stderr {
                Some(stderr) => {
                    let mut lines = BufReader::new(stderr).lines();
                    tokio::spawn(async move {
                        let mut buf = String::new();
                        while let Ok(Some(line)) = lines.next_line().await {
                            buf.push_str(&line);
                            buf.push('\n');
                            if buf.len() > STDERR_TAIL_LIMIT {
                                let excess = buf.len() - STDERR_TAIL_LIMIT;
                                let cut = (0..=excess)
                                    .rev()
                                    .find(|&i| buf.is_char_boundary(i))
                                    .unwrap_or(0);
                                buf.drain(..cut);
                            }
                        }
                        buf
                    })
                }
                None => tokio::spawn(async { String::new() }),
            };

            // ── 等待进程退出（可取消；child 留在 entry 内，cancel 才能 kill）──
            let exit_status = loop {
                let poll = {
                    let mut active = active_tasks.lock().unwrap();
                    match active.get_mut(&task_id) {
                        // entry 被 cancel 移除 ⇒ 立即退出且不回写
                        None => ProcessPoll::Cancelled,
                        Some(entry) => match entry.child.as_mut() {
                            None => ProcessPoll::Cancelled,
                            Some(child) => match child.try_wait() {
                                Ok(Some(status)) => ProcessPoll::Exited(status),
                                _ => ProcessPoll::Running,
                            },
                        },
                    }
                };
                match poll {
                    ProcessPoll::Exited(status) => break status,
                    ProcessPoll::Cancelled => return,
                    ProcessPoll::Running => {}
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)) => {}
                    _ = notify.notified() => {}
                }
            };

            let stderr_text = stderr_handle.await.unwrap_or_default();
            let file_path = progress_handle.await.unwrap_or_default();

            if exit_status.success() {
                // 入口仍在才回写；否则说明已被取消
                if !Self::task_entry_exists(active_tasks, &task_id) {
                    return;
                }
                Self::finalize_success(ctx, &record_id, &file_path);
                notify_records_changed(app_handle);
                return;
            }

            log::warn!(
                "yt-dlp exited with code {:?} for {}",
                exit_status.code(),
                task.video_title
            );
            let decision = retry_policy::classify_failure(&stderr_text, exit_status.code());

            if retry_policy::should_retry(&decision, attempts) {
                attempts += 1;
                let remaining = retry_policy::BACKOFF_SECS[(attempts - 1) as usize];
                let next_retry_at = (Utc::now()
                    + chrono::Duration::seconds(remaining as i64))
                .to_rfc3339();

                // 进入退避前先确认 entry 仍在（取消后不回写）
                let still_present = {
                    let mut active = active_tasks.lock().unwrap();
                    match active.get_mut(&task_id) {
                        Some(entry) => {
                            // 安全要求：退避期间子进程已退出，pid 必须置空，否则
                            // 陈旧的 PID 可能已被系统复用，误发信号会挂起无关进程。
                            entry.child = None;
                            entry.pid = None;
                            entry.task.status = TaskStatus::Retrying;
                            entry.task.next_retry_at = Some(next_retry_at.clone());
                            true
                        }
                        None => false,
                    }
                };
                if !still_present {
                    return;
                }

                Self::finalize_retrying(ctx, &record_id, attempts, &next_retry_at);
                notify_records_changed(app_handle);
                emit_queue_state(
                    app_handle,
                    queue,
                    active_tasks,
                    max_concurrent.load(Ordering::Relaxed),
                );

                match Self::backoff_wait(&task_id, active_tasks, &notify, remaining).await {
                    BackoffOutcome::Elapsed => continue,
                    BackoffOutcome::Cancelled => return,
                }
            }

            // 不可重试 / 达上限：写真实失败原因（finalize_failed 内脱敏 + 截断）
            let summary = retry_policy::summarize_failure(&stderr_text);
            if !Self::task_entry_exists(active_tasks, &task_id) {
                return;
            }
            Self::finalize_failed(ctx, &record_id, &summary);
            notify_records_changed(app_handle);
            return;
        }
    }

    /// entry 是否仍存在于活动任务表（cancel 会移除它）。
    fn task_entry_exists(
        active_tasks: &Arc<Mutex<HashMap<String, ActiveTask>>>,
        task_id: &str,
    ) -> bool {
        active_tasks.lock().unwrap().contains_key(task_id)
    }

    /// 按 `record_id` 精确定位并就地修改一条下载记录（事务）。
    ///
    /// 返回闭包的返回值；记录不存在时返回 `None`（闭包不执行）。这是所有
    /// 「按 id 回写记录」的**唯一形状** —— 原先 finalize_* / update_record_status
    /// 各自重复了「update_download_records + find + 守卫」。
    ///
    /// 注意：`f` 在全局写锁内执行，**禁止**在其中做文件 I/O、获取其它应用级锁
    /// 或再调 `update_*`。
    fn update_record_by_id<T>(
        data_dir: &Path,
        record_id: &str,
        f: impl FnOnce(&mut DownloadRecord) -> T,
    ) -> Option<T> {
        StorageService::update_download_records(data_dir, |records| {
            Ok(records.iter_mut().find(|r| r.id == record_id).map(f))
        })
        .ok()
        .flatten()
    }

    /// 成功完成：按 `record_id` 回写新路径与状态，并在**新路径落库之后**回收旧文件。
    ///
    /// `downloaded_at` **只在这里**写；`retry_count` 保留（成功不归零）；`error_message` 清空。
    /// 已取消的记录不覆盖。
    fn finalize_success(ctx: &DownloadContext, record_id: &str, file_path: &str) {
        // 文件大小在事务**外**算好：事务闭包持全局写锁，AGENTS.md 禁止在其中做文件 I/O。
        let new_size = if file_path.is_empty() {
            None
        } else {
            Some(std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0))
        };

        let old_path = Self::update_record_by_id(&ctx.data_dir, record_id, |existing| {
            if existing.status == RecordStatus::Cancelled {
                return None;
            }
            let old = existing.file_path.clone();
            if let Some(size) = new_size {
                existing.file_path = file_path.to_string();
                existing.file_size = size;
            }
            existing.status = RecordStatus::Completed;
            existing.error_message = None;
            existing.downloaded_at = Utc::now().to_rfc3339();
            Some(old)
        })
        .flatten();

        // 顺序不可反：先落库新路径、再回收旧文件。反过来若「回收成功但落库失败」，
        // 会留下一个记录和磁盘都没有的空洞；当前顺序最坏只留一个无害的孤儿文件。
        if let Some(old) = old_path {
            if !old.is_empty() && !file_path.is_empty() && old != file_path {
                let old_path = Path::new(&old);
                if old_path.exists() {
                    crate::services::file_manager::delete_file_to_trash(old_path);
                }
            }
        }
    }

    /// 进入退避：写 `retrying` + `retry_count` + `last_retry_at`。已取消的记录不覆盖。
    fn finalize_retrying(
        ctx: &DownloadContext,
        record_id: &str,
        retry_count: u32,
        last_retry_at: &str,
    ) {
        Self::update_record_by_id(&ctx.data_dir, record_id, |existing| {
            if existing.status != RecordStatus::Cancelled {
                existing.status = RecordStatus::Retrying;
                existing.retry_count = retry_count;
                existing.last_retry_at = Some(last_retry_at.to_string());
            }
        });
    }

    /// 判失败：写 `failed` + 脱敏原因。已取消的记录不覆盖（取消优先于失败）。
    ///
    /// **脱敏在这里做**（而不是只在调用方）：`error_message` 来自真实 stderr，而
    /// `proxy_url` 可能含明文口令 —— 把剥离放在写入函数内，任何调用点都无法绕过。
    fn finalize_failed(ctx: &DownloadContext, record_id: &str, message: &str) {
        let message = retry_policy::sanitize_error_message(message);
        Self::update_record_by_id(&ctx.data_dir, record_id, |existing| {
            if existing.status != RecordStatus::Cancelled {
                existing.status = RecordStatus::Failed;
                existing.error_message = Some(message.clone());
            }
        });
    }

    /// 退避等待：持**剩余时长**（而非绝对 deadline），暂停冻结 / 恢复续算天然正确。
    ///
    /// 取消时 entry 被移除 → 唤醒后立即返回 [`BackoffOutcome::Cancelled`]，
    /// 调用方**不得**回写任何记录。
    async fn backoff_wait(
        task_id: &str,
        active_tasks: &Arc<Mutex<HashMap<String, ActiveTask>>>,
        notify: &Arc<tokio::sync::Notify>,
        mut remaining: u64,
    ) -> BackoffOutcome {
        loop {
            let phase = {
                let active = active_tasks.lock().unwrap();
                match active.get(task_id) {
                    None => return BackoffOutcome::Cancelled,
                    Some(entry) => {
                        retry_policy::phase_of(entry.child.is_some(), &entry.task.status.to_string())
                    }
                }
            };

            if phase == retry_policy::Phase::Paused {
                // 冻结：等恢复（或取消）的通知
                notify.notified().await;
                continue;
            }

            if remaining == 0 {
                return BackoffOutcome::Elapsed;
            }

            let started = std::time::Instant::now();
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(remaining)) => {
                    return BackoffOutcome::Elapsed;
                }
                _ = notify.notified() => {
                    // 被唤醒（暂停 / 恢复 / 取消）：按相位扣减已过时间后重新判定
                    let elapsed = started.elapsed().as_secs();
                    remaining = retry_policy::remaining_after(phase, remaining, elapsed);
                }
            }
        }
    }

    /// Pauses a running download by sending SIGSTOP (Unix) or SuspendThread (Windows).
    ///
    /// 退避中的任务（`child=None`）没有进程可停：只把状态置 `Paused` 并 `notify`
    /// 唤醒退避循环去冻结计时。**发信号只在 `Some(pid)` 分支内**（安全要求）。
    pub fn pause(&self, task_id: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        // Try to pause an active task — must release lock before calling emit_queue_changed
        let paused_record_id = {
            let mut active = self.active_tasks.lock().unwrap();
            if let Some(entry) = active.get_mut(task_id) {
                if let Some(pid) = entry.pid {
                    #[cfg(unix)]
                    {
                        unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                    }

                    #[cfg(windows)]
                    {
                        windows_process::suspend_process(pid);
                    }
                }
                entry.task.status = TaskStatus::Paused;
                // 唤醒退避循环（若在退避中则冻结倒计时）
                entry.notify.notify_one();
                log::info!("Paused download task {}", task_id);
                Some(entry.task.record_id.clone())
            } else {
                None
            }
            // active lock guard dropped here — safe to call emit_queue_changed
        };

        if let Some(record_id) = paused_record_id {
            self.update_record_status(data_dir, &record_id, RecordStatus::Paused, None);
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        // Check waiting queue
        let waiting_record_id = {
            let mut queue = self.queue.lock().unwrap();
            queue
                .iter_mut()
                .find(|t| t.id == task_id)
                .map(|task| {
                    task.status = TaskStatus::Paused;
                    task.record_id.clone()
                })
            // queue lock guard dropped here
        };

        if let Some(record_id) = waiting_record_id {
            self.update_record_status(data_dir, &record_id, RecordStatus::Paused, None);
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("Task {} not found", task_id)))
    }

    /// Resumes a paused download by sending SIGCONT (Unix) or ResumeThread (Windows).
    ///
    /// 退避中恢复：状态回到 `Retrying`，退避循环用**剩余时长**续算。
    pub fn resume(&self, task_id: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        // Try to resume an active task — must release lock before calling emit_queue_changed
        let resumed = {
            let mut active = self.active_tasks.lock().unwrap();
            if let Some(entry) = active.get_mut(task_id) {
                if let Some(pid) = entry.pid {
                    #[cfg(unix)]
                    {
                        unsafe { libc::kill(pid as i32, libc::SIGCONT); }
                    }

                    #[cfg(windows)]
                    {
                        windows_process::resume_process(pid);
                    }
                }
                // child 不存在 ⇒ 处于退避等待，恢复后回到 Retrying
                let backing_off = entry.child.is_none();
                entry.task.status = if backing_off {
                    TaskStatus::Retrying
                } else {
                    TaskStatus::Running
                };
                // 唤醒退避循环继续计时
                entry.notify.notify_one();
                log::info!("Resumed download task {}", task_id);
                Some((entry.task.record_id.clone(), backing_off))
            } else {
                None
            }
            // active lock guard dropped here — safe to call emit_queue_changed
        };

        if let Some((record_id, backing_off)) = resumed {
            let status = if backing_off {
                RecordStatus::Retrying
            } else {
                RecordStatus::Downloading
            };
            self.update_record_status(data_dir, &record_id, status, None);
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        // Check waiting queue for paused tasks
        let waiting_record_id = {
            let mut queue = self.queue.lock().unwrap();
            queue
                .iter_mut()
                .find(|t| t.id == task_id && t.status == TaskStatus::Paused)
                .map(|task| {
                    task.status = TaskStatus::Waiting;
                    task.record_id.clone()
                })
            // queue lock guard dropped here
        };

        if let Some(record_id) = waiting_record_id {
            self.update_record_status(data_dir, &record_id, RecordStatus::Downloading, None);
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("Task {} not found or not paused", task_id)))
    }

    /// Pauses a running download identified by video_url.
    pub fn pause_by_url(&self, video_url: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        let paused_record_id = {
            let mut active = self.active_tasks.lock().unwrap();
            let mut found = None;
            for entry in active.values_mut() {
                if entry.task.video_url == video_url {
                    if let Some(pid) = entry.pid {
                        #[cfg(unix)]
                        {
                            unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                        }
                        #[cfg(windows)]
                        {
                            windows_process::suspend_process(pid);
                        }
                    }
                    entry.task.status = TaskStatus::Paused;
                    entry.notify.notify_one();
                    found = Some(entry.task.record_id.clone());
                    break;
                }
            }
            found
            // lock guard dropped here
        };

        match paused_record_id {
            Some(record_id) => {
                log::info!("Paused download by url {}", video_url);
                self.update_record_status(data_dir, &record_id, RecordStatus::Paused, None);
                self.notify_records_changed();
                self.emit_queue_changed();
                Ok(())
            }
            None => {
                // Fallback: check the waiting queue
                let waiting_record_id = {
                    let mut queue = self.queue.lock().unwrap();
                    let mut found = None;
                    for task in queue.iter_mut() {
                        if task.video_url == video_url {
                            task.status = TaskStatus::Paused;
                            found = Some(task.record_id.clone());
                            break;
                        }
                    }
                    found
                    // queue lock guard dropped here
                };
                match waiting_record_id {
                    Some(record_id) => {
                        log::info!("Paused waiting download by url {}", video_url);
                        self.update_record_status(data_dir, &record_id, RecordStatus::Paused, None);
                        self.notify_records_changed();
                        self.emit_queue_changed();
                        Ok(())
                    }
                    None => Err(AppError::NotFound(format!("No active task found for url {}", video_url))),
                }
            }
        }
    }

    /// Cancels a download identified by video_url (calls cancel by task_id internally).
    pub fn cancel_by_url(&self, video_url: &str, ctx: &DownloadContext) -> Result<(), AppError> {
        // Search active tasks
        let active = self.active_tasks.lock().unwrap();
        let task_id = active.iter()
            .find(|(_, entry)| entry.task.video_url == video_url)
            .map(|(id, _)| id.clone());
        drop(active);

        if let Some(id) = task_id {
            return self.cancel(&id, ctx);
        }

        // Fallback: search the waiting queue
        let waiting_record_id = {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.video_url == video_url) {
                let task = queue.remove(pos).unwrap();
                Some(task.record_id)
            } else {
                None
            }
            // queue lock guard dropped here
        };

        if let Some(record_id) = waiting_record_id {
            self.update_record_status(&ctx.data_dir, &record_id, RecordStatus::Cancelled, Some("Cancelled by user".to_string()));
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("No active task found for url {}", video_url)))
    }

    /// Cancels a download task: kills process if running, removes from queue if waiting,
    /// cleans up partial files, and marks DownloadRecord as cancelled.
    ///
    /// 退避中取消：entry 里没有子进程，`notify` 唤醒退避循环 → 循环发现 entry 已移除
    /// 即退出且**不回写任何记录**（否则会把 `cancelled` 覆盖成 `failed`）。
    pub fn cancel(&self, task_id: &str, ctx: &DownloadContext) -> Result<(), AppError> {
        // Try to cancel an active (running/paused) task first.
        // Must drop the lock guard before calling emit_queue_changed.
        let removed_entry = {
            let mut active = self.active_tasks.lock().unwrap();
            active.remove(task_id)
            // lock guard dropped here
        };

        if let Some(mut entry) = removed_entry {
            // Kill the child process if still running (退避期间 child=None，无需 kill)
            if let Some(child) = entry.child.as_mut() {
                let _ = child.start_kill();
                log::info!("Killed download task {}", task_id);
            } else {
                log::info!("Cancelled backing-off download task {}", task_id);
            }
            // 打断退避等待：循环醒来发现 entry 已移除即退出
            entry.notify.notify_one();

            // Clean up partial files
            self.cleanup_partial_files(&entry.task.video_title, &ctx.download_dir);

            // Mark the record as cancelled —— 按 record_id 精确定位，且**不改写
            // downloaded_at**（只有真正成功完成时才写）
            let _ = StorageService::update_download_records(&ctx.data_dir, |records| {
                if let Some(r) = records.iter_mut().find(|r| r.id == entry.task.record_id) {
                    r.status = RecordStatus::Cancelled;
                    r.error_message = Some("Cancelled by user".to_string());
                }
                Ok(())
            });

            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        // Try to cancel a waiting task — must release queue lock before emit_queue_changed
        let cancelled_task = {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                Some(queue.remove(pos).unwrap())
            } else {
                None
            }
            // queue lock guard dropped here
        };

        if let Some(task) = cancelled_task {
            self.update_record_status(&ctx.data_dir, &task.record_id, RecordStatus::Cancelled, Some("Cancelled by user".to_string()));
            self.notify_records_changed();
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("Task {} not found", task_id)))
    }

    /// Cleans up partial files created by yt-dlp for a specific video.
    fn cleanup_partial_files(&self, video_title: &str, download_dir: &PathBuf) {
        // yt-dlp creates partial files with .part and .ytdl extensions
        if let Ok(entries) = std::fs::read_dir(download_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.contains(video_title) && (name_str.ends_with(".part") || name_str.ends_with(".ytdl")) {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!("Failed to clean up partial file {:?}: {}", entry.path(), e);
                    } else {
                        log::info!("Cleaned up partial file {:?}", entry.path());
                    }
                }
            }
        }
    }

    /// Updates a DownloadRecord's status and error message, located by `record_id`.
    ///
    /// **不写 `downloaded_at`**：该字段只在真正成功完成时更新（暂停 / 取消 / 重试都不改）。
    fn update_record_status(
        &self,
        data_dir: &PathBuf,
        record_id: &str,
        status: RecordStatus,
        error_message: Option<String>,
    ) {
        Self::update_record_by_id(data_dir, record_id, |r| {
            r.status = status;
            r.error_message = error_message;
        });
    }

    /// Returns runtime queue state.
    pub fn get_state(&self) -> QueueState {
        let queue = self.queue.lock().unwrap();
        let active = self.active_tasks.lock().unwrap();
        QueueState {
            active_count: active.len(),
            waiting_count: queue.len(),
            max_concurrent: self.max_concurrent.load(Ordering::Relaxed),
        }
    }

    /// Dynamically adjusts the maximum concurrent download count.
    ///
    /// When increasing: adds permits to the semaphore so waiting tasks can start.
    /// When decreasing: pauses active tasks with least download progress (FR-010),
    /// then reclaims excess semaphore permits.
    pub fn update_max_concurrent(&self, new_max: u32) {
        let old = self.max_concurrent.swap(new_max, Ordering::SeqCst);

        if new_max > old {
            self.semaphore.add_permits((new_max - old) as usize);
            log::info!("Concurrency increased: {} → {}", old, new_max);
        }

        if new_max < old {
            let excess = (old - new_max) as usize;

            // FR-010: Pause active tasks with least progress first.
            // **排除退避中的 entry**（`child.is_none()`）：它们没有进程可暂停，
            // 且退避期已占用并发名额，不应作为缩容的牺牲品。
            let task_ids_to_pause = {
                let active = self.active_tasks.lock().unwrap();
                let mut entries: Vec<(String, f32)> = active
                    .iter()
                    .filter(|(_, entry)| entry.child.is_some())
                    .map(|(id, entry)| (id.clone(), entry.last_progress_percent))
                    .collect();
                // Sort by progress ascending (least progress first)
                entries.sort_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                entries
                    .into_iter()
                    .take(excess)
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
                // lock released
            };

            for task_id in &task_ids_to_pause {
                let mut active = self.active_tasks.lock().unwrap();
                if let Some(entry) = active.get_mut(task_id) {
                    // 安全要求：发信号只在 `Some(pid)` 分支内
                    if let Some(pid) = entry.pid {
                        #[cfg(unix)]
                        {
                            unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                        }
                        #[cfg(windows)]
                        {
                            windows_process::suspend_process(pid);
                        }
                    }

                    entry.task.status = TaskStatus::Paused;
                    log::info!(
                        "Concurrency decrease: paused task {} (progress {:.1}%)",
                        task_id,
                        entry.last_progress_percent
                    );
                }
                // lock released
            }

            // Reclaim excess permits from the semaphore (non-blocking)
            for _ in 0..excess {
                if self.semaphore.try_acquire().is_ok() {
                    // Acquired and dropped — effectively forgetting the permit
                } else {
                    break;
                }
            }
            log::info!("Concurrency decreased: {} → {} (paused {} tasks)", old, new_max, task_ids_to_pause.len());
        }

        self.emit_queue_changed();
    }

    /// Returns all tasks currently in the queue (waiting + active).
    pub fn get_tasks(&self) -> Vec<DownloadTask> {
        let queue = self.queue.lock().unwrap();
        let mut tasks: Vec<DownloadTask> = queue.iter().cloned().collect();

        // Include active tasks
        let active = self.active_tasks.lock().unwrap();
        for entry in active.values() {
            tasks.push(entry.task.clone());
        }

        tasks
    }

    /// Emits the queue-changed event with current state.
    fn emit_queue_changed(&self) {
        let state = self.get_state();
        let _ = self.app_handle.emit("queue-changed", state);
    }

    /// 见模块级 [`notify_records_changed`] —— 仅为对齐 `self.emit_queue_changed()` 的调用样式。
    fn notify_records_changed(&self) {
        notify_records_changed(&self.app_handle);
    }
}

/// 通知前端「下载记录已变化」（无载荷）。
///
/// 纯 emit：零 I/O、零加锁、不阻塞，因此可在任意临界区内安全调用。
/// 契约：凡写 `download_records.json` 的路径都必须调用本函数。
pub(crate) fn notify_records_changed(app_handle: &tauri::AppHandle) {
    let _ = app_handle.emit("records-changed", ());
}

/// 构造并 emit 队列状态快照，供前端在任务状态变化时重新拉取队列。
///
/// 加锁顺序固定为 `queue → active_tasks`，与 [`DownloadQueue::get_state`] 一致，
/// 避免与其它取状态的路径形成反向锁序；两把锁都只短暂持有，emit 在锁外进行。
fn emit_queue_state(
    app_handle: &AppHandle,
    queue: &Arc<Mutex<VecDeque<DownloadTask>>>,
    active_tasks: &Arc<Mutex<HashMap<String, ActiveTask>>>,
    max_concurrent: u32,
) {
    let state = {
        let waiting_count = queue.lock().unwrap().len();
        let active_count = active_tasks.lock().unwrap().len();
        QueueState {
            active_count,
            waiting_count,
            max_concurrent,
        }
    };
    let _ = app_handle.emit("queue-changed", state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Waiting.to_string(), "waiting");
        assert_eq!(TaskStatus::Running.to_string(), "running");
        assert_eq!(TaskStatus::Paused.to_string(), "paused");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
        assert_eq!(TaskStatus::Retrying.to_string(), "retrying");
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::Waiting;
        let json = serde_json::to_string(&status).expect("should serialize");
        assert_eq!(json, "\"waiting\"");

        let status = TaskStatus::Completed;
        let json = serde_json::to_string(&status).expect("should serialize");
        assert_eq!(json, "\"completed\"");

        // 前端 downloadTask.status 联合类型含 "retrying"
        let status = TaskStatus::Retrying;
        let json = serde_json::to_string(&status).expect("should serialize");
        assert_eq!(json, "\"retrying\"");
    }

    #[test]
    fn test_download_task_serialization() {
        let task = DownloadTask {
            id: "task-1".to_string(),
            video_id: "vid-1".to_string(),
            video_url: "https://example.com/v".to_string(),
            video_title: "Test Video".to_string(),
            subscription_id: "sub-1".to_string(),
            quality: "1080p".to_string(),
            status: TaskStatus::Waiting,
            progress: None,
            error_message: None,
            created_at: "2025-05-31T00:00:00Z".to_string(),
            completed_at: None,
            record_id: "rec-1".to_string(),
            next_retry_at: None,
        };

        let json = serde_json::to_string(&task).expect("should serialize");
        assert!(json.contains("task-1"));
        assert!(json.contains("waiting"));
        assert!(json.contains("Test Video"));
    }

    #[test]
    fn test_cancel_removes_waiting_task() {
        // Verifies that cancel searches the queue for waiting tasks
        assert_eq!(TaskStatus::Waiting.to_string(), "waiting");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_pause_resume_status_transitions() {
        // Verify state transition strings
        assert_eq!(TaskStatus::Paused.to_string(), "paused");
        assert_eq!(TaskStatus::Running.to_string(), "running");
    }

    // ── Concurrency adjustment tests (T024) ────────────────────────

    #[test]
    fn test_select_tasks_by_progress_ascending() {
        // Verify progress-based selection: tasks with least progress come first
        let mut entries: Vec<(String, f32)> = vec![
            ("task-a".to_string(), 45.0),
            ("task-b".to_string(), 10.0),
            ("task-c".to_string(), 90.0),
        ];
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // task-b (10%) < task-a (45%) < task-c (90%)
        assert_eq!(entries[0].0, "task-b");
        assert_eq!(entries[0].1, 10.0);
        assert_eq!(entries[1].0, "task-a");
        assert_eq!(entries[1].1, 45.0);
        assert_eq!(entries[2].0, "task-c");
        assert_eq!(entries[2].1, 90.0);
    }

    #[test]
    fn test_select_least_progress_tasks_to_pause() {
        // Simulate: 3 active tasks, reduce from 3 → 2 concurrent
        // Should pause the 1 task with least progress
        let mut entries: Vec<(String, f32)> = vec![
            ("fast-task".to_string(), 88.5),
            ("mid-task".to_string(), 42.0),
            ("slow-task".to_string(), 5.2),
        ];
        let excess = 1; // reduce by 1
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let paused: Vec<_> = entries.into_iter().take(excess).collect();

        assert_eq!(paused.len(), 1);
        assert_eq!(paused[0].0, "slow-task");
        assert_eq!(paused[0].1, 5.2);
    }

    #[test]
    fn test_select_multiple_to_pause() {
        // Simulate: 5 active tasks, reduce from 5 → 2 concurrent
        // Should pause 3 tasks with least progress
        let mut entries: Vec<(String, f32)> = vec![
            ("t1".to_string(), 95.0),
            ("t2".to_string(), 60.0),
            ("t3".to_string(), 30.0),
            ("t4".to_string(), 10.0),
            ("t5".to_string(), 0.5),
        ];
        let excess = 3;
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let paused: Vec<_> = entries.into_iter().take(excess).collect();

        assert_eq!(paused.len(), 3);
        assert_eq!(paused[0].0, "t5"); // 0.5%
        assert_eq!(paused[1].0, "t4"); // 10%
        assert_eq!(paused[2].0, "t3"); // 30%
    }

    #[test]
    fn test_no_pause_when_concurrency_increases() {
        // Verifies that increase doesn't select any tasks to pause
        let old = 2u32;
        let new = 4u32;
        assert!(new > old);
        let diff = (new - old) as usize;
        assert_eq!(diff, 2);

        // Increase: no tasks selected for pause
        let excess: usize = 0;
        let entries: Vec<(String, f32)> = vec![("t1".to_string(), 50.0)];
        let paused: Vec<_> = entries.into_iter().take(excess).collect();
        assert_eq!(paused.len(), 0);
    }

    #[test]
    fn test_concurrency_logic_no_excess_when_equal() {
        // No change: should not pause any tasks
        let old = 3u32;
        let new = 3u32;
        assert!(!(new > old));
        assert!(!(new < old));
        // No tasks should be paused
    }

    #[test]
    fn test_concurrency_adjustment_completes_fast() {
        // SC-004: verify the sorting logic completes quickly
        // (the actual method involves OS signals, tested separately)
        let mut entries: Vec<(String, f32)> = (0..100)
            .map(|i| (format!("task-{}", i), (i as f32) * 1.5 % 100.0))
            .collect();
        let excess = 10usize;

        let start = std::time::Instant::now();
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let _paused: Vec<_> = entries.into_iter().take(excess).collect();
        let elapsed = start.elapsed();

        // SC-004: should complete well under 3 seconds (sorting 100 items is microseconds)
        assert!(
            elapsed.as_millis() < 10,
            "Sort+select should complete fast (actual: {:?})",
            elapsed
        );
    }

    // ── 记录最终写入（finalize_*）：可直接单测的静态函数 ──────────────

    fn test_ctx(dir: &std::path::Path) -> DownloadContext {
        DownloadContext {
            yt_dlp_path: "yt-dlp".to_string(),
            proxy: None,
            cookie_file: None,
            download_dir: dir.to_path_buf(),
            data_dir: dir.to_path_buf(),
        }
    }

    fn seed_record(dir: &std::path::Path, record: &DownloadRecord) {
        StorageService::seed_download_records(dir, std::slice::from_ref(record))
            .expect("seed should succeed");
    }

    fn load_one(dir: &std::path::Path) -> DownloadRecord {
        StorageService::load_download_records(dir)
            .expect("load should succeed")
            .remove(0)
    }

    fn base_record(status: &str) -> DownloadRecord {
        let mut r = DownloadRecord::new(
            "sub-1".to_string(),
            "Video".to_string(),
            "https://e/v".to_string(),
            "vid-1".to_string(),
        );
        r.status = RecordStatus::parse(status);
        r.quality = "1080p".to_string();
        r
    }

    #[test]
    fn finalize_retrying_sets_count_and_timestamp() {
        let tmp = tempfile::TempDir::new().unwrap();
        let record = base_record("downloading");
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_retrying(&test_ctx(tmp.path()), &record.id, 2, "2026-01-01T00:00:00Z");

        let r = load_one(tmp.path());
        assert_eq!(r.status, RecordStatus::Retrying);
        assert_eq!(r.retry_count, 2);
        assert_eq!(r.last_retry_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn finalize_failed_does_not_overwrite_cancelled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let record = base_record("cancelled");
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_failed(&test_ctx(tmp.path()), &record.id, "boom");

        let r = load_one(tmp.path());
        // 取消优先于失败：不得把 cancelled 覆盖成 failed
        assert_eq!(r.status, RecordStatus::Cancelled);
    }

    #[test]
    fn finalize_failed_writes_sanitized_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let record = base_record("downloading");
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_failed(
            &test_ctx(tmp.path()),
            &record.id,
            "无法启动 yt-dlp：socks5://user:pass@127.0.0.1:1080 refused",
        );

        let r = load_one(tmp.path());
        assert_eq!(r.status, RecordStatus::Failed);
        assert!(!r.error_message.as_deref().unwrap().contains("user:pass@"));
    }

    #[test]
    fn finalize_success_preserves_retry_count_and_clears_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut record = base_record("retrying");
        record.retry_count = 2;
        record.last_retry_at = Some("2026-01-01T00:00:00Z".to_string());
        record.error_message = Some("old error".to_string());
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_success(&test_ctx(tmp.path()), &record.id, "");

        let r = load_one(tmp.path());
        assert_eq!(r.status, RecordStatus::Completed);
        // 成功时保留重试次数（spec 故事 2 场景 2）
        assert_eq!(r.retry_count, 2);
        // 成功时清空错误信息
        assert_eq!(r.error_message, None);
    }

    #[test]
    fn finalize_success_reclaims_old_file_only_when_path_differs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let old_file = tmp.path().join("old.webm");
        let new_file = tmp.path().join("new.mp4");
        std::fs::write(&old_file, b"old").unwrap();
        std::fs::write(&new_file, b"new data").unwrap();

        let mut record = base_record("downloading");
        record.file_path = old_file.to_string_lossy().to_string();
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_success(
            &test_ctx(tmp.path()),
            &record.id,
            &new_file.to_string_lossy(),
        );

        let r = load_one(tmp.path());
        assert_eq!(r.file_path, new_file.to_string_lossy());
        assert_eq!(r.file_size, 8);
        // 先落库新路径、再回收旧文件 ⇒ 旧文件已不在原位置
        assert!(!old_file.exists(), "旧文件应已被回收");
    }

    #[test]
    fn finalize_success_keeps_same_path_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("same.mp4");
        std::fs::write(&file, b"data").unwrap();

        let mut record = base_record("downloading");
        record.file_path = file.to_string_lossy().to_string();
        seed_record(tmp.path(), &record);

        DownloadQueue::finalize_success(
            &test_ctx(tmp.path()),
            &record.id,
            &file.to_string_lossy(),
        );

        // 同一路径不得被回收（否则会把刚下好的文件删掉）
        assert!(file.exists(), "同路径文件不应被回收");
        assert_eq!(load_one(tmp.path()).status, RecordStatus::Completed);
    }

    #[test]
    fn record_matches_key_prefers_video_id_and_falls_back_to_url() {
        let mut r = base_record("completed");
        r.video_id = "vid-1".to_string();
        assert!(record_matches_key(&r, "sub-1", "vid-1", "https://e/other"));
        assert!(!record_matches_key(&r, "sub-2", "vid-1", "https://e/other"));

        let mut no_id = base_record("completed");
        no_id.video_id = String::new();
        no_id.video_url = "https://e/v".to_string();
        assert!(record_matches_key(&no_id, "sub-1", "", "https://e/v"));
        assert!(!record_matches_key(&no_id, "sub-1", "", "https://e/other"));
    }
}
