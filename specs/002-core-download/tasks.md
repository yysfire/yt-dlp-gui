# Tasks: 核心下载功能

**Input**: Design documents from `specs/002-core-download/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/, quickstart.md

**Tests**: 根据项目章程（Constitution I. TDD），Rust 后端所有新功能 MUST 包含单元测试。测试任务在每个 Phase 中优先于实现任务。

**Organization**: 任务按用户故事分组，支持独立实现和测试。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 所属用户故事（US1, US2, US3, US4, US5）
- 包含精确文件路径

## Path Conventions

- **Rust 后端**: `src-tauri/src/`
- **TypeScript 前端**: `src/`

---

## Phase 1: Setup（项目初始化）

**Purpose**: 确认开发环境就绪，现有代码可编译通过

- [X] T001 验证 proyecto 构建通过：运行 `cargo check` 和 `npx tsc --noEmit` 确认无编译错误
- [X] T002 验证现有 Rust 测试通过：运行 `cargo test` 确认全部 77 个测试通过

**Checkpoint**: 开发环境就绪，可以开始开发

---

## Phase 2: Foundational（基础数据模型 — 阻塞所有用户故事）

**Purpose**: 共享数据模型变更和工具模块。所有用户故事依赖此阶段完成。

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests for Foundational（先编写，确保 FAIL）

- [X] T003 [P] 编写 VideoInfo.id 反序列化测试：添加 `id` 字段和有/无 `id` 的反序列化用例在 `src-tauri/src/services/ytdlp.rs` 的 `#[cfg(test)]` 模块中
- [X] T004 [P] 编写 DownloadRecord 新字段测试：`video_id` 默认值、`error_message` 序列化往返、新增状态值 `paused`/`cancelled` 在 `src-tauri/src/models/download.rs` 的 `#[cfg(test)]` 模块中
- [X] T005 [P] 编写 AppSettings 新字段测试：`max_concurrent_downloads` 默认值为 1，序列化往返在 `src-tauri/src/models/settings.rs` 的 `#[cfg(test)]` 模块中
- [X] T006 [P] 编写 progress_parser 测试：正常进度行解析、格式异常回退、空字符串处理在 `src-tauri/src/utils/progress_parser.rs` 的 `#[cfg(test)]` 模块中

### Models Implementation

- [X] T007 在 `VideoInfo` 结构体中新增 `id: Option<String>` 字段（serde alias），更新相关反序列化代码在 `src-tauri/src/services/ytdlp.rs`
- [X] T008 在 `DownloadRecord` 结构体中新增 `video_id: String`（默认 `""`）、`error_message: Option<String>`（默认 `None`），在 `src-tauri/src/models/download.rs`
- [X] T009 在 `AppSettings` 结构体中新增 `max_concurrent_downloads: u32`（默认 1，范围 1-3），在 `src-tauri/src/models/settings.rs`

### Utilities Implementation

- [X] T010 实现 `progress_parser.rs`：函数 `parse_progress_line(line: &str) -> Option<ProgressEvent>` 解析 yt-dlp `--progress-template` 输出格式（`percent|speed|downloaded_bytes|total_bytes|eta`）在 `src-tauri/src/utils/progress_parser.rs`
- [X] T011 在 `lib.rs` 中声明 `mod progress_parser` 和 `pub use` 导出在 `src-tauri/src/lib.rs`

### TypeScript Types & API Stubs

- [X] T012 [P] 更新 TypeScript 类型：`DownloadRecord` 新增 `video_id`、`error_message`，新增 `DownloadProgress`、`TaskStatus`、`DownloadTask`、`QueueState` 接口在 `src/types/index.ts`
- [X] T013 [P] 新增 Tauri invoke 函数类型签名桩（不含实现）：`pauseDownload`、`resumeDownload`、`cancelDownload`、`getDownloadQueue`、`getQueueState` 在 `src/lib/tauri.ts`

**Checkpoint**: 数据模型就绪，运行 `cargo test` 确认新测试全部通过，`npx tsc --noEmit` 类型检查通过

