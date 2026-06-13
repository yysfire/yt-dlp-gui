# Feature Specification: 下载管理增强

**Feature Branch**: `008-download-management-enhanced`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P1 Enhancement Phase)

## User Scenarios & Testing *(mandatory)*

### 用户故事 1：智能重复检测增强（优先级：P1）

作为用户，我希望系统在下载前能更智能地判断视频是否已下载过，不仅基于视频 ID，还要考虑画质和格式变化，避免重复下载但不错过画质升级。

**Why this priority**: 重复下载不仅浪费带宽，还浪费存储空间，增强检测直接影响核心下载效率。

**Independent Test**: 先下载视频的 720p 版本，然后尝试下载同一视频的 1080p 版本，验证系统检测到画质不同并允许下载。

**Acceptance Scenarios**:

1. **Given** 用户已下载视频 A（720p, mp4），**When** 系统检查到视频 A 有新版本但画质与已下载相同，**Then** 系统自动跳过，不重复下载。

2. **Given** 用户已下载视频 B（720p, mp4），**When** 用户升级画质设置为 1080p 并重新检查同一频道，**Then** 系统检测到画质差异，将视频 B 标记为"可升级下载"，用户可选择下载更高画质版本。

3. **Given** 用户已下载视频 C（1080p, mp4），**When** 用户更改格式偏好为 mkv，**Then** 系统检测到格式差异，允许重新下载。

4. **Given** 两个不同频道发布了标题完全相同的视频，**When** 系统检查重复，**Then** 基于视频 ID 判断为不同视频，不误判重复。

5. **Given** 用户手动删除了本地文件但记录仍存在，**When** 系统执行文件存在性检查，**Then** 将对应记录标记为"文件缺失"，允许重新下载。

**Edge Cases**:
- 视频 ID 相同的播放列表项应视为同一视频
- 用户更改下载路径后，应重新检查新路径下是否有文件
- 文件名模板变化导致同名文件但视频 ID 不同时，不应判定为重复
- 文件系统大小写不敏感（macOS/Windows）不应导致误判

### 用户故事 2：失败自动重试（优先级：P1）

作为用户，我希望下载失败时系统能自动重试，而不是立即放弃，以便在网络波动或临时错误时自动恢复下载。

**Why this priority**: 网络临时波动是最常见的下载失败原因，自动重试可以显著减少用户手动干预，提升下载成功率。

**Independent Test**: 模拟一个间歇性网络故障（先失败再恢复），验证系统自动重试并最终成功下载。

**Acceptance Scenarios**:

1. **Given** 一个下载任务因网络超时失败，**When** 失败发生，**Then** 系统自动在 30 秒后重试，最多重试 3 次，每次重试间隔递增（30s、60s、120s）。

2. **Given** 一个下载任务在第 2 次重试时成功，**When** 下载完成，**Then** 记录状态更新为"completed"，重试次数记录为 2。

3. **Given** 一个下载任务重试 3 次全部失败，**When** 达到最大重试次数，**Then** 记录标记为"failed"，显示最终错误信息，用户可手动重新开始。

4. **Given** 多个下载任务同时失败，**When** 系统对它们分别重试，**Then** 各任务独立重试计数，互不影响。

5. **Given** 用户启用了重试自动暂停模式（如夜间模式），**When** 下载失败，**Then** 系统将任务加入"待重试"队列，在下一个检查周期重试。

**Edge Cases**:
- HTTP 4xx 错误（如 404、403、410）不应重试，因为重试也无法恢复
- HTTP 5xx 和网络错误应重试
- 下载被用户手动取消后不自动重试
- 重试期间用户手动暂停该订阅，应中断重试流程

### 用户故事 3：下载速度限制（优先级：P2）

作为用户，我希望限制下载速度，以便在使用网络的同时不影响其他网络活动（如在线会议、流媒体）。

