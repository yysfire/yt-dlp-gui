# Feature Specification: 高级订阅功能 (Advanced Subscription)

**Feature Branch**: `015-advanced-subscription`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-014-01: 订阅规则引擎

用户为订阅频道设置自动下载规则（如"仅下载时长大于 10 分钟的视频"或"仅下载标题包含特定关键词的视频"），实现精细化过滤。

**Given** 用户订阅了一个日更频道，每天上传多条长短不一的视频
**When** 用户为这个频道创建规则："标题包含 '教程' AND 时长 > 5分钟"
**Then** 调度器检查该频道时，仅标记匹配规则的新视频为"待下载"
**And** 不匹配规则的视频标记为"已过滤"，不加入下载队列
**And** 用户可在下载记录中看到被过滤的视频及过滤原因

### US-014-02: 自动分类标签

系统根据视频标题、频道名、分类（从 yt-dlp 元数据获取）自动为下载记录打标签，支持多维度检索和组织。

**Given** 用户有 100 条下载记录，分属 10 个不同频道
**When** 系统采用默认规则（按频道名 + 视频类别自动标记）
**Then** 每条记录被自动分配一个或多个标签
**And** 用户可在侧边栏按标签筛选下载记录
**And** 用户可以自定义标签规则（如"标题包含 'vlog' → 打标签 'Vlog'"）

### US-014-03: 订阅分享

用户将订阅频道列表以 OPML 格式分享给他人，或接收他人分享的订阅列表并选择性导入。

**Given** 用户有 30 个订阅频道，希望分享其中 5 个给朋友
**When** 用户选择要分享的频道，点击"导出分享"
**Then** 系统生成仅包含选中频道的 OPML 文件
**And** OPML 中不包含任何下载历史和用户设置
**And** 对方导入时，系统允许逐条预览和选择性导入

### US-014-04: 多平台深度支持

系统针对 Bilibili（包含 UP 主、番剧、合集）、YouTube（频道、播放列表、Shorts）提供平台特定的元数据解析和优化下载策略。

**Given** 用户输入 Bilibili UP 主的"合集"URL
**When** 系统解析该 URL
**Then** 识别为 Bilibili 合集类型，提取合集名称、视频数量、更新频率
**And** 订阅创建成功后，按合集逻辑检查新视频
**And** 对 YouTube Shorts 和普通视频采用不同画质策略
**And** 对 Bilibili 番剧使用独立的季度/集数逻辑

### US-014-05: 订阅备份/恢复

用户手动或自动备份完整的订阅数据和下载记录到指定位置，支持恢复备份到同一应用。

**Given** 用户准备重装系统
**When** 用户打开设置，点击"立即备份"并选择备份目录
**Then** 系统将 subscriptions.json、download_records.json、settings.json、state.json 打包为带时间戳的 ZIP 文件
**And** 系统支持配置自动备份间隔（每天/每周/每月）和保留最近 N 份备份
**And** 恢复时，系统提示将覆盖现有数据并要求确认

## Requirements *(mandatory)*

### Functional Requirements

#### FR-014-01: 订阅规则引擎
- 系统 SHALL 支持为每个订阅配置独立的过滤规则
- 系统 SHALL 支持以下规则条件：标题匹配（正则/通配符）、时长范围（min/max）、是否直播、上传日期范围、播放量阈值
- 系统 SHALL 支持多条件组合（AND/OR 逻辑）
- 系统 SHALL 在下载记录中标记过滤状态（filtered）和过滤原因
- 系统 SHALL 在 Rule 模型中以 JSON 序列化规则表达式

#### FR-014-02: 自动分类标签
- 系统 SHALL 支持自动标签规则：按频道、按视频类别、按标题关键词、按时长区间
- 系统 SHALL 支持手动标签：用户在下载记录详情中添加/删除标签
- 系统 SHALL 在前端提供标签筛选器：按标签过滤下载列表
- 系统 SHALL 提供标签管理界面：查看所有标签、合并标签、删除标签
- 系统 SHALL 支持标签颜色自定义

#### FR-014-03: 订阅分享
- 系统 SHALL 支持导出选中订阅为 OPML 2.0 格式
- 系统 SHALL 支持导出时包含/排除订阅设置（画质、规则、标签）
- 系统 SHALL 支持接收 OPML 并逐条预览、选择性导入
- 系统 SHALL 在导入时检测重复频道（by URL），提示跳过或覆盖
- 系统 SHALL 保证分享文件中不包含用户的 Cookie 和代理信息

#### FR-014-04: 多平台深度支持
- 系统 SHALL 支持 Bilibili 特殊 URL 类型：UP 主空间、番剧页、合集页、系列页
- 系统 SHALL 支持 YouTube 特殊 URL 类型：频道页、播放列表页、Shorts 页
- 系统 SHALL 对 Bilibili 使用平台特定元数据字段（avid, bvid, cid）
- 系统 SHALL 对 YouTube Shorts 默认使用竖屏画质策略
- 系统 SHALL 在 Subscription 模型中增加 platform_sub_type 字段

#### FR-014-05: 订阅备份/恢复
- 系统 SHALL 支持手动触发完整备份（通过 Tauri dialog 选择目标路径）
- 系统 SHALL 将相关 JSON 文件打包为 ZIP（使用 `zip` crate）
- 系统 SHALL 支持配置自动备份频率：每天/每周/每月/关闭
- 系统 SHALL 支持配置备份保留份数（默认 5）
- 系统 SHALL 支持从备份 ZIP 恢复：预览内容 → 用户确认 → 覆盖写入
- 系统 SHALL 在恢复前自动创建当前状态备份（安全措施）

### Key Entities

- **SubscriptionRule**: 订阅规则，包含 subscription_id, conditions (数组), logic (and/or), enabled
- **RuleCondition**: 规则条件，包含 field (title/duration/is_live/upload_date/view_count), operator (contains/regex/equals/greater_than/less_than/between), value
- **Tag**: 标签，包含 id, name, color, type (auto/manual)
- **TagRule**: 自动标签规则，包含 field, pattern, tag_id
- **PlatformSubType**: 平台子类型枚举（YouTube-Channel, YouTube-Playlist, YouTube-Shorts, Bilibili-User, Bilibili-Bangumi, Bilibili-Collection, Bilibili-Series）
- **BackupEntry**: 备份记录，包含 id, created_at, file_path, size_bytes, subscription_count, record_count, is_auto

## Success Criteria *(mandatory)*

- SC-014-01: 规则引擎匹配准确率 100%（正确实现所有操作符），检查耗时 < 50ms/视频
- SC-014-02: 自动标签生成覆盖 100% 的新下载记录，标签筛选响应时间 < 200ms
- SC-014-03: OPML 导出的文件可被标准 RSS 阅读器正确解析，导入检测重复耗时 < 1 秒
- SC-014-04: Bilibili/YouTube 各 URL 类型识别准确率 100%，平台子类型在订阅创建时自动识别
- SC-014-05: 备份打包 1000 条记录耗时 < 5 秒，恢复后数据完整性校验通过率 100%

## Assumptions

- Bilibili 平台 URL 类型可通过正则模式准确识别
- OPML 2.0 导出/导入沿用现有 opml.rs 服务基础
- 规则引擎在检查新视频阶段执行（download.rs 中），不修改 yt-dlp 调用
- 备份/恢复仅处理应用自身数据，不包含下载的视频文件
- 标签颜色支持使用 MUI 预设颜色调色板
