# Tasks: 订阅管理增强

**Input**: Design documents from `specs/006-subscription-management-enhanced/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: 根据项目章程（Constitution I. TDD），所有功能开发必须遵循 TDD。以下包含测试任务。

**Organization**: 任务按用户故事分组，支持独立实现和测试。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 所属用户故事（US1、US2、US3、US4）
- 描述中包含具体文件路径

---

## Phase 1: Setup (共享基础设施)

**Purpose**: 项目依赖和类型定义准备

- [ ] T001 在 `src-tauri/Cargo.toml` 中添加 `reqwest` 依赖（default-features = false, features = ["rustls-tls"]）
- [ ] T002 [P] 扩展 `Subscription` 结构体，新增 `tags`、`health_status`、`last_health_check` 字段，添加 `#[serde(default)]` 确保向后兼容，在 `src-tauri/src/models/subscription.rs`
- [ ] T003 [P] 新增 `HealthStatus` 枚举（Ok/Warning/Dead）和 `HealthCheckResult` 模型，在 `src-tauri/src/models/health.rs`，并注册到 `src-tauri/src/models/mod.rs`
- [ ] T004 [P] 扩展 TypeScript 类型定义（Subscription 新字段、HealthStatus、HealthCheckResult、HealthCheckSummary、ImportSource、ImportPreview、FilterState、SortState、ChannelInfo、VideoInfo），在 `src/types/index.ts`

---

## Phase 2: Foundational (阻塞性前置任务)

**Purpose**: 所有用户故事都依赖的核心基础设施

**⚠️ CRITICAL**: 此阶段必须完成才能开始任何用户故事实现

- [ ] T005 扩展 `StorageService`，新增标签批量保存和聚合查询方法（`save_tags`、`get_all_tags`），在 `src-tauri/src/services/storage.rs`
- [ ] T006 [P] 扩展 `AppError` 枚举，新增 `HealthCheckError`、`ImportError` 变体，在 `src-tauri/src/utils/error.rs`
- [ ] T007 [P] 注册新 Rust 模块：在 `src-tauri/src/services/mod.rs` 添加 `pub mod health;`，在 `src-tauri/src/commands/mod.rs` 添加 `pub mod health;` 和 `pub mod import_export;`
- [ ] T008 编写 Subscription 新字段的序列化/反序列化往返测试，在 `src-tauri/src/models/subscription.rs` 的 `#[cfg(test)]` 模块中

**Checkpoint**: 基础设施就绪 — 可以开始用户故事实现

---

## Phase 3: User Story 1 — 批量导入订阅 (Priority: P1) 🎯 MVP

**Goal**: 用户可通过 OPML/TXT/URL 列表批量导入订阅，支持预览、进度显示和部分失败隔离

**Independent Test**: 准备包含 10 个频道的 OPML 文件，通过导入对话框导入，验证 10 个订阅全部正确创建且频道信息自动解析

### Tests for User Story 1 ⚠️

> **先编写以下测试，确保 FAIL，然后再开始实现**

- [ ] T009 [P] [US1] 编写 OPML 导入单元测试（正常格式、空文件、损坏文件、非 UTF-8 编码），在 `src-tauri/src/services/opml.rs` 的 `#[cfg(test)]` 模块中
- [ ] T010 [P] [US1] 编写 TXT 导入单元测试（空行、注释行、无效 URL、超长 URL），在 `src-tauri/src/services/storage.rs` 的 `#[cfg(test)]` 模块中
- [ ] T011 [P] [US1] 编写批量导入集成测试（部分失败不影响已成功项、重复检测跳过），在 `src-tauri/src/commands/import_export.rs` 的 `#[cfg(test)]` 模块中

### Implementation for User Story 1

- [ ] T012 [P] [US1] 实现 `batch_import_preview` 命令（解析 OPML/TXT/URL 列表，返回预览数据，含重复检测），在 `src-tauri/src/commands/import_export.rs`
- [ ] T013 [US1] 实现 `batch_import_execute` 命令（异步后台任务，并发调用 `YtDlpService::parse_channel_info()`，通过事件推送进度），在 `src-tauri/src/commands/import_export.rs`
- [ ] T014 [US1] 实现 `cancel_import` 命令（取消正在进行的导入任务），在 `src-tauri/src/commands/import_export.rs`
- [ ] T015 [P] [US1] 在前端 `src/lib/tauri.ts` 中添加 `batchImportPreview`、`batchImportExecute`、`cancelImport` 封装函数
- [ ] T016 [US1] 扩展 `ImportDialog.tsx` 组件（新增文件预览列表，显示成功/失败统计，进度条），在 `src/components/ImportDialog.tsx`
- [ ] T017 [US1] 在 `src/App.tsx` 中注册 `import-progress` 和 `import-complete` 事件监听，连接 ImportDialog 状态

