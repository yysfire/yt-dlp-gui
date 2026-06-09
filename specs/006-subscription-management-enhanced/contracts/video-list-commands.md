# Tauri 命令契约：详情面板视频列表

**Date**: 2026-06-10 | **Spec**: [spec.md](../spec.md)

## 命令

### `get_channel_videos`（新增）

获取订阅频道的最新视频列表（分页）。

```
Rust: get_channel_videos(subscription_id: String, page: u32, page_size: u32)
TypeScript: invoke("get_channel_videos", { subscriptionId, page, pageSize })

返回值: VideoListResult
```

**参数**:
- `subscriptionId` (string): 订阅 ID
- `page` (number): 页码（1-based），默认 1
- `pageSize` (number): 每页条数，默认 10

**返回值 `VideoListResult`**:
```typescript
interface VideoListResult {
  videos: VideoInfo[];
  total: number;       // 频道总视频数（从 yt-dlp 输出推断）
  page: number;
  page_size: number;
  has_more: boolean;   // 是否还有更多页
}

interface VideoInfo {
  id: string;
  title: string;
  url: string;
  duration: string | null;
  upload_date: string | null;
  thumbnail: string | null;
}
```

**Rust 实现**:
1. 从 subscriptions.json 加载目标订阅，获取 URL
2. 构建 yt-dlp 命令：
   ```
   yt-dlp --flat-playlist --dump-json --playlist-start {start} --playlist-end {end} <url>
   ```
   其中 `start = (page-1) * pageSize + 1`, `end = page * pageSize`
3. 解析 JSON 输出，提取 id、title、url、duration、upload_date、thumbnail
4. 对于最后一页，尝试多请求一条来判断 `has_more`
5. 返回 VideoListResult

**频道信息字段**（从首次 yt-dlp `--dump-json --playlist-items 1` 获取并缓存）:
```typescript
interface ChannelInfo {
  channel_name: string;
  description: string | null;
  subscriber_count: string | null;
  video_count: number | null;
  thumbnail_url: string | null;
  last_refreshed: string; // ISO 8601
}
```

## Frontend Tauri 封装

```typescript
// src/lib/tauri.ts 新增函数

export async function getChannelVideos(
  subscriptionId: string,
  page: number = 1,
  pageSize: number = 10
): Promise<VideoListResult> {
  return invoke("get_channel_videos", { subscriptionId, page, pageSize });
}

export async function getChannelInfo(
  subscriptionId: string
): Promise<ChannelInfo> {
  return invoke("get_channel_info", { subscriptionId });
}
```

## 详情面板状态管理

前端 `DetailPanel.tsx` 中的加载流程：

```
用户点击订阅 → setSelectedId → DetailPanel 挂载
  ├── 加载频道信息（getChannelInfo）
  │     └── 成功 → 显示频道名、描述、订阅数、缩略图
  │     └── 失效（health_status="dead"）→ 显示"此频道已失效"
  └── 加载视频列表（getChannelVideos, page=1）
        └── 成功 → 显示视频列表
        └── 用户滚动到底部 → getChannelVideos(page=page+1) → 追加
```

## 边缘情况

- 订阅 `health_status === "dead"` 时，不调用 yt-dlp，直接显示"无法获取"
- 用户快速切换订阅时，通过 AbortController / 请求 ID 取消前一个请求
- 频道无视频时显示空状态
