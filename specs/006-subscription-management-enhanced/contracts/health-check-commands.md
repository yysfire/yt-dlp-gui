# Tauri 命令契约：健康检查

**Date**: 2026-06-10 | **Spec**: [spec.md](../spec.md)

## 命令列表

### 1. `check_all_health`

全量健康检查。异步后台任务，立即返回。

```
Rust: check_all_health()
TypeScript: invoke("check_all_health")

返回值: { started: bool, total: number }
```

**行为**:
1. 快照当前订阅列表
2. 通过 `tokio::spawn` 启动后台检查任务
3. 立即返回 `{ started: true, total: N }`
4. 后台任务完成后通过事件 `health-check-complete` 返回结果
5. 检查期间可通过事件 `health-check-progress` 获取进度

**Frontend 调用**:

```typescript
// src/lib/tauri.ts
export async function checkAllHealth(): Promise<{ started: boolean; total: number }> {
  return invoke("check_all_health");
}
```

### 2. `check_selected_health`

选择性健康检查。检查指定订阅 ID 列表。

```
Rust: check_selected_health(subscription_ids: Vec<String>)
TypeScript: invoke("check_selected_health", { subscriptionIds: [...] })

返回值: { started: bool, total: number }
```

**参数**:
- `subscriptionIds` (string[]): 待检查的订阅 ID 列表

**Frontend 调用**:

```typescript
export async function checkSelectedHealth(
  subscriptionIds: string[]
): Promise<{ started: boolean; total: number }> {
  return invoke("check_selected_health", { subscriptionIds });
}
```

### 3. `get_health_results`

获取最近一次健康检查的结果（运行时缓存）。

```
Rust: get_health_results()
TypeScript: invoke("get_health_results")

返回值: HealthCheckSummary | null
```

**Frontend 调用**:

```typescript
export async function getHealthResults(): Promise<HealthCheckSummary | null> {
  return invoke("get_health_results");
}
```

## 事件

### `health-check-progress`

```typescript
// 事件 payload
interface HealthCheckProgress {
  completed: number;
  total: number;
  current_url: string;
}
```

前端监听:

```typescript
const unlisten = await listen<HealthCheckProgress>("health-check-progress", (event) => {
  // 更新进度条
});
```

### `health-check-complete`

```typescript
// 事件 payload
interface HealthCheckSummary {
  total: number;
  ok: number;
  warning: number;
  dead: number;
  duration_ms: number;
  results: HealthCheckResult[];
}

interface HealthCheckResult {
  subscription_id: string;
  url: string;
  status: "ok" | "warning" | "dead";
  detail: string;
  latency_ms: number;
  checked_at: string;
}
```

前端监听:

```typescript
const unlisten = await listen<HealthCheckSummary>("health-check-complete", (event) => {
  // 显示摘要报告
  // 触发 subscriptions 重新加载（健康状态已持久化）
});
```

## Rust 实现约束

- 使用 `reqwest` 客户端，`tauri::async_runtime::spawn` 启动后台任务
- 并发控制：`tokio::sync::Semaphore::new(3)`
- 超时：连接 5 秒，总 10 秒
- 429 重试：最多 2 次，指数退避（2s / 4s + 随机抖动）
- 不能跨异步边界持有 `MutexGuard`（先 clone，后 await）
- 平台：YouTube 和 Bilibili 使用不同 User-Agent（模拟浏览器）
