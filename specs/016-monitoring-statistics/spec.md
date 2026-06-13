# Feature Specification: 监控与统计 (Monitoring & Statistics)

**Feature Branch**: `016-monitoring-statistics`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-015-01: 详细统计报表

用户在新"统计"面板中查看下载活动的多维统计，包括总下载量、各频道贡献、月度趋势、画质分布等。

**Given** 用户已使用应用下载视频 3 个月，共 500+ 条下载记录
**When** 用户打开"统计"面板
**Then** 系统显示：总视频数、总存储占用、平均每周下载数、最近 30 天下载趋势图
**And** 支持按频道筛选统计数据
**And** 支持按画质、格式、时间段分组统计

### US-015-02: 系统资源监控

用户在状态栏或设置中查看应用的实时系统资源占用（CPU、内存、磁盘 I/O），及时发现异常。

**Given** 应用正在后台运行，下载队列中有 5 个任务
**When** 用户查看状态栏中的资源监控小部件
**Then** 显示当前 CPU 占用百分比、内存占用（MB）、下载目录磁盘 I/O 速率
**And** 资源数据每 5 秒刷新一次
**And** 当内存 > 500MB 或 CPU > 80% 时高亮警告

### US-015-03: 下载历史图表

用户以可视化图表查看下载历史的时间分布（日/周/月），发现下载趋势和活跃频道。

**Given** 用户有 6 个月的下载历史数据
**When** 用户打开"下载历史"图表页面
**Then** 显示折线图：按周/月统计的下载数量趋势
**And** 显示柱状图：各频道下载数量排名（Top 10）
**And** 支持切换时间粒度（日/周/月）
**And** 支持导出图表为 PNG 截图

### US-015-04: 订阅活跃度分析

用户查看各订阅频道的活跃度评分，识别"僵尸频道"（长期不更新）和高产频道。

**Given** 用户有 30 个订阅频道，其中 5 个已超过 3 个月无更新
**When** 用户打开"订阅活跃度"分析页面
**Then** 系统根据最后更新时间、更新频率、历史新视频数计算活跃度评分（0-100）
**And** 活跃度 < 20 的频道标记为"不活跃"，建议用户关注
**And** 显示各频道最近 10 次更新的时间线

### US-015-05: 磁盘使用分析

用户查看下载目录的磁盘空间使用详析，包括各频道占用分布、文件大小排行、大文件识别。

**Given** 下载目录占用 50GB 空间，分属 20 个频道
**When** 用户打开"磁盘使用分析"面板
**Then** 显示各频道占用空间（饼图 + 列表）
**And** 显示按文件大小降序排列的 Top 20 视频
**And** 显示超过 1GB 的单个文件（标注大小和频道）
**And** 支持设置空间使用告警阈值

## Requirements *(mandatory)*

### Functional Requirements

#### FR-015-01: 统计报表
- 系统 SHALL 实现统计计算服务（Rust 端），聚合 download_records.json 数据
- 系统 SHALL 提供以下统计指标：总下载数、成功/失败数、总大小、平均大小、画质分布、格式分布、频道贡献排名、每日/周/月下载趋势
- 系统 SHALL 支持按时间范围筛选（最近 7 天/30 天/90 天/本年/全部）
- 系统 SHALL 支持按订阅频道筛选
- 系统 SHALL 支持导出统计报表为 CSV

#### FR-015-02: 系统资源监控
- 系统 SHALL 通过 `sysinfo` crate 获取 CPU、内存使用率
- 系统 SHALL 通过 `std::fs` 获取下载目录磁盘读写速率
- 系统 SHALL 通过 Tauri 事件将监控数据定时推送到前端（每 5 秒）
- 系统 SHALL 记录历史资源数据（最近 1 小时），支持迷你折线图
- 系统 SHALL 在后台下载时默认显示资源监控，空闲时自动隐藏

#### FR-015-03: 下载历史图表
- 系统 SHALL 使用 Chart.js 或 ECharts 在前端渲染图表
- 系统 SHALL 支持柱状图、折线图、饼图三种图表类型
- 系统 SHALL 支持切换时间粒度（日/周/月）
- 系统 SHALL 支持点击图表元素跳转到对应下载记录
- 系统 SHALL 支持导出图表为图片（Canvas toDataURL → 保存文件）

#### FR-015-04: 订阅活跃度分析
- 系统 SHALL 计算活跃度评分：基于最近更新距离天数 + 更新频率 + 新视频数量加权
- 系统 SHALL 在订阅列表中显示活跃度指示器（绿/黄/红圆点）
- 系统 SHALL 提供活跃度详情页面：显示计算因子和最近更新时间线
- 系统 SHALL 支持按活跃度排序订阅列表

#### FR-015-05: 磁盘使用分析
- 系统 SHALL 遍历下载记录，通过 `std::fs::metadata` 获取文件大小
- 系统 SHALL 按频道汇总存储空间占用
- 系统 SHALL 支持配置大文件阈值（默认 500MB）
- 系统 SHALL 支持在磁盘使用分析页面直接删除视频文件（需确认）
- 系统 SHALL 在总空间超过阈值时推送通知

### Key Entities

- **StatisticsSnapshot**: 统计快照，包含 total_downloads, total_size_bytes, success_count, failed_count, by_channel, by_quality, by_format, daily_trend, weekly_trend, monthly_trend
- **ResourceMetrics**: 资源指标，包含 cpu_percent, memory_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec, timestamp
- **ChannelActivityScore**: 频道活跃度评分，包含 subscription_id, score, factors (last_update_days, avg_interval_days, total_new_videos), last_10_updates
- **DiskUsageSummary**: 磁盘使用摘要，包含 total_used_bytes, by_channel, top_large_files, largest_file_bytes, file_count

## Success Criteria *(mandatory)*

- SC-015-01: 统计计算在 5000 条记录下完成耗时 < 2 秒
- SC-015-02: 资源监控 CPU + 内存开销 < 1%（sysinfo 采样）
- SC-015-03: 图表渲染在 1000 条数据点下流畅（< 1 秒首次渲染）
- SC-015-04: 活跃度评分计算覆盖所有订阅（无遗漏），< 1 秒
- SC-015-05: 磁盘分析遍历 5000 个文件耗时 < 3 秒（缓存后可 < 100ms）

## Assumptions

- 前端图表库使用 Chart.js 或 ECharts（具体选型根据包体积和 MUI 兼容性决定）
- 统计计算在 Rust 后端执行，前端仅负责渲染
- 资源历史数据仅保留最近 1 小时，不写入持久化存储
- 磁盘使用分析结果可缓存 5 分钟，避免频繁扫描
- 活跃度计算基于下载记录中的时间戳，不重新调用 yt-dlp
