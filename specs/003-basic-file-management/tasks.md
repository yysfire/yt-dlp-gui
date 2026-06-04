# Tasks: 基本文件管理

**Input**: Design documents from `specs/003-basic-file-management/`

**Prerequisites**: plan.md (✓), spec.md (✓), research.md (✓), data-model.md (✓), contracts/ (✓), quickstart.md (✓)

**Tests**: 包含测试任务（项目章程要求 TDD）

**Organization**: 按用户故事分组，支持独立实现和独立测试

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 所属用户故事（US1, US2, US3, US4）
- 所有任务包含精确文件路径

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 类型定义和模块声明，所有用户故事的前置条件

- [x] T001 [P] 更新 TypeScript DownloadRecord status 联合类型，追加 "deleted"，新增 FileExistenceResult 接口 in `src/types/index.ts`
- [x] T002 [P] 更新 Rust DownloadRecord 注释，补充 "deleted" 状态说明 in `src-tauri/src/models/download.rs`
- [x] T003 声明 file_manager 模块 in `src-tauri/src/services/mod.rs`
- [x] T004 在 invoke_handler 中注册 4 个新命令 in `src-tauri/src/lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 所有用户故事依赖的 Service 层和 IPC 封装

**⚠️ CRITICAL**: 此阶段完成后才能开始任何用户故事

- [x] T005 实现 DownloadRecord status "deleted" 序列化/反序列化测试 in `src-tauri/src/models/download.rs`（测试先行）
- [x] T006 创建 FileManagerService 骨架（模块文件 + pub struct） in `src-tauri/src/services/file_manager.rs`
- [x] T007 实现 check_files_exist 测试（build 多个临时文件 + 向量化结果断言） in `src-tauri/src/services/file_manager.rs`
- [x] T008 实现 check_files_exist — spawn_blocking 并发 50 批量检测文件存在性 in `src-tauri/src/services/file_manager.rs`
- [x] T009 在前端 tauri.ts 中新增 openInFolder、checkFileExistence、deleteFile、syncFileStates 4 个 invoke 封装 in `src/lib/tauri.ts`

**Checkpoint**: 基础设施就绪 — 可以开始用户故事实现

---

## Phase 3: User Story 1 - 查看已下载视频列表 (Priority: P1) 🎯 MVP

**Goal**: 用户通过左侧边栏"已下载"导航进入文件管理视图，浏览所有已下载视频列表（标题、频道、下载时间、文件大小）

**Independent Test**: 下载几个视频后，点击侧边栏"已下载"导航项，验证列表正确显示

### Tests for User Story 1

> **先编写测试，确认测试 FAIL 后再实现**

- [x] T010 [P] [US1] 编写 DownloadedList 空状态渲染测试（无记录时显示"暂无已下载的视频"） in `src/components/DownloadedList.tsx`
- [x] T011 [P] [US1] 编写 DownloadedItem 正常渲染测试（标题、频道、下载时间、文件大小均显示） in `src/components/DownloadedItem.tsx`

### Implementation for User Story 1

- [x] T012 [P] [US1] 在 AppShell 左侧边栏新增"已下载"导航项，实现视图切换状态 in `src/components/AppShell.tsx`
- [x] T013 [P] [US1] 创建 DownloadedList 组件（获取所有下载记录并渲染为列表） in `src/components/DownloadedList.tsx`
- [x] T014 [P] [US1] 创建 DownloadedItem 组件（单行：标题/频道/大小/时间，文件缺失状态灰色显示） in `src/components/DownloadedItem.tsx`
- [x] T015 [US1] 扩展 useDownloadRecords Hook，新增 getAllRecords 和按 status 筛选逻辑 in `src/hooks/useDownloadRecords.ts`

**Checkpoint**: US1 完成 — 侧边栏导航 + 已下载列表完整可用，可独立演示

---

## Phase 4: User Story 2 - 打开文件所在位置 (Priority: P2)

**Goal**: 用户从已下载视频列表中点击按钮，一键打开文件所在文件夹（跨平台）

**Independent Test**: 在列表中点击"打开文件夹"按钮，验证系统文件管理器在正确路径打开

### Tests for User Story 2

- [x] T016 [P] [US2] 编写 open_in_folder 命令测试 — 正常路径在 `src-tauri/src/commands/download.rs`（测试环境跳过实际 shell 调用）
- [x] T017 [P] [US2] 编写 delete_file 命令测试 — mock StorageService 验证 status 更新为 "deleted" in `src-tauri/src/commands/download.rs`
- [x] T018 [P] [US2] 编写 DownloadedItem "打开文件夹"按钮交互测试 in `src/components/DownloadedItem.tsx`

### Implementation for User Story 2

- [x] T019 [P] [US2] 实现 open_in_folder 命令（解析父目录，调用 ShellExt::open） in `src-tauri/src/commands/download.rs`
- [x] T020 [P] [US2] 实现 delete_file 命令（std::fs::remove_file + 更新 status 为 "deleted" + 持久化） in `src-tauri/src/commands/download.rs`
- [x] T021 [US2] 在 DownloadedItem 中添加"打开文件夹"和"删除"操作按钮 in `src/components/DownloadedItem.tsx`

**Checkpoint**: US2 完成 — 打开文件夹和删除文件操作均可独立验证

---

## Phase 5: User Story 3 - 搜索已下载视频 (Priority: P2)

**Goal**: 用户通过搜索框输入关键词，实时筛选已下载视频列表

**Independent Test**: 下载多个不同标题的视频，在搜索框中输入部分标题关键词，验证筛选结果准确且不区分大小写

### Tests for User Story 3

- [x] T022 [P] [US3] 编写搜索过滤纯函数测试（不区分大小写匹配、中文匹配、空字符串还原、无匹配显示空状态） in `src/components/FileSearchBar.tsx`
- [x] T023 [P] [US3] 编写 DownloadedList 搜索集成测试（输入关键词 → 列表过滤 → 清除关键词 → 恢复全量） in `src/components/DownloadedList.tsx`

### Implementation for User Story 3

- [x] T024 [US3] 创建 FileSearchBar 组件（受控输入框 + 实时 onChange 触发过滤） in `src/components/FileSearchBar.tsx`
- [x] T025 [US3] 在 DownloadedList 中集成 FileSearchBar 和 useMemo 搜索过滤逻辑 in `src/components/DownloadedList.tsx`

**Checkpoint**: US3 完成 — 搜索功能独立可用，不影响其他用户故事

---

## Phase 6: User Story 4 - 文件系统同步检测 (Priority: P3)

**Goal**: 系统启动时 + 手动触发 + 后台定时（5 分钟）检测文件是否存在，标记已缺失的文件

**Independent Test**: 在文件管理器中手动删除某个已下载视频文件，触发同步（手动或等待定时），验证该视频被标记为"文件已缺失"

### Tests for User Story 4

- [x] T026 [P] [US4] 编写 check_file_existence 命令测试 — 存在和不存在的文件混合 in `src-tauri/src/commands/download.rs`
- [x] T027 [P] [US4] 编写 sync_file_states 命令测试 — 模拟加载记录 → 批量检测 → 返回结果 in `src-tauri/src/commands/download.rs`
- [x] T028 [P] [US4] 编写文件同步完成后事件处理测试（前端的 file-sync-complete 监听） in `src/hooks/useDownloadRecords.ts`

### Implementation for User Story 4

- [x] T029 [P] [US4] 实现 check_file_existence 命令（调用 FileManagerService::check_files_exist） in `src-tauri/src/commands/download.rs`
- [x] T030 [US4] 实现 sync_file_states 命令（加载 completed 记录 → 批量检测 → 发送 file-sync-complete 事件） in `src-tauri/src/commands/download.rs`
- [x] T031 [US4] 在启动流程中调用 sync_file_states（应用启动时自动检测） in `src-tauri/src/lib.rs`
- [x] T032 [US4] 实现后台定时同步（复用 SchedulerService::start(5, callback)，发送 file-sync-complete 事件） in `src-tauri/src/services/file_manager.rs`
- [x] T033 [US4] 在 useDownloadRecords 中监听 "file-sync-complete" 事件并更新文件存在性标记 in `src/hooks/useDownloadRecords.ts`
- [x] T034 [US4] 在 DownloadedList 中添加"手动刷新"按钮触发 sync_file_states in `src/components/DownloadedList.tsx`

**Checkpoint**: US4 完成 — 文件同步检测完整可用，三种触发方式均能正常工作

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 跨用户故事的改进和验证

- [x] T035 在 DownloadedItem 中根据 status="deleted" 渲染"已删除"标记（灰色 + 图标变化） in `src/components/DownloadedItem.tsx`
- [x] T036 [P] 验证 500 条记录加载性能 <2s（SC-001） in performance profiling
- [x] T037 [P] 验证搜索响应时间 <500ms（SC-002） in performance profiling
- [x] T038 [P] 验证文件同步 500 个文件 <10s（SC-004） in performance profiling
- [x] T039 运行全部 Rust 测试 (`cargo test`) 确认通过
- [x] T040 运行 TypeScript 类型检查 (`npx tsc --noEmit`) 确认通过
- [x] T041 按 quickstart.md 验收场景清单逐条验证所有 Given/When/Then

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖 — 可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成 — BLOCKS 所有用户故事
- **User Story 1 (Phase 3)**: 依赖 Foundational 完成
- **User Story 2 (Phase 4)**: 依赖 Foundational 完成 — 可与 US1 并行，但 UI 集成依赖 US1 的 DownloadedItem
- **User Story 3 (Phase 5)**: 依赖 Foundational 完成 — 可与 US1 并行，但 UI 集成依赖 US1 的 DownloadedList
- **User Story 4 (Phase 6)**: 依赖 Foundational 完成 — 可与其他故事并行
- **Polish (Phase 7)**: 依赖所有用户故事完成

### User Story Dependencies

```
Phase 1: Setup
    │
    ▼
