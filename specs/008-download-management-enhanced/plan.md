# Implementation Plan: 下载管理增强（P1）

**Branch**: `008-download-management-enhanced`（⚠️ 实际 git 分支仍为 `master` —— 见「已知偏差」） | **Date**: 2026-10-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-download-management-enhanced/spec.md`

> 📌 **决策细节一律不在此重复。** 分类器逐条匹配式、重试状态转移表、16 行真值表、字段与命令契约、UI 形态全部在 **[decisions.md](./decisions.md)**（wayfinder 地图折叠件，实现阶段的唯一真相源）。本计划只写「怎么做」与为什么这么做。

## Summary

在既有的下载管线上加两件事：**智能重复检测增强**（基于画质档次的「可升级」判定、文件缺失的用户触发式重下）与**失败自动重试**（30/60/120 秒退避、最多 3 次、按失败原因决定是否重试）。

技术路线：**把能纯化的逻辑全部抽成纯函数**（分类器、退避剩余时长、相位判断），把它们塞进 `utils/retry_policy.rs` 与前端 `lib/unifiedVideoList.ts` 两个已有/自然的纯函数层；`download_queue.rs` 里的循环骨架只负责接线。这样做同时满足章程的「可测试性优先」与前提 P11「不引入 runner trait」。

顺带修掉 4 个既有缺陷（见「已知偏差」与 decisions.md §4.2 / §6.4 / §7.2）。

## Technical Context

**Language/Version**: Rust（edition 2021，tokio 1.x full features）；TypeScript 5.5 + React 18.3

**Primary Dependencies**: Tauri v2（tray-icon / image-png）、tokio、serde + serde_json、chrono、uuid、log；前端 MUI 5 + @mui/icons-material 5、Tailwind 3、Vite 5、vitest 4 + @testing-library/react 16。**本次不新增任何依赖。**

**Storage**: JSON 文件，位于 `~/.yt-dlp-sub-gui/`：`download_records.json`（本次扩展）、`subscriptions.json`、`settings.json`、`state.json`

**Testing**: `cargo test`（源码内 `#[cfg(test)]`，现有 259 个）+ `npm test`（vitest，现有 7 个测试文件）。**仓库没有任何跑真实 yt-dlp 的集成测试** —— 这是既成事实，验收策略据此设计（见 decisions.md §10）

**Target Platform**: 桌面 —— Windows / macOS / Linux（通过 Tauri 打包）

**Project Type**: desktop-app（单仓库：`src-tauri/` Rust 后端 + `src/` React 前端，前端单页无路由）

**Performance Goals**: 无新增硬指标。**退避期间占用并发槽是有意接受的代价**（前提 P2）；默认并发为 1 时，一条重试中的任务最多阻塞其它任务 210 秒

**Constraints**:
- **不改输出文件名模板**（`%(title)s.%(ext)s`）—— 它是「同一视频只有一份文件」的物理前提（decisions.md §3）
- **不引入新依赖**（章程 YAGNI + 本次无此必要）
- stderr 读取**内存有界**（只保留尾部 64 KiB）
- 重试参数**硬编码**（3 次 / 30·60·120 秒），不提供设置项 —— 见 Complexity Tracking
- 前端**禁止**在组件内直接 `invoke()`，一律经 `src/lib/tauri.ts`（章程 IV）

**Scale/Scope**: 3 个用户故事里实现 2 个（故事 1 + 2）。改动面横跨 Rust 6 个文件 + 前端 6 个文件 + 1 个契约 fixture，无新模块层、无新依赖。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 判定 | 依据 |
|---|---|---|
| **I. TDD（不可协商）** | ⚠️ **有条件通过** | 全部新增逻辑都有先写测试的落点（decisions.md §10：真值表 13 行、分类器实测样本、fixture 契约、rank 表、三个纯函数）。**唯一缺口**：退避循环骨架按 P11 无自动化测试 → 记入 Complexity Tracking，并以 5 步人工验收兜底 |
| **II. 可测试性优先** | ✅ | 新增逻辑**一律**抽成纯函数（`classify_failure` / `error_lines` / `remaining_after` / `phase_of` / `should_retry`）；Rust 服务层继续通过参数接收路径与配置；队列状态仍经 `QueueContext` 注入（既有例外，不新增全局可变状态） |
| **III. KISS** | ✅ | 保持 JSON 文件存储；复用既有 `DownloadQueue` / `StorageService` / `buildUnifiedVideoList`；不新建服务层或抽象层 |
| **IV. DRY** | ✅ | 本次反而**消除**一处既有重复：`spawn_file_sync` 与 `sync_file_states` 各自内联的存在性检查合并为 `sync_completed_records`（decisions.md §4.5）；错误统一走 `AppError`；前端统一走 `tauri.ts` |
| **V. YAGNI** | ✅ | 无新依赖、无预留接口；并且**删除**了原 spec 里的未来扩展点 `RetryConfig`（decisions.md §1.1） |
| 附加约束（技术栈） | ✅ | 技术栈与版本均不变 |

**Gate 结论：通过。** 两处偏差记入 Complexity Tracking（重试参数硬编码、循环骨架人工验收）。

*Phase 1 设计后复检：见文末「Post-Design Constitution Re-check」。*

## Project Structure

### Documentation (this feature)

```text
specs/008-download-management-enhanced/
├── spec.md              # 需求（已修订，含「修订记录」表）
├── decisions.md         # 📌 决策折叠件 —— 实现阶段的唯一真相源（非 spec-kit 产物，不会被覆盖）
├── plan.md              # 本文件
├── research.md          # Phase 0 产出
├── data-model.md        # Phase 1 产出
├── quickstart.md        # Phase 1 产出
├── contracts/           # Phase 1 产出
│   ├── tauri-commands.md
│   ├── download-record.md
│   ├── unified-video-item.md
│   └── retry-classifier.md
└── checklists/
    └── requirements.md  # 实现时逐项勾选
```