**Why this priority**: 下载抢占带宽是常见痛点，但用户可以在下载时手动暂停，因此优先级略低。

**Independent Test**: 设置下载限速为 1MB/s，启动下载并监控实际网络速度，验证下载速度不超过设定值。

**Acceptance Scenarios**:

1. **Given** 用户在设置中将下载限速设为 2MB/s，**When** 下载开始，**Then** yt-dlp 以 `--limit-rate 2M` 参数调用，下载速度不超过 2MB/s。

2. **Given** 用户设置下载限速为"无限制"，**When** 下载开始，**Then** yt-dlp 不传递 `--limit-rate` 参数，以最大可用带宽下载。

3. **Given** 下载正在进行中，**When** 用户动态调整限速值，**Then** 新限速对后续下载任务生效，当前进行中的任务不变。

4. **Given** 用户设置限速后没有网络活动，**When** 下载速度低于限速值，**Then** 以实际可用带宽下载。

**Edge Cases**:
- 限速值设为 0 或负数时，视为"无限制"
- 限速值超过物理带宽时以实际带宽为准
- 限速单位转换清晰（KB/s vs Kbps），前端统一使用 KB/s

### 用户故事 4：网络代理支持（优先级：P2）

作为需要通过代理访问网络的用户，我希望为下载配置 HTTP/HTTPS/SOCKS 代理，以便在受限网络环境中正常下载。

**Why this priority**: 中国大陆等地区访问 YouTube 需要代理，但代理配置是一次性操作，优先级略低于核心功能。

**Independent Test**: 配置一个 SOCKS5 代理，下载一个视频，验证 yt-dlp 使用代理参数并成功下载。

**Acceptance Scenarios**:

1. **Given** 用户在设置中配置了 SOCKS5 代理 `127.0.0.1:1080`，**When** 下载开始，**Then** yt-dlp 以 `--proxy socks5://127.0.0.1:1080/` 参数调用。

2. **Given** 用户配置了 HTTP 代理 `https://proxy.example.com:8080`，**When** 下载开始，**Then** yt-dlp 以 `--proxy https://proxy.example.com:8080/` 参数调用。

3. **Given** 用户未配置代理，**When** 下载开始，**Then** yt-dlp 不传递 `--proxy` 参数，使用直连。

4. **Given** 代理需要认证，**When** 用户配置了用户名和密码，**Then** yt-dlp 正确使用带认证的代理 URL（如 `socks5://user:pass@127.0.0.1:1080/`）。

5. **Given** 代理连接失败，**When** 下载失败，**Then** 错误信息明确提示"代理连接失败"，而非通用网络错误。

**Edge Cases**:
- 代理 URL 格式无效时，保存前进行格式校验
- 代理密码包含特殊字符时进行 URL 编码
- 仅填写主机名未填端口时使用默认端口（HTTP 80、SOCKS5 1080）
- 代理配置变更后立即对所有后续下载生效

### 用户故事 5：音频下载支持（优先级：P2）

作为用户，我希望在视频下载之外支持仅下载音频，以便节省存储空间和带宽。

**Why this priority**: 音频下载是许多用户的需求，但核心功能是视频下载，音频模式作为扩展功能。

**Independent Test**: 设置下载模式为"仅音频"，下载一个视频，验证仅下载音频，文件大小为纯音频大小。

**Acceptance Scenarios**:

1. **Given** 用户在设置中选择"仅音频"模式并指定格式为 m4a，**When** 下载开始，**Then** yt-dlp 以 `-f bestaudio[ext=m4a]` 参数调用。

2. **Given** 用户选择"仅音频"模式，**When** 视频源不提供独立音频流，**Then** yt-dlp 使用 `bestaudio/best` 回退格式，确保能获取音频。

3. **Given** 用户在"视频+音频"模式下下载，**When** 下载完成，**Then** 获取包含视频和音频的完整文件。

