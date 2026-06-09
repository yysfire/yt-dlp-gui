# Tauri 命令契约：订阅管理

**Date**: 2026-06-10 | **Spec**: [spec.md](../spec.md)

## 命令列表

### 1. `update_subscription_tags`

更新订阅标签。

```
Rust: update_subscription_tags(id: String, tags: Vec<String>)
TypeScript: invoke("update_subscription_tags", { id, tags })

返回值: Subscription
```

**参数**:
- `id` (string): 订阅 ID
- `tags` (string[]): 新标签名列表（覆盖式更新）

### 2. `get_all_tags`

获取所有已定义的标签（从所有订阅的 tags 中聚合去重）。

```
Rust: get_all_tags()
TypeScript: invoke("get_all_tags")

返回值: string[]
```

### 3. `batch_delete_subscriptions`

批量删除订阅及其下载记录。

```
Rust: batch_delete_subscriptions(ids: Vec<String>)
TypeScript: invoke("batch_delete_subscriptions", { ids })

返回值: { deleted_count: number }
```

**参数**:
- `ids` (string[]): 待删除的订阅 ID 列表

**行为**:
1. 从 subscriptions.json 中移除
2. 移除关联的下载记录
3. 返回删除数量

### 4. `update_subscription_health`（内部）

健康检查完成后由后台任务调用，更新订阅的健康状态字段。

```
Rust: update_subscription_health(id: String, status: HealthStatus, checked_at: String, auto_pause: bool)
TypeScript: 无（后端内部调用，不暴露给前端）

返回值: Result<(), String>
```

**参数**:
- `id` (string): 订阅 ID
- `status` (HealthStatus): "ok" | "warning" | "dead"
- `checked_at` (string): ISO 8601 检查时间
- `auto_pause` (bool): 当 status 为 "dead" 时是否自动将 enabled 设为 false

**行为**:
1. 加载 subscriptions.json
2. 查找对应订阅，更新 health_status 和 last_health_check
3. 若 auto_pause=true 且 status="dead"，将 enabled 设为 false
4. 保存 subscriptions.json

## TypeScript 类型定义

```typescript
// src/types/index.ts 新增或修改

export type HealthStatus = "ok" | "warning" | "dead";

export interface Subscription {
  id: string;
  url: string;
  channel_name: string;
  platform: string;
  enabled: boolean;
  quality: string;
  tags: string[];                    // 新增
  health_status: HealthStatus | null;  // 新增
  last_health_check: string | null;    // 新增
}

export interface FilterState {
  platform: "all" | "youtube" | "bilibili";
  status: "all" | "active" | "paused";
  group: string;
  health: "all" | "ok" | "warning" | "dead" | "unchecked";
  keyword: string;
}

export interface SortState {
  field: "name" | "created_at" | "last_health_check";
  direction: "asc" | "desc";
}
```

## Frontend Tauri 封装

```typescript
// src/lib/tauri.ts 新增函数

export async function updateSubscriptionTags(
  id: string,
  tags: string[]
): Promise<Subscription> {
  return invoke("update_subscription_tags", { id, tags });
}

export async function getAllTags(): Promise<string[]> {
  return invoke("get_all_tags");
}

export async function batchDeleteSubscriptions(
  ids: string[]
): Promise<{ deleted_count: number }> {
  return invoke("batch_delete_subscriptions", { ids });
}
```
