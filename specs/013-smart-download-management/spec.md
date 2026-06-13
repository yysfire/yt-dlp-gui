# Feature Specification: 智能下载管理 (Smart Download Management)

**Feature Branch**: `013-smart-download-management`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-012-01: 智能下载队列

用户同时触发多个订阅频道的下载，系统根据优先级和并发限制自动调度，避免资源争用。

**Given** 用户有 5 个订阅频道，同时触发下载检查，发现共有 20 个新视频待下载
**When** 系统执行智能队列调度，并发限制为 3
**Then** 同一时刻最多 3 个下载任务并发运行，其余任务进入等待队列
**And** 当前并发任务完成后自动调度队列中下一个任务
**And** 用户可在 UI 中查看队列状态：运行中、等待中、已完成

### US-012-02: 下载时间段限制

用户配置仅在指定时间段内执行下载，其余时间新视频仅标记为"待下载"，等待时间窗口开启后自动开始。

**Given** 用户在设置中将下载时间段设为 02:00 - 06:00
**When** 调度器在 14:00 发现某频道有 3 个新视频
**Then** 系统创建"pending_time_window"状态的下载记录，不发起下载
**And** 当时间到达 02:00 时，系统自动开始下载这些排队视频
**And** 当时间到达 06:00 时，正在下载的任务完成后不再启动新任务

### US-012-03: 网络空闲检测

系统在下载新视频前检测网络是否空闲（无大量其他流量），避免影响用户正常上网体验。

**Given** 用户的网络正在进行大文件下载（占用 > 80% 带宽）
**When** 调度器触发下载检查，发现新视频
**Then** 系统检测到网络繁忙，将下载任务标记为"pending_network"
**And** 系统等待网络空闲（带宽占用 < 10% 持续 30 秒）后才开始下载
**And** 用户可手动覆盖网络空闲检测，立即开始下载

### US-012-04: 断点续传

下载过程中应用崩溃或网络中断，重新启动后系统从断点继续下载，无需重新下载已完成部分。

**Given** 用户正在下载一个 2GB 的视频，已经下载了 800MB
**When** 应用崩溃或系统重启
**Then** 重新启动后，下载记录状态为"paused"，bytes_downloaded 为 800MB
**And** 用户点击"续传"按钮，系统以断点模式重新发起下载
**And** yt-dlp 的续传功能完成剩余部分，避免重复下载

### US-012-05: 磁盘空间监控

下载前检查目标磁盘剩余空间，空间不足时暂停下载并通知用户。

**Given** 用户设置了 5GB 的磁盘空间最低阈值
**When** 下载队列中下一个视频预计占用 2GB，但当前剩余空间仅 1GB
**Then** 系统暂停下载队列，通知用户"磁盘空间不足"
**And** 用户清理空间后点击"恢复下载"，系统重新检查空间后继续

## Requirements *(mandatory)*

### Functional Requirements

#### FR-012-01: 并发下载控制
- 系统 SHALL 支持可配置的并发下载数量（默认 3，范围 1-10）
- 系统 SHALL 维护下载任务队列，按发现时间 FIFO 排序
- 系统 SHALL 允许用户手动调整队列中任务优先级（上移/下移/置顶）
- 系统 SHALL 在并发任务完成或被取消后自动调度队列中下一任务

#### FR-012-02: 下载时间段限制
- 系统 SHALL 支持配置每日允许下载的时间窗口（开始 HH:MM - 结束 HH:MM）
- 系统 SHALL 支持跨天时间窗口（如 22:00 - 06:00）
- 系统 SHALL 支持按星期几分别配置不同时间窗口
- 在非允许时段发现新视频时 SHALL 创建"pending_time_window"记录，不立即下载
- 进入允许时间段时 SHALL 自动启动排队的下载任务
- 退出允许时间段时 SHALL 完成当前下载后停止启动新任务

#### FR-012-03: 网络空闲检测
- 系统 SHALL 通过 `/proc/net/dev`（Linux）、系统 API（Windows/macOS）检测网络流量
- 系统 SHALL 支持配置网络空闲阈值（带宽占用百分比，默认 < 10%）
- 系统 SHALL 支持配置空闲持续时间（默认 30 秒）
- 系统 SHALL 在网络繁忙时暂停新下载任务，标记为"pending_network"
- 系统 SHALL 提供"忽略网络检测"手动选项

#### FR-012-04: 断点续传
- 系统 SHALL 记录每个下载任务的实时进度（已下载字节数）
- 系统 SHALL 在下载记录中保存部分下载文件的完整路径
- 系统 SHALL 支持通过 `yt-dlp --continue` 标志恢复中断的下载
- 系统 SHALL 检测部分下载文件是否存在，不存在时从头开始下载
- 系统 SHALL 提供"续传"和"重新下载"两个选项

#### FR-012-05: 磁盘空间监控
- 系统 SHALL 通过 `std::fs` 或 `statvfs` 检测目标下载目录所在分区的可用空间
- 系统 SHALL 支持配置磁盘空间最低阈值（默认 5GB）
- 系统 SHALL 在每个下载任务启动前检查可用空间
- 空间不足时 SHALL 暂停整个下载队列并显示空间不足通知
- 系统 SHALL 在空间恢复后提示用户恢复下载

### Key Entities

- **DownloadQueueEntry**: 下载队列条目，包含 subscription_id、video_id、priority、queued_at、status（queued/running/completed/failed/cancelled）
- **NetworkMonitorState**: 网络监控状态，包含 current_bandwidth_bytes_per_sec、is_idle、last_idle_since
- **DownloadProgress**: 下载进度，包含 bytes_downloaded、total_bytes、percentage、speed_bytes_per_sec、eta_seconds
- **TimeWindowConfig**: 时间窗口配置，包含 start_time、end_time、days_of_week、enabled
- **DiskCheckResult**: 磁盘检查结果，包含 free_bytes、total_bytes、is_sufficient、configured_threshold

## Success Criteria *(mandatory)*

- SC-012-01: 下载队列可正确管理 100+ 个待下载任务，调度延迟 < 1 秒
- SC-012-02: 时间段限制精确到分钟级别，跨天窗口切换正确率达 100%
- SC-012-03: 网络空闲检测响应时间 < 5 秒，误判率 < 5%
- SC-012-04: 断点续传成功率达 95%（yt-dlp 支持的范围内）
- SC-012-05: 磁盘空间检查开销 < 100ms，零误报（不会在空间充足时错误暂停）

## Assumptions

- yt-dlp 的 `--continue` 标志可稳定用于断点续传
- 系统网络接口统计信息在不同平台上可通过标准方法获取
- 磁盘空间信息通过 POSIX 标准调用获取，覆盖三大平台
- 下载队列管理在当前 JSON 存储架构上实现，暂不引入消息队列
- 时间段限制基于系统本地时间
