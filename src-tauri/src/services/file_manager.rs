use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Semaphore;

use crate::models::DownloadRecord;
use crate::services::StorageService;
use crate::utils::AppError;

/// Result of checking whether a single file exists on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileExistenceResult {
    pub file_path: String,
    pub exists: bool,
}

/// Batch-checks whether files exist on disk.
/// Uses `spawn_blocking` with a semaphore to limit concurrency to 50.
pub async fn check_files_exist(file_paths: &[String]) -> Vec<FileExistenceResult> {
    let semaphore = Arc::new(Semaphore::new(50));
    let mut handles = Vec::with_capacity(file_paths.len());

    for path in file_paths {
        let path = path.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        handles.push(tokio::task::spawn_blocking(move || {
            let exists = Path::new(&path).exists();
            drop(permit);
            FileExistenceResult { file_path: path, exists }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                log::error!("check_files_exist: join error: {}", e);
            }
        }
    }
    results
}

/// 检查所有 `completed` 且 `file_path` 非空的记录，文件是否仍存在于磁盘，并
/// emit `file-sync-complete`（前端据此派生「文件缺失」）。
///
/// **存在性检查只有这一条实现**：周期任务（`scheduler::spawn_file_sync`）与命令
/// （`commands::file_manager::sync_file_states`）都调用它 —— 原先两处各自内联了一遍
/// 完全相同的过滤逻辑，必然漂移。
///
/// 注意：只检查 `completed` 的记录；`deleted` / `failed` 等状态的记录本就不该有文件。
pub async fn sync_completed_records(
    data_dir: &Path,
    app_handle: &tauri::AppHandle,
) -> Vec<FileExistenceResult> {
    let records = StorageService::load_download_records(data_dir).unwrap_or_default();
    let file_paths: Vec<String> = records
        .iter()
        .filter(|r| r.status == "completed" && !r.file_path.is_empty())
        .map(|r| r.file_path.clone())
        .collect();

    let results = check_files_exist(&file_paths).await;
    let _ = app_handle.emit("file-sync-complete", &results);
    results
}

/// Moves a file to the system trash (recycle bin) instead of permanently deleting it.
///
/// On Linux: uses `gio trash` (preferred, supports undo via desktop file manager).
/// On macOS: uses `osascript` to tell Finder to delete the file.
/// On Windows: uses `std::fs::remove_file` as fallback (the Windows Shell API for
/// recycle bin requires the `trash` crate or winapi, both not yet in scope).
///
/// Falls back to `std::fs::remove_file` when the trash command is not available.
pub(crate) fn delete_file_to_trash(path: &Path) {
    #[cfg(target_os = "linux")]
    {
        // gio trash is the most reliable Linux trash implementation
        if std::process::Command::new("gio")
            .arg("trash")
            .arg(path.to_string_lossy().as_ref())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            log::info!("Moved to trash: {}", path.display());
            return;
        }
        log::warn!("gio trash failed for {}, falling back to permanent delete", path.display());
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                r#"tell app "Finder" to delete POSIX file "{}""#,
                path.to_string_lossy()
            ))
            .output()
        {
            if output.status.success() {
                log::info!("Moved to Trash: {}", path.display());
                return;
            }
        }
        log::warn!("macOS trash failed for {}, falling back to permanent delete", path.display());
    }

    // Fallback: permanent delete
    let _ = std::fs::remove_file(path);
    log::info!("Permanently deleted: {}", path.display());
}

/// Deletes a downloaded file and updates the record status to "deleted".
pub fn delete_file_and_update_record(
    record_id: &str,
    data_dir: &Path,
) -> Result<DownloadRecord, AppError> {
    // 阶段 1：只读定位（不加锁），先确认记录存在
    let file_path = {
        let records = StorageService::load_download_records(data_dir)?;
        let record = records
            .iter()
            .find(|r| r.id == record_id)
            .ok_or_else(|| AppError::NotFound(format!("下载记录不存在: {}", record_id)))?;
        record.file_path.clone()
    };

    // 阶段 2：物理删除放在写事务之外 —— 可能走回收站或跨设备，耗时不可控
    let file_path = Path::new(&file_path);
    if file_path.exists() {
        delete_file_to_trash(file_path);
    }

    // 阶段 3：事务内按 id 置为 deleted
    StorageService::update_download_records(data_dir, |records| {
        match records.iter_mut().find(|r| r.id == record_id) {
            Some(record) => {
                record.status = "deleted".to_string();
                Ok(record.clone())
            }
            None => Err(AppError::NotFound(format!("下载记录不存在: {}", record_id))),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_files_exist_mixed() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tmp = TempDir::new().unwrap();

        let existing = tmp.path().join("exists.mp4");
        std::fs::write(&existing, b"data").unwrap();

        let missing = tmp.path().join("missing.mp4");

        let paths = vec![
            existing.to_string_lossy().to_string(),
            missing.to_string_lossy().to_string(),
        ];

        let results = rt.block_on(check_files_exist(&paths));
        assert_eq!(results.len(), 2);

        let existing_result = results.iter().find(|r| r.file_path.contains("exists")).unwrap();
        assert!(existing_result.exists);

        let missing_result = results.iter().find(|r| r.file_path.contains("missing")).unwrap();
        assert!(!missing_result.exists);
    }

    #[test]
    fn test_delete_file_and_update_record() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().to_path_buf();

        let file_path = tmp.path().join("video.mp4");
        std::fs::write(&file_path, b"fake video").unwrap();

        let mut record = DownloadRecord::new(
            "sub-1".to_string(),
            "Test Video".to_string(),
            "https://example.com/v".to_string(),
            "vid-1".to_string(),
        );
        record.status = "completed".to_string();
        record.file_path = file_path.to_string_lossy().to_string();

        StorageService::seed_download_records(&data_dir, &[record.clone()]).unwrap();

        let result = delete_file_and_update_record(&record.id, &data_dir).unwrap();
        assert_eq!(result.status, "deleted");
        assert!(!file_path.exists());

        let records = StorageService::load_download_records(&data_dir).unwrap();
        assert_eq!(records[0].status, "deleted");
    }

    #[test]
    fn test_delete_file_already_missing() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().to_path_buf();

        let mut record = DownloadRecord::new(
            "sub-1".to_string(),
            "Test Video".to_string(),
            "https://example.com/v".to_string(),
            "vid-2".to_string(),
        );
        record.status = "completed".to_string();
        record.file_path = "/nonexistent/video.mp4".to_string();

        StorageService::seed_download_records(&data_dir, &[record.clone()]).unwrap();

        let result = delete_file_and_update_record(&record.id, &data_dir).unwrap();
        assert_eq!(result.status, "deleted");
    }

    #[test]
    fn test_delete_file_record_not_found() {
        let tmp = TempDir::new().unwrap();
        StorageService::seed_download_records(tmp.path(), &[]).unwrap();

        let result = delete_file_and_update_record("nonexistent-id", tmp.path());
        assert!(result.is_err());
    }
}
