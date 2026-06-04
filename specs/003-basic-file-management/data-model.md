# Data Model: 基本文件管理

**Feature**: specs/003-basic-file-management
**Date**: 2026-06-04

## 实体

### DownloadRecord（扩展）

现有实体，新增状态值和关联字段。

| 字段 | 类型 | 必填 | 说明 | 变更 |
|------|------|------|------|------|
| `id` | `String` (UUID v4) | ✅ | 记录唯一标识 | 无变更 |
| `subscription_id` | `String` | ✅ | 关联订阅 ID | 无变更 |
| `video_title` | `String` | ✅ | 视频标题 | 无变更 |
| `video_url` | `String` | ✅ | 视频 URL | 无变更 |
| `video_id` | `String` | ✅ | 平台视频 ID，去重依据 | 无变更 |
| `file_path` | `String` | ✅ | 本地文件完整路径 | 无变更 |
| `file_size` | `u64` | ✅ | 文件大小（字节） | 无变更 |
| `status` | `String` | ✅ | 记录状态（见状态机） | **新增 `"deleted"` 值** |
| `downloaded_at` | `String` (ISO 8601) | ✅ | 下载完成时间 | 无变更 |
| `error_message` | `Option<String>` | ❌ | 错误信息 | 无变更 |

### 状态机

```
                   ┌──────────────┐
                   │  waiting     │
                   └──────┬───────┘
                          │ (开始下载)
                          ▼
                   ┌──────────────┐
                   │ downloading  │
                   └──┬───────┬───┘
            (成功)    │       │    (失败)
                      ▼       ▼
              ┌──────────┐ ┌──────────┐
              │ completed │ │  failed  │
              └────┬─────┘ └──────────┘
                   │
      ┌────────────┼────────────┐
      │ (用户手动)  │ (外部删除) │
      ▼            ▼            ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ deleted  │ │ 文件不存在 │ │ paused(※)│
│(保留记录)│ │ 但记录保留 │ └──────────┘
└──────────┘ └──────────┘
                              (*) paused/cancelled 为下载队列状态
```

**状态值完整列表**（7 种）：

| 状态 | 含义 | 来源 |
|------|------|------|
| `waiting` | 等待下载 | 下载队列 |
| `downloading` | 下载中 | 下载引擎 |
| `completed` | 下载完成，文件存在 | 下载完成后 |
| `failed` | 下载失败 | 下载引擎 |
| `paused` | 已暂停 | 用户操作（队列中） |
| `cancelled` | 已取消 | 用户操作（队列中） |
| `deleted` | **文件已通过应用内删除，记录保留** | **本次新增** |

> **注意**: 外部删除的文件不改变 `status`，而是在前端通过文件存在性检测结果渲染"文件已缺失"标记。`status="deleted"` 仅由应用内删除操作触发。

### 与 Subscription 的关系

```
Subscription (1) ───────── (N) DownloadRecord
  subscription_id ◄──────── subscription_id
```

下载记录通过 `subscription_id` 关联订阅。获取记录时可选择按 `subscription_id` 过滤（现有 `get_download_records` 命令）。

### 序列化兼容性

- `status` 字段为 `String` 类型，JSON 反序列化天然兼容任何新状态值
- 旧记录不会因新增 `"deleted"` 状态而反序列化失败
- 前端 TypeScript `DownloadRecord` 的 `status` 联合类型需追加 `"deleted"`

### 索引考量

- `id`（UUID v4）：记录唯一主键
- `file_path`：文件存在性检测的查找依据
- `video_id` + `subscription_id`：去重组合键
- 当前基于 JSON 文件存储（全量读写），不涉及数据库索引
