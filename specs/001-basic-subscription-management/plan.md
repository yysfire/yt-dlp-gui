# Implementation Plan: 基本订阅管理

**Branch**: `027-subscription-management` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-basic-subscription-management/spec.md`

## Summary

实现 yt-dlp 桌面端订阅管理器的核心订阅管理功能：添加/删除/查看订阅、启用/禁用订阅、手动检查更新、基础订阅分组。这是整个应用的入口功能，所有后续功能依赖此模块。

技术方案：Rust 后端通过 yt-dlp CLI 子进程解析频道信息，JSON 文件持久化存储，React 前端展示订阅列表。复用现有项目架构（commands/services/models 四层结构）。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18
**Primary Dependencies**: Tauri v2, tokio 1.x (full), serde 1.x, uuid 1.x, chrono 0.4, MUI 5, Tailwind CSS 3
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/subscriptions.json`), StorageService 读写
**Testing**: `cargo test` (Rust), `npx tsc --noEmit` (TypeScript)
**Target Platform**: Windows, macOS, Linux 桌面端 (Tauri v2)
**Project Type**: Desktop application (Tauri v2, WebView frontend)
**Performance Goals**: 添加订阅 < 5s, 列表 100+ 项滚动 ≥ 30fps, 状态切换 < 500ms
**Constraints**: 内存 < 200MB, 无外部数据库依赖, 离线期间列表查看仍可用
**Scale/Scope**: 单用户本地应用, 订阅数 < 1000

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 每个命令函数 MUST 先有失败的单元测试 | ✅ 现有代码已有 69 个单元测试 |
| I. TDD | 前端关键交互 MUST 有手动测试验证 | ✅ 订阅添加/删除流程可手动验证 |
| II. 可测试性 | Services 层 MUST 无状态 | ✅ StorageService/YtDlpService 通过参数接收依赖 |
| II. 可测试性 | 前端 MUST 通过 tauri.ts 封装层调用 | ✅ src/lib/tauri.ts 是唯一入口 |
| III. KISS | 优先 JSON 文件存储 | ✅ 使用 subscriptions.json |
| III. KISS | 复用现有组件和 Service | ✅ 复用 StorageService, YtDlpService |
| IV. DRY | 无重复 invoke() 调用 | ✅ 所有前端调用通过 src/lib/tauri.ts |
| V. YAGNI | 不实现当前需求不需要的功能 | ✅ 分组功能 MVP 仅预定义分组 |

**Gate Result**: ✅ ALL PASS — 无需复杂性论证

## Project Structure

### Documentation (this feature)

```text
specs/001-basic-subscription-management/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (IPC contracts)
│   └── subscription-commands.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── commands/
│   └── subscription.rs    # add, delete, get, toggle_pause, update_quality
├── services/
│   ├── storage.rs         # load_save subscriptions
│   └── ytdlp.rs           # parse_channel_info()
├── models/
│   └── subscription.rs    # Subscription struct
└── utils/
    └── error.rs           # AppError (Duplicate, NotFound)

src/
├── components/
│   ├── SubscriptionList.tsx
│   ├── SubscriptionItem.tsx
│   └── AddSubscriptionDialog.tsx
├── hooks/
│   └── useSubscriptions.ts
├── lib/
│   └── tauri.ts            # add/delete/get/toggle_subscription_pause
└── types/
    └── index.ts            # Subscription interface
```

**Structure Decision**: 复用现有项目结构。Command → Service → Model 三层分离，前端 Hooks 管理状态。所有新增代码在已有文件中扩展，无需新增目录。

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。
