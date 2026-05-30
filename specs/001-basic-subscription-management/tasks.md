# Tasks: 基本订阅管理

**Input**: Design documents from `/specs/001-basic-subscription-management/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), data-model.md, contracts/, research.md, quickstart.md

**Tests**: 根据章程 I (TDD)，所有实现任务 MUST 先有测试。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust backend**: `src-tauri/src/`
- **Frontend**: `src/`
- 文件路径相对于仓库根目录

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 确认现有基础设施满足需求，添加缺失字段到数据模型

- [ ] T001 确认 yt-dlp CLI 可用，验证 `yt-dlp --dump-json --playlist-items 1` 命令正常输出
- [~] T002 [P] 在 `src-tauri/src/models/subscription.rs` 的 Subscription 结构体中新增 `group_name: String` 字段（默认值 "未分组"）
- [~] T003 [P] 在 `src-tauri/src/models/subscription.rs` 的 Subscription 结构体中新增 `last_checked_at: Option<String>` 字段（默认值 None）
- [~] T004 [P] 在 `src/types/index.ts` 的 Subscription 接口中新增 `group_name: string` 和 `last_checked_at: string | null` 字段

**Checkpoint**: 数据模型已扩展，向后兼容现有 JSON 数据

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心基础设施，所有用户故事依赖

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T005 编写 `get_subscriptions` 命令的单元测试，验证 JSON 反序列化包含新增 `group_name` 字段，在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T006 [P] 编写 `add_subscription` 命令的单元测试，验证新创建的订阅 `group_name` 为 "未分组"，在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T007 [P] 编写 Subscription 模型 `new()` 构造函数的单元测试，验证默认值包含 `group_name: "未分组"`，在 `src-tauri/src/models/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T008 确保 `src-tauri/src/models/subscription.rs::Subscription::new()` 构造函数初始化 `group_name = "未分组".to_string()` 和 `last_checked_at = None`
- [ ] T009 确认现有 `serialize`/`deserialize` 测试通过新增字段后仍正确，运行 `cargo test models::subscription`

**Checkpoint**: 基础模型就绪，新增字段已测试并可用 — 用户故事实现现在可以开始

---

## Phase 3: User Story 1 - 添加新订阅 (Priority: P1) 🎯 MVP

**Goal**: 用户通过输入频道 URL 添加 YouTube/Bilibili 频道，系统解析频道信息并持久化

**Independent Test**: 输入有效 YouTube URL → 频道出现在订阅列表，`group_name` 为 "未分组"

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T010 [P] [US1] 编写 `YtDlpService::parse_channel_info()` 对 YouTube URL 的单元测试（mock 或真实 CLI），在 `src-tauri/src/services/ytdlp.rs` 的 `#[cfg(test)]` 模块
- [ ] T011 [P] [US1] 编写 `StorageService::save_subscriptions` 新订阅写入后读取验证的测试，在 `src-tauri/src/services/storage.rs` 的 `#[cfg(test)]` 模块
- [ ] T012 [P] [US1] 编写前端 `AddSubscriptionDialog` 组件渲染测试，验证 URL 输入框和提交按钮存在，使用 React Testing Library 在 `src/components/` 目录（手动测试即可）

### Implementation for User Story 1

- [ ] T013 [US1] 验证 `src-tauri/src/commands/subscription.rs::add_subscription()` 命令正确使用 `Subscription::new()`，传递 platform/channel_name/channel_avatar_url
- [ ] T014 [US1] 更新 `src/hooks/useSubscriptions.ts::addSubscription()` 确保返回的 Subscription 类型包含 `group_name`
- [ ] T015 [US1] 验证 `src/components/AddSubscriptionDialog.tsx` 添加成功后显示频道名称，关闭弹窗，列表自动刷新
- [ ] T016 [US1] 验证重复 URL 检测逻辑：`src-tauri/src/commands/subscription.rs` 中 load_subscriptions 后比对 URL，返回 AppError::Duplicate
- [ ] T017 [US1] 验证无效 URL 的错误提示：`src/components/AddSubscriptionDialog.tsx` 中 catch 错误并展示 MUI Alert

**Checkpoint**: 添加订阅功能完整可用，支持重复检测和错误提示

---

## Phase 4: User Story 2 - 查看订阅列表 (Priority: P1)

**Goal**: 列表中展示所有订阅，显示名称、封面、状态，分组筛选

**Independent Test**: 添加 3 个不同分组的订阅 → 列表显示所有订阅，可按分组筛选

### Tests for User Story 2

- [ ] T018 [P] [US2] 编写 `StorageService::load_subscriptions()` 读取含 `group_name` 字段的 JSON 文件测试，在 `src-tauri/src/services/storage.rs` 的 `#[cfg(test)]` 模块
- [ ] T019 [US2] 编写前端 `SubscriptionList` 包含分组筛选下拉框的渲染验证（手动测试）

