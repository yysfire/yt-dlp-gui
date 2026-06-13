# Tasks: DetailPanel 统一视频列表

**Input**: Design documents from `/specs/007-unified-video-list/`

**Prerequisites**: plan.md (required), spec.md (required), data-model.md, research.md

**Tests**: 前端无测试框架，验证方式为 `npx tsc --noEmit` + `cargo test`（Rust 后端无变更）+ 手动验证关键交互路径。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **前端**: `src/components/`, `src/types/`, `src/lib/`
- **后端**: `src-tauri/src/`（本功能无变更）

---

## Phase 1: Setup (共享基础设施)

**Purpose**: 新增类型定义，为所有用户故事提供数据基础

- [ ] T001 [P] 新增 `UnifiedVideoItem` 接口与 `VideoStatus` 类型到 `src/types/index.ts`
- [ ] T002 [P] 新增 `formatDuration()` 工具函数（若尚未存在）或确认已在 `src/components/DetailPanel.tsx` 中可用

**Checkpoint**: 类型定义就绪，所有用户故事可并行开始

---

## Phase 2: 基础（阻塞性前置）

**Purpose**: DetailPanel 基础重构——创建合并逻辑与统一渲染结构，移除旧区域

**⚠️ CRITICAL**: 无此基础，US1/US2/US3 均无法开始

- [ ] T003 在 `src/components/DetailPanel.tsx` 中实现 `mergeToUnifiedItems()` 合并逻辑：遍历 records → queueTasks → videoList，按 video_id/video_url 去重生成 `UnifiedVideoItem[]`
- [ ] T004 在 `src/components/DetailPanel.tsx` 中实现 `sortUnifiedItems()` 排序函数：按下载中/等待中 → 已暂停 → 已完成 → 未下载 → 仅记录旧视频 → 失败 排序
- [ ] T005 在 `src/components/DetailPanel.tsx` 中重构渲染结构：替换「最新视频」和「下载记录」两个独立区域为单一 `<List>`，移除 `<DownloadRecordList>` 引用
- [ ] T006 实现三行三列网格布局条目组件（内联于 DetailPanel）：
  - 第 1 列：状态图标（垂直居中，40px）
  - 第 2 列：第 1 行标题 / 第 2 行左元信息+右统计 / 第 3 行进度条
  - 第 3 列：操作按钮（垂直居中，40px）
- [ ] T007 运行 `npx tsc --noEmit` 验证 TypeScript 类型正确，运行 `cargo test` 确认 Rust 测试保持通过

**Checkpoint**: DetailPanel 渲染单一合并列表，旧双区域结构已移除，类型检查通过

---

## Phase 3: User Story 1 - 查看订阅的完整视频状态（Priority: P1）🎯 MVP

**Goal**: 用户点击订阅后看到合并的单一视频列表，已下载和未下载视频在同一列表中显示，无重复

**Independent Test**: 选中任意有下载记录的订阅，右侧面板渲染单一视频列表，同一视频仅出现一次

### Implementation for User Story 1

- [ ] T008 [US1] 在 DetailPanel 的 `useMemo` 中连接 `records + queueTasks + videoList` 到 `mergeToUnifiedItems()`，替换当前 `uniqueVideos` 逻辑（`src/components/DetailPanel.tsx`）
- [ ] T009 [US1] 实现各状态的条目渲染差异（`src/components/DetailPanel.tsx`）：
  - 未下载：▶ 灰色图标 + 标题 + 上传日期 · 时长
  - 等待中：⏳ + 标题 + "等待中·日期·时长" + 第 3 列取消按钮
  - 已完成：✓ 绿色图标 + 标题 + "已完成·日期·时长" + 右侧文件大小·下载时间
  - 失败：✕ 红色图标 + 标题 + "失败·日期·时长·错误" + 右侧下载时间 + 第 3 列重试按钮
- [ ] T010 [US1] 订阅详情首次加载时先展示下载记录（已在内存中），再异步加载第 1 页频道视频并合并（`src/components/DetailPanel.tsx`）
- [ ] T011 [US1] 旧下载记录中不在频道视频列表内的视频保留在统一列表中（`channelInfo` 为 null 时仅展示标题+下载状态，省略日期·时长）
- [ ] T012 [US1] 频道失效率时不加载频道视频，仅展示已有下载记录（`src/components/DetailPanel.tsx`）
- [ ] T013 运行 `npx tsc --noEmit` + `npm run build` 验证编译通过

**Checkpoint**: US1 可独立验证——统一列表正确合并、无重复、各状态渲染正确

---

## Phase 4: User Story 2 - 后台自动加载全部频道视频（Priority: P2）

**Goal**: 第 1 页加载完后自动逐页加载剩余视频，用户无需点击"加载更多"

**Independent Test**: 选中 50+ 视频频道，列表自动增长直到全部加载完成，无"加载更多"按钮

### Implementation for User Story 2