---

## Phase 3: User Story 5 - 重复下载避免（Priority: P1）

**Goal**: 基于视频 ID 判断重复下载，失败记录允许重试，记录失败原因

**Independent Test**: 手动触发两次对同一频道的检查，验证已下载的视频不会再次进入下载队列；模拟下载失败后再次检查，确认失败视频可重试

### Tests for US5

- [X] T014 [P] [US5] 编写去重逻辑测试：`VideoInfo` 的 `id` 存在时按 `video_id` 去重，`id` 为 `None` 时回退按 `video_url` 去重在 `src-tauri/src/services/ytdlp.rs` 的 `#[cfg(test)]` 模块中

### Implementation for US5

- [X] T015 [US5] 重构 `check_and_download()` 中的去重逻辑：优先使用 `video_id` 建立 `HashSet`，`video_id` 不存在时回退到 `video_url`，在 `src-tauri/src/commands/download.rs`
- [X] T016 [US5] 更新去重跳过逻辑：状态为 `failed` 的记录允许重新加入下载队列，状态为 `completed` 或 `cancelled` 的记录跳过，在 `src-tauri/src/commands/download.rs`
- [X] T017 [US5] 在下载失败路径中记录 `error_message`：将 yt-dlp 的 stderr 错误信息写入 `DownloadRecord.error_message`，在 `src-tauri/src/commands/download.rs`
- [X] T018 [US5] 更新 `DownloadRecord::new()` 构造函数：接收 `video_id` 参数，在 `src-tauri/src/models/download.rs`

**Checkpoint**: US5 独立可测 — 重复视频不下载，失败视频可重试，失败原因可见

---

## Phase 4: User Story 2 - FIFO 下载队列（Priority: P1）

**Goal**: 下载任务按 FIFO 顺序排队，支持可配置并发数，队列任务状态在前端可见

**Independent Test**: 触发多个视频下载，观察队列按检测顺序依次进行下载

### Tests for US2

- [X] T019 [P] [US2] 编写 `DownloadQueue` 单元测试：入队顺序（FIFO）、并发控制（Semaphore 限制）、状态转换（Waiting→Running→Completed）、清空队列在 `src-tauri/src/services/download_queue.rs` 的 `#[cfg(test)]` 模块中

### Implementation for US2

- [X] T020 [US2] 创建 `DownloadQueue` 结构体：`queue: Arc<Mutex<VecDeque<DownloadTask>>>`、`semaphore: Arc<Semaphore>`、`tasks: Arc<Mutex<JoinSet>>>`、`app_handle: AppHandle`，方法 `new()`、`enqueue()`、`start()` 在 `src-tauri/src/services/download_queue.rs`
- [X] T021 [US2] 实现 `DownloadTask` 结构体：包含 `id`、`video_id`、`video_url`、`video_title`、`subscription_id`、`quality`、`status: TaskStatus`、`progress`、`error_message`、`created_at`、`completed_at` 在 `src-tauri/src/services/download_queue.rs`
- [X] T022 [US2] 实现 `TaskStatus` 枚举：`Waiting`、`Running`、`Paused`、`Completed`、`Failed`、`Cancelled` 及 `Display` trait，在 `src-tauri/src/services/download_queue.rs`
- [X] T023 [US2] 实现 `enqueue()` 方法：创建 `DownloadTask` 并 push 到 `VecDeque`，发射 `queue-changed` 事件，尝试 `start()` 调度，在 `src-tauri/src/services/download_queue.rs`
- [X] T024 [US2] 实现 `start()` 方法：从 `VecDeque` pop 并 `tokio::spawn` 下载任务（通过 Semaphore 控制并发），调用 `download_video()` 完成后更新状态和记录，release Semaphore 后递归调用 `start()` 在 `src-tauri/src/services/download_queue.rs`
- [X] T025 [US2] 实现 `get_queue_state()` 命令：返回 `QueueState { active_count, waiting_count, max_concurrent }` 在 `src-tauri/src/commands/download.rs`
- [X] T026 [US2] 实现 `get_download_queue()` 命令：返回 `Vec<DownloadTask>`（Serialize），在 `src-tauri/src/commands/download.rs`
- [X] T027 [US2] 修改 `check_and_download()`：不再直接调用 `download_video()`，而是创建 `DownloadTask` 并入队到 `DownloadQueue`，保留记录创建和 `records-changed` 事件在 `src-tauri/src/commands/download.rs`
- [X] T028 [US2] 在 `lib.rs` 中初始化 `DownloadQueue`：作为 `AppContext` 的一部分或通过 `app.manage()` 注入全局实例，注册新命令在 `src-tauri/src/lib.rs`
- [X] T029 [US2] 实现前端 `useDownloadRecords` hook 新增方法：`getQueueState()` 获取队列状态，监听 `queue-changed` 事件在 `src/hooks/useDownloadRecords.ts`
- [X] T030 [US2] 实现前端 Tauri API 函数：`getDownloadQueue()`、`getQueueState()` 在 `src/lib/tauri.ts`

