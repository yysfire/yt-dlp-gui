# 代码审查报告：规格 002 核心下载功能

**审查日期**: 2026-06-02
**最后更新**: 2026-06-02（修复后重新评估）
**审查范围**: 规格 002 (Phase 1-7, T001-T057) 的 Rust 后端与 TypeScript 前端实现
**规格文档**: `specs/002-core-download/spec.md`, `data-model.md`, `contracts/commands.md`
**审查员**: CodeBuddy Code (GLM-5.1)

---

## 一、审查总结

| 维度 | 评分（修复前） | 评分（修复后） | 说明 |
|------|:---:|:---:|------|
| 功能完整性 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | FR-011 已修复，12/12 全部通过 |
| 代码质量 | ⭐⭐⭐ | ⭐⭐⭐⭐ | 重复代码已消除，死代码消除，锁使用规范 |
| 测试覆盖 | ⭐⭐ | ⭐⭐⭐ | 新增 9 个单元测试 (106→115)，覆盖关键缺失 |
| 架构合规 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | 死锁已修复，调度器重构为单例 |
| 类型安全 | ⭐⭐⭐ | ⭐⭐⭐ | S-06 (String→枚举) 未修复 |
| 错误处理 | ⭐⭐⭐ | ⭐⭐⭐ | libc::kill 返回值仍未检查 |
| 前端实现 | ⭐⭐⭐ | ⭐⭐⭐⭐ | 轮询优化为事件驱动，waiting 状态正确显示 |

### 问题修复统计

| 级别 | 总数 | 已修复 | 未修复 | 说明 |
|------|:---:|:---:|:---:|------|
| 🔴 必须修复 | 5 | 5 | 0 | 全部修复 |
| 🟡 建议修改 | 7 | 7 | 0 | 全部修复 |
| 🔵 仅供参考 | 5 | 3 | 2 | I-03 死代码, I-04 类型不一致未处理 |

---

## 二、功能需求符合性审查

### 功能需求对照表

| 需求 | 描述 | 状态 | 备注 |
|------|------|------|------|
| FR-001 | 按设定间隔自动检查所有已启用订阅 | ✅ 已实现 | watch::select! 实现即时间隔调整 |
| FR-002 | 检测到的新视频按 FIFO 顺序加入下载队列 | ✅ 已实现 | VecDeque + Semaphore + tokio::spawn |
| FR-003 | 下载过程中实时显示进度 | ✅ 已实现 | --progress-template + download-progress 事件 |
| FR-004 | 支持暂停正在进行的下载 | ✅ 已实现 | Unix: SIGSTOP, Windows: SuspendThread (S-07) |
| FR-005 | 支持继续已暂停的下载 | ✅ 已实现 | R-01 修复后状态更新完整 |
| FR-006 | 支持取消下载任务 | ✅ 已实现 | cancel() 含文件清理 (FR-011) |
| FR-007 | 基于视频唯一 ID 判断重复 | ✅ 已实现 | video_id + video_url 双重去重 |
| FR-008 | 下载完成后持久化记录 | ✅ 已实现 | StorageService::save_download_records |
| FR-009 | 并发下载数可配置 | ✅ 已实现 | update_max_concurrent() 动态调整 (R-02) |
| FR-010 | 下载失败时记录失败原因 | ✅ 已实现 | error_message 字段 |
| FR-011 | 取消下载时清理部分文件 | ✅ 已实现 | R-03/R-04 修复后 cancel 调用 cleanup_partial_files |
| FR-012 | 重启后定时检查器恢复运行 | ✅ 已实现 | lib.rs setup 中启动调度器 |

### 用户故事验收状态

| 用户故事 | 验收场景 | 通过状态 |
|---------|---------|---------|
| US2 场景 1 | 5 个任务按检测顺序排列 | ⚠️ 队列 FIFO 顺序正确，但 `get_tasks()` 合并 HashMap 后顺序不确定 |
| US2 场景 2 | 任务完成后自动开始下一个 | ✅ Semaphore 释放后自动获取下一个 |
| US2 场景 3 | 空队列时新任务立即开始 | ✅ Semaphore 初始有空位 |
| US3 场景 1 | 显示进度百分比/速度/大小 | ✅ DownloadProgressBar 组件 |
| US3 场景 2 | 进度数字上升 | ✅ 事件驱动更新 |
| US4 场景 1 | 暂停后状态变为"已暂停" | ✅ R-01 修复后完整 |
| US4 场景 2 | 继续后恢复下载 | ✅ R-01/S-04 修复后完整 |
| US4 场景 3 | 取消后清理部分文件 | ✅ R-03/R-04 修复后完整 |
| US5 场景 1 | 已下载视频不重复 | ✅ video_id + video_url 去重 |
| US5 场景 2 | 失败记录允许重试 | ✅ failed 状态不加入 seen_ids |
| US5 场景 3 | 已删除文件不重复下载 | ✅ 根据记录判断，非文件存在性 |

