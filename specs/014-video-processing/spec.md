# Feature Specification: 视频处理功能 (Video Processing)

**Feature Branch**: `014-video-processing`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-013-01: 缩略图生成

用户查看已下载视频列表时，自动显示视频缩略图，便于快速识别内容。

**Given** 已下载 50 个视频，散放在下载目录中
**When** 用户打开下载记录列表页面
**Then** 系统从视频元数据中提取/生成缩略图并显示
**And** 缩略图尺寸为 320x180（16:9），缓存于应用数据目录
**And** 如果缩略图提取失败，显示占位图标

### US-013-02: 元数据提取

用户查看视频详情时，看到完整的视频元数据信息（标题、时长、分辨率、编码格式等）。

**Given** 已下载视频文件存在本地
**When** 用户打开某条下载记录详情
**Then** 系统提取并展示：标题、作者/频道、时长、分辨率、编码（视频/音频）、帧率、文件大小、下载日期
**And** 元数据通过 `ffprobe` 或 yt-dlp 提取，以结构化的 key-value 展示
**And** 对于无法提取的字段，显示"未知"

### US-013-03: 视频信息编辑

用户批量选中多个下载记录，统一修改视频标题前缀/后缀或分类标记。

**Given** 用户选中 10 个已下载视频
**When** 用户在批量操作菜单中选择"编辑视频信息"
**Then** 系统显示批量编辑界面：标题添加前缀/后缀、设置标签
**And** 用户确认后，系统更新内部下载记录中的 title/modified_title 和 tags 字段
**And** 可选：同时修改视频文件系统中的元数据标记

### US-013-04: 批量重命名

用户根据模式模板（如"频道名 - 标题 - 日期"）批量重命名下载的视频文件。

**Given** 用户选中 5 个下载记录，文件名为 yt-dlp 默认格式
**When** 用户在批量操作中选择"重命名"，模板为 `{channel} - {title} - {upload_date}`
**Then** 系统根据下载记录中的元数据生成新文件名并执行重命名
**And** 重命名后更新下载记录中的 file_path 字段
**And** 如目标文件名已存在，提示冲突并提供覆盖/跳过/自动编号选项

### US-013-05: 格式转换

用户选择一个或多个已下载视频，将其转换为其他格式（如 webm → mp4）。

**Given** 已下载视频为 FLV 格式，用户希望在手机播放
**When** 用户选择该视频，在操作菜单中选择"格式转换" → MP4
**Then** 系统通过 ffmpeg 执行转换（如 `ffmpeg -i input.webm -c copy output.mp4`）
**And** 转换完成后保留原始文件，在下载记录中新增 converted_path 字段
**And** 转换进度以百分比显示在状态栏中

## Requirements *(mandatory)*

### Functional Requirements

#### FR-013-01: 缩略图生成
- 系统 SHALL 使用 ffmpeg 从视频文件中提取关键帧作为缩略图
- 系统 SHALL 支持配置缩略图尺寸（默认 320x180）和质量（默认 JPEG 80%）
- 系统 SHALL 将缩略图缓存至 `~/.yt-dlp-sub-gui/thumbnails/`，以视频 URL hash 命名
- 系统 SHALL 在下载完成时自动生成缩略图
- 系统 SHALL 在生成失败时使用默认占位图

#### FR-013-02: 元数据提取
- 系统 SHALL 使用 `ffprobe -v quiet -print_format json -show_format -show_streams <file>` 提取元数据
- 系统 SHALL 解析并返回：标题、时长、视频编码、音频编码、分辨率、帧率、比特率、文件大小、容器格式
- 系统 SHALL 缓存提取结果（写入下载记录），避免重复调用 ffprobe
- 系统 SHALL 在 ffprobe 不可用时尝试从 yt-dlp 记录中获取元数据

#### FR-013-03: 视频信息编辑
- 系统 SHALL 支持批量编辑：添加标题前缀/后缀、设置 tags（字符串数组）
- 系统 SHALL 支持编辑备注/描述字段
- 系统 SHALL 提供撤销功能（记录编辑前的值）
- 编辑操作 SHALL 仅修改应用内部记录，不修改视频文件元数据（提供可选扩展）

#### FR-013-04: 批量重命名
- 系统 SHALL 支持可配置的重命名模板，模板变量包括：{title}, {channel}, {uploader}, {upload_date}, {ext}, {id} {quality}, {seq}
- 系统 SHALL 预检查文件名合法性，过滤非法字符
- 系统 SHALL 检测目标路径冲突，提供处理策略选择
- 系统 SHALL 重命名后自动更新对应 download_record 的 file_path
- 系统 SHALL 提供预览功能：在确认前显示重命名前后对比

#### FR-013-05: 格式转换
- 系统 SHALL 支持发送格式转换任务，由 ffmpeg 执行（作为外部依赖检测）
- 系统 SHALL 支持预设输出格式：MP4（H.264 + AAC）、MKV、WebM、MP3（仅音频）
- 系统 SHALL 在下载记录中新增 converted_path、converted_format、conversion_status 字段
- 系统 SHALL 显示转换进度和剩余时间估算
- 系统 SHALL 支持取消正在进行的转换任务

### Key Entities

- **VideoMetadata**: 视频元数据，包含 title, duration, video_codec, audio_codec, width, height, frame_rate, bitrate, file_size, container, thumbnail_path
- **RenameTemplate**: 重命名模板配置，包含 pattern 字符串、合法字符集
- **ConversionTask**: 格式转换任务，包含 source_path, target_format, target_path, status（pending/running/completed/failed）, progress
- **ThumbnailCache**: 缩略图缓存元数据，包含 source_video_hash, thumbnail_path, size, generated_at

## Success Criteria *(mandatory)*

- SC-013-01: 缩略图生成成功率 ≥ 95%，单张生成耗时 < 2 秒（1080p 视频）
- SC-013-02: 元数据提取耗时 < 3 秒/视频，支持至少 15 个元数据字段
- SC-013-03: 批量编辑操作响应时间 < 500ms（100 条记录）
- SC-013-04: 批量重命名预览时间 < 2 秒（100 条记录）
- SC-013-05: 格式转换速度不低于 0.8x 实时（即 1 分钟视频在 80 秒内完成转换）

## Assumptions

- 用户系统已安装 ffmpeg/ffprobe，应用中提供下载引导和路径配置
- 元数据提取依赖视频文件未损坏，损坏文件优雅降级处理
- 格式转换中默认使用 `-c copy` 无损重新封装，仅在必要时转码
- 重命名模板变量的值从下载记录中获取，不重新解析视频文件
- 缩略图缓存基于视频文件路径的 hash 作为唯一标识
