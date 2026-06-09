# Quickstart: 订阅管理增强开发指南

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md)

## 环境准备

```bash
# 确保在项目根目录
cd /home/yys/projects/yt-dlp-gui

# 确认 yt-dlp 可用
yt-dlp --version

# 安装依赖（如首次）
npm install
```

## 开发启动

```bash
# 启动 Tauri 开发模式（前端 + Rust 后端）
npm run tauri dev

# 仅前端开发（Vite，端口 5173）
npm run dev

# Rust 编译检查
cargo check

# TypeScript 类型检查
npx tsc --noEmit
```

## 新增 Rust 依赖

在 `src-tauri/Cargo.toml` 中添加：

```toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
```

## 实现顺序（建议）

遵循 TDD 原则，按以下顺序实现：

### Phase 2a: 数据模型扩展
1. 扩展 `Subscription` 结构体（添加 tags、health_status、last_health_check）
2. 新增 `HealthStatus` 枚举和 `HealthCheckResult` 模型
3. 扩展 TypeScript 类型定义
4. 编写序列化/反序列化单元测试

### Phase 2b: 健康检查
1. 实现 `services/health.rs`（HealthService）
2. 实现 `commands/health.rs`（Tauri 命令）
3. 编写单元测试（Rust #[cfg(test)]）
4. 前端：实现 `useHealthCheck` hook + `HealthCheckPanel` 组件

### Phase 2c: 筛选与排序
1. 前端：实现 `useFilter` hook
2. 前端：实现 `FilterBar` 组件
3. 前端：扩展 `SubscriptionList`（使用 useFilter）
4. 前端：实现搜索高亮（SubscriptionItem 修改）

### Phase 2d: 订阅标签
1. 扩展 `StorageService`（标签 CRUD）
2. 实现 `update_subscription_tags` 命令
3. 前端：标签选择器 UI

### Phase 2e: 详情面板增强
1. 实现 `get_channel_videos` 命令（分页）
2. 前端：`DetailPanel` 视频列表 + 分页加载
3. 前端：频道信息展示

### Phase 2f: 批量操作
1. 实现 `batch_delete_subscriptions` 命令
2. 前端：批量删除 UI（集成到 HealthCheckPanel）

## 关键文件清单

### Rust 后端

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/Cargo.toml` | 修改 | 添加 reqwest 依赖 |
| `src-tauri/src/models/subscription.rs` | 修改 | 新增字段 |
| `src-tauri/src/models/health.rs` | 新建 | 健康检查模型 |
| `src-tauri/src/models/mod.rs` | 修改 | 注册新模块 |
| `src-tauri/src/services/health.rs` | 新建 | 健康检查服务 |
| `src-tauri/src/services/mod.rs` | 修改 | 注册新模块 |
| `src-tauri/src/services/storage.rs` | 修改 | 标签 CRUD |
| `src-tauri/src/commands/health.rs` | 新建 | 健康检查命令 |
| `src-tauri/src/commands/subscription.rs` | 修改 | 标签更新、批量删除 |
| `src-tauri/src/commands/mod.rs` | 修改 | 注册新模块 |
| `src-tauri/src/lib.rs` | 修改 | 注册新 Tauri 命令 |

### 前端

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/types/index.ts` | 修改 | 新增类型定义 |
| `src/lib/tauri.ts` | 修改 | 新增 invoke 封装 |
| `src/hooks/useFilter.ts` | 新建 | 筛选排序 Hook |
| `src/hooks/useHealthCheck.ts` | 新建 | 健康检查 Hook |
| `src/components/FilterBar.tsx` | 新建 | 筛选条件栏 |
| `src/components/HealthCheckPanel.tsx` | 新建 | 健康检查面板 |
| `src/components/SubscriptionItem.tsx` | 修改 | 高亮渲染 |
| `src/components/SubscriptionList.tsx` | 修改 | 集成筛选 |
| `src/components/DetailPanel.tsx` | 修改 | 视频列表 |
| `src/components/ImportDialog.tsx` | 修改 | 批量导入预览 |
| `src/App.tsx` | 修改 | 新增 filterState/sortState |
| `src/components/AppShell.tsx` | 修改 | 透传新 props |

## 验证命令

```bash
# Rust 单元测试
cargo test -p yt-dlp-gui

# TypeScript 类型检查
npx tsc --noEmit

# 完整构建
npm run build && cargo build

# 验证新增的 Tauri 命令已在 lib.rs 中注册
grep "check_all_health\|check_selected_health\|batch_import_preview\|get_channel_videos" src-tauri/src/lib.rs
```