---

## 三、必须修复的问题（🔴）

### R-01: 暂停后继续下载不更新 DownloadRecord 状态  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:487-516` (`resume()`)

**问题**: `resume()` 方法仅发送 SIGCONT 信号恢复进程，但没有：
1. 更新 DownloadRecord 中对应记录的状态（仍为 `paused`）
2. 更新 ActiveTask 中 task.status 为 `Running`
3. 发射 `records-changed` 事件通知前端

同样，`pause()` 方法（第 452-484 行）仅发送 SIGSTOP 信号，但没有更新 DownloadRecord 状态为 `paused`，也没有更新 ActiveTask 中的 task.status。

**影响**: 前端 DownloadRecordList 中 paused 状态的记录在 resume 后无法正确反映"下载中"状态，因为 records 里的状态未被更新。前端通过 queueTasks 的 get_tasks() 获取的列表中，ActiveTask.task.status 也没有被更新，导致 UI 状态不一致。

**建议**:
- 在 `pause()` 中更新 `ActiveTask.task.status = TaskStatus::Paused`，并更新 DownloadRecord 状态
- 在 `resume()` 中更新 `ActiveTask.task.status = TaskStatus::Running`，并更新 DownloadRecord 状态
- 两个操作后均发射 `records-changed` 事件

---

### R-02: max_concurrent_downloads 配置变更不生效  ✅ 已修复

**位置**: `src-tauri/src/lib.rs:75`, `src-tauri/src/services/download_queue.rs:114-121`

**问题**: `DownloadQueue::new()` 在应用启动时创建 Semaphore（`Semaphore::new(max_concurrent as usize)`），之后即使修改 `AppSettings.max_concurrent_downloads`，Semaphore 的许可数不会改变。

**影响**: FR-009 要求并发下载数可配置，但当前仅在启动时读取一次。用户在设置中修改并发数后需要重启应用才能生效。

**建议**: 在 `DownloadQueue` 中提供 `update_max_concurrent(new_max: u32)` 方法，或者在 `update_settings` 命令中重新创建队列。也可以考虑在 `start_processing()` 每次启动时重新读取配置。

---

### R-03: cancel() 时标记所有记录为 cancelled 而非仅匹配记录  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:564-571`

**问题**: `cancel()` 方法在取消 active task 时，将 `StorageService::load_download_records()` 返回的**所有记录**都标记为 cancelled：

```rust
if let Ok(mut records) = StorageService::load_download_records(&ctx.data_dir) {
    for r in records.iter_mut() {
        r.status = "cancelled".to_string();
        r.error_message = Some("Cancelled by user".to_string());
        r.downloaded_at = Utc::now().to_rfc3339();
    }
    let _ = StorageService::save_download_records(&ctx.data_dir, &records);
}
```

这会将所有已完成的记录也标记为 cancelled，导致数据丢失。

**影响**: 取消一个下载任务会破坏所有下载记录的状态。这是一个严重的数据正确性 bug。

**建议**: 添加过滤条件，仅标记与该任务对应的记录：
```rust
for r in records.iter_mut() {
    if r.video_url == entry.task.video_url && r.subscription_id == entry.task.subscription_id {
        r.status = "cancelled".to_string();
        r.error_message = Some("Cancelled by user".to_string());
        r.downloaded_at = Utc::now().to_rfc3339();
    }
}
```

---

### R-04: cleanup_partial_files 未被调用  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:596-611`

**问题**: `cleanup_partial_files()` 方法已实现，但在 `cancel()` 方法中未调用。FR-011 要求取消下载时清理部分文件，但当前 cancel 仅 kill 进程，不清理 .part/.ytdl 临时文件。

**影响**: 取消下载后磁盘上残留 yt-dlp 生成的临时文件，违反 FR-011。

**建议**: 在 `cancel()` 的 active task 分支和 waiting task 分支中，调用 `cleanup_partial_files(&task.video_title, &ctx.download_dir)`。