### Implementation for User Story 2

- [ ] T020 [US2] 在 `src/components/SubscriptionList.tsx` 中新增分组筛选下拉框（选项：全部、未分组、学习、娱乐、音乐、科技、其他）
- [ ] T021 [US2] 在 `src/components/SubscriptionList.tsx` 中实现 `filteredSubscriptions` 逻辑：根据选中的分组过滤列表
- [ ] T022 [US2] 在 `src/components/SubscriptionItem.tsx` 中显示 `group_name`（如用 Chip 标签展示当前分组）
- [ ] T023 [US2] 验证空状态处理：`src/components/SubscriptionList.tsx` 中 subscriptions.length === 0 时显示引导提示

**Checkpoint**: 订阅列表完整显示，支持分组筛选

---

## Phase 5: User Story 3 - 删除订阅 (Priority: P1)

**Goal**: 用户可删除订阅，删除前二次确认，下载记录不级联删除

**Independent Test**: 选中一个订阅 → 点击删除 → 确认 → 订阅从列表消失

### Tests for User Story 3

- [ ] T024 [P] [US3] 编写 `delete_subscription` 命令测试，验证从列表中移除指定 ID 的订阅，在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T025 [P] [US3] 编写前端删除确认对话框的交互验证（手动测试）

### Implementation for User Story 3

- [ ] T026 [US3] 验证 `src-tauri/src/commands/subscription.rs::delete_subscription()` 仅从 subscriptions.json 移除，不修改 download_records.json
- [ ] T027 [US3] 验证订阅不存在的错误处理：delete 不存在的 ID 返回 AppError::NotFound
- [ ] T028 [US3] 验证 `src/hooks/useSubscriptions.ts::deleteSubscription()` 从本地状态中 filter 移除
- [ ] T029 [US3] 在 `src/components/SubscriptionItem.tsx` 中实现删除按钮 + MUI Dialog 二次确认

**Checkpoint**: 删除功能完整，下载记录不受影响

---

## Phase 6: User Story 4 - 启用/禁用订阅 (Priority: P1)

**Goal**: 每个订阅可独立切换启用/禁用状态，禁用的订阅被定时检查跳过

**Independent Test**: 切换订阅为禁用 → 状态栏/图标变化 → 定时检查跳过该订阅

### Tests for User Story 4

- [ ] T030 [P] [US4] 编写 `toggle_subscription_pause` 命令测试，验证 paused 字段翻转，在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T031 [P] [US4] 编写前端暂停/恢复切换按钮的渲染验证（手动测试）

### Implementation for User Story 4

- [ ] T032 [US4] 验证 `src-tauri/src/commands/subscription.rs::toggle_subscription_pause()` 翻转 paused 字段并持久化
- [ ] T033 [US4] 验证 `src/hooks/useSubscriptions.ts::togglePause()` 更新本地状态
- [ ] T034 [US4] 在 `src/components/SubscriptionItem.tsx` 中显示暂停/启用图标按钮，视觉区分两种状态
- [ ] T035 [US4] 验证 `src-tauri/src/lib.rs` 后台调度器（第 92 行 `if sub.paused { continue; }`）正确跳过禁用的订阅

**Checkpoint**: 启用/禁用功能完整，调度器尊重状态

---

## Phase 7: User Story 5 - 手动检查更新 (Priority: P2)

**Goal**: 用户可手动触发单个订阅或全部已启用订阅的更新检查

**Independent Test**: 点击单个订阅的"检查更新"按钮 → 检查启动 → 进度显示

### Tests for User Story 5

- [ ] T036 [US5] 编写前端 `SubscriptionItem` 中"检查更新"按钮触发的交互验证（手动测试）

### Implementation for User Story 5

- [ ] T037 [US5] 验证 `src-tauri/src/commands/download.rs::check_subscription()` 调用 `check_and_download`
- [ ] T038 [US5] 验证 `src/lib/tauri.ts::checkSubscription()` 正确调用 invoke
- [ ] T039 [US5] 在 `src/hooks/useDownloadRecords.ts::checkSubscription()` 中实现 loading 状态下的进度反馈
- [ ] T040 [US5] 在 `src/components/SubscriptionItem.tsx` 中实现"检查更新"按钮（单条）
- [ ] T041 [P] [US5] 在 `src/components/SubscriptionList.tsx` 中实现"检查全部"按钮（调用 checkAll）
- [ ] T042 [US5] 实现重复检查保护：检查中再次点击时提示"正在检查中"

**Checkpoint**: 手动检查功能完整

---

## Phase 8: User Story 6 - 订阅分组 (Priority: P3)

**Goal**: 用户可将订阅分配到预定义分组，按分组筛选和管理