**Checkpoint**: 批量导入功能可独立验证 — 用户可通过 FileDialog 导入 OPML/TXT/URL 列表并看到进度和结果

---

## Phase 4: User Story 2 — 订阅健康检查 (Priority: P1)

**Goal**: 用户可运行全量或选择性健康检查，识别失效/警告/正常订阅，失效订阅自动暂停

**Independent Test**: 在订阅列表中选择 3 个活跃频道和 2 个已删除频道，运行健康检查，验证系统正确识别活跃与失效频道并显示结果

### Tests for User Story 2 ⚠️

> **先编写以下测试，确保 FAIL，然后再开始实现**

- [ ] T018 [P] [US2] 编写 HealthService 单元测试（各 HTTP 状态码处理：200/301/404/429/500/超时），在 `src-tauri/src/services/health.rs` 的 `#[cfg(test)]` 模块中
- [ ] T019 [P] [US2] 编写健康检查命令单元测试（全量检查、选择性检查、无订阅时返回），在 `src-tauri/src/commands/health.rs` 的 `#[cfg(test)]` 模块中
- [ ] T020 [P] [US2] 编写健康状态持久化测试（检查完成后 health_status 和 last_health_check 正确写入 subscriptions.json），在 `src-tauri/src/services/storage.rs` 的 `#[cfg(test)]` 模块中
- [ ] T021 [P] [US2] 编写失效订阅自动暂停测试（health_status="dead" 后 enabled 自动置为 false），在 `src-tauri/src/commands/health.rs` 的 `#[cfg(test)]` 模块中

### Implementation for User Story 2

- [ ] T022 [US2] 实现 `HealthService`（`check_single`：HTTP HEAD 验证 + 超时 + 429 重试；`check_batch`：Semaphore(3) 并发控制 + 快照隔离），在 `src-tauri/src/services/health.rs`
- [ ] T023 [US2] 实现 `check_all_health` 命令（快照订阅列表 → spawn 后台任务 → 事件推送进度/结果），在 `src-tauri/src/commands/health.rs`
- [ ] T024 [US2] 实现 `check_selected_health` 命令（仅检查指定订阅 ID 列表），在 `src-tauri/src/commands/health.rs`
- [ ] T025 [US2] 实现健康检查完成后更新订阅健康状态并自动暂停失效订阅的逻辑（在命令层或服务层调用 StorageService），在 `src-tauri/src/commands/health.rs`
- [ ] T026 [P] [US2] 在前端 `src/lib/tauri.ts` 中添加 `checkAllHealth`、`checkSelectedHealth`、`getHealthResults` 封装函数
- [ ] T027 [P] [US2] 实现 `useHealthCheck` Hook（管理检查状态、进度、结果），在 `src/hooks/useHealthCheck.ts`
- [ ] T028 [US2] 实现 `HealthCheckPanel.tsx` 组件（摘要统计、详细结果列表、批量删除失效率订阅按钮），在 `src/components/HealthCheckPanel.tsx`
- [ ] T029 [US2] 在 `src/App.tsx` 中集成 `useHealthCheck`，注册 `health-check-progress` 和 `health-check-complete` 事件

**Checkpoint**: 健康检查功能可独立验证 — 用户可运行全量/选择性健康检查，看到结果面板，失效订阅自动暂停

---

## Phase 5: User Story 3 — 高级筛选与排序 (Priority: P1)

**Goal**: 用户可通过平台、状态、分组、健康状态、关键字五维筛选订阅，支持按名称/日期排序

**Independent Test**: 创建 20 个不同类型订阅（不同平台、分组、启停状态、健康状态），使用筛选和排序功能，验证结果准确

### Tests for User Story 3 ⚠️

> **先编写以下测试，确保 FAIL，然后再开始实现**