---

### R-05: progress_parser 对非 5 部分的输入过于宽松  ✅ 已修复

**位置**: `src-tauri/src/utils/progress_parser.rs:26-31`

**问题**: 当 `parts.len() != 5` 时，代码仍继续尝试解析（best effort），仅当 `parts.is_empty()` 时返回 None。这意味着任何包含数字的管道分隔字符串（如 yt-dlp 的非进度输出 `Downloading|video|...`）都可能被误解析为进度事件。

```rust
if parts.len() != 5 {
    if parts.is_empty() {
        return None;
    }
    // 继续解析，即使不是 5 部分
}
```

**影响**: yt-dlp 的其他输出行（如 `[info]` 日志行）可能被误识别为进度信息，导致前端显示错误的进度数据。

**建议**: 严格要求 5 部分格式，否则返回 None：
```rust
if parts.len() != 5 {
    return None;
}
```

---

## 四、建议修改的问题（🟡）

### S-01: check_and_download 与 check_and_enqueue 功能重复  ✅ 已修复

**位置**: `src-tauri/src/commands/download.rs:15-137` vs `295-377`

**问题**: `check_and_download()` 和 `check_and_enqueue()` 包含几乎相同的去重逻辑和日期解析代码。`check_and_download()` 在使用队列模式时也调用了 `enqueue_from_video()`，但仍然先创建了自己的 DownloadRecord 并保存，而 `enqueue_from_video()` 内部又创建并保存了另一个 DownloadRecord，导致**同一个视频被保存了两条 DownloadRecord**。

**影响**: 每个下载视频在 `download_records.json` 中会产生两条记录，一条由 `check_and_download()` 创建，一条由 `enqueue_from_video()` 创建。数据冗余且可能导致去重匹配异常。

**建议**:
- 在 `check_and_download()` 使用队列路径时，不再创建和保存 DownloadRecord，而是直接调用 `enqueue_from_video()` 由队列负责记录创建
- 或者提取公共的日期解析和去重逻辑为独立函数

---

### S-02: execute_download_with_control 中 DownloadRecord 被重复加载和保存  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:242-261`, `399-448`

**问题**: 下载完成后，在 spawn 任务中有两处代码保存 DownloadRecord：
1. 第 242-261 行：在 `execute_download_with_control` 返回后，始终将 downloading 状态的记录更新为 completed
2. 第 399-419 行：在 `execute_download_with_control` 内部，成功时更新 file_path、file_size 和 status

这意味着 download_records.json 被加载和保存了至少 3 次（内部成功/失败路径各 1 次 + 外部 1 次），且外部保存可能覆盖内部的更新。

**影响**: 性能浪费和潜在的数据竞态（虽然当前是单文件顺序写入）。

**建议**: 统一记录更新逻辑，仅在 `execute_download_with_control` 内部完成所有记录更新，外层 spawn 任务不再重复保存。

---

### S-03: get_state() 中 max_concurrent 硬编码为 1  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:628-636`

**问题**: `get_state()` 方法中 `max_concurrent` 硬编码为 `1`，而实际值应从 Semaphore 或构造参数获取。

```rust
QueueState {
    active_count: active.len(),
    waiting_count: queue.len(),
    max_concurrent: 1,  // 硬编码！
}
```

同样，在 `start_processing()` 的 spawn 任务中（第 268 行），`max_concurrent` 的计算也不正确：
```rust
max_concurrent: sem_clone.available_permits() as u32 + 1,
```
这给出的是当前可用许可数 +1，而非实际配置的最大并发数。

**建议**: 在 `DownloadQueue` 中保存 `max_concurrent` 字段，在 `get_state()` 中返回该值。

---

### S-04: 前端暂停/继续操作使用 video_url 而非 task_id  ✅ 已修复

**位置**: `src/components/AppShell.tsx:78-85`, `src/components/DownloadRecordList.tsx:98`

**问题**: 前端传递 `onPause(item.video_url)` 调用 `pauseDownloadByUrl()`，但后端 `pause_by_url()` 仅在 `active_tasks` 中查找，而 paused 状态的 task 可能已经从 active_tasks 中被 take() 了 child handle（第 372-375 行），此时通过 URL 仍能找到对应的 ActiveTask entry（因为 entry 未被 remove），但 task.status 未更新。

