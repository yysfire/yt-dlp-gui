# Research: 基本订阅管理

**Feature**: 001-basic-subscription-management
**Date**: 2026-05-31

## Research Tasks

### 1. yt-dlp 频道信息解析

**Decision**: 使用 `yt-dlp --dump-json --playlist-items 1 <url>` 解析频道元数据

**Rationale**:
- `--dump-json` 输出 JSON 格式，易于解析
- `--playlist-items 1` 仅获取第一条（频道信息），避免拉取整个视频列表
- stdout 输出为单行 JSON，包含 `channel`, `channel_url`, `thumbnails`, `description` 等字段
- 平台识别通过 URL 模式匹配：`youtube.com`/`youtu.be` → youtube，`bilibili.com` → bilibili

**Alternatives considered**:
- yt-dlp Python API：需要 Python 运行时，增加依赖复杂度
- 独立 HTTP API 调用各平台：需维护多个 API 实现，yt-dlp 统一了接口

### 2. 去重策略

**Decision**: 以频道 URL 为唯一标识符进行去重检查

**Rationale**:
- 频道 URL 是用户输入的原始标识
- StorageService 在添加前加载现有订阅列表，比对 URL
- `AppError::Duplicate` 在前端转为用户友好的提示

**Alternatives considered**:
- 频道 ID（yt-dlp 解析的 channel_id）：更精确但依赖 yt-dlp 解析成功，且跨平台 ID 格式不统一
- 组合键（platform + channel_id）：增加复杂度，URL 比对已足够可靠

### 3. 订阅状态管理

**Decision**: React Hook (`useSubscriptions`) 管理本地状态，通过 Tauri invoke 与后端同步

**Rationale**:
- `useSubscriptions()` 持有 `Subscription[]` 状态
- 增删改操作先调用 invoke，成功后乐观更新本地状态
- 不引入 Redux/Zustand：订阅数据量小（< 1000），props 传递足够

**Alternatives considered**:
- Redux/Zustand：过度设计，YAGNI 原则不通过
- 纯前端轮询：无法保证数据一致性，invoke 调用更可靠

### 4. 分组功能设计（MVP）

**Decision**: MVP 使用预定义固定分组，存储为 Subscription 的 `group_name` 字段

**Rationale**:
- 分组选项：未分组、学习、娱乐、音乐、科技、其他
- Subscription 模型新增 `group_name: Option<String>` 字段（兼容现有数据）
- 前端 `SubscriptionList` 增加分组筛选下拉框
- KISS 原则：不创建独立 Group 实体或关联表

**Alternatives considered**:
- 独立 Group 表 + 多对多关系：过度设计，当前需求仅需简单分类
- 标签系统：P1 阶段实现，MVP 用固定分组

### 5. 并发安全

**Decision**: 利用 Tauri 串行命令执行机制，避免后端并发冲突

**Rationale**:
- Tauri 命令在同一线程串行执行（tokio 单线程运行时）
- StorageService 的 JSON 文件读写为原子操作（先写临时文件再 rename）
- 前端通过 loading 状态防止用户重复提交

## 技术选型确认

| 决策点 | 方案 | 确认 |
|--------|------|------|
| 频道解析 | yt-dlp CLI | ✅ 已实现 |
| 存储格式 | JSON 文件 | ✅ 已实现 |
| 状态管理 | React Hook + props | ✅ 已实现 |
| 去重标识 | 频道 URL | ✅ 已实现 |
| 分组方案 | 固定预定义分组 | 需新增字段 |
| 错误处理 | AppError 枚举 | ✅ 已实现 |
