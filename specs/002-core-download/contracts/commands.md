# Tauri Commands: 下载功能

**Spec**: [../spec.md](../spec.md) | **Data Model**: [../data-model.md](../data-model.md)

本文档定义核心下载功能的所有 Tauri `invoke` 命令接口约定。命名遵循 kebab-case (Rust) → camelCase (TypeScript) 映射。

---

## 新增命令

### 1. `pause_download`

暂停正在下载的任务。

| 项目 | 内容 |
|------|------|
| Rust 函数签名 | `async fn pause_download(id: String, app_handle: AppHandle) -> Result<(), String>` |
| TypeScript invoke | `invoke<void>("pause_download", { id })` |
| 参数 | `id: string` — DownloadTask 的 UUID |
| 返回 | 成功时返回空，失败时返回错误字符串 |
| 错误 | `NotFound` — ID 不存在；任务不在 `Running` 状态 |
| 副作用 | SIGSTOP 子进程(Unix) / SuspendThread(Windows)；更新内存中任务状态为 `Paused` |

### 2. `resume_download`

继续已暂停的下载任务。

| 项目 | 内容 |
|------|------|
| Rust 函数签名 | `async fn resume_download(id: String, app_handle: AppHandle) -> Result<(), String>` |
| TypeScript invoke | `invoke<void>("resume_download", { id })` |
| 参数 | `id: string` — DownloadTask 的 UUID |
| 返回 | 成功时返回空，失败时返回错误字符串 |
| 错误 | `NotFound` — ID 不存在；任务不在 `Paused` 状态 |
| 副作用 | SIGCONT 子进程(Unix) / ResumeThread(Windows)；更新内存中任务状态为 `Running` |

### 3. `cancel_download`

取消下载任务（等待中、运行中、已暂停均可取消）。

| 项目 | 内容 |
|------|------|
| Rust 函数签名 | `async fn cancel_download(id: String, app_handle: AppHandle) -> Result<(), String>` |
| TypeScript invoke | `invoke<void>("cancel_download", { id })` |
| 参数 | `id: string` — DownloadTask 的 UUID |
| 返回 | 成功时返回空，失败时返回错误字符串 |
| 错误 | `NotFound` — ID 不存在 |
| 副作用 | 如果 Running/Paused：kill 子进程，删除部分文件（.part, .ytdl）；清除队列中的 Waiting 任务；更新 DownloadRecord 状态为 `cancelled` |

### 4. `get_download_queue`

获取当前下载队列状态（内存中）。

| 项目 | 内容 |
|------|------|
| Rust 函数签名 | `async fn get_download_queue() -> Result<Vec<DownloadTask>, String>` |
| TypeScript invoke | `invoke<DownloadTask[]>("get_download_queue")` |
| 参数 | 无 |
| 返回 | 当前队列中的所有任务（含进度信息） |

### 5. `get_queue_state`

获取下载队列的运行时状态。

| 项目 | 内容 |
|------|------|
| Rust 函数签名 | `async fn get_queue_state() -> Result<QueueState, String>` |
| TypeScript invoke | `invoke<QueueState>("get_queue_state")` |
| 返回 | `{ active_count, waiting_count, max_concurrent, is_running }` |

---

## 修改的命令（接口不变，内部实现变更）

### `check_all_subscriptions`

| 变更 | 说明 |
|------|------|
| 内部实现 | 检测到新视频后创建 `DownloadTask` 并推入 `DownloadQueue` 而非立即下载 |
| 接口签名 | 不变 |

### `check_subscription`

| 变更 | 说明 |
|------|------|
| 内部实现 | 同上，推入队列而非直接在命令中下载 |
| 接口签名 | 不变 |

### `manual_check_all`

| 变更 | 说明 |
|------|------|
| 内部实现 | 同上 |
| 接口签名 | 不变 |

---

## Tauri Events（推送）

### 现有事件（保持兼容）

| 事件名 | Payload | 触发时机 |
|--------|---------|----------|
| `records-changed` | `()` | 任何 DownloadRecord 变更 |
| `download-complete` | `{ title: string, channel: string }` | 单个视频下载完成（仅通知开启时） |
| `scheduler-check-complete` | `()` | 调度器完成一轮检查 |

### 新增事件

| 事件名 | Payload | 触发时机 |
|--------|---------|----------|
| `download-progress` | `DownloadProgressEvent` | 下载过程中每秒推送 |
| `queue-changed` | `QueueChangedEvent` | 队列状态变更（入队、出队、暂停、取消） |

### 事件 Payload 结构

```rust
// download-progress
#[derive(Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub task_id: String,
    pub percent: f32,
    pub speed: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub eta: String,
}

// queue-changed
#[derive(Clone, Serialize)]
pub struct QueueChangedEvent {
    pub active_count: usize,
    pub waiting_count: usize,
    pub max_concurrent: u32,
}
```
