use std::path::Path;

use tauri::{Emitter, State};

use crate::services::file_manager::{self, FileExistenceResult};
use crate::services::StorageService;
use crate::AppContext;

/// Opens the parent directory of a file in the system file manager.
#[tauri::command]
pub async fn open_in_folder(
    file_path: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    #[allow(deprecated)]
    use tauri_plugin_shell::ShellExt;

    let path = Path::new(&file_path);
    let dir = path.parent().unwrap_or(path);

    if !dir.exists() {
        return Err(format!("父目录不存在: {}", dir.display()));
    }

    #[allow(deprecated)]
    app_handle
        .shell()
        .open(dir.to_string_lossy().to_string(), None)
        .map_err(|e| e.to_string())
}

/// Batch-checks whether files exist on disk.
#[tauri::command]
pub async fn check_file_existence(
    file_paths: Vec<String>,
) -> Result<Vec<FileExistenceResult>, String> {
    let results = file_manager::check_files_exist(&file_paths).await;
    Ok(results)
}

/// Deletes a downloaded file and marks the record as "deleted".
#[tauri::command]
pub async fn delete_file(
    id: String,
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    file_manager::delete_file_and_update_record(&id, &state.data_dir)
        .map_err(|e| e.to_string())?;

    // Notify frontend to refresh records
    let _ = app_handle.emit("records-changed", ());
    Ok(())
}

/// Triggers a file state sync for all completed download records.
#[tauri::command]
pub async fn sync_file_states(
    state: State<'_, AppContext>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<FileExistenceResult>, String> {
    let records = StorageService::load_download_records(&state.data_dir)
        .map_err(|e| e.to_string())?;

    // Only check completed records that have file paths
    let file_paths: Vec<String> = records
        .iter()
        .filter(|r| r.status == "completed" && !r.file_path.is_empty())
        .map(|r| r.file_path.clone())
        .collect();

    let results = file_manager::check_files_exist(&file_paths).await;

    let _ = app_handle.emit("file-sync-complete", &results);
    Ok(results)
}