**Checkpoint**: US2 独立可测 — 检测到的视频自动排队，按 FIFO 顺序下载，并发数可配置

---

## Phase 5: User Story 1 - 自动定时检查更新（Priority: P1）

**Goal**: 调度器与下载队列集成，应用重启后状态自动恢复

**Independent Test**: 设置 5 分钟检查间隔，添加新视频后等待检查触发，验证新视频被检测到并加入下载队列

### Tests for US1

- [X] T031 [P] [US1] 编写状态恢复测试：模拟 `download_records.json` 中有 `downloading`/`paused` 状态的记录，验证 `recover_state()` 将其标记为 `failed` 并设置 `error_message` 在 `src-tauri/src/commands/download.rs` 的 `#[cfg(test)]` 模块中
- [X] T032 [P] [US1] 编写调度器集成测试：验证 `SchedulerService::start()` callback 调用队列的 `enqueue()` 在 `src-tauri/src/services/scheduler.rs` 的 `#[cfg(test)]` 模块中

### Implementation for US1

- [X] T033 [US1] 修改后台调度器（`lib.rs` setup 闭包）：检测到新视频后调用 `DownloadQueue::enqueue()` 而非直接下载，保持错误日志在 `src-tauri/src/lib.rs`
- [X] T034 [US1] 修改 `start_scheduler` 命令（commands/settings.rs）：同步简化，移除内联调度逻辑（调度由 lib.rs setup 统一管理），仅记录日志在 `src-tauri/src/commands/settings.rs`
- [X] T035 [US1] 实现 `recover_state()` 函数：启动时读取 `download_records.json`，将状态为 `downloading` 或 `paused` 的记录标记为 `failed`（设置 `error_message = "Application restarted"`），在 `src-tauri/src/commands/download.rs` 或 `lib.rs` setup 中调用
- [X] T036 [US1] 更新 `lib.rs` setup：添加 `recover_state()` 调用，添加 `scheduler-check-complete` 事件中携带队列统计信息，在 `src-tauri/src/lib.rs`

**Checkpoint**: US1 独立可测 — 定时检查正常触发，新视频自动入队，重启后状态正确恢复

---

## Phase 6: User Story 3 - 下载进度显示（Priority: P2）

**Goal**: 实时显示下载进度（百分比、速度、大小、ETA）

**Independent Test**: 启动大文件下载，观察进度条和速度信息实时更新

### Tests for US3

- [X] T037 [P] [US3] 编写流式下载测试：构造 `yt-dlp --progress-template` 输出模拟管道，验证 `LineStream` 行级解析和 `parse_progress_line()` 返回正确 `ProgressEvent` 在 `src-tauri/src/services/ytdlp.rs` 的 `#[cfg(test)]` 模块中

### Implementation for US3 (Rust Backend)