Phase 2: Foundational
    │
    ├──► Phase 3: US1 (P1) ★ MVP
    │         │
    │         ├──► Phase 4: US2 (P2)  ── 依赖 US1 的 DownloadedItem 组件存在
    │         └──► Phase 5: US3 (P2)  ── 依赖 US1 的 DownloadedList 组件存在
    │
    └──► Phase 6: US4 (P3) ── 独立，可与 US1 并行
              │
              ▼
         Phase 7: Polish
```

### Within Each User Story

- 测试 MUST 先编写并确认 FAIL 后再实现
- Service 层 → 命令层 → 前端 UI 组件
- 每个用户故事完成后可独立验证

### Parallel Opportunities

- Phase 1 中 T001、T002 可并行
- Phase 2 中 T005、T007 可并行
- Phase 3 中 T010、T011 可并行；T012、T013、T014 可并行
- Phase 4 中 T016、T017、T018 可并行
- Phase 5 中 T022、T023 可并行
- Phase 6 中 T026、T027、T028 可并行；T029 和 T032 可并行
- US2、US3、US4 在 Foundational 完成后可并行启动（如有多人）
- Phase 7 中 T036、T037、T038 可并行

---

## Parallel Example: User Story 1

```bash
# 并行启动 US1 的所有测试:
Task: "T010 [P] [US1] 编写 DownloadedList 空状态渲染测试 in src/components/DownloadedList.tsx"
Task: "T011 [P] [US1] 编写 DownloadedItem 正常渲染测试 in src/components/DownloadedItem.tsx"

