# Data Model: DetailPanel 统一视频列表

**Feature**: 007-unified-video-list
**Date**: 2026-06-13

## 新增类型

### UnifiedVideoItem

统一视频列表条目，合并频道视频和下载记录两种数据源。

```typescript
interface UnifiedVideoItem {
  /** 频道视频信息（仅当该视频来自频道播放列表时有值） */
  channelInfo?: VideoInfo;

  /** 持久化的下载记录（仅当该视频已被下载/下载中/失败时有值） */
  downloadInfo?: DownloadRecord;

  /** 内存中的下载队列任务（仅当该视频在活跃队列中时有值） */
  queueTask?: DownloadTask;

  /** 唯一标识：优先 video_id，回退 video_url */
  readonly id: string;

  /** 显示标题：优先频道视频标题，回退下载记录标题 */
  readonly title: string;

  /** 视频链接 */
  readonly url: string;

  /** 视频当前状态 */
  readonly status: VideoStatus;
}
```

### VideoStatus

```typescript
type VideoStatus =
  | "new"           // 未下载（仅频道视频）
  | "downloading"   // 下载中（队列 + 进度）
  | "paused"        // 已暂停（队列暂停）
  | "waiting"       // 等待中（队列排队）
  | "completed"     // 已完成（下载记录）
  | "failed"        // 失败（下载记录）
  | "cancelled";    // 已取消（下载记录/队列）
```

### 字段来源映射

| UnifiedVideoItem 字段 | 优先来源 | 回退来源 |
|----------------------|---------|---------|
| `id` | `channelInfo.id` | `downloadInfo.video_id` \\| `queueTask.video_id` \\| `channelInfo.url` |
| `title` | `channelInfo.title` | `downloadInfo.video_title` \\| `queueTask.video_title` |
| `url` | `channelInfo.url` | `downloadInfo.video_url` \\| `queueTask.video_url` |
| `status` | 派生规则见下方 | - |

### 状态派生规则

按优先级从高到低：

1. 有 `queueTask` 且状态为 `running` → `"downloading"`
2. 有 `queueTask` 且状态为 `paused` → `"paused"`
3. 有 `queueTask` 且状态为 `waiting` → `"waiting"`
4. 有 `queueTask` 且状态为 `cancelled` → `"cancelled"`
5. 有 `downloadInfo` 且状态为 `"downloading"` → `"downloading"`
6. 有 `downloadInfo` 且状态为 `"paused"` → `"paused"`
7. 有 `downloadInfo` 且状态为 `"completed"` → `"completed"`
8. 有 `downloadInfo` 且状态为 `"failed"` → `"failed"`
9. 有 `downloadInfo` 且状态为 `"cancelled"` → `"cancelled"`
10. 有 `channelInfo` 无下载信息 → `"new"`

## 现有类型（无变更）

### VideoInfo（复用）

```typescript
interface VideoInfo {
  id: string;
  title: string;
  url: string;
  duration: number | null;
  upload_date: string | null;
  thumbnail: string | null;
}
```

### DownloadRecord（复用）

```typescript
interface DownloadRecord {
  id: string;
  subscription_id: string;
  video_id: string;
  video_title: string;
  video_url: string;
  file_path: string;
  file_size: number;
  status: "downloading" | "completed" | "failed" | "paused" | "cancelled" | "waiting" | "deleted";
  error_message: string | null;
  downloaded_at: string;
}
```

### DownloadTask（复用）

```typescript
interface DownloadTask {
  id: string;
  subscription_id: string;
  video_id: string;
  video_title: string;
  video_url: string;
  status: "running" | "paused" | "waiting" | "failed" | "completed" | "cancelled";
  error_message: string | null;
  created_at: string;
}
```

## 合并算法

```typescript
function mergeToUnifiedItems(
  videoList: VideoInfo[],
  records: DownloadRecord[],
  queueTasks: DownloadTask[]
): UnifiedVideoItem[] {
  // 1. 构建 index: Map<key, UnifiedVideoItem>
  // 2. 遍历 records → 创建/更新条目（downloadInfo）
  // 3. 遍历 queueTasks → 更新条目（queueTask）
  // 4. 遍历 videoList → 创建/更新条目（channelInfo）
  // 5. 排序：下载中/等待中 → 已暂停 → 已完成 → 未下载 → 仅记录旧视频 → 失败
  // 6. 返回 Array.from(index.values())
}
```

key 生成规则: `channelInfo.id || downloadInfo.video_id || queueTask.video_id || url`
