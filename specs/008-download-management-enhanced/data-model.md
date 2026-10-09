# Phase 1 Data Model: 下载管理增强（P1）

**Date**: 2026-10-02 | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

> **只写形状与不变量。** 每条规则的**为什么**在 [`decisions.md`](./decisions.md)（实现阶段的唯一真相源）；本文件回答「数据长什么样、必须满足什么」。

---

## 1. 实体总览

| 实体 | 位置 | 持久化 | 本次变化 |
|---|---|---|---|
| `DownloadRecord` | `src-tauri/src/models/download.rs` ↔ `src/types/index.ts` | `download_records.json` | **+3 字段** |
| `DownloadTask` | `src-tauri/src/services/download_queue.rs` ↔ `src/types/index.ts` | 内存（序列化给前端） | **+2 字段** |
| `ActiveTask` | 同上（内部结构，不序列化） | 内存 | **`pid` 改 `Option`；+`notify`** |
| `Subscription` | `models/subscription.rs` ↔ `types/index.ts` | `subscriptions.json` | 不变（`quality_preset` 是升级判定的输入） |
| `UnifiedVideoItem` | `src/lib/unifiedVideoList.ts` ↔ `types/index.ts` | 前端派生 | **+3 正交字段** |

---

## 2. `DownloadRecord`

### 2.1 字段

| 字段 | 类型 | 持久化 | 不变量 / 说明 |
|---|---|---|---|
| `id` | `String`（UUID v4） | ✓ | 记录主键；**升级 / 重下复用同一条** |
| `subscription_id` | `String` | ✓ | **归属**：首次下载它的订阅（不是「谁在展示它」） |
| `video_id` | `String` | ✓ | **去重主键（全局）**；为空时回退用 `video_url` |
| `video_title` | `String` | ✓ | |
| `video_url` | `String` | ✓ | |
| `file_path` | `String` | ✓ | 绝对路径；重下时**先落库新值再回收旧文件** |
| `file_size` | `u64` | ✓ | 0 表示未知 |
| `status` | `String` | ✓ | 见 2.2 |
| `downloaded_at` | `String`（ISO 8601） | ✓ | **只在成功完成时写**；暂停 / 取消 / 重试不改写 |
| `error_message` | `Option<String>` | ✓ | 真实 stderr 摘要，**≤ 200 字符** |
| **`quality`** | `String` | ✓ | 下载时使用的 preset；**空串 = 未知** |
| **`retry_count`** | `u32` | ✓ | **3 个新字段全部 `#[serde(default)]`** |
| **`last_retry_at`** | `Option<String>`（ISO 8601） | ✓ | `skip_serializing_if = "Option::is_none"` |

### 2.2 `status` 取值

`downloading` · `completed` · `failed` · `paused` · `cancelled` · `waiting` · `deleted` · **`retrying`**

### 2.3 不变量

1. **全局唯一**：库中同一 `video_id` 最多一条记录。启动时由 `deduplicate_records()` 在写事务内强制（decisions.md §3）。
2. `retry_count ∈ 0..=3`。
3. `quality` 为空串或未知档 ⇒ **永不参与升级判定**（decisions.md §2.1）。
4. `downloaded_at` 仅当 `status` 变为 `completed` 时更新。
5. `error_message` **不得包含代理凭证**（`user:pass@`）—— 它来自真实 stderr，而 `proxy_url` 可能含明文口令；落库前必须剥离（decisions.md §6.4）。
6. `quality` 存**请求的 preset**，**不存**实际分辨率、**不存**扩展名（扩展名从 `file_path` 后缀派生）。

### 2.4 兼容性

- **向后**：三个新字段全部 `#[serde(default)]` → 旧 `download_records.json` 正常加载（`quality = ""`、`retry_count = 0`、`last_retry_at = None`）。**不做数据迁移。**
- **向前**：无 `deny_unknown_fields` → 旧版本读到新字段静默忽略，新字段在旧版本下一次写入时丢失。**接受。**

---

