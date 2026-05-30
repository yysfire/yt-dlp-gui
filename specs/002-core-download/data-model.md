# Data Model: 核心下载功能

**Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

## 实体概览

```
DownloadTask (内存中，不持久化)
    ↓ 完成后
DownloadRecord (持久化到 download_records.json)
    ↓ 关联
Subscription (subscriptions.json)
```

---

## 1. DownloadTask（下载任务 — 内存队列）

管理下载队列中的任务生命周期。**不持久化**，应用重启后通过 `recover_state()` 将未完成任务标记为失败。

### 字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | `String (UUID v4)` | ✅ | 唯一标识 |
| `video_id` | `String` | ✅ | 平台视频 ID（用于去重） |
| `video_url` | `String` | ✅ | 视频完整 URL |
| `video_title` | `String` | ✅ | 视频标题 |
| `subscription_id` | `String` | ✅ | 所属订阅 ID |
| `quality` | `String` | ✅ | 画质预设 |
| `status` | `TaskStatus` | ✅ | 任务状态 |
| `progress` | `Option<DownloadProgress>` | ❌ | 实时进度（运行中时填充） |
| `error_message` | `Option<String>` | ❌ | 失败原因 |
| `created_at` | `String (ISO 8601)` | ✅ | 入队时间 |
| `completed_at` | `Option<String>` | ❌ | 完成时间 |

### TaskStatus 枚举（Rust）

```rust
pub enum TaskStatus {
    Waiting,     // 等待下载
    Running,     // 正在下载
    Paused,      // 已暂停
    Completed,   // 已完成
    Failed,      // 失败（含错误信息）
    Cancelled,   // 已取消
}
```

### DownloadProgress 结构体

```rust
pub struct DownloadProgress {
    pub percent: f32,          // 0.0 - 100.0
    pub speed: String,         // e.g. "2.3MiB/s"
    pub downloaded_bytes: u64, // 已下载字节数
    pub total_bytes: u64,      // 总字节数（可能为 0）
    pub eta: String,           // 预计剩余时间 e.g. "00:02:15"
}
```

### 状态转换图

```
         ┌─────────────────────────────────┐
         │                                 │
         ▼                                 │
  ┌──────────┐    ┌─────────┐    ┌──────────────┐
  │ Waiting  │───→│ Running │───→│  Completed   │
  └──────────┘    └─────────┘    └──────────────┘
       │              │    │
       │              │    └──────────→┌──────────┐
       │              │                │  Failed  │
       │              │                └──────────┘
       │              │
       │         ┌────▼────┐
       │         │ Paused  │────→ Running (resume)
       │         └────┬────┘
       │              │
       │              └────→ Cancelled (cancel from paused)
       │
       └─────────────────────→ Cancelled (cancel from waiting)

  Running ──────────────────→ Cancelled (cancel from running)
```

**转换规则**：
- `Waiting` → 可转为 `Running`（队列调度）或 `Cancelled`（用户取消）
- `Running` → 可转为 `Paused`（用户暂停）、`Completed`（下载完成）、`Failed`（下载错误）、`Cancelled`（用户取消）
- `Paused` → 可转为 `Running`（resume）或 `Cancelled`（cancel）
- 终态：`Completed`、`Failed`、`Cancelled` 不可再转换

---

## 2. DownloadRecord（下载记录 — 持久化）

已持久化的下载记录，存储在 `~/.yt-dlp-sub-gui/download_records.json`。

### 当前字段（已存在，无需改动）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | UUID |
| `subscription_id` | `String` | 所属订阅 |
| `video_title` | `String` | 视频标题 |
| `video_url` | `String` | 视频 URL |
| `file_path` | `String` | 本地文件路径 |
| `file_size` | `u64` | 文件大小 |
| `status` | `String` | 下载状态 |
| `downloaded_at` | `String` | 下载时间 ISO 8601 |

### 新增字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `video_id` | `String` | `""` | 平台视频 ID（用于去重） |
| `error_message` | `Option<String>` | `None` | 失败原因（FR-010） |

### 新增状态值

| 状态 | 含义 |
|------|------|
| `downloading` | 下载中（已有） |
| `completed` | 已完成（已有） |
| `failed` | 失败（已有） |
| `paused` | 已暂停（新增） |
| `cancelled` | 已取消（新增，含已清理部分文件） |

### 去重规则

```
video_id + video_url 双重去重
  └─ 已完成/completed：跳过
  └─ 已失败/failed：允许重试
  └─ 已取消/cancelled：跳过（除非手动触发重新下载）
  └─ downloading/paused：视为未完成，在状态恢复时标记为 failed
```

---

## 3. AppSettings 新增字段

在 `~/.yt-dlp-sub-gui/settings.json` 中新增 `max_concurrent_downloads` 字段（FR-009）。

```rust
pub struct AppSettings {
    // ... 现有字段不变 ...
    
    /// Maximum concurrent download tasks (default: 1, range: 1-3)
    pub max_concurrent_downloads: u32,
}
```

默认值为 `1`，用户可在设置中调整为 `2` 或 `3`。

---

## 4. TypeScript 类型映射

```typescript
// 新增：下载任务状态
export type TaskStatus = "waiting" | "running" | "paused" | "completed" | "failed" | "cancelled";

// 新增：实时进度
export interface DownloadProgress {
  percent: number;
  speed: string;
  downloaded_bytes: number;
  total_bytes: number;
  eta: string;
}

// 修改：DownloadRecord 新增字段
export interface DownloadRecord {
  // ... 现有字段 ...
  video_id: string;           // 新增
  error_message: string | null; // 新增
}

// 新增：下载任务（内存队列中的任务）
export interface DownloadTask {
  id: string;
  video_id: string;
  video_url: string;
  video_title: string;
  subscription_id: string;
  quality: string;
  status: TaskStatus;
  progress: DownloadProgress | null;
  error_message: string | null;
  created_at: string;
  completed_at: string | null;
}

// 修改：AppSettings 新增字段
export interface AppSettings {
  // ... 现有字段 ...
  max_concurrent_downloads: number; // 新增
}
```