- [ ] T030 [P] [US3] 编写 useFilter Hook 逻辑测试（AND 组合、各维度独立筛选、空/null 值处理），在 `src/hooks/__tests__/useFilter.test.ts`
- [ ] T031 [P] [US3] 编写 FilterBar 组件渲染测试（各筛选控件正确渲染、条件变更触发回调），在 `src/components/__tests__/FilterBar.test.tsx`
- [ ] T032 [P] [US3] 编写搜索高亮组件测试（字面匹配、特殊字符转义、多关键字匹配），在 `src/components/__tests__/SubscriptionItem.test.tsx`

### Implementation for User Story 3

- [ ] T033 [US3] 实现 `useFilter` Hook（输入：Subscription[] + FilterState + SortState → 输出：filtered + sorted result），在 `src/hooks/useFilter.ts`
- [ ] T034 [US3] 实现 `FilterBar.tsx` 组件（平台下拉、状态切换、分组选择、健康状态选择、关键字输入框），在 `src/components/FilterBar.tsx`
- [ ] T035 [US3] 修改 `SubscriptionItem.tsx`，新增 `keyword` props，实现搜索高亮渲染（`highlightText` 递归 JSX，字面匹配），在 `src/components/SubscriptionItem.tsx`
- [ ] T036 [US3] 修改 `SubscriptionList.tsx`，移除现有本地筛选逻辑，集成 `useFilter` 输出，将筛选条件传入 FilterBar 和 SubscriptionItem，在 `src/components/SubscriptionList.tsx`
- [ ] T037 [US3] 在 `src/App.tsx` 中新增 `filterState`/`sortState` 状态，通过 AppShell → SubscriptionList 传递

**Checkpoint**: 筛选排序功能可独立验证 — 用户可在列表中切换各筛选维度，列表实时更新，关键字高亮显示

---

## Phase 6: User Story 4 — 订阅详情面板 (Priority: P2)

**Goal**: 用户点击订阅后查看频道详细信息，包括视频列表（分页加载）

**Independent Test**: 点击一个 YouTube 订阅，验证详情面板展示频道名称、描述、统计数据，视频列表支持分页

### Tests for User Story 4 ⚠️

> **先编写以下测试，确保 FAIL，然后再开始实现**

- [ ] T038 [P] [US4] 编写 `get_channel_videos` 命令单元测试（分页参数、空频道处理），在 `src-tauri/src/commands/download.rs` 的 `#[cfg(test)]` 模块中
- [ ] T039 [P] [US4] 编写 DetailPanel 渲染测试（频道信息展示、失效率订阅状态、视频列表分页），在 `src/components/__tests__/DetailPanel.test.tsx`

### Implementation for User Story 4

- [ ] T040 [US4] 实现 `get_channel_videos` 命令（通过 yt-dlp `--flat-playlist --playlist-start` + `--playlist-end` 分页获取视频列表），在 `src-tauri/src/commands/download.rs`
- [ ] T041 [US4] 实现 `get_channel_info` 命令（从 subscriptions.json 读取频道信息 + 缓存数据），在 `src-tauri/src/commands/subscription.rs`
- [ ] T042 [P] [US4] 在前端 `src/lib/tauri.ts` 中添加 `getChannelVideos`、`getChannelInfo` 封装函数
- [ ] T043 [US4] 修改 `DetailPanel.tsx`，新增频道信息展示区（名称、描述、平台、订阅数、缩略图、健康状态标注）、视频列表区（分页加载、滚动触发），在 `src/components/DetailPanel.tsx`
- [ ] T044 [US4] 修改 `DownloadRecordList.tsx`，集成视频列表分页（初始 10 条，"加载更多"按钮/滚动加载），在 `src/components/DownloadRecordList.tsx`

**Checkpoint**: 详情面板功能可独立验证 — 用户点击订阅后看到频道信息和分页视频列表，失效率订阅明确标注

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 跨故事优化和收尾