- [X] T038 [US3] 实现 `YtDlpService::download_video_streaming()`：使用 `tokio::process::Command` 替代 `std::process::Command`，添加 `--progress-template` 参数，通过 `stdout` pipe 行级读取进度输出，在 `src-tauri/src/services/ytdlp.rs`
- [X] T039 [US3] 在 `DownloadQueue::start()` 中替换下载调用：使用 `download_video_streaming()` 替代 `download_video()`，解析进度行并通过 `app_handle.emit("download-progress", ...)` 推送 `ProgressEvent`，在 `src-tauri/src/services/download_queue.rs`
- [X] T040 [US3] 新增 Tauri Event `download-progress` payload 类型定义：`DownloadProgressEvent { task_id, percent, speed, downloaded_bytes, total_bytes, eta }`，在 `src-tauri/src/services/download_queue.rs` 或单独模块

### Implementation for US3 (Frontend)

- [X] T041 [US3] 实现 `useDownloadProgress` hook：监听 `download-progress` 事件，维护 `Map<taskId, DownloadProgress>` 状态，提供 `getProgress(taskId)` 方法，在 `src/hooks/useDownloadProgress.ts`
- [X] T042 [US3] 实现 `DownloadProgressBar` 组件：显示进度条（`LinearProgress`）、百分比、速度、已下载/总大小、ETA，接收 `DownloadProgress` props，在 `src/components/DownloadProgressBar.tsx`
- [X] T043 [US3] 更新 `App.tsx`：添加 `download-progress` 事件监听，传递 progress 数据到 `AppShell`，在 `src/App.tsx`

**Checkpoint**: US3 独立可测 — 下载过程中进度条实时更新，百分比/速度/大小/ETA 可见

---

## Phase 7: User Story 4 - 暂停/继续/取消（Priority: P2）

**Goal**: 用户可暂停正在下载的任务、继续已暂停的任务、取消不需要的下载

**Independent Test**: 下载过程中依次点击暂停、继续、取消，验证每个操作正确响应

### Tests for US4

- [X] T044 [P] [US4] 编写暂停/继续逻辑测试：`pause_download` 后任务状态转为 `Paused`，`resume_download` 后恢复为 `Running`，状态转换合法性校验在 `src-tauri/src/services/download_queue.rs` 的 `#[cfg(test)]` 模块中
- [X] T045 [P] [US4] 编写取消逻辑测试：`cancel_download` 后状态转为 `Cancelled`，`DownloadRecord` 持久化为 `cancelled`，部分文件路径验证在 `src-tauri/src/services/download_queue.rs` 的 `#[cfg(test)]` 模块中

### Implementation for US4 (Rust Backend)

- [X] T046 [US4] 实现 `DownloadQueue::pause()` 方法：Unix 上通过 `libc::kill(pid, SIGSTOP)` 暂停子进程，Windows 上通过 `kernel32::SuspendThread` 暂停，更新内存中任务状态为 `Paused`，发射 `queue-changed` 事件在 `src-tauri/src/services/download_queue.rs`
- [X] T047 [US4] 实现 `DownloadQueue::resume()` 方法：Unix 上 `libc::kill(pid, SIGCONT)`，Windows 上 `kernel32::ResumeThread`，更新状态为 `Running`，发射 `queue-changed` 事件在 `src-tauri/src/services/download_queue.rs`
- [X] T048 [US4] 实现 `DownloadQueue::cancel()` 方法：取消 Waiting 任务（从 VecDeque 移除），取消 Running/Paused 任务（`Child::kill()` + 清理 `.part`/`.ytdl` 临时文件），更新 `DownloadRecord` 状态为 `cancelled`，发射 `records-changed` 事件在 `src-tauri/src/services/download_queue.rs`
- [X] T049 [US4] 实现 `DownloadQueue::cleanup_partial_files()` 方法：根据下载目录和视频标题匹配删除 `.part`、`.ytdl` 后缀的临时文件，在 `src-tauri/src/services/download_queue.rs`
- [X] T050 [US4] 实现 Tauri 命令 `pause_download`：接收 `id: String`，调用 `DownloadQueue::pause()`，在 `src-tauri/src/commands/download.rs`
- [X] T051 [US4] 实现 Tauri 命令 `resume_download`：接收 `id: String`，调用 `DownloadQueue::resume()`，在 `src-tauri/src/commands/download.rs`
- [X] T052 [US4] 实现 Tauri 命令 `cancel_download`：接收 `id: String`，调用 `DownloadQueue::cancel()` 含文件清理，在 `src-tauri/src/commands/download.rs`
- [X] T053 [US4] 在 `lib.rs` 中注册新命令：`pause_download`、`resume_download`、`cancel_download`、`get_download_queue`、`get_queue_state`，在 `src-tauri/src/lib.rs`
- [X] T054 [US4] 在 `Cargo.toml` 中添加条件依赖（如需要）：`[target.'cfg(unix)'.dependencies]` 添加 `libc` crate 用于信号处理，在 `src-tauri/Cargo.toml`

