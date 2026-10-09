# Contract: Tauri 命令与事件（前端 ↔ 后端）

**Feature**: 008-download-management-enhanced | **Plan**: [../plan.md](../plan.md)

---

## 1. 新增命令：`redownload_video`

```rust
#[tauri::command]
pub async fn redownload_video(
    record_id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<DownloadRecord, String>
```

### 语义

「按**订阅当前**的画质 preset，重下这一条视频的记录」。

- **不区分「升级」与「重新下载」** —— 二者行为完全一致，差别只在 UI 展示理由。**一个命令**，不是两个。
- 后端据 `record_id` 反查记录 → 反查其订阅 → **取订阅当前的 `quality_preset`**（前端不传画质，避免两侧各持一份真相）。

### 入参

| 参数 | 类型 | 说明 |
|---|---|---|
| `record_id` | `String` | **`DownloadRecord.id`**（不是 `video_id`）—— 前端从 `UnifiedVideoItem.downloadInfo.id` 取 |

### 返回

`DownloadRecord` —— 重置后的记录（`status = "downloading"`、`quality` 已更新为订阅当前 preset、`error_message = None`、`retry_count = 0`、`last_retry_at = None`，且**保留** `file_path` / `file_size`）。

### 错误

| 情形 | 错误 |
|---|---|
| 记录不存在 | `AppError::NotFound` → 前端收到 `String` |
| 订阅不存在 | `AppError::NotFound` |
| 队列未初始化 | `"Download queue not initialized"`（与既有命令一致） |

### 幂等

若「`queue` 待处理或 `active_tasks` 中已有同 `record_id` 的任务」**或**「记录状态 ∈ `{waiting, downloading, retrying}`」→ **不重复入队**，直接返回当前记录。

> 守卫放在**入队这一个收口**，因此检查路径与命令路径拿到的是同一份保证。

### 副作用

1. 写 `download_records.json`（重置记录）→ 必须调 `notify_records_changed`
2. 入队 → `emit_queue_changed`
3. 若后续成功且新 `file_path` ≠ 旧 `file_path` → **先落库新路径，再回收旧文件**

---

## 2. 既有命令的改动

| 命令 | 改动 |
|---|---|
| `check_subscription` / `check_all_subscriptions` | **不改**（去重判定语义逐字不变） |
| `sync_file_states` | 改调共享函数 `sync_completed_records`（行为不变，去掉重复实现） |
| `pause_download_by_url` / `cancel_download_by_url` | 语义不变，但内部必须支持**退避中（无子进程）**的任务 |
| `delete_file` | 不变 |

---

## 3. 事件契约（既有，本次不改名、不改载荷）

| 事件 | 载荷 | 触发 |
|---|---|---|
| `records-changed` | **无** | 凡写 `download_records.json` 的路径 |
| `queue-changed` | 队列状态 | 队列增删改 |
| `download-progress` | 进度 | 下载中 |
| `file-sync-complete` | `FileExistenceResult[]` | 周期同步 / 手动同步 |
| `scheduler-check-complete` | 无 | 一次检查完成 |

> 本次**新增消费方**：`App.tsx` 需要开始监听 `file-sync-complete` 并汇聚成 `missingPaths` 下发给两个面板（现状只有 `DownloadedList` 组件本地持有）。

---

## 4. 前端封装（`src/lib/tauri.ts`）

```ts
/** 按订阅当前画质重下某条记录（升级与重新下载共用）。 */
export async function redownloadVideo(recordId: string): Promise<DownloadRecord> {
  return invoke<DownloadRecord>("redownload_video", { recordId });
}
```

> 章程原则 II：**禁止在组件内直接 `invoke()`**，组件一律经此封装层。
