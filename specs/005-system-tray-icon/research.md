# Research: 系统托盘图标

**Phase 0** | **Date**: 2026-06-09

## 1. Tauri v2 托盘 API 使用方式

### Decision
**使用 Tauri v2 的 `tauri::tray::TrayIconBuilder` + `on_menu_event` + `on_tray_icon_event` API，在 `lib.rs::setup()` 中初始化。**

### Rationale
Tauri v2 提供了完整的托盘 API（`tauri::tray` 模块）：

- **`TrayIconBuilder::new()`** — 创建托盘图标实例，设置图标文件、tooltip、菜单
- **`.on_menu_event()`** — 处理右键菜单项点击
- **`.on_tray_icon_event()`** — 处理左键单击/双击事件（`TrayIconEvent::Click` / `DoubleClick`）
- **`.build(app_handle)`** — 构建并注册托盘图标
- **`tray_icon.set_tooltip()`** — 运行时更新 tooltip 文本
- **`tray_icon.set_icon()`** — 运行时更换图标
- **`tray_icon.set_menu()`** — 运行时更新菜单（用于动态切换"暂停/恢复"项）
- **`tray_icon.set_visible()`** — 运行时显示/隐藏图标（平台不支持时用于降级）

框架权限 `core:tray:default` 已在项目生成配置中就绪（`src-tauri/gen/schemas/*-schema.json`），无需额外 Cargo features。

```rust
// 核心创建模式
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};

let menu = MenuBuilder::new(app)
    .item(&MenuItemBuilder::with_id("show", "显示主窗口").build(app)?)
    .separator()
    .item(&MenuItemBuilder::with_id("check_all", "检查全部订阅更新").build(app)?)
    .item(&MenuItemBuilder::with_id("toggle_scheduler", "暂停定时检查").build(app)?)
    .separator()
    .item(&MenuItemBuilder::with_id("quit", "退出").build(app)?)
    .build()?;

let tray = TrayIconBuilder::new()
    .icon(app.default_window_icon().unwrap().clone())
    .tooltip("yt-dlp 订阅管理器 - 空闲")
    .menu(&menu)
    .on_menu_event(|app, event| {
        match event.id.as_ref() {
            "show" => { /* 切换窗口可见性 */ }
            "check_all" => { /* 调用 check_all 命令 */ }
            "toggle_scheduler" => { /* 暂停/恢复调度器 */ }
            "quit" => { /* 退出流程 */ }
            _ => {}
        }
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event {
            // 切换窗口显示/隐藏
        }
    })
    .build(app)?;
```

### Alternatives Considered
- 使用第三方 crate（如 `tray-icon`）→ 已排除，Tauri v2 原生支持
- 前端通过 `@tauri-apps/api/tray` 管理 → 已排除，托盘是 Rust 端独占能力，不暴露给前端直接操作

## 2. 窗口事件处理（关闭到托盘）

### Decision
**在 `lib.rs::setup()` 中注册 `on_window_event` 回调，拦截 `close_requested` 事件。**

### Rationale
Tauri v2 的 `WindowEvent::CloseRequested { api, .. }` 允许在关闭窗口时阻止默认行为：

```rust
window.on_window_event(|event| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if close_to_tray_enabled {
            api.prevent_close();  // 阻止关闭
            window.hide().unwrap(); // 隐藏窗口到托盘
        }
    }
});
```

窗口最小化事件的拦截逻辑类似，可在 `on_window_event` 中处理。

`close_to_tray` 和 `minimize_to_tray` 配置值从 `AppContext` 读取，每次事件触发时重新从 `AppSettings` 获取（通过 `StorageService::load_settings()` 或内存中的缓存值）。

### Alternatives Considered
- 使用 `#[cfg(target_os)]` 条件编译 → 已排除，托盘行为应在所有平台上统一
- 前端处理 `close_requested` → **可行性待验证**。Tauri v2 的窗口事件可在前端通过 `getCurrentWindow().onCloseRequested()` 监听，但 `api.prevent_close()` 可能仅 Rust 端可用。

## 3. 托盘图标状态 overlay 实现

### Decision
**准备 3 个图标文件，通过 `tray.set_icon()` 在运行时切换。**

### Rationale
Tauri v2 的 `TrayIconBuilder::icon()` 和 `TrayIcon::set_icon()` 接受 `tauri::image::Image` 类型。准备 3 个 PNG 图标文件：

- `src-tauri/icons/tray-idle.png` — 空闲状态（默认图标）
- `src-tauri/icons/tray-downloading.png` — 下载中（叠加下载箭头）
- `src-tauri/icons/tray-checking.png` — 检查中（叠加刷新标记）

状态切换时机由 `services/tray.rs` 管理，通过 `TrayState` 枚举驱动：

