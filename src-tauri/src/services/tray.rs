//! System tray icon service.
//!
//! Manages the lifecycle of the system tray icon: creation, menu events,
//! state updates (idle/downloading/checking), and cleanup.
//!
//! See `specs/005-system-tray-icon/spec.md` for full feature specification.

use serde::Serialize;
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::menu::{
    Menu, MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder, PredefinedMenuItem,
};
use tauri::{AppHandle, Emitter, Manager, Wry};
use std::sync::Mutex;

use crate::services::StorageService;
use crate::utils::error::AppError;
use std::path::PathBuf;

macro_rules! include_icon {
    ($path:literal) => {
        include_bytes!(concat!("../../icons/", $path))
    };
}

/// Tray icon runtime status.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TrayStatus {
    Idle,
    Downloading { active_count: u32 },
    Checking,
}

/// Runtime state of the system tray icon.
#[derive(Debug, Clone, Serialize)]
pub struct TrayState {
    pub status: TrayStatus,
    pub window_visible: bool,
    pub tray_supported: bool,
    pub active_downloads: u32,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            status: TrayStatus::Idle,
            window_visible: true,
            tray_supported: true,
            active_downloads: 0,
        }
    }
}

impl TrayState {
    pub fn set_status(&mut self, new_status: TrayStatus) -> Result<(), String> {
        match (&self.status, &new_status) {
            (_, TrayStatus::Idle) => {}
            (TrayStatus::Idle, TrayStatus::Downloading { .. }) => {}
            (TrayStatus::Idle, TrayStatus::Checking) => {}
            (TrayStatus::Downloading { .. }, TrayStatus::Checking) => {
                return Err("Cannot transition from Downloading to Checking".into());
            }
            (TrayStatus::Checking, TrayStatus::Downloading { .. }) => {
                return Err("Cannot transition from Checking to Downloading".into());
            }
            (TrayStatus::Downloading { .. }, TrayStatus::Downloading { active_count }) => {
                self.active_downloads = *active_count;
                self.status = new_status;
                return Ok(());
            }
            _ => {}
        }
        if let TrayStatus::Downloading { active_count } = &new_status {
            self.active_downloads = *active_count;
        } else {
            self.active_downloads = 0;
        }
        self.status = new_status;
        Ok(())
    }

    pub fn tooltip_text(&self) -> String {
        match &self.status {
            TrayStatus::Idle => "yt-dlp 订阅管理器 - 空闲".to_string(),
            TrayStatus::Downloading { active_count } => {
                format!("正在下载 {} 个视频", active_count)
            }
            TrayStatus::Checking => "正在检查订阅更新...".to_string(),
        }
    }

    pub fn window_menu_text(&self) -> &str {
        if self.window_visible {
            "隐藏主窗口"
        } else {
            "显示主窗口"
        }
    }

    pub fn scheduler_menu_text(scheduler_paused: bool) -> &'static str {
        if scheduler_paused {
            "恢复定时检查"
        } else {
            "暂停定时检查"
        }
    }
}

/// Manages the system tray icon lifecycle and event handling.
///
/// Uses `tauri::Wry` as the concrete runtime for desktop platforms.
pub struct TrayService {
    pub tray_icon: Option<TrayIcon<Wry>>,
    pub state: Mutex<TrayState>,
    pub menu_show_item: MenuItem<Wry>,
    pub menu_scheduler_item: MenuItem<Wry>,
    pub data_dir: PathBuf,
}

impl TrayService {
    fn build_menu(
        app: &AppHandle,
    ) -> Result<(Menu<Wry>, MenuItem<Wry>, MenuItem<Wry>), Box<dyn std::error::Error>> {
        let show_item = MenuItemBuilder::with_id("tray_show", "显示主窗口").build(app)?;
        let separator1 = PredefinedMenuItem::separator(app)?;
        let check_all_item =
            MenuItemBuilder::with_id("tray_check_all", "检查全部订阅更新").build(app)?;
        let scheduler_item =
            MenuItemBuilder::with_id("tray_toggle_scheduler", "暂停定时检查").build(app)?;
        let separator2 = PredefinedMenuItem::separator(app)?;
        let quit_item = MenuItemBuilder::with_id("tray_quit", "退出").build(app)?;

        let menu = MenuBuilder::new(app)
            .item(&show_item)
            .item(&separator1)
            .item(&check_all_item)
            .item(&scheduler_item)
            .item(&separator2)
            .item(&quit_item)
            .build()?;

        Ok((menu, show_item, scheduler_item))
    }