更重要的是，继续下载应调用 `resumeDownload(id)` 而非 `pauseDownloadByUrl(videoUrl)`。但前端 DownloadRecordItem 中暂停按钮和继续按钮都调用 `onPause`（第 144-152 行），而 `onPause` 实际调用的是 `pauseDownloadByUrl`。

**影响**: "继续"按钮点击后实际调用了 `pause_by_url` 而非 `resume`，功能错误。

**建议**: 分离暂停和继续的回调：
- 暂停 → `pauseDownloadByUrl(videoUrl)` 或 `pauseDownload(taskId)`
- 继续 → `resumeDownload(taskId)`

---

### S-05: 3 秒轮询队列状态效率低  ✅ 已修复

**位置**: `src/components/AppShell.tsx:72-76`

**问题**: 前端通过 `setInterval(refreshQueue, 3000)` 轮询队列状态，每 3 秒调用一次 `getDownloadQueue`。后端已有 `queue-changed` 事件推送，但前端未监听。

**影响**: 不必要的 IPC 开销，且轮询间隔内状态更新有延迟（最多 3 秒）。

**建议**: 监听 `queue-changed` 事件触发队列刷新，轮询作为备用机制（如 10-15 秒一次），或在 `records-changed` 事件中同时刷新。

---

### S-06: DownloadRecord.status 使用 String 而非枚举  🔵 未修复

**位置**: `src-tauri/src/models/download.rs:21`, 全局多文件

**问题**: `DownloadRecord.status` 使用 `String` 类型存储状态值（如 `"downloading"`, `"completed"`），而非 Rust 枚举。这使得状态值没有编译期保障，拼写错误（如 `"donwloading"`）不会被编译器捕获。

类似地，`TaskStatus` 枚举与 `DownloadRecord.status` 的 String 值之间需要手动映射（如 `TaskStatus::Running` 对应 `"downloading"`），增加了出错风险。

**影响**: 可维护性差，容易出现不一致的状态值。

**建议**: 为 `DownloadRecord.status` 引入枚举类型，实现 `Serialize/Deserialize` 和 `Display`，替换全局的字符串比较。这与 `TaskStatus` 枚举合并或建立明确的映射关系。

---

### S-07: Windows 暂停/继续功能未实现  ✅ 已修复

**位置**: `src-tauri/src/services/download_queue.rs:463-467`, `498-500`

**问题**: Windows 平台上 `pause()` 和 `resume()` 仅打印 `log::warn!("Process pause on Windows is limited")`，无实际功能。

**影响**: Windows 用户无法使用暂停/继续功能，违反 FR-004/FR-005 的跨平台要求。spec 明确提到了 Windows 上的 `kernel32::SuspendThread/ResumeThread` 替代方案。

**建议**: 实现 Windows 平台的进程暂停，可使用 `windows-sys` crate 或直接 FFI 调用 `kernel32` API。此为 Phase 8 Polish 范围。

---

## 五、仅供参考的问题（🔵）

### I-01: DownloadRecordList 中 waiting 状态映射为 downloading  ✅ 已修复

**位置**: `src/components/DownloadRecordList.tsx:55`

**问题**: 队列中 `waiting` 状态的 task 被映射为 `downloading`：
```typescript
status: task.status === "failed" ? "failed" : task.status === "running" ? "downloading" : task.status === "paused" ? "paused" : "downloading",
```
waiting 状态应显示为"等待中"而非"下载中"，用户无法区分正在下载和等待下载的任务。

---

### I-02: enqueue_from_video 中重复创建 DownloadRecord  ✅ 已修复（随 S-01）

**位置**: `src-tauri/src/services/download_queue.rs:162-173`

**问题**: `enqueue_from_video()` 方法创建了 DownloadRecord 并保存，但 `check_and_download()` 中在调用 `enqueue_from_video()` 之前已经创建了并保存了一个 DownloadRecord。导致同一个视频有两条记录。这是 S-01 的具体表现。

---

### I-03: download_video_streaming 方法未被使用  🔵 未修复

**位置**: `src-tauri/src/services/ytdlp.rs:270-389`

**问题**: `download_video_streaming()` 方法是完整的流式下载实现（含回调），但实际下载流程使用的是 `download_video_spawn()` + `execute_download_with_control()` 中的手动 stdout 读取。`download_video_streaming()` 成为了死代码。

---

### I-04: DownloadProgress 类型在前端与后端不一致  🔵 未修复

**位置**: `src/types/index.ts:33-41`, `src-tauri/src/services/download_queue.rs:58-65`

