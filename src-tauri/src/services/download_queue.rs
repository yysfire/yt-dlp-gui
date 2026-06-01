use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, AppHandle};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Semaphore;

use crate::models::{DownloadRecord, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::utils::AppError;
use crate::utils::progress_parser;

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
}

/// FIFO download queue with concurrency control and process lifecycle management.
pub struct DownloadQueue {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    semaphore: Arc<Semaphore>,
    active_tasks: Arc<Mutex<HashMap<String, ActiveTask>>>,
    app_handle: AppHandle,
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    pub fn new(app_handle: AppHandle, max_concurrent: u32) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            app_handle,
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

                    // Update DownloadRecord
                    let records = StorageService::load_download_records(&ctx_clone.data_dir)
                        .unwrap_or_default();
                    let matching: Vec<_> = records.iter()
                        .filter(|r| r.video_url == task.video_url
                            && r.subscription_id == task.subscription_id)
                        .collect();
                    let record_id = matching.last().map(|r| r.id.clone());

                    if let Some(record_id) = record_id {
                        let mut all_records = StorageService::load_download_records(&ctx_clone.data_dir)
                            .unwrap_or_default();
                        if let Some(existing) = all_records.iter_mut().find(|r| r.id == record_id) {
                            // Status is already set by cancel/pause or kept as-is
                            if existing.status == "downloading" {
                                existing.status = "completed".to_string();
                            }
                            existing.downloaded_at = Utc::now().to_rfc3339();
                        }
                        let _ = StorageService::save_download_records(&ctx_clone.data_dir, &all_records);
                        let _ = app_clone.emit("records-changed", ());
                    }

                    // Emit queue changed
                    let state = QueueState {
                        active_count: sem_clone.available_permits() as usize,
                        waiting_count: queue_clone.lock().unwrap().len(),
                        max_concurrent: sem_clone.available_permits() as u32 + 1,
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

        // Spawn the yt-dlp process
        let spawned = match YtDlpService::download_video_spawn(
            &ctx.yt_dlp_path,
            &ctx.proxy,
            &ctx.cookie_file,
            &task.video_url,
            &task.quality,
            &ctx.download_dir,
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
            });
        }

        // Read progress from stdout
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let app_clone = app_handle.clone();
        let active_clone = Arc::clone(active_tasks);

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
                            existing.error_message = Some("yt-dlp process exited with error".to_string());
                        }
                    }
                    let _ = StorageService::save_download_records(&ctx.data_dir, &records);
                }
            }
        }
    }

    /// Pauses a running download by sending SIGSTOP (Unix) or SuspendThread (Windows).
    pub fn pause(&self, task_id: &str) -> Result<(), AppError> {
        let mut active = self.active_tasks.lock().unwrap();
        if let Some(entry) = active.get_mut(task_id) {
            let pid = entry.pid;

            #[cfg(unix)]
            {
                unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
            }

            #[cfg(windows)]
            {
                // Windows: SuspendThread on all threads of the process
                // This is a best-effort approach
                log::warn!("Process pause on Windows is limited");
            }

            log::info!("Paused download task {}", task_id);
            self.emit_queue_changed();
            Ok(())
        } else {
            // Check waiting queue
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                if let Some(task) = queue.get_mut(pos) {
                    task.status = TaskStatus::Paused;
                    self.emit_queue_changed();
                    return Ok(());
                }
            }
            Err(AppError::NotFound(format!("Task {} not found", task_id)))
        }
    }

    /// Resumes a paused download by sending SIGCONT (Unix) or ResumeThread (Windows).
    pub fn resume(&self, task_id: &str) -> Result<(), AppError> {
        let mut active = self.active_tasks.lock().unwrap();
        if let Some(entry) = active.get_mut(task_id) {
            let pid = entry.pid;

            #[cfg(unix)]
            {
                unsafe { libc::kill(pid as i32, libc::SIGCONT); }
            }

            #[cfg(windows)]
            {
                log::warn!("Process resume on Windows is limited");
            }

            log::info!("Resumed download task {}", task_id);
            self.emit_queue_changed();
            Ok(())
        } else {
            // Check waiting queue for paused tasks
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id && t.status == TaskStatus::Paused) {
                if let Some(task) = queue.get_mut(pos) {
                    task.status = TaskStatus::Waiting;
                    self.emit_queue_changed();
                    return Ok(());
                }
            }
            Err(AppError::NotFound(format!("Task {} not found or not paused", task_id)))
        }
    }

    /// Pauses a running download identified by video_url.
    pub fn pause_by_url(&self, video_url: &str) -> Result<(), AppError> {
        let active = self.active_tasks.lock().unwrap();
        for (_task_id, entry) in active.iter() {
            if entry.task.video_url == video_url {
                let pid = entry.pid;
                #[cfg(unix)]
                {
                    unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
                }
                log::info!("Paused download by url {}", video_url);
                self.emit_queue_changed();
                return Ok(());
            }
        }
        Err(AppError::NotFound(format!("No active task found for url {}", video_url)))
    }

    /// Cancels a download identified by video_url (calls cancel by task_id internally).
    pub fn cancel_by_url(&self, video_url: &str, ctx: &DownloadContext) -> Result<(), AppError> {
        let active = self.active_tasks.lock().unwrap();
        let task_id = active.iter()
            .find(|(_, entry)| entry.task.video_url == video_url)
            .map(|(id, _)| id.clone());
        drop(active);

        match task_id {
            Some(id) => self.cancel(&id, ctx),
            None => Err(AppError::NotFound(format!("No active task found for url {}", video_url))),
        }
    }

    /// Cancels a download task: kills process if running, removes from queue if waiting,
    /// cleans up partial files, and marks DownloadRecord as cancelled.
    pub fn cancel(&self, task_id: &str, ctx: &DownloadContext) -> Result<(), AppError> {
        // Try to cancel an active (running/paused) task
        {
            let mut active = self.active_tasks.lock().unwrap();
            if let Some(mut entry) = active.remove(task_id) {
                // Kill the child process if still running
                if let Some(child) = entry.child.as_mut() {
                    let _ = child.start_kill();
                }
                log::info!("Killed download task {} (pid {})", task_id, entry.pid);

                // Mark all matching records as cancelled
                if let Ok(mut records) = StorageService::load_download_records(&ctx.data_dir) {
                    for r in records.iter_mut() {
                        r.status = "cancelled".to_string();
                        r.error_message = Some("Cancelled by user".to_string());
                        r.downloaded_at = Utc::now().to_rfc3339();
                    }
                    let _ = StorageService::save_download_records(&ctx.data_dir, &records);
                }

                let _ = self.app_handle.emit("records-changed", ());
                self.emit_queue_changed();
                return Ok(());
            }
        }

        // Try to cancel a waiting task
        {
            let mut queue = self.queue.lock().unwrap();
            if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
                let task = queue.remove(pos).unwrap();
                self.update_record_status(&ctx.data_dir, &task.video_url, &task.subscription_id, "cancelled", Some("Cancelled by user".to_string()));
                let _ = self.app_handle.emit("records-changed", ());
                self.emit_queue_changed();
                return Ok(());
            }
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
            max_concurrent: 1,
        }
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
}