### Implementation for US4 (Frontend)

- [X] T055 [US4] 实现前端 API 调用函数：`pauseDownload(id)`、`resumeDownload(id)`、`cancelDownload(id)` 在 `src/lib/tauri.ts`
- [X] T056 [US4] 实现 `DownloadQueuePanel` 组件：展示队列中所有 `DownloadTask`（含状态标签、进度条），每个任务行的操作按钮（暂停/继续/取消），根据当前状态显示/隐藏按钮在 `src/components/DownloadQueuePanel.tsx`
- [X] T057 [US4] 集成 `DownloadQueuePanel` 到 `AppShell`：替换或增强当前的 `DownloadRecordList`，传递 `useDownloadProgress` 数据和操作回调在 `src/components/AppShell.tsx`

**Checkpoint**: US4 独立可测 — 暂停/继续/取消操作正常，文件清理正确，UI 状态联动

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: 边界条件处理、错误恢复、文档完善

- [ ] T058 [P] 实现网络中断处理：下载失败后 `DownloadQueue::start()` 自动启动下一个排队任务（已由现有逻辑覆盖，确认 `start()` 方法在 task 完成/失败后递归调用 `start()`），在 `src-tauri/src/services/download_queue.rs`
- [ ] T059 [P] 实现磁盘空间不足检测：下载前检查下载目录磁盘剩余空间（< 100MB 拒绝入队），返回 `AppError` 变体，在 `src-tauri/src/services/download_queue.rs`
- [ ] T060 [P] 实现 yt-dlp 进程崩溃恢复：`Child::wait()` 返回非零退出码时，将 `stderr` 写入 `error_message`，标记任务为 `Failed`，在 `src-tauri/src/services/download_queue.rs`
- [ ] T061 为 `AppError` 新增变体：`DiskFull(String)`、`ProcessKill(String)`、`QueueError(String)`，在 `src-tauri/src/utils/error.rs`
- [ ] T062 运行 `cargo test` 确认全部单元测试通过（现有 69 + 新增测试），运行 `npx tsc --noEmit` 确认类型检查通过
- [ ] T063 按 `quickstart.md` 执行手动集成测试：添加订阅→手动检查→验证队列→验证进度→暂停/继续/取消→重启恢复

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: 无依赖，可立即开始
- **Phase 2 (Foundational)**: 依赖 Phase 1 — **BLOCKS 所有用户故事**
- **Phase 3 (US5)**: 依赖 Phase 2
- **Phase 4 (US2)**: 依赖 Phase 3（queue 需要 video_id 字段）
- **Phase 5 (US1)**: 依赖 Phase 4（scheduler 需要 queue）
- **Phase 6 (US3)**: 依赖 Phase 4（progress 需要 streaming queue）
- **Phase 7 (US4)**: 依赖 Phase 4（pause/resume/cancel 需要 queue 任务管理）
- **Phase 8 (Polish)**: 依赖 Phase 3-7 全部完成

### User Story Dependencies

