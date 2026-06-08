# Implementation Plan: 系统托盘图标

**Branch**: `005-system-tray-icon` | **Date**: 2026-06-09 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-system-tray-icon/spec.md`

## Summary

实现应用的系统托盘图标功能：应用启动后在系统托盘区域显示图标，支持最小化/关闭到托盘、右键菜单快捷操作（显示主窗口、检查全部更新、暂停/恢复定时检查、退出）、托盘图标状态指示（空闲/下载中/检查中，通过 overlay 徽标体现）、以及用户可配置的托盘行为（最小化到托盘、关闭到托盘、启动时最小化）。

技术方案：使用 Tauri v2 的 `tauri::tray::TrayIconBuilder` API 创建托盘图标，通过 `on_menu_event` 处理右键菜单操作，通过 `on_tray_icon_event` 处理左键单击。窗口关闭行为通过拦截 `close_requested` 事件实现"关闭到托盘"。托盘配置项新增到 `AppSettings` 中。状态更新通过 `set_tooltip` 和 `set_icon` 动态刷新。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18  
**Primary Dependencies**: Tauri v2 (`tauri::tray::TrayIconBuilder`), tokio 1.x (full), serde 1.x, MUI 5  
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/settings.json`), 新增 3 个托盘配置字段  
**Testing**: `cargo test` (Rust), `npx tsc --noEmit` (TypeScript)  
**Target Platform**: Windows 10+, macOS 11+, Linux (GNOME, KDE, XFCE) 桌面端 (Tauri v2)  
**Project Type**: Desktop application (Tauri v2, WebView frontend)  
**Performance Goals**: 托盘图标启动 2s, 菜单弹出 300ms, 窗口恢复 500ms, 状态更新 1s  
**Constraints**: 单用户本地应用, 部分 Linux DE 不支持托盘时降级为普通窗口模式  
**Scale/Scope**: 单窗口应用, 3 个托盘状态, 5 个菜单项, 3 个托盘配置开关

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 托盘图标创建逻辑 MUST 先有单元测试 | ✅ 可测试 TrayIconBuilder 配置正确性 |
| I. TDD | 菜单事件处理逻辑 MUST 先有测试 | ✅ 菜单事件处理函数为纯逻辑，可独立测试 |
| I. TDD | 托盘配置持久化 MUST 先有测试 | ✅ 扩展现有 `AppSettings` 序列化测试 |
| II. 可测试性 | 托盘配置与 Tauri 运行时解耦 | ✅ 配置读写通过 StorageService 抽象 |
| II. 可测试性 | 状态更新逻辑可独立测试 | ✅ `TrayState` 枚举及转换逻辑为纯数据 |
| III. KISS | 托盘图标使用现有应用图标 | ✅ 复用 `src-tauri/icons/icon.png` |
| III. KISS | overlay 状态使用简单图标文件 | ✅ 每状态一个图标文件，运行时切换 |
| IV. DRY | 右键菜单事件复用现有命令 | ✅ "检查全部更新"→`check_all`，"暂停/恢复"→调度器 control |
| IV. DRY | 退出逻辑复用现有流程 | ✅ 复用 `on_exit` 中的状态保存逻辑 |
| V. YAGNI | 不实现自定义托盘图标主题 | ✅ 规格未要求 |
| V. YAGNI | 不实现托盘通知/气泡提示 | ✅ 通知为已有独立功能（clarifications Q1） |

**Gate Result**: ✅ ALL PASS — 无需复杂性论证

## Project Structure

### Documentation (this feature)

```text
specs/005-system-tray-icon/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── tray-commands.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── commands/
│   ├── settings.rs        # [扩展] 新增托盘配置字段的序列化/反序列化
│   ├── download.rs        # [引用] 托盘状态更新时读取活跃下载数
│   └── mod.rs
├── services/
│   ├── tray.rs            # [新增] 托盘管理 Service：创建、状态更新、清理
│   ├── storage.rs         # [复用] 托盘配置的 JSON 读写
│   └── mod.rs             # [扩展] 声明 tray 模块
├── models/
│   ├── settings.rs        # [扩展] 新增 minimize_to_tray, close_to_tray, start_in_tray 字段
│   └── mod.rs
├── utils/
│   └── error.rs           # [扩展] 新增 TrayError 错误变体
├── lib.rs                 # [扩展] setup() 阶段初始化托盘，注册窗口事件处理
└── main.rs                # [不变]

src/
├── components/
│   └── SettingsDialog.tsx # [扩展] 新增托盘行为设置区（3 个开关）
├── lib/
│   └── tauri.ts           # [扩展] 新增托盘相关 invoke 调用和事件监听
└── types/
    └── index.ts           # [扩展] AppSettings 新增 3 个托盘字段
```

**Structure Decision**: 新增 `services/tray.rs` Service 封装 Tauri v2 托盘 API，与现有 `services/` 层同行。托盘配置作为 `AppSettings` 的扩展字段，不创建独立实体。前端 `SettingsDialog.tsx` 新增"托盘行为"设置区。托盘菜单操作复用现有命令处理函数。

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。
