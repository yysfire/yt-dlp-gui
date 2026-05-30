# IPC Contracts: 基本订阅管理

**Feature**: 001-basic-subscription-management
**Date**: 2026-05-31

## 1. add_subscription

添加新的频道订阅。

```
Tauri Command: add_subscription
```

**输入**:
```json
{
  "url": "https://www.youtube.com/@example"
}
```

**成功输出**:
```json
{
  "id": "uuid-v4",
  "url": "https://www.youtube.com/@example",
  "platform": "youtube",
  "channel_name": "示例频道",
  "channel_avatar_url": "https://yt3.ggpht.com/...",
  "description": "频道描述",
  "paused": false,
  "quality_preset": "1080p",
  "group_name": "未分组",
  "created_at": "2026-05-31T10:00:00Z",
  "last_checked_at": null
}
```

**错误**:
| 错误类型 | 消息 | 场景 |
|----------|------|------|
| Duplicate | "该频道已订阅" | URL 已存在 |
| YtDlp | yt-dlp stderr | 频道解析失败 |
| Io | 文件读写错误 | 存储失败 |

**后端流程**:
1. `StorageService::load_subscriptions()` — 检查 URL 重复
2. `YtDlpService::parse_channel_info(url)` — 解析频道
3. 创建 `Subscription::new()` — 生成 UUID, 设置默认值
4. `StorageService::save_subscriptions()` — 持久化

---

## 2. delete_subscription

删除指定订阅。

```
Tauri Command: delete_subscription
```

**输入**:
```json
{
  "id": "uuid-v4"
}
```

**成功输出**: `null` (void)

**错误**:
| 错误类型 | 消息 | 场景 |
|----------|------|------|
| NotFound | "订阅不存在" | ID 无效 |

**后端流程**:
1. `StorageService::load_subscriptions()` — 获取列表
2. 按 id 过滤移除
3. `StorageService::save_subscriptions()` — 持久化

---

## 3. get_subscriptions

获取所有订阅列表。

```
Tauri Command: get_subscriptions
```

**输入**: 无参数

**成功输出**:
```json
[{...Subscription...}, ...]
```

**后端流程**:
1. `StorageService::load_subscriptions()` — 直接返回

---

## 4. toggle_subscription_pause

切换订阅的启用/禁用状态。

```
Tauri Command: toggle_subscription_pause
```

**输入**:
```json
{
  "id": "uuid-v4"
}
```

**成功输出**: 更新后的完整 Subscription 对象

**后端流程**:
1. 加载订阅列表, 找到目标
2. 翻转 `paused` 字段
3. 保存并返回

---

## 5. update_subscription_quality

更新订阅的画质预设。

```
Tauri Command: update_subscription_quality
```

**输入**:
```json
{
  "id": "uuid-v4",
  "quality_preset": "720p"
}
```

**成功输出**: 更新后的完整 Subscription 对象

---

## 6. update_subscription_group [MVP 新增]

更新订阅的分组。

```
Tauri Command: update_subscription_group
```

**输入**:
```json
{
  "id": "uuid-v4",
  "group_name": "学习"
}
```

**成功输出**: 更新后的完整 Subscription 对象

**验证**: `group_name` 必须在预定义列表中: ["未分组", "学习", "娱乐", "音乐", "科技", "其他"]

---

## 前端调用封装 (src/lib/tauri.ts)

```typescript
export async function addSubscription(url: string): Promise<Subscription>;
export async function deleteSubscription(id: string): Promise<void>;
export async function getSubscriptions(): Promise<Subscription[]>;
export async function toggleSubscriptionPause(id: string): Promise<Subscription>;
export async function updateSubscriptionQuality(id: string, qualityPreset: string): Promise<Subscription>;
export async function updateSubscriptionGroup(id: string, groupName: string): Promise<Subscription>; // 新增
```
