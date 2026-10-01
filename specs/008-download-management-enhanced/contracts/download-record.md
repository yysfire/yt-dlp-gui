# Contract: `DownloadRecord`（持久化文件 ↔ Rust ↔ TypeScript）

**Feature**: 008-download-management-enhanced | **Plan**: [../plan.md](../plan.md)

三方共用同一份数据：`download_records.json` 文件、Rust `DownloadRecord`、TS `DownloadRecord`。两侧类型是**手写断言、无校验机制**，因此用**共享 fixture + 两侧各一个测试**兜底（decisions.md §1.4）。

---

## 1. 基准 fixture

**路径**：`src-tauri/tests/fixtures/download_record.json`（`src-tauri/tests/` 目前是空目录）

内容是一条**字段填满**的记录（`status = "completed"`），Rust 与前端测试读**同一个文件**：

- Rust：`include_str!` 读入，断言 `serde_json::to_value(基准记录) == 解析出的 fixture`
- 前端：`node:fs` 读入，`JSON.parse(...) satisfies DownloadRecord`（`npm run build` 的 `tsc --noEmit` 覆盖），并断言 `status` 落在联合类型内

改 fixture ⇒ 两侧必有一侧变红。

---

## 2. 字段契约

| JSON 键 | Rust 类型 | TS 类型 | serde | 说明 |
|---|---|---|---|---|
| `id` | `String` | `string` | 必填 | UUID v4 |
| `subscription_id` | `String` | `string` | 必填 | **归属**：首次下载它的订阅 |
| `video_id` | `String` | `string` | `#[serde(default)]` | 全局去重主键；空则回退 `video_url` |
| `video_title` | `String` | `string` | 必填 | |
| `video_url` | `String` | `string` | 必填 | |
| `file_path` | `String` | `string` | 必填 | 绝对路径 |
| `file_size` | `u64` | `number` | 必填 | 0 = 未知 |
| `status` | `String` | `DownloadRecordStatus`（字符串联合） | 必填 | 见 §3 |
| `downloaded_at` | `String` | `string` | 必填 | ISO 8601；**仅成功完成时写** |
| `error_message` | `Option<String>` | `string \| null` | `skip_serializing_if = "Option::is_none"` | ≤200 字符；**落库前剥离 `user:pass@`** |
| **`quality`** | `String` | `string` | `#[serde(default)]` | 下载时的 preset；**`""` = 未知** |
| **`retry_count`** | `u32` | `number` | `#[serde(default)]` | 0..=3 |
| **`last_retry_at`** | `Option<String>` | `string \| null` | `#[serde(default, skip_serializing_if = "Option::is_none")]` | ISO 8601 |

### TypeScript 联合类型

```ts
export type DownloadRecordStatus =
  | "downloading" | "completed" | "failed" | "paused"
  | "cancelled" | "waiting" | "deleted" | "retrying";   // ← 新增 retrying
```

---

## 3. 基准 fixture 内容（可直接落盘）

```json
{
  "id": "0f0a9c1e-6b0a-4a7c-9f1b-2d3e4f5a6b7c",
  "subscription_id": "1a2b3c4d-5e6f-4a8b-9c0d-1e2f3a4b5c6d",
  "video_id": "dQw4w9WgXcQ",
  "video_title": "示例视频",
  "video_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
  "file_path": "/home/user/Videos/yt-dlp/示例视频.mp4",
  "file_size": 123456789,
  "status": "completed",
  "downloaded_at": "2026-10-02T12:34:56.789+00:00",
  "error_message": null,
  "quality": "1080p",
  "retry_count": 2,
  "last_retry_at": "2026-10-02T12:30:00+00:00"
}
```

> 注意 `retry_count = 2` + `status = "completed"` 是**有意**的组合 —— 它同时锁住「成功时保留重试次数」这条规则（spec 故事 2 场景 2）。

---

## 4. 兼容性契约

| 方向 | 行为 |
|---|---|
| **旧文件 → 新版本** | 三个新字段缺失 ⇒ 取默认值（`""` / `0` / `None`）；不迁移 |
| **新文件 → 旧版本** | 无 `deny_unknown_fields` ⇒ 新字段被静默忽略，并在旧版本下一次写入时丢失 |

**不做任何数据迁移脚本。**

---

## 5. 不变量（测试必须覆盖）

1. 同一 `video_id` 在文件中**最多一条**（启动时 `deduplicate_records` 强制）
2. `quality = ""` ⇒ **永不**产生 `upgradeable`
3. `error_message` **不含** `user:pass@`
4. `retry_count` 在 `status == "completed"` 时**不被**归零
5. `last_retry_at` 为 `None` 时**不出现**在该 JSON 中（而非 `null`）