    /// Initialize the system tray icon.
    pub fn init(
        app: &AppHandle,
        data_dir: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let icon = app
            .default_window_icon()
            .ok_or_else(|| {
                AppError::TrayInitialization("Failed to get default window icon".into())
            })?
            .clone();

        let (menu, show_item, scheduler_item) = Self::build_menu(app)?;

        let tray_result = TrayIconBuilder::new()
            .icon(icon)
            .tooltip("yt-dlp 订阅管理器 - 空闲")
            .menu(&menu)
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let app_handle = tray.app_handle();
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let is_visible = window.is_visible().unwrap_or(true);
                        if is_visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            })
            .build(app);

        match tray_result {
            Ok(tray_icon) => {
                log::info!("Tray icon created successfully");
                Ok(TrayService {
                    tray_icon: Some(tray_icon),
                    state: Mutex::new(TrayState::default()),
                    menu_show_item: show_item,
                    menu_scheduler_item: scheduler_item,
                    data_dir,
                })
            }
            Err(e) => {
                log::warn!(
                    "System tray not supported on this desktop environment: {}",
                    e
                );
                Ok(TrayService {
                    tray_icon: None,
                    state: Mutex::new(TrayState {
                        tray_supported: false,
                        ..Default::default()
                    }),
                    menu_show_item: show_item,
                    menu_scheduler_item: scheduler_item,
                    data_dir,
                })
            }
        }
    }

    /// Handle menu events from the tray context menu.
    pub fn handle_menu_event(&self, app: &AppHandle, event: MenuEvent) {
        let id = event.id().0.as_str();
        log::info!("Tray menu event: {}", id);

        match id {
            "tray_show" => self.toggle_window(app),
            "tray_check_all" => self.handle_check_all(app),
            "tray_toggle_scheduler" => self.handle_toggle_scheduler(),
            "tray_quit" => self.handle_quit(app),
            _ => log::warn!("Unknown tray menu event: {}", id),
        }
    }

    fn toggle_window(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let is_visible = window.is_visible().unwrap_or(true);
            if is_visible {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
            if let Ok(mut state) = self.state.lock() {
                state.window_visible = !is_visible;
                let text = state.window_menu_text();
                let _ = self.menu_show_item.set_text(text);
            }
        }
    }

    fn handle_check_all(&self, app: &AppHandle) {
        let app_handle = app.clone();
        let data_dir = self.data_dir.clone();
        tauri::async_runtime::spawn(async move {
            let subs =
                StorageService::load_subscriptions(&data_dir).unwrap_or_default();
            let subs: Vec<_> = subs.into_iter().filter(|s| !s.paused).collect();
            if subs.is_empty() {
                return;
            }
            let records =
                StorageService::load_download_records(&data_dir).unwrap_or_default();
            let settings = StorageService::load_settings(&data_dir);
            let yt_dlp_path = settings.yt_dlp_path.clone();
            let proxy = Some(settings.proxy_url.clone());
            let cookie_file = Some(settings.cookie_file.clone());
            let download_dir = std::path::PathBuf::from(&settings.download_dir);

            for sub in &subs {
                let _ = crate::commands::download::check_and_download(
                    sub,
                    &yt_dlp_path,
                    &proxy,
                    &cookie_file,
                    &download_dir,
                    &data_dir,
                    &None,
                    &records,
                    &app_handle,
                )
                .await;
            }
            let _ = app_handle.emit("scheduler-check-complete", ());
        });
    }

    fn handle_toggle_scheduler(&self) {
        let settings = StorageService::load_settings(&self.data_dir);
        let new_paused = !settings.scheduler_paused;
        let mut updated = settings;
        updated.scheduler_paused = new_paused;
        let _ = StorageService::save_settings(&self.data_dir, &updated);

        let text = TrayState::scheduler_menu_text(new_paused);
        let _ = self.menu_scheduler_item.set_text(text);

        log::info!("Scheduler toggled via tray menu: paused={}", new_paused);
    }

    fn handle_quit(&self, app: &AppHandle) {
        let has_active = if let Ok(state) = self.state.lock() {
            matches!(state.status, TrayStatus::Downloading { .. })
        } else {
            false
        };
        if has_active {
            log::info!("Active downloads detected during tray quit, exiting per user request");
        }
        log::info!("Tray quit: exiting application");
        app.exit(0);
    }

    /// Hide the main window (minimize to tray).
    pub fn hide_window(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
            if let Ok(mut state) = self.state.lock() {
                state.window_visible = false;
                let text = state.window_menu_text();
                let _ = self.menu_show_item.set_text(text);
            }
        }
    }

    /// Show the main window.
    pub fn show_window(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
            if let Ok(mut state) = self.state.lock() {
                state.window_visible = true;
                let text = state.window_menu_text();
                let _ = self.menu_show_item.set_text(text);
            }
        }
    }

    /// Update tray visual status (icon + tooltip).
    pub fn update_status(
        &self,
        app: &AppHandle,
        new_status: TrayStatus,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.set_status(new_status)?;

        if let Some(ref tray) = self.tray_icon {
            let tooltip = state.tooltip_text();
            let _ = tray.set_tooltip(Some(&tooltip));

            let icon_bytes: &[u8] = match &state.status {
                TrayStatus::Idle => include_icon!("tray-idle.png"),
                TrayStatus::Downloading { .. } => include_icon!("tray-downloading.png"),
                TrayStatus::Checking => include_icon!("tray-checking.png"),
            };
            match tauri::image::Image::from_bytes(icon_bytes) {
                Ok(image) => {
                    let _ = tray.set_icon(Some(image));
                }
                Err(e) => {
                    log::error!("Failed to load tray icon image: {}", e);
                }
            }
        }

        let _ = app.emit("tray-state-changed", &*state);
        log::info!("Tray status updated: tooltip={}", state.tooltip_text());
        Ok(())
    }

    pub fn is_supported(&self) -> bool {
        self.tray_icon.is_some()
    }

    pub fn update_scheduler_menu_text(&self, paused: bool) {
        let text = TrayState::scheduler_menu_text(paused);
        let _ = self.menu_scheduler_item.set_text(text);
    }

    pub fn cleanup(&self) {
        log::info!("Tray service cleanup");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tray_state() {
        let state = TrayState::default();
        assert_eq!(state.status, TrayStatus::Idle);
        assert!(state.window_visible);
        assert!(state.tray_supported);
        assert_eq!(state.active_downloads, 0);
    }

    #[test]
    fn test_status_transition_idle_to_downloading() {
        let mut state = TrayState::default();
        assert!(state.set_status(TrayStatus::Downloading { active_count: 3 }).is_ok());
        assert_eq!(state.status, TrayStatus::Downloading { active_count: 3 });
        assert_eq!(state.active_downloads, 3);
    }

    #[test]
    fn test_status_transition_downloading_to_idle() {
        let mut state = TrayState::default();
        state.set_status(TrayStatus::Downloading { active_count: 2 }).unwrap();
        assert!(state.set_status(TrayStatus::Idle).is_ok());
        assert_eq!(state.status, TrayStatus::Idle);
    }

    #[test]
    fn test_status_transition_downloading_to_checking_invalid() {
        let mut state = TrayState::default();
        state.set_status(TrayStatus::Downloading { active_count: 1 }).unwrap();
        assert!(state.set_status(TrayStatus::Checking).is_err());
    }

    #[test]
    fn test_tooltip_idle() {
        assert_eq!(TrayState::default().tooltip_text(), "yt-dlp 订阅管理器 - 空闲");
    }

    #[test]
    fn test_tooltip_downloading() {
        let mut state = TrayState::default();
        state.set_status(TrayStatus::Downloading { active_count: 3 }).unwrap();
        assert_eq!(state.tooltip_text(), "正在下载 3 个视频");
    }

    #[test]
    fn test_tooltip_checking() {
        let mut state = TrayState::default();
        state.set_status(TrayStatus::Checking).unwrap();
        assert_eq!(state.tooltip_text(), "正在检查订阅更新...");
    }

    #[test]
    fn test_window_menu_text() {
        let mut state = TrayState::default();
        assert_eq!(state.window_menu_text(), "隐藏主窗口");
        state.window_visible = false;
        assert_eq!(state.window_menu_text(), "显示主窗口");
    }

    #[test]
    fn test_scheduler_menu_text() {
        assert_eq!(TrayState::scheduler_menu_text(false), "暂停定时检查");
        assert_eq!(TrayState::scheduler_menu_text(true), "恢复定时检查");
    }
}