**Independent Test**: 将订阅移到"学习"分组 → 筛选视图显示正确 → 分组信息持久化

### Tests for User Story 6

- [ ] T043 [P] [US6] 编写 `update_subscription_group` 命令测试，验证 group_name 更新并持久化，在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块
- [ ] T044 [P] [US6] 编写分组名称验证测试（值必须在预定义列表中），在 `src-tauri/src/commands/subscription.rs` 的 `#[cfg(test)]` 模块

### Implementation for User Story 6

- [ ] T045 [US6] 在 `src-tauri/src/commands/subscription.rs` 中实现 `update_subscription_group` 命令
  - 参数：`id: String`, `group_name: String`
  - 验证 `group_name` 在 ["未分组", "学习", "娱乐", "音乐", "科技", "其他"] 中
  - 加载、修改、保存、返回
- [ ] T046 [US6] 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 宏中注册 `update_subscription_group` 命令
- [ ] T047 [US6] 在 `src/lib/tauri.ts` 中新增 `updateSubscriptionGroup(id: string, groupName: string): Promise<Subscription>`
- [ ] T048 [US6] 在 `src/hooks/useSubscriptions.ts` 中新增 `updateGroup` 回调函数
- [ ] T049 [US6] 在 `src/components/SubscriptionItem.tsx` 中添加分组切换选择器（下拉菜单或 Chip 点击弹出）
- [ ] T050 [US6] 实现"按分组检查"功能：在 `src/components/SubscriptionList.tsx` 分组筛选后，点击"检查该分组"仅检查筛选结果中的已启用订阅

**Checkpoint**: 分组功能完整，支持分配、筛选、按分组检查

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: 完善测试覆盖、代码质量和文档

- [ ] T051 [P] 运行 `cargo test` 确保所有 69+ 原有测试和新测试通过
- [ ] T052 [P] 运行 `npx tsc --noEmit` 确保 TypeScript 类型检查通过
- [ ] T053 验证向后兼容性：新建带 `group_name` 字段的 JSON 文件能被旧代码降级读取（group_name 默认 "未分组"）
- [ ] T054 验证 quickstart.md 中的手动测试场景全部通过
- [ ] T055 [P] 检查代码无 `unwrap()` 调用（改用 `?` 或模式匹配）
- [ ] T056 检查 Constitution Check 5 条原则仍然全部通过

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-8)**: All depend on Foundational phase completion
  - US1, US2, US3, US4 are all P1 and independent of each other
  - US5 depends on US1 (needs subscriptions to check)
  - US6 depends on US2 (needs list + filtering to show groups)
- **Polish (Phase 9)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational - No dependencies on other stories
- **US2 (P1)**: Can start after Foundational - No dependencies on other stories
- **US3 (P1)**: Can start after Foundational - Depends on US1 having subscriptions to delete
- **US4 (P1)**: Can start after Foundational - Depends on US1 having subscriptions to toggle
- **US5 (P2)**: Can start after US1 - Needs subscriptions to check
- **US6 (P3)**: Can start after US2 - Needs list to display group filters

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Models → Commands → Frontend tauri.ts → Hook → Component
- Core implementation before UI integration

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel (T002, T003, T004)
- Foundational tests T005, T006, T007 can run in parallel
- US1, US2 can start in parallel after Foundational
- US3, US4 can start in parallel after US1
- All tests within a story marked [P] can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for US1 together:
Task: "T010 [P] [US1] Write YtDlpService parse_channel_info test in src-tauri/src/services/ytdlp.rs"
Task: "T011 [P] [US1] Write StorageService save/load test in src-tauri/src/services/storage.rs"
Task: "T012 [P] [US1] Write AddSubscriptionDialog render test"

# Implementation after tests fail:
Task: "T013 [US1] Verify add_subscription command"
Task: "T014 [US1] Update useSubscriptions addSubscription"
Task: "T015 [US1] Verify AddSubscriptionDialog"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T009)
3. Complete Phase 3: User Story 1 (T010-T017)
4. **STOP and VALIDATE**: Test add subscription independently
5. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 (添加订阅) → Test independently → MVP!
3. Add US2 (查看列表) + US3 (删除) + US4 (暂停) → Core management ready
4. Add US5 (手动检查) → Interaction loop complete
5. Add US6 (分组) → Organization features
6. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (add)
   - Developer B: User Story 2 (list + filter)
3. After US1/US2 done:
   - Developer A: User Story 3 + 4 (delete + toggle)
   - Developer B: User Story 5 (manual check)
4. After all P1 done:
   - Developer A: User Story 6 (groups)

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
- 现有 MVP 代码大部分已实现，任务主要聚焦于新增 `group_name`/`last_checked_at` 字段和 `update_subscription_group` 命令