## 3. `DownloadTask`（内存队列，序列化给前端）

| 字段 | 类型 | 本次 | 说明 |
|---|---|---|---|
| `id` | `String` | | 任务 id（≠ 记录 id） |
| `video_id` / `video_url` / `video_title` / `subscription_id` / `quality` | `String` | | |
| `status` | `TaskStatus` | | `Waiting` / `Running` / `Paused` / `Completed` / `Failed` / `Cancelled` |
| `progress` | `Option<ProgressInfo>` | | |
| `error_message` | `Option<String>` | | |
| `created_at` / `completed_at` | `String` / `Option<String>` | | |
| **`record_id`** | `String` | **新增** | 完成 / 失败 / 状态回写**全部按它定位**（取代 `(video_url, subscription_id)` 过滤） |
| **`next_retry_at`** | `Option<String>` | **新增** | 退避期的下次重试时刻；前端据此驱动倒计时。**不落 `download_records.json`** |

---

## 4. `ActiveTask`（内部，不序列化）

| 字段 | 类型 | 本次 | 说明 |
|---|---|---|---|
| `child` | `Option<tokio::process::Child>` | | 退避期间为 `None` |
| `pid` | **`Option<u32>`** | **改** | 与 `child` **同步**：`Some` 当且仅当 `child` 存在 |
| `task` | `DownloadTask` | | |
| `last_progress_percent` | `f32` | | 供缩容时挑选「进度最少」的任务 |
| `notify` | **`Arc<tokio::sync::Notify>`** | **新增** | 打断退避（取消 / 暂停） |

### 4.1 不变量（安全关键）

1. `pid.is_some() ⇔ child.is_some()`。
2. 任何发信号动作（`SIGSTOP` / `SIGCONT` / `start_kill`）**只允许**出现在 `Some(pid)` / `Some(child)` 分支内。
3. `update_max_concurrent` 缩容挑选任务时**必须排除 `child.is_none()`（退避中）的 entry**。
4. 进入退避 ⇒ `child = None`、`pid = None`、`task.status ∈ {Retrying, Paused}`。

> 不变量 1–2 是**安全修复**：退避期间子进程已退出，陈旧 PID 可能已被系统复用，误发 `SIGSTOP` 会挂起无关进程。

---

## 5. 值对象

### 5.1 画质档（升级判定的比较域）

```
480p(1) < 720p(2) < 1080p(3) < 1440p(4) < 2160p(5) < best(6)
其它 / 空串 / audio → 未知：不可比较
```

### 5.2 `RetryDecision`

```
Retry | NoRetry | Unknown      # 调用方把 Unknown 视同 Retry
```

### 5.3 `UnifiedVideoItem`（前端派生）

在既有 `id` / `title` / `url` / `status` / `channelInfo?` / `downloadInfo?` / `queueTask?` 之外新增三个**正交**字段：

| 字段 | 含义 |
|---|---|
| `upgradeable` | 订阅当前画质档次高于记录中的档次（且满足第 7 节的前置条件） |
| `missing` | 该记录的文件不在磁盘上（**优先于 `upgradeable`**） |
| `downloadedElsewhere` | 该视频已被**其它订阅**下载（本订阅没有记录） |

---

## 6. 关系

```text
Subscription 1 ──── N DownloadRecord        （经 subscription_id = 归属）
DownloadRecord 1 ── 1 文件                    （全局唯一；输出模板无订阅判别符）
DownloadTask   1 ── 1 DownloadRecord         （经 record_id）
```

- 同一视频被多个订阅收录 ⇒ 仍只有 **1 条记录、1 个文件**；其余订阅在 UI 上呈现 `downloadedElsewhere`（**不提供重下、不提供升级**）。
- 记录与任务分离：任务出队后 entry 移除，记录保留。

---

## 7. 校验规则（源自需求）

### 7.1 升级判定（按序，全过才置 `upgradeable = true`）

