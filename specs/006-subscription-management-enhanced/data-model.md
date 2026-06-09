# Phase 1: Data Model — 订阅管理增强

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md)

## 实体定义

### 1. Subscription（扩展现有）

现有字段保持不变，新增健康状态和标签字段：

| 字段 | 类型 | 变更 | 说明 |
|------|------|------|------|
| `id` | `string` (UUID) | 不变 | 唯一标识 |
| `url` | `string` | 不变 | 频道 URL |
| `channel_name` | `string` | 不变 | 频道名称 |
| `platform` | `string` | 不变 | "youtube" / "bilibili" |
| `enabled` | `boolean` | 不变 | 是否启用调度检查 |
| `quality` | `string` | 不变 | 画质预设 |
| `tags` | `string[]` | **新增** | 分组标签名数组（如 `["学习", "音乐"]`） |
| `health_status` | `"ok" \| "warning" \| "dead" \| null` | **新增** | 健康状态，null 表示未检查 |
| `last_health_check` | `string \| null` (ISO 8601) | **新增** | 上次健康检查时间 |

**Rust 结构体更新** (`models/subscription.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub url: String,
    pub channel_name: String,
    pub platform: String,
    pub enabled: bool,
    pub quality: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub health_status: Option<HealthStatus>,
    #[serde(default)]
    pub last_health_check: Option<String>,
}
```

### 2. HealthStatus（新增枚举）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Warning,
    Dead,
}
```

TypeScript 对应:

```typescript
export type HealthStatus = "ok" | "warning" | "dead";
```

### 3. HealthCheckResult（运行时传输用）

不持久化，仅在命令返回值和事件中使用。

| 字段 | 类型 | 说明 |
|------|------|------|
| `subscription_id` | `string` | 关联的订阅 ID |
| `url` | `string` | 检查的 URL |
| `status` | `HealthStatus` | 检查结果 |
| `detail` | `string` | 详细原因（如 "404 Not Found"） |
| `latency_ms` | `number` | 响应延迟（毫秒） |
| `checked_at` | `string` (ISO 8601) | 检查时间 |

### 4. HealthCheckSummary（运行时传输用）

| 字段 | 类型 | 说明 |
|------|------|------|
| `total` | `number` | 检查总数 |
| `ok` | `number` | 正常数 |
| `warning` | `number` | 警告数 |
| `dead` | `number` | 失效数 |
| `duration_ms` | `number` | 总耗时（毫秒） |
| `results` | `HealthCheckResult[]` | 详细结果列表 |

### 5. ImportResult（运行时传输用）

现有模型扩展，增加进度推送。

| 字段 | 类型 | 说明 |
|------|------|------|
| `total` | `number` | 待导入总数 |
| `success` | `number` | 成功数 |
| `failed` | `number` | 失败数 |
| `errors` | `{url: string, reason: string}[]` | 失败详情 |
| `progress` | `number` | 进度百分比 (0-100) |

### 6. FilterState（前端会话状态）

不持久化到 JSON 文件，仅存在于 React 状态。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `platform` | `"all" \| "youtube" \| "bilibili"` | `"all"` | 平台筛选 |
| `status` | `"all" \| "active" \| "paused"` | `"all"` | 启停状态筛选 |
| `group` | `string` | `"全部"` | 分组筛选（"全部" / "未分组" / 具体分组名） |
| `health` | `"all" \| "ok" \| "warning" \| "dead" \| "unchecked"` | `"all"` | 健康状态筛选 |
| `keyword` | `string` | `""` | 搜索关键字 |

### 7. SortState（前端会话状态）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `field` | `"name" \| "created_at" \| "last_health_check"` | `"name"` | 排序字段 |
| `direction` | `"asc" \| "desc"` | `"asc"` | 排序方向 |

## 状态转换

### Subscription.health_status 生命周期

```
null (未检查) ──健康检查──→ "ok" ──────后续检查正常──→ "ok"
                             │                          │
                             ├── HTTP 429/5xx ──────────→ "warning"
                             │                          │
                             └── 404/DNS 错误 ──────────→ "dead"
                                                         │
                                                       后续检查恢复正常 → "ok"
```

### Subscription.enabled 与健康检查交互

```
enabled=true, health_status="dead"
  │
  └── 健康检查完成后自动 → enabled=false（调度器排除）
                            │
                            └── 用户手动重新启用 → enabled=true
```

## 数据持久化

| 数据 | 存储位置 | 格式 | 说明 |
|------|----------|------|------|
| 订阅（含健康状态、标签） | `~/.yt-dlp-sub-gui/subscriptions.json` | JSON 数组 | 扩展现有文件，新增字段 |
| 应用状态 | `~/.yt-dlp-sub-gui/state.json` | JSON | 不变 |
| 设置 | `~/.yt-dlp-sub-gui/settings.json` | JSON | 不变 |
| 下载记录 | `~/.yt-dlp-sub-gui/download_records.json` | JSON | 不变 |
| 筛选偏好 | React 状态（内存） | 不持久化 | 会话级别，应用关闭即丢失 |

## 向后兼容

- `tags` 字段使用 `#[serde(default)]`，旧版 subscriptions.json 缺少该字段时自动设为 `[]`
- `health_status` 使用 `#[serde(default)]`，旧数据自动设为 `null`
- `last_health_check` 使用 `#[serde(default)]`，旧数据自动设为 `null`