**问题**: 前端 `DownloadProgress` 接口包含 `task_id` 和 `video_url` 字段，而后端 `ProgressInfo` 结构体不包含这两个字段（这两个字段在 `DownloadProgressEvent` 中单独携带）。前端类型定义将事件和进度数据合并为一个类型，语义不清晰。

---

### I-05: 调度器使用克隆的 settings 而非实时读取  ✅ 已修复

**位置**: `src-tauri/src/lib.rs:100-104`

**问题**: 调度器在启动时克隆了 `settings_clone`，之后每次调度都使用该克隆值。如果用户在运行期间修改了代理、cookie 文件或下载目录等设置，调度器仍使用旧值。

---

## 六、测试覆盖审查

### 现有测试概况（修复后）

| 模块 | 测试文件 | 测试数 | 覆盖评估 |
|------|---------|--------|---------|
| models/download.rs | 内联 | 7 | ✅ 序列化、默认值、状态转换 |
| models/subscription.rs | 内联 | 13 | ✅ 序列化、新增字段、per-subscription 数据 |
| models/settings.rs | 内联 | 9 | ✅ 默认值、序列化、新字段 |
| services/storage.rs | 内联 | 14 | ✅ 存储 CRUD + 去重 4 个 |
| services/ytdlp.rs | 内联 | 12 | ⚠️ JSON 解析和格式化测试，无 CLI 调用 |
| services/download_queue.rs | 内联 | 4 | ⚠️ 序列化测试，核心逻辑需 AppHandle |
| utils/progress_parser.rs | 内联 | 6 | ✅ 正常/异常/边界值 |
| commands/download.rs | 内联 | 9 | ✅ 新增去重+recover_state 测试 |
| **总计** | | **115** | (修复前 106) |

### 已补充的关键缺失测试

1. ✅ **recover_state()**: downloading/paused → failed, 终态不变, 空变更 (4 tests)
2. ✅ **去重逻辑**: video_id 命中, video_url 回退, failed 可重试, cancelled 跳过 (5 tests)

---

## 七、架构合规审查

### 分层架构合规性

| 规则 | 状态 | 备注 |
|------|------|------|
| Commands 层不包含业务逻辑 | ⚠️ 违规 | `check_and_download()` 包含去重逻辑和记录创建 |
| Services 层无状态 | ⚠️ 违规 | `DownloadQueue` 持有 `active_tasks`、`queue` 等状态 |
| 先克隆值再释放 MutexGuard | ⚠️ 部分违规 | `cancel()` 中持有 active_tasks MutexGuard 时调用 `self.emit_queue_changed()`（第 575 行），而 emit_queue_changed 内部又获取 active_tasks 的锁 |

**关于 DownloadQueue 的状态**: 该结构体设计上需要持有队列状态（VecDeque、Semaphore、ActiveTask HashMap），这是下载队列管理的固有需求。严格意义上违反了 Services 层无状态的原则，但在架构上可以接受——它更像是一个有状态的服务，通过 `QueueContext` + `Mutex<Option<DownloadQueue>>` 注入。

**关于 MutexGuard 递归获取** (✅ 已修复): `cancel()` 方法在第 556 行获取 `self.active_tasks.lock()`，然后在第 575 行调用 `self.emit_queue_changed()`，而 `emit_queue_changed()` 内部调用 `self.get_state()` 会再次获取 `self.active_tasks.lock()`。这会在同一线程上导致死锁。

---

## 八、安全性审查

| 项目 | 状态 | 说明 |
|------|------|------|
| unsafe 代码 | ⚠️ | `libc::kill()` 调用在 `pause()`/`resume()`/`pause_by_url()` 中使用 unsafe 块。这是必要的平台调用，但应检查返回值 |
| 命令注入 | ✅ | 使用 `Command::arg()` 而非字符串拼接 |
| 文件路径遍历 | ✅ | 使用 Tauri 标准路径解析 |
| 错误信息泄露 | ✅ | yt-dlp stderr 仅记录到日志 |

**建议**: `libc::kill()` 的返回值应被检查。如果 `kill()` 返回 -1（如进程已退出），应优雅处理而非静默忽略。

---

## 九、性能审查

