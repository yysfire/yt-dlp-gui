# Feature Specification: 高级设置 (Advanced Settings)

**Feature Branch**: `016-advanced-settings`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-016-01: 计划任务高级设置

用户配置定时调度器的精细参数，包括 Cron 表达式级别的时间控制、按频道分别配置检测间隔、突发事件触发策略。

**Given** 用户有 10 个订阅频道，更新频率差异大（日更到月更）
**When** 用户打开计划任务高级设置
**Then** 可为每个频道单独设置检查间隔：高频频道每 2 小时，低频频道每 12 小时
**And** 支持定义"安静时段"：该时段内不执行任何检查
**And** 支持在用户长时间离开（闲置 > 1 小时）后自动启动全频道检查
**And** 全局支持 Cron 表达式自定义调度时间

### US-016-02: 自定义脚本钩子

用户在下载流程的特定节点（下载前、下载后、失败时）执行自定义 Shell 脚本或 Python 脚本，实现自动化后处理。

**Given** 用户希望下载完成后自动将视频移动到 NAS 并发送通知
**When** 用户在设置中配置"下载完成后"钩子脚本路径 `/home/user/scripts/post_download.sh`
**Then** 每次视频下载完成后，系统以视频文件路径和下载记录 JSON 作为参数执行该脚本
**And** 可查看钩子执行日志（stdout/stderr 和退出码）
**And** 钩子失败不影响下载记录的正常状态更新

### US-016-03: Webhook 支持

用户在特定事件（下载开始、完成、失败、调度器周期结束）发生时，向指定 URL 发送 HTTP POST 请求，实现外部系统联动。

**Given** 用户使用 Home Assistant 智能家居系统
**When** 用户在设置中配置 Webhook URL `http://homeassistant.local:8123/api/webhook/download_complete`
**Then** 每次视频下载完成后，系统向该 URL 发送 POST（JSON payload 包含频道名、视频标题、文件路径）
**And** 支持自定义 Header 和认证方式（Bearer Token / API Key）
**And** 发送失败时进行最多 3 次重试
**And** 记录最近 20 条 Webhook 发送历史

### US-016-04: 数据库管理工具

用户通过应用内的数据库管理界面查看数据存储详情、执行数据修复、重建索引、清理孤立记录。

**Given** 使用 6 个月后，download_records.json 中出现孤立记录（订阅已删除但记录仍在）
**When** 用户打开"数据库管理"工具
**Then** 系统分析：孤立记录数、文件缺失记录数、总文件数、JSON 文件大小
**And** 提供"清理孤立记录"按钮，一键删除无对应订阅的下载记录
**And** 提供"验证文件"按钮，检查下载记录指向的文件是否存在
**And** 提供"压缩数据库"按钮，重新整理 JSON 格式减小文件体积

### US-016-05: 日志管理

用户查看应用的运行日志，配置日志级别，按模块过滤，导出日志用于问题排查。

**Given** 应用在后台运行中出现下载失败
**When** 用户打开"日志管理"页面
**Then** 显示按时间排序的日志列表，支持按级别（TRACE/DEBUG/INFO/WARN/ERROR）筛选
**And** 支持按模块筛选（调度器、下载、存储、UI）
**And** 支持实时日志流（自动滚动到最新）
**And** 支持导出日志为文件（带时间戳的 .log 文件）
**And** 支持配置日志保留天数（默认 7 天，自动轮转）

## Requirements *(mandatory)*

### Functional Requirements

#### FR-016-01: 计划任务高级设置
- 系统 SHALL 在 Subscription 模型中增加 check_interval_minutes 字段（默认使用全局值）
- 系统 SHALL 在调度器中实现按频道独立间隔的检查逻辑
- 系统 SHALL 支持 Cron 表达式解析（通过 `cron` crate）
- 系统 SHALL 支持"安静时段"：在该时段内暂停所有后台检查
- 系统 SHALL 支持"闲置检测"：基于用户鼠标/键盘事件，超过设定时间触发自动检查
- 系统 SHALL 在设置页面中提供 Cron 表达式验证和自然语言描述

