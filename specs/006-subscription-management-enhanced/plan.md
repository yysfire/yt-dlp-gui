# Implementation Plan: 订阅管理增强

**Branch**: `006-subscription-management-enhanced` | **Date**: 2026-06-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/006-subscription-management-enhanced/spec.md`

## Summary

本计划实现规格 006 的四个用户故事：批量导入订阅（OPML/TXT/URL 列表）、订阅健康检查（HTTP HEAD + yt-dlp 验证）、高级筛选排序（平台/状态/分组/健康状态/关键字五维筛选）、订阅详情面板（频道信息 + 分页视频列表）。健康状态将持久化到 Subscription 实体，失效订阅自动暂停调度。

技术方案沿用现有架构模式：前端通过 `src/lib/tauri.ts` 调用新增 Tauri 命令，Rust 后端在 `commands/` 层新增命令并在 `services/` 层实现业务逻辑。不引入新依赖，复用现有 `StorageService`、`YtDlpService`、`OpmlService`。

## Technical Context

**Language/Version**: Rust（Tauri v2）+ TypeScript 5.x / React 18

**Primary Dependencies**: MUI 5, Tailwind CSS 3, Vite 5, tokio 1.x, serde 1.x, yt-dlp CLI

**Storage**: JSON 文件（`~/.yt-dlp-sub-gui/subscriptions.json`、`download_records.json`、`settings.json`）

**Testing**: `cargo test`（Rust 单元测试）+ `npx tsc --noEmit`（TypeScript 类型检查）+ React Testing Library（前端交互测试）

**Target Platform**: Windows / macOS / Linux（Tauri v2 跨平台桌面应用）

**Project Type**: Desktop app（单页应用 + Rust 后端）

**Performance Goals**:
- 50 条批量导入 + 频道解析 < 60 秒（SC-001）
- 50 个订阅健康检查 < 30 秒（SC-002）
- 搜索/筛选响应 < 200ms（SC-003，本地过滤）
- 详情面板加载 < 1 秒（SC-004）
- 批量删除失效订阅 < 2 秒（SC-006）

**Constraints**:
- 筛选仅会话内持久化（assumption 5），不写入 settings.json
- 健康检查需分批执行避免触发平台反爬（edge case）
- yt-dlp 作为外部依赖必须在用户设备上可用
- 不引入新的 Rust 依赖包（复用 reqwest/http/quick-xml）

**Scale/Scope**:
- 典型订阅量 50，最大支持 500+
- 4 个新前端弹窗/面板组件 + 5-7 个新 Tauri 命令
- 约 500 行 Rust 新增代码 + 800 行 TypeScript 新增代码

## Constitution Check

*GATE: 必须通过后才能进入 Phase 0 研究。Phase 1 设计后重新检查。*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 所有新功能必须先编写测试用例（GWT 场景已定义于 spec.md） | ✅ 通过 |
| II. 可测试性 | 新 Services 保持无状态，通过参数注入依赖 | ✅ 通过 |
| III. KISS | 继续使用 JSON 文件存储，不引入数据库；复用现有 Service 模式 | ✅ 通过 |
| IV. DRY | 复用现有 `OpmlService::parse_opml()`、`YtDlpService::parse_channel_info()`；前端 invoke 统一通过 `lib/tauri.ts` | ✅ 通过 |
| V. YAGNI | 不新增依赖包，所有功能均为规格明确要求 | ✅ 通过 |

**结论**: 无违规，通过门禁检查。

## Project Structure

### Documentation (this feature)

```text
specs/006-subscription-management-enhanced/
├── plan.md              # 本文件
├── research.md          # Phase 0 输出
├── data-model.md        # Phase 1 输出
├── quickstart.md        # Phase 1 输出
├── contracts/           # Phase 1 输出（Tauri 命令契约）
│   ├── subscription-commands.md
│   ├── health-check-commands.md
│   ├── import-export-commands.md
│   └── filter-commands.md
└── tasks.md             # Phase 2 输出（/speckit.tasks 生成）
```

### Source Code (repository root)

```text
src-tauri/src/
├── commands/
│   ├── subscription.rs      # 新增：batch_delete、update_health_status、get_tags
│   ├── health_check.rs      # 新增：check_subscription_health、check_all_health、check_selected_health
│   ├── import_export.rs     # 扩展：batch_import_with_progress 增强
│   └── download.rs          # 扩展：调度器排除失效订阅
├── services/
│   ├── storage.rs           # 扩展：标签 CRUD、健康状态持久化
│   ├── ytdlp.rs             # 扩展：视频列表分页获取（--playlist-end）
│   ├── opml.rs              # 复用：OPML 解析/导出
│   └── health.rs            # 新增：健康检查服务（HTTP HEAD + yt-dlp 验证）
├── models/
│   ├── subscription.rs      # 扩展：增加 health_status、last_health_check、tags 字段
│   ├── download.rs          # 不变
│   ├── settings.rs          # 不变
│   └── health.rs            # 新增：HealthCheckResult、ImportProgress 等
└── utils/
    └── error.rs             # 扩展：新增 HealthCheckError、ImportError

src/
├── components/
│   ├── ImportDialog.tsx         # 扩展：OPML/TXT/URL 批量导入 + 进度
│   ├── HealthCheckPanel.tsx     # 新增：健康检查结果面板
│   ├── FilterBar.tsx            # 新增：五维筛选条件栏
│   └── DetailPanel.tsx          # 扩展：频道详情 + 分页视频列表
├── hooks/
│   ├── useSubscriptions.ts      # 扩展：标签管理、健康状态更新
│   ├── useFilter.ts             # 新增：筛选条件管理（会话内持久化）
│   └── useHealthCheck.ts        # 新增：健康检查状态管理
├── lib/
│   └── tauri.ts                 # 扩展：新增 invoke 命令封装
└── types/
    └── index.ts                 # 扩展：新增类型定义
```

**Structure Decision**: 沿用现有前后端分离架构。前端新组件通过 props 从 App.tsx 接收状态。Rust 后端在 commands/ 和 services/ 层新增模块，遵循"Commands 不包含业务逻辑"的设计规则。

## Complexity Tracking

> 无违反章程的项，无需记录。
