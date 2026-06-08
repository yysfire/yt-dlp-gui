# Quickstart: 系统托盘图标

**Phase 1** | **Date**: 2026-06-09

## 开发者快速上手

### 1. 理解现有实现

在阅读本计划之前，先熟悉以下文件：

```bash
# 托盘功能的入口——这是添加托盘初始化的位置
src-tauri/src/lib.rs

# 设置模型——这是新增托盘配置字段的位置
src-tauri/src/models/settings.rs

# 调度器——托盘菜单的"暂停/恢复"需要集成
src-tauri/src/services/scheduler.rs (如果存在)

# 现有命令——"检查全部更新"复用现有 check_all
src-tauri/src/commands/download.rs

# 前端设置页面——添加托盘行为开关
src/components/SettingsDialog.tsx

# 前端类型定义——新增托盘相关类型
src/types/index.ts
src/lib/tauri.ts
```

### 2. 实现顺序（推荐）

遵循 TDD 和从底层到高层、从 Rust 到前端的顺序：

```
Phase 2 / 第 1 步: Rust 数据模型
  ├── 在 AppSettings 中新增 4 个字段
  ├── 更新 serde default 逻辑（minimize_to_tray/close_to_tray 默认 true）
  └── 运行 cargo test 验证序列化/默认值

Phase 2 / 第 2 步: TrayService
  ├── 创建 src-tauri/src/services/tray.rs
  ├── 定义 TrayStatus 枚举、TrayState 结构体
  ├── 实现 TrayService::init() / update_state() / cleanup()
  └── 编写单元测试

Phase 2 / 第 3 步: lib.rs 集成
  ├── setup() 中初始化 TrayService
  ├── 注册 on_window_event（关闭到托盘）
  ├── 注册 on_menu_event（托盘菜单操作）
  └── 注册 on_tray_icon_event（左键单击）

Phase 2 / 第 4 步: IPC 命令
  ├── 新增 get_tray_state 命令
  ├── 扩展 get_settings/update_settings 支持新字段
  └── 编写单元测试

Phase 2 / 第 5 步: 前端
  ├── 扩展 SettingsDialog.tsx：托盘行为设置区
  ├── 扩展 tauri.ts：getTrayState
  ├── 扩展 types/index.ts：新字段类型
  └── 手动验证交互流程
```

### 3. 关键测试点

| 测试文件 | 测试内容 |
|----------|----------|
| `settings.rs` (models) | 新增字段的 serde 默认值、往返序列化 |
| `tray.rs` (services) | TrayStatus 状态转换逻辑、tooltip 文本生成、overlay 图标选择逻辑 |
| `download.rs` (commands) | get_tray_state 命令返回值正确性 |

### 4. 验证命令

```bash
# Rust 类型检查
cargo check

# Rust 单元测试
cargo test

# TypeScript 类型检查
npx tsc --noEmit

# 完整开发构建
npm run tauri dev
```

### 5. 平台特定注意事项

| 平台 | 注意事项 |
|------|----------|
| **Windows 10+** | 托盘图标在通知区域，右键菜单为标准 Win32 菜单 |
| **macOS 11+** | 托盘图标在菜单栏（不占用 Dock 旁空间），图标建议 18x18 @2x |
| **Linux (GNOME/KDE)** | 通过 `StatusNotifierItem` (libappindicator) 或 X11 系统托盘协议 |
| **Linux (i3/sway)** | Tauri 可能返回错误，需测试并实现降级逻辑 |

### 6. 常见问题

**Q: 托盘图标不出现？**
- 检查 `src-tauri/icons/icon.png` 是否存在
- Windows: 确认图标未隐藏在溢出区域
- Linux: 确认桌面环境支持系统托盘（GNOME Shell 需安装 `AppIndicator` 扩展）

**Q: macOS 上 Dock 图标和菜单栏图标同时存在？**
- 这是预期行为。macOS 上 Tauri 托盘图标在菜单栏，Dock 图标独立。如需隐藏 Dock 图标需通过 `set_activation_policy()` 控制。

**Q: 关闭到托盘不生效？**
- 检查 `AppSettings.close_to_tray` 是否为 `true`
- 检查 `on_window_event` 中是否正确调用了 `api.prevent_close()`
