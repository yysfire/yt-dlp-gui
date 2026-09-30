//! System tray icon service.
//!
//! Manages the lifecycle of the system tray icon: creation and menu events.
//!
//! See `specs/005-system-tray-icon/spec.md` for full feature specification.

use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::menu::{
    Menu, MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder, PredefinedMenuItem,
};
use tauri::{AppHandle, Emitter, Manager, Wry};
use std::sync::Mutex;

use crate::services::StorageService;
use crate::utils::error::AppError;
use crate::AppContext;
use std::path::PathBuf;

/// Runtime state of the system tray icon.
#[derive(Debug, Clone)]
pub struct TrayState {
    pub window_visible: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            window_visible: true,
        }
    }
}

impl TrayState {
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
                    state: Mutex::new(TrayState::default()),
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
            "tray_toggle_scheduler" => self.handle_toggle_scheduler(app),
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
            // 与自动调度共用同一轮检查逻辑：会写回 last_checked_at 等字段并 emit 事件
            if let Err(e) = crate::commands::download::run_check_round(&data_dir, &app_handle).await {
                log::error!("tray: check round failed: {}", e);
            }
        });
    }

    fn handle_toggle_scheduler(&self, app: &AppHandle) {
        let ctx = app.state::<AppContext>();

        // 1) 读盘取反（不持任何应用级锁）
        let mut updated = StorageService::load_settings(&self.data_dir);
        let new_paused = !updated.scheduler_paused;
        updated.scheduler_paused = new_paused;

        // 2) 先落盘：save_settings 内部只拿全局写锁，并在返回前释放
        if let Err(e) = StorageService::save_settings(&self.data_dir, &updated) {
            log::error!("Tray: persist scheduler_paused failed: {}", e);
            // 写盘失败就不动缓存与菜单，避免内存与磁盘分叉
            return;
        }

        // 3) 再同步运行时缓存（独立 Mutex，用完即释放）
        if let Ok(mut cached) = ctx.settings.lock() {
            cached.scheduler_paused = new_paused;
        }

        // 广播设置变更（契约：凡写 settings 的路径都必须 emit 本事件）。
        // 此刻磁盘与缓存都已生效，且不持任何应用级锁。
        let _ = app.emit("settings-changed", &updated);

        // 4) 唤醒调度循环，使「恢复定时检查」立即生效而不是等下一个间隔
        let _ = ctx.scheduler_notify.send(());

        // 5) 更新菜单文案
        let text = TrayState::scheduler_menu_text(new_paused);
        let _ = self.menu_scheduler_item.set_text(text);

        log::info!("Scheduler toggled via tray menu: paused={}", new_paused);
    }

    fn handle_quit(&self, app: &AppHandle) {
        // 托盘状态更新从未实现（set_status/update_status 无调用方），故原先「有活跃下载时
        // 再退出」的判断恒为 false、从未生效，已移除。
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

    pub fn is_supported(&self) -> bool {
        self.tray_icon.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tray_state() {
        let state = TrayState::default();
        assert!(state.window_visible);
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
