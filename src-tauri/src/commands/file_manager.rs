use std::path::Path;

use tauri::{Emitter, State};

use crate::services::file_manager::{self, FileExistenceResult};
use crate::services::StorageService;
use crate::AppContext;

/// Opens the parent directory of a file in the system file manager.
/// Uses `std::process::Command` to call the platform-native opener,
/// bypassing the tauri shell plugin scope restrictions.
///
/// On Linux, tries a fallback chain: gio open → xdg-open → nautilus → dolphin → thunar → pcmanfm
#[tauri::command]
pub fn open_in_folder(
    file_path: String,
) -> Result<(), String> {
    let path = Path::new(&file_path);
    let dir = path.parent().unwrap_or(path);

    if !dir.exists() {
        return Err(format!("父目录不存在: {}", dir.display()));
    }

    let dir_str = dir.to_string_lossy().to_string();

    #[cfg(target_os = "linux")]
    {
        // Fallback chain: try multiple file-openers in order
        let openers: &[&str] = &[
            "gio",     // GLib I/O — works on most GTK-based desktops
            "xdg-open",// XDG standard — works when MIME types are configured
            "nautilus",// GNOME Files
            "dolphin", // KDE Dolphin
            "thunar",  // XFCE Thunar
            "pcmanfm", // LXDE/LXQt PCManFM
        ];
        for opener in openers {
            let status = std::process::Command::new(opener)
                .arg(&dir_str)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            match status {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
        return Err("无法打开文件夹：未找到可用的文件管理器（已尝试 gio、xdg-open、nautilus、dolphin、thunar、pcmanfm）".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("无法打开文件夹: {}", e))?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return Err("不支持的操作系统".to_string());
    }

    Ok(())
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