4. **Given** 用户切换为音频模式，**When** 下载列表中的文件扩展名从 .mp4 变为 .m4a/.mp3，**Then** 重复检测系统正确区分不同格式。

**Edge Cases**:
- 音频质量预设映射到 yt-dlp 音频格式选择器（如 "高"→`bestaudio`、"中"→`bestaudio[abr<=128]`）
- 仅音频模式下视频信息面板显示"仅音频"标识
- 无可用音频流时回退到最佳通用格式

## Requirements *(mandatory)*

### Functional Requirements

**智能重复检测**:
- FR-001: 系统必须基于视频 ID 作为重复检测的主键
- FR-002: 系统必须对比已下载文件的画质和格式，识别画质升级场景
- FR-003: 系统必须在下载前执行文件存在性检查（文件可能被手动删除）
- FR-004: 重复检测结果必须在 UI 中呈现为"已下载""可升级""可重下载"三种状态
- FR-005: 文件名模板变化不能影响重复检测的准确性

**失败自动重试**:
- FR-006: 系统必须对网络错误和 HTTP 5xx 错误自动重试
- FR-007: HTTP 4xx 错误（404/403/410）不得自动重试
- FR-008: 重试间隔必须递增（30s → 60s → 120s），最多 3 次
- FR-009: 系统必须在下载记录中保留重试次数和时间
- FR-010: 用户手动取消的下载不得自动重试

**下载速度限制**:
- FR-011: 系统必须通过 `--limit-rate` 参数将限速传递给 yt-dlp
- FR-012: 限速值以 KB/s 为单位，前端输入范围为 0-99999
- FR-013: 值为 0 或空表示无限制
- FR-014: 动态调整限速仅对新任务生效

**网络代理支持**:
- FR-015: 系统必须支持 HTTP、HTTPS、SOCKS5 代理协议
- FR-016: 系统必须支持带认证的代理
- FR-017: 代理 URL 必须在保存时进行格式校验
- FR-018: 未配置代理时 yt-dlp 不得传递 `--proxy` 参数

**音频下载支持**:
- FR-019: 系统必须提供"视频+音频"和"仅音频"两种下载模式
- FR-020: 仅音频模式必须支持指定首选音频格式（m4a、mp3、opus、aac）
- FR-021: 无独立音频流时必须回退到通用格式
- FR-022: 下载模式变更后重复检测必须正确识别格式差异

### Key Entities

- **RetryConfig**: `{ max_retries: number, retry_delays_secs: number[], retry_on_http_errors: number[] }` — 从 settings.json 读取
- **ProxyConfig**: `{ enabled: boolean, url: string, auth: { username: string, password: string } | null }` — 持久化到 settings.json
- **DownloadMode**: `"video" | "audio_only"` — 订阅级设置，需持久化

## Success Criteria *(mandatory)*

### Measurable Outcomes

- SC-001: 重复检测准确率 > 99%（1 万次检测中误判 < 100 次）
- SC-002: 自动重试使下载成功率提升 20% 以上（在间歇性网络故障环境中）
- SC-003: 速度限制偏差 < 10%（实际下载速度与设定值偏差）
- SC-004: 代理连接成功率达到直连的 95% 以上（代理服务器正常前提下）
- SC-005: 音频下载文件大小比同视频小 80% 以上

## Assumptions

1. yt-dlp 原生支持 `--limit-rate`、`--proxy`、`-f bestaudio[ext=xxx]` 参数
2. 视频 ID 由 yt-dlp 提供，格式为平台特定字符串（YouTube 为 11 位 Base64）
3. 下载记录持久化在 `download_records.json`，包含画质和格式字段
4. 代理配置为全局设置，不支持订阅级代理（超出 P1 范围）
5. 音频下载文件扩展名由 yt-dlp 根据 `-o` 模板和格式选择器自动确定
6. 失败重试的最大次数和间隔为默认值，用户不可配置（简化实现）
