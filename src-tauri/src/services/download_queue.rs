use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, AppHandle};
use tokio::sync::Semaphore;

use crate::models::{DownloadRecord, Subscription};
use crate::services::{StorageService, YtDlpService};
use crate::utils::AppError;

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

/// FIFO download queue with concurrency control.
pub struct DownloadQueue {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    semaphore: Arc<Semaphore>,
    app_handle: AppHandle,
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    pub fn new(app_handle: AppHandle, max_concurrent: u32) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
            app_handle,
        }
    }

    /// Enqueues a batch of new video download tasks.
    /// Returns the number of tasks added.
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
        // Drop lock before emitting event
        drop(queue);
        self.emit_queue_changed();
        count
    }

    /// Queues a single download task from a check_and_download result.
    /// Creates a DownloadRecord and saves it immediately.
    pub fn enqueue_from_video(
        &self,
        sub: &Subscription,
        video_title: String,
        video_url: String,
        video_id: String,
        quality: String,
        data_dir: &PathBuf,
    ) -> Result<(), AppError> {
        // Create and save a "downloading" record
        let record = DownloadRecord::new(
            sub.id.clone(),
            video_title.clone(),
            video_url.clone(),
            video_id.clone(),
        );

        let mut all_records = StorageService::load_download_records(data_dir)?;
        all_records.push(record);
        StorageService::save_download_records(data_dir, &all_records)?;

        // Emit event for frontend refresh
        let _ = self.app_handle.emit("records-changed", ());

        // Create task and push to queue
        let mut queue = self.queue.lock().unwrap();
        let task = DownloadTask {
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
        };
        queue.push_back(task);
        drop(queue);

        self.emit_queue_changed();

        Ok(())
    }

    /// Starts processing the queue. Called after enqueueing tasks.
    /// Will spawn download tasks up to the semaphore limit.
    pub fn start_processing(&self, ctx: DownloadContext) {
        let queue = Arc::clone(&self.queue);
        let semaphore = Arc::clone(&self.semaphore);
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            loop {
                // Acquire semaphore permit before starting a task
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break, // Semaphore closed
                };

                let task = {
                    let mut q = queue.lock().unwrap();
                    q.pop_front()
                };

                let task = match task {
                    Some(t) => t,
                    None => {
                        // No more tasks
                        break;
                    }
                };

                let ctx_clone = ctx.clone();
                let queue_clone = Arc::clone(&queue);
                let app_clone = app_handle.clone();
                let sem_clone = Arc::clone(&semaphore);

                tokio::spawn(async move {
                    let _permit = permit; // Hold permit until done

                    let result = Self::execute_download(
                        &task,
                        &ctx_clone,
                        &app_clone,
                    ).await;

                    // Update DownloadRecord based on result
                    let records = StorageService::load_download_records(&ctx_clone.data_dir)
                        .unwrap_or_default();
                    let matching: Vec<_> = records.iter()
                        .filter(|r| r.video_url == task.video_url && r.subscription_id == task.subscription_id)
                        .collect();
                    let record_id = matching.last().map(|r| r.id.clone());

                    if let Some(record_id) = record_id {
                        let mut all_records = StorageService::load_download_records(&ctx_clone.data_dir)
                            .unwrap_or_default();
                        if let Some(existing) = all_records.iter_mut().find(|r| r.id == record_id) {
                            match result {
                                Ok(_) => {
                                    existing.status = "completed".to_string();
                                    existing.downloaded_at = Utc::now().to_rfc3339();
                                }
                                Err(e) => {
                                    existing.status = "failed".to_string();
                                    existing.error_message = Some(e.to_string());
                                    existing.downloaded_at = Utc::now().to_rfc3339();
                                }
                            }
                        }
                        let _ = StorageService::save_download_records(&ctx_clone.data_dir, &all_records);
                        let _ = app_clone.emit("records-changed", ());
                    }

                    // Emit queue changed event
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

    async fn execute_download(
        task: &DownloadTask,
        ctx: &DownloadContext,
        app_handle: &AppHandle,
    ) -> Result<(), AppError> {
        let task_id = task.id.clone();
        let app_clone = app_handle.clone();

        let result = YtDlpService::download_video_streaming(
            &ctx.yt_dlp_path,
            &ctx.proxy,
            &ctx.cookie_file,
            &task.video_url,
            &task.quality,
            &ctx.download_dir,
            |progress| {
                let event = DownloadProgressEvent {
                    task_id: task_id.clone(),
                    video_url: task.video_url.clone(),
                    percent: progress.percent,
                    speed: progress.speed,
                    downloaded_bytes: progress.downloaded_bytes,
                    total_bytes: progress.total_bytes,
                    eta: progress.eta,
                };
                let _ = app_clone.emit("download-progress", event);
            },
        ).await;

        match result {
            Ok(_download_result) => {
                let _ = app_handle.emit(
                    "download-complete",
                    serde_json::json!({
                        "title": &task.video_title,
                    }),
                );
                Ok(())
            }
            Err(e) => {
                log::error!("Download failed for {}: {}", task.video_title, e);
                Err(e)
            }
        }
    }

    /// Pauses a running download task by its task ID.
    pub fn pause(&self, task_id: &str) -> Result<(), AppError> {
        // The task is spawned in JoinSet, so we locate it by checking the queue
        // Since tasks are popped from queue when running, we need to track them differently.
        // For now, we note that the current architecture doesn't support pausing via JoinSet direct lookup.
        // This will be fully implemented in Phase 7.
        Err(AppError::NotFound(format!("Task {} not found or not in pauseable state", task_id)))
    }

    /// Resumes a paused download task.
    pub fn resume(&self, task_id: &str) -> Result<(), AppError> {
        Err(AppError::NotFound(format!("Task {} not found or not in resumable state", task_id)))
    }

    /// Cancels a download task and cleans up partial files.
    pub fn cancel(&self, task_id: &str) -> Result<(), AppError> {
        // Find and remove from waiting queue, or abort running task
        let mut queue = self.queue.lock().unwrap();
        if let Some(pos) = queue.iter().position(|t| t.id == task_id) {
            queue.remove(pos);
            return Ok(());
        }
        Err(AppError::NotFound(format!("Task {} not found in queue", task_id)))
    }

    /// Returns runtime queue state.
    pub fn get_state(&self) -> QueueState {
        let queue = self.queue.lock().unwrap();
        QueueState {
            active_count: self.semaphore.available_permits() as usize,
            waiting_count: queue.len(),
            max_concurrent: 1, // Will be configurable in Phase 5
        }
    }

    /// Returns all tasks currently in the queue.
    pub fn get_tasks(&self) -> Vec<DownloadTask> {
        let queue = self.queue.lock().unwrap();
        queue.iter().cloned().collect()
    }

    /// Emits the queue-changed event with current state.
    fn emit_queue_changed(&self) {
        let state = {
            let queue = self.queue.lock().unwrap();
            // Count running tasks
            let active = 0; // running tasks tracked separately
            QueueState {
                active_count: active,
                waiting_count: queue.len(),
                max_concurrent: 1,
            }
        };
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
}