# 并行启动 US1 的所有组件:
Task: "T012 [P] [US1] AppShell 侧边栏导航项 in src/components/AppShell.tsx"
Task: "T013 [P] [US1] DownloadedList 组件 in src/components/DownloadedList.tsx"
Task: "T014 [P] [US1] DownloadedItem 组件 in src/components/DownloadedItem.tsx"
```

---

## Implementation Strategy

### MVP First (仅 User Story 1)

1. 完成 Phase 1: Setup
2. 完成 Phase 2: Foundational（关键 — 阻塞所有故事）
3. 完成 Phase 3: User Story 1
4. **STOP 并验证**: 独立测试 US1
5. 可演示/合并 MVP

### Incremental Delivery

1. Setup + Foundational → 基础就绪
2. US1 (P1) → 测试 → 演示 (MVP!)
3. US2 (P2) → 测试 → 演示
4. US3 (P2) → 测试 → 演示
5. US4 (P3) → 测试 → 演示
6. Polish → 最终验证 → 合并

### Parallel Team Strategy

多人协作时：

1. 全队完成 Setup + Foundational
2. Foundational 完成后：
   - 开发者 A: User Story 1（P1，关键路径）
   - 开发者 B: User Story 4（P3，独立，可并行启动）
3. US1 完成后：
   - 开发者 A: User Story 2（P2）
   - 开发者 B: User Story 3（P2）

---

## Notes

- [P] 任务 = 不同文件，无依赖，可并行
- [Story] 标签将任务映射到特定用户故事以便追溯
- 每个用户故事应可独立完成和测试
- 测试先行——确认测试 FAIL 后再实现
- 每个任务或逻辑组完成后提交
- 可在任何 Checkpoint 处停止以独立验证故事
- FileManagerService 保持无状态（遵循项目设计规则）
- 所有 Tauri 命令层不包含业务逻辑（遵循 Commands → Services 分层）