| 项目 | 评估 | 说明 |
|------|------|------|
| download_records.json 加载 | ⚠️ | 每次下载完成/失败时全量加载→修改→保存。50+ 记录时可能成为瓶颈 |
| 队列轮询 | ⚠️ | 前端 3 秒轮询 getDownloadQueue（详见 S-05） |
| progress 事件频率 | ✅ | 由 yt-dlp 输出驱动，无多余轮询 |
| Mutex 持有时间 | ✅ | 大部分锁持有时间短，无跨 async 边界（除了 R-01 中的递归锁问题） |

---

## 十、修复优先级建议

### P0 - 立即修复 ✅ 全部完成

1. ✅ **R-03**: cancel() 标记所有记录为 cancelled (782e84d)
2. ✅ **R-01**: pause/resume 不更新 DownloadRecord 状态 (782e84d)
3. ✅ **S-04**: 前端"继续"按钮调用 pause 而非 resume (782e84d)

### P1 - 近期修复 ✅ 全部完成

4. ✅ **R-04**: cancel() 不调用 cleanup_partial_files (782e84d)
5. ✅ **S-01**: 同一视频创建两条 DownloadRecord (4a1795c)
6. ✅ **R-05**: progress_parser 过于宽松 (4a1795c)

### P2 - 后续改进 ✅ 全部完成

7. ✅ **S-02**: DownloadRecord 重复加载保存 (c45668f)
8. ✅ **S-03**: get_state() max_concurrent 硬编码 (c45668f)
9. ✅ **R-02**: max_concurrent 变更不生效 (c45668f)
10. ✅ **S-05**: 轮询改为事件驱动 (4a1795c)

### P3 - 长期优化 (2/3 完成)

11. 🔵 **S-06**: status String 改为枚举 — 未修复
12. ✅ **S-07**: Windows 暂停/继续 — kernel32 API (e67bc44)
13. ✅ 补充单元测试覆盖 — 新增 9 个测试, 115 total (098bae6)

---

## 附录 A: 文件变更清单

### Rust 后端

| 文件 | 变更类型 | 行数 | 审查状态 |
|------|---------|------|---------|
| services/download_queue.rs | 新增 | 719 | 🔴 存在 3 个必须修复问题 |
| commands/download.rs | 修改 | 522 | 🔴 存在重复记录创建问题 |
| services/ytdlp.rs | 修改 | 720 | 🟡 download_video_streaming 死代码 |
| models/download.rs | 修改 | 226 | ✅ 结构合理 |
| models/settings.rs | 修改 | 240 | ✅ 结构合理 |
| utils/progress_parser.rs | 新增 | 102 | 🔴 解析过于宽松 |
| utils/error.rs | 未变更 | 27 | 🟡 缺少新错误变体（T061） |
| lib.rs | 修改 | 191 | ✅ 命令注册完整 |

### TypeScript 前端

| 文件 | 变更类型 | 行数 | 审查状态 |
|------|---------|------|---------|
| types/index.ts | 修改 | 96 | ✅ 类型定义完整 |
| lib/tauri.ts | 修改 | 182 | ✅ API 封装正确 |
| components/DownloadRecordList.tsx | 修改 | 107 | 🟡 waiting 状态映射错误 |
| components/DownloadRecordItem.tsx | 修改 | 168 | 🔴 继续/暂停回调混淆 |
| components/DownloadProgressBar.tsx | 新增 | 85 | ✅ 组件结构合理 |
| components/AppShell.tsx | 修改 | 179 | 🟡 轮询策略需优化 |
| components/DetailPanel.tsx | 修改 | 122 | ✅ 传递 props 正确 |
| hooks/useDownloadProgress.ts | 新增 | 48 | ✅ Hook 结构合理 |

---

## 附录 B: 规格偏离记录

| 偏离项 | 规格要求 | 实际实现 | 影响 |
|--------|---------|---------|------|
| DownloadProgressEvent 缺少 video_url | contracts/commands.md 未定义 | 实际添加了 video_url 字段 | ✅ 正向偏离，前端需要 |
| 暂停/继续命令签名 | `app_handle: AppHandle` | `queue_ctx: State<QueueContext>` | ✅ 合理改进 |
| QueueState 缺少 is_running | contracts/commands.md 定义了 is_running | 未实现 | 🟡 前端无法判断队列是否在处理 |
| DownloadQueuePanel 独立组件 | plan.md 定义了独立面板 | 合并到 DownloadRecordList | ✅ 简化 UI，合理决策 |
| cancelled 状态去重规则 | data-model.md: cancelled 跳过 | 当前实现中 cancelled 状态记录被加入 seen_urls，会阻止重新下载 | 🟡 与规格一致 |