### Source Code (repository root)

```text
src-tauri/
├── src/
│   ├── models/
│   │   └── download.rs          # ← DownloadRecord 新增 3 字段（quality / retry_count / last_retry_at）
│   ├── services/
│   │   ├── download_queue.rs    # ← 核心：退避循环、stderr 并发读、upsert 入队、ActiveTask.pid 转 Option、
│   │   │                        #   DownloadTask 加 record_id / next_retry_at、全部发信号处收进 Some(pid)
│   │   ├── file_manager.rs      # ← 新增 sync_completed_records；delete_file_to_trash 改 pub(crate)
│   │   ├── storage.rs           # ← deduplicate_vec 的 rank 表插入 retrying
│   │   └── scheduler.rs         # ← spawn_file_sync 改调共享函数
│   ├── commands/
│   │   ├── download.rs          # ← 新增命令 redownload_video；recover_state 纳入 retrying
│   │   └── file_manager.rs      # ← sync_file_states 改调共享函数
│   ├── utils/
│   │   ├── retry_policy.rs      # ← 新增（纯函数）：classify_failure / error_lines /
│   │   │                        #   remaining_after / phase_of / should_retry + 单元测试
│   │   └── progress_parser.rs   # （不动，同层先例）
│   └── lib.rs                   # ← 注册 redownload_video
└── tests/
    └── fixtures/
        └── download_record.json # ← 新增：Rust ↔ TS 契约基准（`src-tauri/tests/` 目前为空目录）

src/
├── types/index.ts               # ← DownloadRecord 加 3 字段 + retrying；DownloadTask 加 record_id / next_retry_at；
│                                #   UnifiedVideoItem 加 upgradeable / missing / downloadedElsewhere
├── lib/
│   ├── tauri.ts                 # ← 新增 redownloadVideo(recordId)
│   └── unifiedVideoList.ts      # ← 签名加 qualityPreset / missingPaths / subscriptionId；三个正交标记的派生
└── components/
    ├── App.tsx                  # ← 汇聚 file-sync-complete → missingPaths，下发给两个面板
    ├── AppShell.tsx             # ← 不再只传 filteredRecords；handleRetry 的「重查整订阅」语义移除
    ├── DetailPanel.tsx          # ← 色条 / chip / 三个按钮改调 redownload_video；重试行倒计时
    └── DownloadedList.tsx       # ← 复用同一文案与同一命令
```

**Structure Decision**: 沿用既有的**单仓库 desktop-app 结构**（`src-tauri/` + `src/`），不新建任何顶层目录或服务层。新增的纯函数模块 `utils/retry_policy.rs` 与既有 `utils/progress_parser.rs` 同层同性质（无状态、纯函数、可单测），前端同理落在 `lib/unifiedVideoList.ts`。契约 fixture 放 `src-tauri/tests/fixtures/`，因为两侧都要读同一个文件。

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| **重试参数硬编码**（章程附加约束：「禁止硬编码…配置值」） | spec `Assumptions` 第 6 条明确要求「最大次数与间隔为默认值，**用户不可配置**」，前提 P3 据此定为编译期常量 | 做成设置项会得到一个**永远只有默认值**的死配置，还要连带 `AppSettings` 序列化、校验与设置 UI 三处成本 —— 正是 YAGNI 要挡的东西。常量集中在 `utils/retry_policy.rs` 并以参数传给纯函数，满足「可测试」的意图 |
| **退避循环骨架无自动化测试**（原则 I：「所有功能开发必须遵循 TDD」） | 前提 P11 定「只抽纯函数、不引入 runner trait」；为循环骨架引入可注入 runner 会破坏 `download_queue.rs` 的既有结构 | 把边界推到极限：`remaining_after` / `phase_of` / `should_retry` 全部纯函数化并单测（decisions.md §10），人工只走 5 步骨架接线。引入 runner trait 的收益（多测一段接线）不抵对既有结构的破坏与治理成本 |

## Post-Design Constitution Re-check

*Phase 1 设计完成后复检。*

| 原则 | 复检结论 |
|---|---|
| **I. TDD** | ⚠️ 同前 —— 唯一缺口仍是退避循环骨架（P11 决定），已登记。Phase 1 未引入新的未测逻辑：所有新逻辑都有明确测试落点（见 quickstart.md §1.2 的分层表） |
| **II. 可测试性优先** | ✅ 设计后更好：`retry_policy.rs` 把 5 个纯函数集中在一处（`contracts/retry-classifier.md`），`utils/progress_parser.rs` 是既有同层先例；没有为测试性引入新抽象层 |
| **III. KISS** | ✅ 无新依赖、无新服务层；`quickstart.md` 的验证路径全部复用既有命令（`cargo test` / `npm test` / `npm run tauri dev`） |
| **IV. DRY** | ✅ 新增的 `sync_completed_records` 消除一处既有重复；契约以**单一 fixture** 承载两侧一致性，而不是两套手工清单 |
| **V. YAGNI** | ✅ 4 个契约文件只描述本次真正暴露的接口（1 个新命令 + 1 个持久化结构 + 1 个前端派生结构 + 1 个纯函数模块）；没有为将来预留 |
| 附加约束（技术栈） | ✅ 未变 |

**Gate 结论：通过。** 与 Phase 0 前的判定一致，**无新增偏差**。