1. 存在 `record` 且**无**活跃 `queueTask` → 否则 false
2. `record.status == "completed"` → 否则 false
3. `missing == false` → 否则 false（**缺失优先于升级**）
4. `record.quality` 已知 → 否则 false
5. `subscription.quality_preset` 已知 → 否则 false
6. `rank(订阅) > rank(记录)` → 否则 false（`==` 与 `<` 都 false）

> 穷尽的 13 行真值表见 decisions.md §2.4（直接可翻译为测试用例）。

### 7.2 重试判定（按序，首个命中即返回）

1. 退出码 `None` / `2` / `101` → `NoRetry`
2. 最后一条 `ERROR:` 行命中**终态黑名单** → `NoRetry`
3. 命中 `Tunnel connection failed: <code>` → `5xx/408/429` 才 `Retry`
4. 命中 `HTTP Error <code>:` → `5xx/408/429` 才 `Retry`
5. 命中**网络白名单** → `Retry`
6. 兜底 → `Unknown`（调用方视同 `Retry`）

> 逐条匹配式见 decisions.md §5.3 / §5.4；回归样本见 §5.7。

### 7.3 字段写入 / 重置时机

| 字段 | 写入 | 归零 / 清理 |
|---|---|---|
| `quality` | 入队时 | 重下时覆盖 |
| `retry_count` | 每次重试尝试前 +1 | **新一轮下载开始时归零**；**成功时保留** |
| `last_retry_at` | 每次重试尝试前 | 新一轮下载开始时清 `None` |
| `error_message` | 失败时写 | **成功时清空** |
| `downloaded_at` | **仅成功完成时** | — |

### 7.4 计数口径（两套并存，不得互相替换）

- **记录总数** = `records.length`（含 `deleted`）
- **已完成数** = `status === "completed"` 的条数
- `retrying` **计入**总数、**不计入**已完成数

---

## 8. 状态转移

### 8.1 记录状态 × 队列任务 × 触发

| 触发 | `DownloadRecord.status` | `ActiveTask` |
|---|---|---|
| 入队 | `downloading` | 无（任务在 `queue`，`Waiting`） |
| 开始执行 | `downloading` | `Running`，`child=Some`，`pid=Some` |
| 失败 · 可重试 · 未达上限 | **`retrying`** | `BackingOff`：`child=None`，`pid=None`，`next_retry_at=Some` |
| 退避到点 → 再次 spawn | `retrying` | `BackingOff` → `Running` |
| 重试成功 | `completed` | entry 移除 |
| 达上限仍失败 | `failed` | entry 移除 |
| **不可重试**的失败 | `failed` | entry 移除（**不经过退避**） |
| **spawn 失败** | `failed` | entry 移除（补写：`无法启动 yt-dlp：<io error>`） |
| 取消（`Running`） | `cancelled` | entry 移除 + kill child |
| 取消（`BackingOff`） | `cancelled` | entry 移除 + `notify`；循环醒来发现 entry 不存在 → 退出且**不回写** |
| 暂停任务（`Running`） | `paused` | `Running` + SIGSTOP |
| 暂停任务（`BackingOff`） | `paused` | 冻结剩余时长，等恢复 |
| 恢复任务（`Running`） | `downloading` | `Running` + SIGCONT |
| 恢复任务（`BackingOff`） | `retrying` | 用剩余时长重新计时 |
| 应用退出（任何在途） | `failed`（`error_message = "Application restarted"`） | — |
| **暂停订阅** | **不变** | **不变**（暂停只让检查跳过该订阅） |

### 8.2 `deduplicate_vec` 的 rank（同 `video_id` 多条时保留优先级）

```
completed(0) < downloading(1) < retrying(2) < failed(3) < paused(4) < cancelled(5) < 其它(9)
```

同 rank 内取最新 `downloaded_at`。去重**键与策略不变**，只扩这张表。

### 8.3 前端 `statusPriority`（排序权重）

```
downloading(0) < retrying(1) < waiting(2) < paused(3) < completed(4) < new(5) < cancelled(6) < failed(7)
```