- [ ] T045 [P] 实现 `batch_delete_subscriptions` 命令（从健康检查结果中批量删除失效率订阅），在 `src-tauri/src/commands/subscription.rs`
- [ ] T046 [P] 在健康检查结果面板中集成批量删除按钮，触发 `batch_delete_subscriptions` 后刷新订阅列表，在 `src/components/HealthCheckPanel.tsx`
- [ ] T047 [P] 扩展导出测试，验证 OPML/JSON 导出包含新增字段（tags、health_status、last_health_check），在 `src-tauri/src/services/opml.rs` 和 `src-tauri/src/commands/import_export.rs` 的 `#[cfg(test)]` 模块中
- [ ] T048 运行 `cargo test` 确认所有 Rust 测试通过，追加 SC-005（导入失败率）和 SC-006（批量删除 < 2s）验证；运行 `npx tsc --noEmit` 确认 TypeScript 类型检查通过
- [ ] T049 [P] 按 `quickstart.md` 中的验证命令执行端到端功能验证

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖 — 可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成 — **阻塞所有用户故事**
- **User Stories (Phase 3-6)**: 全部依赖 Foundational 完成
  - US1 (P1)、US2 (P1)、US3 (P1) 可并行进行
  - US4 (P2) 依赖 US2（需要 health_status 持久化）
- **Polish (Phase 7)**: 依赖所有用户故事完成

### User Story Dependencies

- **US1 (批量导入)**: 无跨故事依赖，可独立完成
- **US2 (健康检查)**: 需要 US3 中扩展的 Subscription 模型（已在 Foundational 中完成），无需等待 US1 或 US3
- **US3 (筛选排序)**: 需要 US2 的 health_status 字段（已在 Foundational 中完成），筛选的健康状态维度需 US2 完成后才有数据
  - 可以部分并行：筛选逻辑可在 US2 完成前编写（用 mock 数据测试）
- **US4 (详情面板)**: 依赖 US2（需要 health_status 标记失效率订阅）

### Within Each User Story

- 测试必须**先编写**并**确认失败**再进行实现
- Models → Services → Commands → Frontend 封装 → UI 组件
- 核心实现完成后再做集成连接

### Parallel Opportunities

- Phase 1 中 T002、T003、T004 可并行（不同文件）
- Phase 2 中 T006、T007 可并行
- Phase 3-5 (US1/US2/US3) 可同时由不同开发者实现
- 每个用户故事中的 [P] 任务可并行执行
- Rust 后端和 TypeScript 前端任务可交叉并行

---

## Parallel Example: User Story 2

```bash
# 编写所有 US2 测试（并行）:
Task: "T018 编写 HealthService 单元测试 in services/health.rs"
Task: "T019 编写健康检查命令单元测试 in commands/health.rs"
Task: "T020 编写健康状态持久化测试 in services/storage.rs"
Task: "T021 编写失效订阅自动暂停测试 in commands/health.rs"

# 实现 Rust 后端时，前端可并行准备:
Task: "T026 添加健康检查 tauri.ts 封装函数 in src/lib/tauri.ts"
Task: "T027 实现 useHealthCheck Hook in src/hooks/useHealthCheck.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1: Setup
2. 完成 Phase 2: Foundational (CRITICAL)
3. 完成 Phase 3: User Story 1 (批量导入)
4. **STOP and VALIDATE**: 独立测试批量导入
5. 此时已可演示 MVP

### Incremental Delivery

1. Setup + Foundational → 基础设施就绪
2. + US1 批量导入 → 测试独立 → MVP!
3. + US2 健康检查 → 测试独立 → 增量交付
4. + US3 筛选排序 → 测试独立 → 增量交付
5. + US4 详情面板 → 测试独立 → 完整交付

### P1 Parallel Strategy (3 个 P1 故事)

由于 US1、US2、US3 都是 P1，如果有多人开发：

1. 团队共同完成 Phase 1 + Phase 2
2. Foundational 完成后:
   - 开发者 A: US1 (批量导入)
   - 开发者 B: US2 (健康检查)
   - 开发者 C: US3 (筛选排序)
3. US2 和 US3 完成后 → 开发者 A 或 B 接手 US4 (详情面板)

---

## Notes

- [P] 任务 = 不同文件，无依赖，可并行
- [Story] 标签将任务映射到特定用户故事，便于追踪
- 每个用户故事应可独立完成和测试
- TDD: 先写测试 → 确认失败 → 实现 → 确认通过 → 重构
- 每个任务或逻辑组完成后提交
- 在任何 Checkpoint 处可停止验证该故事的独立性
- 避免：模糊任务、同文件冲突、破坏独立性的跨故事依赖
