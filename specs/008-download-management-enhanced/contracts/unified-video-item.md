# Contract: 统一视频列表条目（前端派生）

**Feature**: 008-download-management-enhanced | **Plan**: [../plan.md](../plan.md)

---

## 1. 函数签名

```ts
buildUnifiedVideoList({
  videos,          // VideoInfo[]       —— 频道视频
  records,         // DownloadRecord[]  —— **全局去重后**的记录列表（不再按订阅预过滤）
  tasks,           // DownloadTask[]    —— 内存队列
  qualityPreset,   // string            —— 当前订阅的 quality_preset
  missingPaths,    // Set<string>       —— 文件缺失的 file_path 集合
  subscriptionId,  // string            —— 当前订阅 id
}): UnifiedVideoItem[]
```

**签名变化**：新增 `qualityPreset` / `missingPaths` / `subscriptionId` 三个入参。

> `records` 必须是**全局去重后**的列表（`getAllDownloadRecords` 的返回），因为两个新标记需要看到**其它订阅**的记录。「本订阅的记录」由函数内部按 `subscriptionId` 区分。

---

## 2. 条目形状

```ts
export interface UnifiedVideoItem {
  readonly id: string;            // 优先 video_id，回退 video_url
  readonly title: string;
  readonly url: string;
  readonly status: UnifiedVideoStatus;
  channelInfo?: VideoInfo;
  downloadInfo?: DownloadRecord;
  queueTask?: DownloadTask;

  // 新增：三个**正交**标记（不新增 status 值）
  readonly upgradeable: boolean;
  readonly missing: boolean;
  readonly downloadedElsewhere: boolean;
}
```

---

## 3. 派生规则（按序）

### `upgradeable`

1. 有 `record`（**属于本订阅**）且无活跃 `queueTask`
2. `record.status === "completed"`
3. `!missing`
4. `record.quality` 已知
5. `qualityPreset` 已知
6. `rank(qualityPreset) > rank(record.quality)`

任一不满足 ⇒ `false`。

### `missing`

`record` 存在 且 `record.file_path` ∈ `missingPaths`。

> **优先于 `upgradeable`**：两者都为真时，UI 只呈现「文件缺失」。

### `downloadedElsewhere`

**本订阅没有**该视频的记录，**但全局记录里存在**同一 `video_id` 或同一 `video_url` 的记录。

> 该情形下**不提供重下、不提供升级**（文件已在库中且全局唯一）。

---

## 4. 排序

`statusPriority` 插入 `retrying`：

```
downloading(0) < retrying(1) < waiting(2) < paused(3) < completed(4) < new(5) < cancelled(6) < failed(7)
```

同状态内按时间倒序（既有行为不变）。

---

## 5. 测试契约（必须覆盖）

沿用真值表（[../decisions.md](../decisions.md) §2.4）逐行断言，至少包含：

- 可升级（目标 `1080p` / 目标 `best`）
- 同档 ⇒ 不打标
- 降级 ⇒ 不打标
- 记录 `quality` 为空（旧记录）⇒ 不打标
- 订阅 preset 未知 ⇒ 不打标
- `missing` 与 `upgradeable` 同时为真 ⇒ **只有 `missing`**
- `downloadedElsewhere`：本订阅无记录、全局有 ⇒ 该标记为真，且 `upgradeable === false`
- `retrying` 的排序位次在 `downloading` 之后
