# Data Model: 基本订阅管理

**Feature**: 001-basic-subscription-management
**Date**: 2026-05-31

## Entity: Subscription

```rust
// src-tauri/src/models/subscription.rs
pub struct Subscription {
    pub id: String,             // UUID v4
    pub url: String,            // 频道 URL (去重键)
    pub platform: String,       // "youtube" | "bilibili" | "other"
    pub channel_name: String,   // 频道名称
    pub channel_avatar_url: String, // 封面图 URL
    pub description: String,    // 频道描述
    pub paused: bool,           // 禁用状态 (false = 启用)
    pub quality_preset: String, // 画质预设 ("1080p" 默认)
    pub group_name: String,     // 分组名称 ("未分组" 默认) -- [MVP 新增]
    pub created_at: String,     // ISO 8601 创建时间
    pub last_checked_at: Option<String>, // ISO 8601 最后检查时间 -- [MVP 新增]
}
```

### 字段说明

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| id | UUID v4 | 是 | 自动生成 | 主键，唯一标识 |
| url | String | 是 | — | 频道 URL，去重依据 |
| platform | String | 是 | "other" | youtube / bilibili / other |
| channel_name | String | 是 | — | 频道显示名称 |
| channel_avatar_url | String | 否 | "" | 封面图 URL |
| description | String | 否 | "" | 频道描述 |
| paused | bool | 是 | false | 禁用状态 |
| quality_preset | String | 是 | "1080p" | 画质预设 |
| group_name | String | 是 | "未分组" | **新增** MVP 分组字段 |
| created_at | ISO 8601 | 是 | 当前时间 | 创建时间戳 |
| last_checked_at | Option\<ISO 8601> | 否 | None | **新增** 最后检查时间 |

### TypeScript 对应接口

```typescript
// src/types/index.ts
export interface Subscription {
  id: string;
  url: string;
  platform: "youtube" | "bilibili" | "other";
  channel_name: string;
  channel_avatar_url: string;
  description: string;
  paused: boolean;
  quality_preset: string;
  group_name: string;
  created_at: string;
  last_checked_at: string | null;
}
```

### 状态转换

```
Subscription 生命周期:
  [创建] → paused=false, group="未分组", last_checked_at=null
    ├── 用户点击暂停 → paused=true
    ├── 用户点击启用 → paused=false
    ├── 用户移动到分组 → group_name=新分组
    ├── 检查完成 → last_checked_at=当前时间
    └── 用户删除 → 从存储中移除
```

### 预定义分组

```
"未分组" | "学习" | "娱乐" | "音乐" | "科技" | "其他"
```

### 验证规则

- `url`: 非空, 必须是 http/https 开头, 包含 youtube.com / youtu.be / bilibili.com
- `platform`: 必须在 ["youtube", "bilibili", "other"] 中
- `channel_name`: 非空
- `group_name`: 必须在预定义分组列表中
- `quality_preset`: 必须在 ["best", "2160p", "1440p", "1080p", "720p", "480p"] 中

### 存储格式

```json
// ~/.yt-dlp-sub-gui/subscriptions.json
[
  {
    "id": "a1b2c3d4-...",
    "url": "https://www.youtube.com/@example",
    "platform": "youtube",
    "channel_name": "示例频道",
    "channel_avatar_url": "https://...",
    "description": "频道描述文字",
    "paused": false,
    "quality_preset": "1080p",
    "group_name": "科技",
    "created_at": "2026-05-31T10:00:00Z",
    "last_checked_at": "2026-05-31T14:30:00Z"
  }
]
```
