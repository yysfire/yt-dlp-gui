# DetailPanel 统一视频列表重构

**日期**：2026-06-13
**状态**：已批准

## 问题

`DetailPanel.tsx` 渲染两个完全独立的区域：「最新视频」（来自 `getChannelVideos` 的频道播放列表）和「下载记录」（来自 `records + queueTasks` 的 `DownloadRecordList`）。两者从未交叉去重，导致已下载的视频在两个区域各出现一次，构成视觉重复和信息割裂。

## 设计

### 核心变更

将两个独立区域合并为一个统一的「全部视频」列表：

1. **即时渲染下载记录**：`records + queueTasks` 已在内存中，无需等待即可展示
2. **加载第 1 页频道视频**：调用 `getChannelVideos(subscriptionId, 1, 10)`，合并到统一列表
3. **后台自动加载剩余页**：异步递归加载后续页面，静默追加到列表尾部，无「加载更多」按钮
4. **旧下载记录保留**：不在频道视频列表中的旧下载记录也保留在统一列表中

### 统一数据结构

```typescript
interface UnifiedVideoItem {
  channelInfo?: VideoInfo;       // 频道视频信息（仅来自频道视频时有值）
  downloadInfo?: DownloadRecord;  // 下载记录（仅已下载/下载中有值）
  queueTask?: DownloadTask;       // 队列任务（仅下载等待中有值）

  readonly id: string;
  readonly title: string;
  readonly url: string;
  readonly status: VideoStatus;
}

type VideoStatus = "new" | "downloading" | "paused" | "waiting" | "completed" | "failed" | "cancelled";
```

### 合并规则

| 匹配键 | 规则 | 示例 |
|--------|------|------|
| `video_id`（优先） | 频道视频 id === 下载记录 video_id | 同一个 YouTube 视频 ID |
| `video_url`（回退） | 频道视频 url === 下载记录 video_url | 无 video_id 时的降级匹配 |
| 仅下载记录 | 下载记录的 video_id/video_url 不在频道视频中 | 很久以前下载的旧视频 |
| 仅频道视频 | 频道视频 id/url 不在下载记录中 | 新发布未下载的视频 |

### 列表排序

1. 下载中/等待中的任务 — 置顶
2. 已暂停的任务
3. 已完成的下载记录 + 匹配到下载记录的频道视频 — 按下载时间倒序
4. 未下载的频道视频 — 按上传日期倒序
5. 仅来自下载记录的旧视频 — 按下载时间倒序
6. 失败的记录 — 最后

### 条目布局：三行三列网格

每个视频条目内部采用 3 列 × 3 行网格：

```
┌──────┬──────────────────────────────────────────┬──────┐
│ 第1列 │ 第1行：视频标题                            │ 第3列 │
│      ├──────────────────────────────────────────┤      │
│ 状态  │ 第2行：左(状态文本·发布日期间·时长)  右(进度/大小/时间) │ 操作  │
│ 图标  ├──────────────────────────────────────────┤ 按钮  │
│      │ 第3行：进度条（仅下载中）                    │      │
└──────┴──────────────────────────────────────────┴──────┘
```

列规范：

| 列 | 宽度 | 内容 | 对齐 |
|----|------|------|------|
| 第1列 | 40px | 状态图标 (▶/⬇/✓/✕/⏳) | 垂直居中，水平居中 |
| 第2列 | flex: 1 | 标题 + 元信息 + 进度条 | 左对齐（标题、元信息）、右对齐（尾部信息） |
| 第3列 | 40px | 操作按钮（暂停/取消/重试） | 垂直居中，水平居中 |

各状态的视觉呈现：

- **未下载**：第1列 ▶ 灰色图标，第2列显示标题+日期+时长，第3列空
- **等待中**：第1列 ⏳ 灰色图标，第2列标题+"等待中·日期·时长"，第3列 ✕ 取消按钮
- **下载中**：第1列 ⬇ 蓝色图标，第2列标题+"下载中·日期·时长"+"进度%+文件大小"+进度条，第3列 ⏸ ✕ 暂停/取消
- **已完成**：第1列 ✓ 绿色图标，第2列标题+"已完成·日期·时长"+"文件大小·下载时间"，第3列空
- **失败**：第1列 ✕ 红色图标，第2列标题+"失败·日期·时长·错误信息"+"下载时间"，第3列 ↻ 重试

### 组件关系与 DownloadRecordList 移除

当前组件依赖图：

```
AppShell
  ├─ activeView === "detail"   → DetailPanel
  │                                 ├── 「最新视频」区域（videoList）
  │                                 └── DownloadRecordList → DownloadRecordItem
  └─ activeView === "downloads" → DownloadedList → DownloadedItem
```

**DownloadRecordList 仅被 DetailPanel 使用**（`src/components/DetailPanel.tsx:12`），与其他视图完全解耦。

- 侧边栏「已下载」按钮的全局视图使用 `DownloadedList` → `DownloadedItem`，不受影响
- `subscription.download_count` 由 Rust 后端 `recompute_subscription_download_counts()` 计算，不受前端渲染组件影响

**移除决策**：`DownloadRecordList` 和 `DownloadRecordItem` 在统一列表实现后可安全移除。DetailPanel 内部自行管理合并与渲染逻辑。

### 进度条实现

`DownloadProgressBar` 组件保留。下载中条目的第3行进度条直接使用该组件，位于第2列底部占满整行宽度。

### 数据同步

| 触发事件 | 列表行为 |
|----------|----------|
| 切换订阅 | 清空列表，取消进行中的后台加载，重新执行加载流程 |
| 新下载开始 | 若列表顶部已有该视频，状态变为"下载中"并显示进度 |
| 下载完成 | 状态变为"已完成"，更新文件大小和时间 |
| 后台加载新页 | 静默追加到列表尾部，无闪烁 |
| 旧视频已下载 | 始终保留在列表中（无频道元数据也显示） |

## 改动范围

| 文件 | 改动 | 说明 |
|------|------|------|
| `src/components/DetailPanel.tsx` | 重写 | 合并两个区域；新增 UnifiedVideoItem 合并逻辑、后台自动分页、三行三列布局 |
| `src/components/DownloadRecordList.tsx` | 移除 | 不再需要 |
| `src/components/DownloadRecordItem.tsx` | 保留或移除 | 如统一列表中复用其部分渲染逻辑则保留，否则移除 |
| `src/components/DownloadProgressBar.tsx` | 保留 | 统一列表的下载中条目仍需要进度条 |
| `src/types/index.ts` | 新增类型 | 添加 UnifiedVideoItem、VideoStatus 等类型 |
| 其他文件 | 无改动 | AppShell、DownloadedList、SubscriptionList 等均不受影响 |

## 自检

- **占位符**：无
- **一致性**：布局规范统一（三行三列网格），与现有 AppShell 的双视图切换机制兼容
- **范围**：集中于 DetailPanel 重写，不涉及后端 API 或全局状态变更
- **模糊性**：合并规则已明确（video_id 优先 / video_url 回退）；排序规则已枚举；各状态的左右布局已限定
