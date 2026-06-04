use std::path::Path;

use tauri::{Emitter, State};

use crate::services::file_manager::{self, FileExistenceResult};
use crate::services::StorageService;
use crate::AppContext;

/// Opens the parent directory of a file in the system file manager.
/// Uses `std::process::Command` to call the platform-native opener,
/// bypassing the tauri shell plugin scope restrictions.
///
/// On Linux, tries a fallback chain: pcmanfm-qt → pcmanfm → nautilus → dolphin → thunar → gio → xdg-open
/// Explicitly passes DISPLAY and DBUS_SESSION_BUS_ADDRESS to child processes.
#[tauri::command]
pub fn open_in_folder(
    file_path: String,
) -> Result<(), String> {
    let path = Path::new(&file_path);
    let dir = path.parent().unwrap_or(path);

    log::info!("open_in_folder called with file_path={}, dir={}", file_path, dir.display());

    if !dir.exists() {
        return Err(format!("父目录不存在: {}", dir.display()));
    }

    let dir_str = dir.to_string_lossy().to_string();

    #[cfg(target_os = "linux")]
    {
        // Capture session environment variables so child processes can
        // connect to the user's X11 / D-Bus session.
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
        let dbus_addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .ok();

        // Fallback chain — direct file managers first (they connect via X11),
        // then D-Bus-based openers (gio, xdg-open) as last resort.
        let openers: &[&str] = &[
            "pcmanfm-qt", // LXQt native (Qt/X11, no D-Bus needed)
            "pcmanfm",    // LXDE native
            "nautilus",   // GNOME Files
            "dolphin",    // KDE Dolphin
            "thunar",     // XFCE Thunar
            "gio",        // GLib I/O (D-Bus based)
            "xdg-open",   // XDG standard (D-Bus based)
        ];
        for opener in openers {
            log::info!("open_in_folder: trying {} {}", opener, dir_str);
            let mut cmd = std::process::Command::new(opener);
            cmd.arg(&dir_str)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .env("DISPLAY", &display);
            if let Some(ref addr) = dbus_addr {
                cmd.env("DBUS_SESSION_BUS_ADDRESS", addr);
            }
            match cmd.spawn() {
                Ok(mut child) => {
                    // Wait briefly to see if the process exits with an error
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    match child.try_wait() {
                        Ok(Some(status)) if !status.success() => {
                            log::warn!("open_in_folder: {} exited with {}", opener, status);
                            continue;
                        }
                        Ok(None) => {
                            // Still running — success
                            log::info!("open_in_folder: {} is running (pid {})", opener, child.id());
                            return Ok(());
                        }
                        _ => {
                            // Exited successfully or wait error — treat as success
                            log::info!("open_in_folder: {} exited OK", opener);
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    log::warn!("open_in_folder: {} not found: {}", opener, e);
                    continue;
                }
            }
        }
        return Err("无法打开文件夹：未找到可用的文件管理器（已尝试 pcmanfm-qt、pcmanfm、nautilus、dolphin、thunar、gio、xdg-open）".to_string());
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
