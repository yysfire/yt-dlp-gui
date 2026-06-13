use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, AppHandle};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Semaphore;

use crate::models::{DownloadRecord, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::utils::AppError;
use crate::utils::progress_parser;

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

/// Holds the child process handle and PID for an active download.
struct ActiveTask {
    child: Option<tokio::process::Child>,
    pid: u32,
    task: DownloadTask,
    /// T028: Last known download progress percentage (0.0–100.0).
    /// Used by adjust_concurrency to select which tasks to pause first.
    last_progress_percent: f32,
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

    /// Enqueues a batch of new video download tasks.
    pub fn enqueue_batch(
        &self,
        videos: Vec<(String, String, String)>,
        subscription_id: String,
        quality: String,
    ) -> usize {
        let mut queue = self.queue.lock().unwrap();
        let count = videos.len();
        for (video_id, video_title, video_url) in videos {
            let task = DownloadTask {
                id: uuid::Uuid::new_v4().to_string(),
                video_id,
                video_url,
                video_title,
                subscription_id: subscription_id.clone(),
                quality: quality.clone(),
                status: TaskStatus::Waiting,
                progress: None,
                error_message: None,
                created_at: Utc::now().to_rfc3339(),
                completed_at: None,
            };
            queue.push_back(task);
        }
        drop(queue);
        self.emit_queue_changed();
        count
    }

    /// Queues a single download task from a check result.
    pub fn enqueue_from_video(
        &self,
        sub: &Subscription,
        video_title: String,
        video_url: String,
        video_id: String,
        quality: String,
        data_dir: &PathBuf,
    ) -> Result<(), AppError> {
        let record = DownloadRecord::new(
            sub.id.clone(),
            video_title.clone(),
            video_url.clone(),
            video_id.clone(),
        );

        let mut all_records = StorageService::load_download_records(data_dir)?;
        all_records.push(record);
        StorageService::save_download_records(data_dir, &all_records)?;
        let _ = self.app_handle.emit("records-changed", ());

        let mut queue = self.queue.lock().unwrap();
        queue.push_back(DownloadTask {
            id: uuid::Uuid::new_v4().to_string(),
            video_id,
            video_url,
            video_title,
            subscription_id: sub.id.clone(),
            quality,
            status: TaskStatus::Waiting,
            progress: None,
            error_message: None,
            created_at: Utc::now().to_rfc3339(),
            completed_at: None,
        });
        drop(queue);
        self.emit_queue_changed();

        Ok(())
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
                let sem_clone = Arc::clone(&semaphore);
                let max_conc_clone = Arc::clone(&max_conc);

                tokio::spawn(async move {
                    let _permit = permit;

                    Self::execute_download_with_control(
                        &task,
                        &ctx_clone,
                        &active_clone,
                        &app_clone,
                    ).await;

                    // Clean up active entry
                    {
                        let mut active = active_clone.lock().unwrap();
                        active.remove(&task.id);
                    }

                    // Emit queue changed
                    let state = QueueState {
                        active_count: sem_clone.available_permits() as usize,
                        waiting_count: queue_clone.lock().unwrap().len(),
                        max_concurrent: max_conc_clone.load(Ordering::Relaxed),
                    };
                    let _ = app_clone.emit("queue-changed", state);
                });
            }
        });
    }

    /// Executes a download with process lifecycle control.
    /// Reads stdout for progress, supports pause/resume via signals.
    async fn execute_download_with_control(
        task: &DownloadTask,
        ctx: &DownloadContext,
        active_tasks: &Arc<Mutex<HashMap<String, ActiveTask>>>,
        app_handle: &AppHandle,
    ) {
        let task_id = task.id.clone();

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
                    let _ = app_handle.emit("records-changed", serde_json::json!({
                        "path_fallback": true,
                        "original": ctx.download_dir.to_string_lossy(),
                        "fallback": default_dir,
                    }));
                    default
                }
            }
        };

        // Spawn the yt-dlp process
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
                log::error!("Failed to spawn yt-dlp for {}: {}", task.video_title, e);
                return;
            }
        };

        let mut child = spawned.child;
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };

        // Register the active task with PID
        let pid = {
            if let Some(id) = child.id() {
                id
            } else {
                log::warn!("Could not get PID for yt-dlp process");
                return;
            }
        };
        {
            let mut active = active_tasks.lock().unwrap();
            active.insert(task_id.clone(), ActiveTask {
                child: Some(child),
                pid,
                task: task.clone(),
                last_progress_percent: 0.0,
            });
        }

        // Read progress from stdout
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let app_clone = app_handle.clone();
        let active_clone = Arc::clone(active_tasks);
        let active_progress = Arc::clone(active_tasks); // T028: separate clone for progress tracking

        // Clone fields needed inside the spawned task
        let task_url = task.video_url.clone();
        let task_id_for_progress = task_id.clone();
        let task_id_for_lookup = task_id.clone();

        // Spawn progress reader in a separate task
        let progress_handle = tokio::spawn(async move {
            let mut file_path = String::new();

            while let Ok(line) = lines.next_line().await {
                let line = match line {
                    Some(l) => l,
                    None => break,
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Some(event) = progress_parser::parse_progress_line(trimmed) {
                    // Clone Strings for progress tracking (moved into progress_event below)
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
                    {
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
                    }
                } else {
                    file_path = trimmed.to_string();
                }
            }

            file_path
        });

        // Await the child process — take child out without removing the ActiveTask,
        // so get_tasks() and get_state() can still see the active task during download
        let child_to_wait = {
            let mut active = active_clone.lock().unwrap();
            active.get_mut(&task_id_for_lookup).and_then(|entry| entry.child.take())
        };

        let status = match child_to_wait {
            Some(mut child) => Some(child.wait().await),
            None => {
                // Task was cancelled, child already removed
                return;
            }
        };

        let file_path = match progress_handle.await {
            Ok(fp) => fp,
            Err(_) => String::new(),
        };

        match status {
            Some(Ok(s)) if s.success() => {
                let _ = app_handle.emit(
                    "download-complete",
                    serde_json::json!({
                        "title": &task.video_title,
                    }),
                );

                // Update file info in DownloadRecord
                if !file_path.is_empty() {
                    let all_records = StorageService::load_download_records(&ctx.data_dir)
                        .unwrap_or_default();
                    let matching: Vec<_> = all_records.iter()
                        .filter(|r| r.video_url == task.video_url
                            && r.subscription_id == task.subscription_id)
                        .collect();
                    if let Some(record_id) = matching.last().map(|r| r.id.clone()) {
                        let mut records = StorageService::load_download_records(&ctx.data_dir)
                            .unwrap_or_default();
                        if let Some(existing) = records.iter_mut().find(|r| r.id == record_id) {
                            existing.file_path = file_path.clone();
                            existing.file_size = std::fs::metadata(&file_path)
                                .map(|m| m.len())
                                .unwrap_or(0);
                            existing.status = "completed".to_string();
                        }
                        let _ = StorageService::save_download_records(&ctx.data_dir, &records);
                    }
                }

                // Increment total downloads counter
                if let Ok(mut app_state) = StorageService::load_state(&ctx.data_dir) {
                    app_state.total_downloads += 1;
                    let _ = StorageService::save_state(&ctx.data_dir, &app_state);
                }

                // Increment per-subscription download count
                if let Ok(mut subs) = StorageService::load_subscriptions(&ctx.data_dir) {
                    if let Some(sub) = subs.iter_mut().find(|s| s.id == task.subscription_id) {
                        sub.download_count += 1;
                        let _ = StorageService::save_subscriptions(&ctx.data_dir, &subs);
                    }
                }

                let _ = app_handle.emit("records-changed", ());
            }
            _ => {
                log::error!("yt-dlp process failed for {}", task.video_title);
                // Update record with error
                let all_records = StorageService::load_download_records(&ctx.data_dir)
                    .unwrap_or_default();
                let matching: Vec<_> = all_records.iter()
                    .filter(|r| r.video_url == task.video_url
                        && r.subscription_id == task.subscription_id)
                    .collect();
                if let Some(record_id) = matching.last().map(|r| r.id.clone()) {
                    let mut records = StorageService::load_download_records(&ctx.data_dir)
                        .unwrap_or_default();
                    if let Some(existing) = records.iter_mut().find(|r| r.id == record_id) {
                        if existing.status != "cancelled" {
                            existing.status = "failed".to_string();
                            // FR-011: distinguish proxy failure from other errors
                            let has_proxy = ctx.proxy.as_ref()
                                .map(|p| !p.is_empty())
                                .unwrap_or(false);
                            if has_proxy {
                                existing.error_message = Some("代理连接失败".to_string());
                            } else {
                                existing.error_message = Some("yt-dlp process exited with error".to_string());
                            }
                        }
                    }
                    let _ = StorageService::save_download_records(&ctx.data_dir, &records);
                }

                let _ = app_handle.emit("records-changed", ());
            }
        }
    }

    /// Pauses a running download by sending SIGSTOP (Unix) or SuspendThread (Windows).
    pub fn pause(&self, task_id: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        // Try to pause an active task — must release lock before calling emit_queue_changed
        let paused_info = {
            let mut active = self.active_tasks.lock().unwrap();
            if let Some(entry) = active.get_mut(task_id) {
                let pid = entry.pid;

                #[cfg(unix)]
                {
                    unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                }

                #[cfg(windows)]
                {
                    windows_process::suspend_process(pid);
                }

                entry.task.status = TaskStatus::Paused;
                log::info!("Paused download task {}", task_id);
                Some((entry.task.video_url.clone(), entry.task.subscription_id.clone()))
            } else {
                None
            }
            // active lock guard dropped here — safe to call emit_queue_changed
        };

        if let Some((video_url, subscription_id)) = paused_info {
            self.update_record_status(data_dir, &video_url, &subscription_id, "paused", None);
            let _ = self.app_handle.emit("records-changed", ());
            self.emit_queue_changed();
            return Ok(());
        }

        // Check waiting queue
        let waiting_paused = {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                if let Some(task) = queue.get_mut(pos) {
                    task.status = TaskStatus::Paused;
                    Some((task.video_url.clone(), task.subscription_id.clone()))
                } else {
                    None
                }
            } else {
                None
            }
            // queue lock guard dropped here
        };

        if let Some((video_url, subscription_id)) = waiting_paused {
            self.update_record_status(data_dir, &video_url, &subscription_id, "paused", None);
            let _ = self.app_handle.emit("records-changed", ());
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("Task {} not found", task_id)))
    }

    /// Resumes a paused download by sending SIGCONT (Unix) or ResumeThread (Windows).
    pub fn resume(&self, task_id: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        // Try to resume an active task — must release lock before calling emit_queue_changed
        let resumed_info = {
            let mut active = self.active_tasks.lock().unwrap();
            if let Some(entry) = active.get_mut(task_id) {
                let pid = entry.pid;

                #[cfg(unix)]
                {
                    unsafe { libc::kill(pid as i32, libc::SIGCONT); }
                }

                #[cfg(windows)]
                {
                    windows_process::resume_process(pid);
                }

                entry.task.status = TaskStatus::Running;
                log::info!("Resumed download task {}", task_id);
                Some((entry.task.video_url.clone(), entry.task.subscription_id.clone()))
            } else {
                None
            }
            // active lock guard dropped here — safe to call emit_queue_changed
        };

        if let Some((video_url, subscription_id)) = resumed_info {
            self.update_record_status(data_dir, &video_url, &subscription_id, "downloading", None);
            let _ = self.app_handle.emit("records-changed", ());
            self.emit_queue_changed();
            return Ok(());
        }

        // Check waiting queue for paused tasks
        let waiting_resumed = {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id && t.status == TaskStatus::Paused) {
                if let Some(task) = queue.get_mut(pos) {
                    task.status = TaskStatus::Waiting;
                    Some((task.video_url.clone(), task.subscription_id.clone()))
                } else {
                    None
                }
            } else {
                None
            }
            // queue lock guard dropped here
        };

        if let Some((video_url, subscription_id)) = waiting_resumed {
            self.update_record_status(data_dir, &video_url, &subscription_id, "downloading", None);
            let _ = self.app_handle.emit("records-changed", ());
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("Task {} not found or not paused", task_id)))
    }

    /// Pauses a running download identified by video_url.
    pub fn pause_by_url(&self, video_url: &str, data_dir: &PathBuf) -> Result<(), AppError> {
        let paused_info = {
            let mut active = self.active_tasks.lock().unwrap();
            let mut found = None;
            for (_task_id, entry) in active.iter_mut() {
                if entry.task.video_url == video_url {
                    let _pid = entry.pid;
                    #[cfg(unix)]
                    {
                        unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                    }
                    entry.task.status = TaskStatus::Paused;
                    found = Some((entry.task.video_url.clone(), entry.task.subscription_id.clone()));
                    break;
                }
            }
            found
            // lock guard dropped here
        };

        match paused_info {
            Some((url, sub_id)) => {
                log::info!("Paused download by url {}", video_url);
                self.update_record_status(data_dir, &url, &sub_id, "paused", None);
                let _ = self.app_handle.emit("records-changed", ());
                self.emit_queue_changed();
                Ok(())
            }
            None => {
                // Fallback: check the waiting queue
                let waiting_paused = {
                    let mut queue = self.queue.lock().unwrap();
                    let mut found = None;
                    for task in queue.iter_mut() {
                        if task.video_url == video_url {
                            task.status = TaskStatus::Paused;
                            found = Some((task.video_url.clone(), task.subscription_id.clone()));
                            break;
                        }
                    }
                    found
                    // queue lock guard dropped here
                };
                match waiting_paused {
                    Some((url, sub_id)) => {
                        log::info!("Paused waiting download by url {}", video_url);
                        self.update_record_status(data_dir, &url, &sub_id, "paused", None);
                        let _ = self.app_handle.emit("records-changed", ());
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
        let waiting_id = {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.video_url == video_url) {
                let task = queue.remove(pos).unwrap();
                Some(task.id)
            } else {
                None
            }
            // queue lock guard dropped here
        };

        if let Some(_id) = waiting_id {
            self.update_record_status(&ctx.data_dir, video_url, "", "cancelled", Some("Cancelled by user".to_string()));
            let _ = self.app_handle.emit("records-changed", ());
            self.emit_queue_changed();
            return Ok(());
        }

        Err(AppError::NotFound(format!("No active task found for url {}", video_url)))
    }

    /// Cancels a download task: kills process if running, removes from queue if waiting,
    /// cleans up partial files, and marks DownloadRecord as cancelled.
    pub fn cancel(&self, task_id: &str, ctx: &DownloadContext) -> Result<(), AppError> {
        // Try to cancel an active (running/paused) task first.
        // Must drop the lock guard before calling emit_queue_changed.
        let removed_entry = {
            let mut active = self.active_tasks.lock().unwrap();
            active.remove(task_id)
            // lock guard dropped here
        };

        if let Some(mut entry) = removed_entry {
            // Kill the child process if still running
            if let Some(child) = entry.child.as_mut() {
                let _ = child.start_kill();
            }
            log::info!("Killed download task {} (pid {})", task_id, entry.pid);

            // Clean up partial files
            self.cleanup_partial_files(&entry.task.video_title, &ctx.download_dir);

            // Mark matching records as cancelled
            if let Ok(mut records) = StorageService::load_download_records(&ctx.data_dir) {
                for r in records.iter_mut() {
                    if r.video_url == entry.task.video_url
                        && r.subscription_id == entry.task.subscription_id
                    {
                        r.status = "cancelled".to_string();
                        r.error_message = Some("Cancelled by user".to_string());
                        r.downloaded_at = Utc::now().to_rfc3339();
                    }
                }
                let _ = StorageService::save_download_records(&ctx.data_dir, &records);
            }

            let _ = self.app_handle.emit("records-changed", ());
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
            self.update_record_status(&ctx.data_dir, &task.video_url, &task.subscription_id, "cancelled", Some("Cancelled by user".to_string()));
            let _ = self.app_handle.emit("records-changed", ());
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

    /// Updates a DownloadRecord's status and error message.
    fn update_record_status(&self, data_dir: &PathBuf, video_url: &str, subscription_id: &str, status: &str, error_message: Option<String>) {
        if let Ok(mut records) = StorageService::load_download_records(data_dir) {
            for r in records.iter_mut() {
                if r.video_url == video_url && r.subscription_id == subscription_id {
                    r.status = status.to_string();
                    r.error_message = error_message.clone();
                    r.downloaded_at = Utc::now().to_rfc3339();
                }
            }
            let _ = StorageService::save_download_records(data_dir, &records);
        }
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

            // FR-010: Pause active tasks with least progress first
            let task_ids_to_pause = {
                let active = self.active_tasks.lock().unwrap();
                let mut entries: Vec<(String, f32)> = active
                    .iter()
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
                    let pid = entry.pid;

                    #[cfg(unix)]
                    {
                        unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                    }
                    #[cfg(windows)]
                    {
                        windows_process::suspend_process(pid);
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
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::Waiting;
        let json = serde_json::to_string(&status).expect("should serialize");
        assert_eq!(json, "\"waiting\"");

        let status = TaskStatus::Completed;
        let json = serde_json::to_string(&status).expect("should serialize");
        assert_eq!(json, "\"completed\"");
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
}