- [ ] T014 [US2] 实现后台自动分页加载逻辑：在 `useCallback` 中递归调用 `getChannelVideos(page, 10)`，每页加载完成后追加到 `videoList` 并触发下一页面（`src/components/DetailPanel.tsx`）
- [ ] T015 [US2] 使用 `requestIdRef` 模式：切换订阅时递增 `requestIdRef`，后台加载循环中每页检查，若 ID 不匹配则取消（`src/components/DetailPanel.tsx`）
- [ ] T016 [US2] 后台加载出错时静默处理：已加载的视频保留，不再继续加载，不显示错误提示（`src/components/DetailPanel.tsx`）
- [ ] T017 [US2] 移除"加载更多"按钮及相关状态（`hasMore`、`videoPage`、`handleLoadMore`，在 `src/components/DetailPanel.tsx` 中）
- [ ] T018 运行 `npx tsc --noEmit` + `npm run build` 验证

**Checkpoint**: US2 可独立验证——后台自动加载，切换订阅取消，错误静默处理

---

## Phase 5: User Story 3 - 下载中视频实时展示进度（Priority: P3）

**Goal**: 下载中的视频条目实时更新进度条和百分比

**Independent Test**: 触发下载任务，观察条目进度条从 0%→100%，完成后状态变为"已完成"

### Implementation for User Story 3

- [ ] T019 [US3] 在统一列表的下载中条目中嵌入 `DownloadProgressBar` 组件（复用，位于第 2 列第 3 行），传入来自 `progressMap` 的 `DownloadProgress`（`src/components/DetailPanel.tsx`）
- [ ] T020 [US3] 监听 `records-changed` 事件：下载完成/失败时自动更新对应条目的状态，从"下载中"切换为"已完成"或"失败"（`src/components/DetailPanel.tsx`）
- [ ] T021 [US3] 监听 `queue-changed` 事件：新下载加入队列时更新列表顶部，移除已完成/取消的队列任务条目（`src/components/DetailPanel.tsx`）
- [ ] T022 [US3] 实现下载中条目的第 3 列操作按钮：暂停（⏸）调用 `onPauseDownload`，取消（✕）调用 `onCancelDownload`（`src/components/DetailPanel.tsx`）
- [ ] T023 运行 `npx tsc --noEmit` + `npm run build` 验证

**Checkpoint**: US3 可独立验证——进度条实时更新，状态自动切换，操作按钮可用

---

## Phase 6: 清理与收尾

**Purpose**: 移除废弃组件，最终验证

- [ ] T024 [P] 删除 `src/components/DownloadRecordList.tsx`
- [ ] T025 [P] 删除 `src/components/DownloadRecordItem.tsx`（确认 DetailPanel 内联了所有渲染逻辑）
- [ ] T026 [P] 确认 `src/components/DownloadProgressBar.tsx` 保留（被 US3 复用），`src/components/DownloadedList.tsx` 和 `src/components/DownloadedItem.tsx` 无变更
- [ ] T027 运行 `npx tsc --noEmit` + `cargo test` + `npm run build` 全量验证
- [ ] T028 执行 `quickstart.md` 中的手动验证清单

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖 — 可立即开始
- **基础 (Phase 2)**: 依赖 Setup 完成 — **阻塞所有用户故事**
- **US1 (Phase 3)**: 依赖基础阶段完成
- **US2 (Phase 4)**: 依赖 US1 完成（需要在统一列表中追加数据）
- **US3 (Phase 5)**: 依赖 US1 完成（需要在统一列表中显示进度）
- **清理 (Phase 6)**: 依赖所有用户故事完成

### User Story Dependencies

- **US1 (P1)**: 基础阶段后即可开始 — 无其他故事依赖
- **US2 (P2)**: 基础阶段后即可开始 — 与 US1 同文件但逻辑独立
- **US3 (P3)**: 基础阶段后即可开始 — 与 US1 同文件但逻辑独立

### Within Each User Story

- 合并逻辑 → 渲染结构 → 状态特定渲染
- 基础功能 → 编译验证

### Parallel Opportunities

- T001 和 T002 可并行（不同文件/函数）
- T024、T025、T026 可并行（不同文件）
- US2 和 US3 可在 US1 完成后并行开发（虽然同文件，但逻辑正交）

---

## Parallel Example: Phase 6 Cleanup

```bash
# 删除废弃组件（并行）：
Task: "删除 src/components/DownloadRecordList.tsx"
Task: "删除 src/components/DownloadRecordItem.tsx"
Task: "确认保留/无变更组件"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup — 类型定义
2. Complete Phase 2: 基础 — 合并逻辑 + 基本渲染
3. Complete Phase 3: US1 — 统一列表展示
4. **STOP and VALIDATE**: 手动验证 US1 的 3 个验收场景
5. 可交付 MVP

### Incremental Delivery

1. Setup + 基础 → 类型与合并框架就绪
2. US1 → 统一列表可用（MVP）
3. US2 → 后台自动加载（增强体验）
4. US3 → 实时进度（增强体验）
5. Phase 6 → 清理旧代码

### 建议顺序

由于所有 US 都在同一文件 `DetailPanel.tsx` 中实现，建议按优先级顺序（US1 → US2 → US3）依次完成，避免合并冲突。

---

## Notes

- [P] 任务 = 不同文件，无依赖
- [Story] 标签映射到 spec.md 中的用户故事，便于追踪
- 所有变更集中在 `src/components/DetailPanel.tsx`、`src/types/index.ts` 和组件删除
- 后端 `src-tauri/` 无变更，`cargo test` 保持全部通过
- `DownloadedList` / `DownloadedItem` / `DownloadProgressBar` 保留不变
- 每次 checkpoint 后运行 `npx tsc --noEmit` 验证类型