```
Phase 2: Foundational
    ↓
Phase 3: US5 (重复下载避免)
    ↓
Phase 4: US2 (FIFO 下载队列)
    ↓
    ├── Phase 5: US1 (自动定时检查)
    ├── Phase 6: US3 (下载进度显示)
    └── Phase 7: US4 (暂停/继续/取消)
```

### Within Each Phase

- Tests（TDD）MUST 先编写并确认 FAIL
- Models 在 services 之前
- Rust 后端在 TypeScript 前端之前
- 核心逻辑在前端 UI 之前

---

## Parallel Execution Opportunities

### Phase 2 (Foundational) Parallelism

```bash
# 4 个测试任务可并行：
T003 ─ VideoInfo.id 反序列化测试
T004 ─ DownloadRecord 新字段测试
T005 ─ AppSettings 新字段测试
T006 ─ progress_parser 测试

# 模型实现可并行：
T007 ─ VideoInfo.id 字段
T008 ─ DownloadRecord 新字段
T009 ─ AppSettings 新字段

# 类型和 API stub 可并行：
T012 ─ TypeScript 类型更新
T013 ─ API 函数签名桩
```

### Phase 6-7 (US3 & US4) Parallelism

```bash
# US3 和 US4 可并行（独立模块，无交叉依赖）：
US3: T038-T043 (进度显示)
US4: T044-T057 (暂停/继续/取消)
```

### Within a Phase - Frontend Parallelism

```bash
# 前端组件可并行开发：
T042 ─ DownloadProgressBar 组件
T056 ─ DownloadQueuePanel 组件
```

---

## Implementation Strategy

### MVP Scope（最小可行产品）

**仅实现 P1 优先级**：Phase 1 → 2 → 3 → 4 → 5

完成后即具备核心自动下载能力：
- 定时检查 → 视频去重 → FIFO 排队 → 自动下载 → 状态持久化

```
Phase 1  Setup        （验证环境）
Phase 2  Foundational （数据模型）
Phase 3  US5          （重复避免 + 失败重试）
Phase 4  US2          （FIFO 队列）
Phase 5  US1          （调度器集成 + 状态恢复）
  ↑ STOP: MVP ready — 核心自动下载功能完成
```

### 完整交付

```
MVP (Phase 1-5) → 验证 → 
Phase 6 (US3 进度显示) → 验证 → 
Phase 7 (US4 暂停/继续/取消) → 验证 → 
Phase 8 (Polish) → 完整版本
```

### TDD 要求（章程 I — 不可协商）

每个 Implementation task 之前 MUST 先完成对应 Test task，确认测试 FAIL，然后实现代码，确认测试 PASS。

---

## Task Summary

| Phase | User Story | 任务数 | 优先级 |
|-------|-----------|--------|--------|
| Phase 1 | Setup | 2 | - |
| Phase 2 | Foundational | 11 | 阻塞 |
| Phase 3 | US5 重复下载避免 | 5 | P1 |
| Phase 4 | US2 FIFO下载队列 | 12 | P1 |
| Phase 5 | US1 自动定时检查 | 6 | P1 |
| Phase 6 | US3 下载进度显示 | 7 | P2 |
| Phase 7 | US4 暂停/继续/取消 | 14 | P2 |
| Phase 8 | Polish | 6 | - |
| **Total** | | **63** | |

### 文件变更统计

| 类型 | 文件数 | 说明 |
|------|--------|------|
| Rust 新增 | 2 | `download_queue.rs`, `progress_parser.rs` |
| Rust 修改 | 6 | `download.rs`, `settings.rs`, `ytdlp.rs`, `scheduler.rs`, `error.rs`, `lib.rs` |
| TypeScript 新增 | 3 | `DownloadQueuePanel.tsx`, `DownloadProgressBar.tsx`, `useDownloadProgress.ts` |
| TypeScript 修改 | 4 | `types/index.ts`, `lib/tauri.ts`, `useDownloadRecords.ts`, `AppShell.tsx` |