#### FR-016-02: 自定义脚本钩子
- 系统 SHALL 支持三个钩子点：before_download（下载前）、after_download（下载完成）、on_download_error（下载失败）
- 系统 SHALL 通过 `Command::new()` 执行脚本，传递参数：file_path, video_title, channel_name, video_url, download_status
- 系统 SHALL 设置脚本超时时间（默认 300 秒）
- 系统 SHALL 记录钩子执行结果：exit_code, stdout, stderr, duration_ms，存入 hooks_log.json
- 系统 SHALL 提供钩子测试按钮：使用模拟数据测试脚本是否正常工作

#### FR-016-03: Webhook 支持
- 系统 SHALL 通过 `reqwest` crate 发送 HTTP POST 请求
- 系统 SHALL 支持事件类型：download_started, download_completed, download_failed, scheduler_cycle_complete
- 系统 SHALL 支持自定义 Webhook URL 和 HTTP Header（key-value 对）
- 系统 SHALL 支持认证方式：无认证、Bearer Token、Basic Auth
- 系统 SHALL 支持失败重试：最多 3 次，指数退避（1s, 2s, 4s）
- 系统 SHALL 记录最近 20 条 Webhook 发送历史

#### FR-016-04: 数据库管理工具
- 系统 SHALL 分析数据完整性：孤立记录检测、文件存在性验证、数据格式校验
- 系统 SHALL 支持数据修复操作：清理孤立记录、标记缺失文件、重建索引
- 系统 SHALL 显示存储概览：各 JSON 文件大小、记录数、最后修改时间
- 系统 SHALL 提供 JSON 导出功能（用于手动备份/修复）
- 系统 SHALL 在所有修改操作前创建临时备份

#### FR-016-05: 日志管理
- 系统 SHALL 使用 `log` + `env_logger` 或 `tracing` crate 实现分级日志
- 系统 SHALL 将日志写入 `~/.yt-dlp-sub-gui/logs/` 目录，按天轮转
- 系统 SHALL 通过 Tauri 命令暴露日志查询接口（按级别/模块/时间范围）
- 系统 SHALL 在前端提供日志查看器组件
- 系统 SHALL 支持配置日志级别和保留天数

### Key Entities

- **ScheduleConfig**: 调度配置，包含 global_interval_minutes, cron_expression, quiet_hours (可选), idle_threshold_minutes
- **ScriptHook**: 脚本钩子配置，包含 hook_point (enum), script_path, enabled, timeout_seconds, description
- **WebhookConfig**: Webhook 配置，包含 event_type, url, http_method, headers, auth_type, auth_credential, enabled
- **WebhookHistory**: Webhook 发送历史，包含 webhook_id, sent_at, status_code, response_body, retry_count, duration_ms
- **DataHealthReport**: 数据库健康报告，包含 orphan_count, missing_file_count, total_records, json_file_size, last_repair_at

## Success Criteria *(mandatory)*

- SC-016-01: Cron 表达式解析支持标准 5 字段格式，准确率 100%
- SC-016-02: 脚本钩子执行不影响下载主流程，失败率 < 1%（配置错误除外）
- SC-016-03: Webhook 首次发送延迟 < 2 秒，重试成功恢复率 > 80%
- SC-016-04: 数据库分析 10000 条记录耗时 < 3 秒，修复操作 100% 可逆（自动备份）
- SC-016-05: 日志查看器支持 10MB 日志文件流畅浏览（虚拟滚动），过滤延迟 < 100ms

## Assumptions

- 脚本钩子仅支持本地文件系统中的可执行文件或脚本
- Webhook 目标 URL 应为 HTTP/HTTPS 端点，不支持其他协议
- 数据库管理操作仅处理 JSON 文件，不涉及 SQL 数据库迁移
- 日志写入采用追加模式，不使用复杂日志框架（如 log4rs）
- 闲置检测复用 Tauri 的窗口事件或系统 API，不实现内核级钩子