```rust
pub enum TrayState {
    Idle,
    Downloading { active_count: u32 },
    Checking,
}
```

图标文件在 `build.rs` 中通过 `include_bytes!` 或 `tauri::image::Image::from_bytes()` 加载到内存。每次状态变化时调用 `tray.set_icon(Some(image))` 更新。

tooltip 文本通过 `tray.set_tooltip(Some(text))` 同步更新。

### Alternatives Considered
- 运行时合成图标（像素级操作）→ 已排除，增加 PNG 编解码依赖（YAGNI）
- 使用系统通知替代图标变化 → 已排除，通知是独立功能

## 4. macOS 菜单栏与 Dock 图标交互

### Decision
**在 macOS 上，托盘图标置于菜单栏（menu bar），Dock 图标行为不受托盘影响。**

### Rationale
Tauri v2 在 macOS 上自动将 `TrayIcon` 渲染为菜单栏图标（系统托盘区域为 `NSStatusBar`），这是正确的平台行为。Dock 图标的显示/隐藏需通过 `app.set_activation_policy()` 控制：

- 默认：`NSApplicationActivationPolicyRegular`（Dock 可见）
- 最小化到托盘时：若用户期望完全隐藏，可设为 `Accessory`（Dock 不显示），但不建议默认这么做（规格未要求隐藏 Dock 图标）

macOS 菜单栏图标推荐大小：16x16 或 18x18 像素（非 Retina），需为 Retina 显示准备 2x（36x36）和 3x（54x54）版本。

## 5. 集成现有调度器（暂停/恢复）

### Decision
**复用现有的 `tokio::sync::watch` 通知机制和 `AppState.scheduler_running` 标记。**

### Rationale
根据 004-essential-settings 的 research（research.md §1），当前调度器已使用 `tokio::sync::watch` + `tokio::select!` 模式：

- `AppContext` 持有 `watch::Sender<()>` （`scheduler_notify`）
- 调度器 task 通过 `rx.changed()` 监听变更通知
- 收到通知后重新从磁盘读取状态

托盘菜单的"暂停/恢复"操作流程：

1. 菜单事件触发 → 调用 `toggle_scheduler_pause()`
2. 更新 `AppSettings.scheduler_paused: bool` 并持久化
3. 通过 `watch::Sender::send(())` 通知调度器 task
4. 调度器收到通知后读取 `scheduler_paused`，决定是否跳过本周期执行
5. 更新托盘菜单项文本："暂停定时检查" ↔ "恢复定时检查"

若当前无 `scheduler_paused` 字段，需在 `AppSettings` 中新增此布尔字段。

### Alternatives Considered
- 直接 kill/restart tokio task → 已排除，太重且复杂
- 使用 `tokio::sync::Notify` → 已排除，已有 `watch` 机制

## 6. 桌面环境托盘支持检测

### Decision
**在 Linux 上通过尝试创建托盘图标来检测环境支持，失败时设置降级标志。**

### Rationale
Tauri v2 在 Linux 上通过 GTK 的 `StatusNotifierItem` (libappindicator) 或 X11 系统托盘协议提供托盘支持。部分平铺窗口管理器（i3、sway、dwm）不提供系统托盘协议或需要额外配置。

检测方式：
1. 尝试 `TrayIconBuilder::build()` — 若 Tauri 检测到环境不支持，可能会返回错误或创建失败
2. 捕获错误后，设置 `tray_supported: false` 标志
3. 降级模式：不创建托盘图标，"关闭到托盘"行为退化为"直接退出"，最小化正常执行但不隐藏到托盘

**需要在不同 Linux 桌面环境中实际测试确认 Tauri v2 的行为**（当前没有这些环境的测试环境）。

### Alternatives Considered
- 预检测环境变量（XDG_CURRENT_DESKTOP, WAYLAND_DISPLAY）→ 不可靠且复杂
- 完全不做检测 → 已排除，规格 FR-013 要求降级处理

## 7. 退出时保存托盘状态

### Decision
**在 `lib.rs::setup()` 的 `on_menu_event` 退出处理中和 `on_window_event` 中保存状态。**

### Rationale
现有的退出流程可能在 `commands` 层有部分处理。托盘退出需添加：

1. 标记 `AppState.is_shutting_down = true`（防止退出过程中的新任务）
2. 保存当前调度器状态到持久化存储
3. 清理图标资源（tray_icon 的 Drop 由 Rust 自动处理，但显式调用可确保资源释放）
4. 调用 `app.exit(0)`

通过关机/强制终止信号退出时，Tauri 的 `RunEvent::Exit` 事件可拦截并执行状态保存。

参见现有 `lib.rs::setup()` 中 `app.on_event(|app, event| { ... })` 的生命周期事件处理（如已有实现）。
