# Feature Specification: 设置系统增强

**Feature Branch**: `011-settings-system-enhanced`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P1 Enhancement Phase)

## User Scenarios & Testing *(mandatory)*

### 用户故事 1：订阅级参数覆盖（优先级：P1）

作为用户，我希望为个别订阅设置不同的下载参数（如画质、路径、格式），覆盖全局设置，以适应不同频道的特殊需求。

**Why this priority**: 不同频道的下载需求差异很大（如课程频道需 1080p，音乐频道仅需音频），订阅级覆盖是核心定制能力。

**Independent Test**: 全局设置画质为 720p，为单个订阅覆盖为 1080p，下载该订阅视频，验证实际使用 1080p。

**Acceptance Scenarios**:

1. **Given** 全局画质设置为 720p，**When** 用户为订阅 A 设置画质覆盖为 1080p，**Then** 订阅 A 的下载使用 1080p，其他订阅使用 720p。

2. **Given** 订阅 A 有画质覆盖 1080p 和路径覆盖 `/custom/path`，**When** 下载订阅 A 的视频，**Then** 视频以 1080p 下载到 `/custom/path`。

3. **Given** 用户清除了订阅 A 的画质覆盖，**When** 下次下载，**Then** 使用全局设置的画质。

4. **Given** 用户在订阅编辑面板中，**When** 点击"重置为全局设置"，**Then** 所有覆盖参数清空，恢复使用全局配置。

**Edge Cases**:
- 多层覆盖冲突时（订阅级 > 标签级 > 全局），使用最低粒度设置
- 覆盖值无效时（如不存在的画质预设），回退到全局设置
- 订阅删除时，其参数覆盖自然清除

### 用户故事 2：视频质量预设（优先级：P1）

作为用户，我希望配置多种视频质量预设（如"最佳画质""720p""480p""仅音频"），以便在不同场景快速切换。

**Why this priority**: 画质选择是每次下载都会用到的设置，便捷的预设系统直接影响操作效率。

**Independent Test**: 创建一个"手机观看"预设（480p, mp4），应用到订阅，验证下载使用预期参数。

**Acceptance Scenarios**:

1. **Given** 用户在设置中选择了"1080p"画质预设，**When** 下载开始，**Then** yt-dlp 使用格式选择器 `bestvideo[height<=1080]+bestaudio/best[height<=1080]`。

2. **Given** 用户创建了自定义预设"高画质音频"（1080p + m4a 音频），**When** 选择该预设，**Then** 下载参数组合为对应视频和音频格式。

3. **Given** 用户选择了"最佳画质"（无限制），**When** 下载开始，**Then** yt-dlp 使用 `bestvideo+bestaudio/best` 选择器。

4. **Given** 用户删除一个自定义预设，**When** 有订阅正在使用该预设，**Then** 提示用户选择新预设或回退到全局默认。

**Edge Cases**:
- 预设名称不得重复
- 预设使用的格式选择器必须在 yt-dlp 中合法
- 视频源不提供指定画质时，yt-dlp 自动使用最接近的画质（回退逻辑）

### 用户故事 3：音频格式选择（优先级：P2）

作为仅下载音频的用户，我希望选择不同的音频格式（m4a、mp3、opus、aac），以适应不同播放设备的兼容性。

**Why this priority**: 音频格式选择提升音频下载体验，但默认格式已能满足多数需求。

**Independent Test**: 设置音频格式为 mp3，启用"仅音频"模式下载，验证输出文件为 .mp3 格式。

**Acceptance Scenarios**:

1. **Given** 用户选择音频格式为 m4a，**When** 以"仅音频"模式下载，**Then** yt-dlp 使用 `bestaudio[ext=m4a]` 格式选择器。

2. **Given** 用户选择音频格式为 mp3，**When** yt-dlp 需要转码（源非 mp3），**Then** yt-dlp 自动调用 ffmpeg 转码为 mp3。

3. **Given** 用户选择"自动"音频格式，**When** 下载，**Then** 系统不指定格式扩展名，yt-dlp 自动选择最佳可用格式。

4. **Given** 用户想在视频下载同时指定音频编码，**When** 设置视频模式 + 首选音频格式 aac，**Then** 使用 `bestvideo+bestaudio[ext=aac]/bestvideo+bestaudio/best` 格式选择器。

**Edge Cases**:
- 源媒体无独立音频流，仅视频流时，回退到最佳通用格式
- 用户选择 opus 格式但设备不支持播放，下载后由用户自行处理
- 格式转换（如转 mp3）需要 ffmpeg 依赖，系统需检测可用性

### 用户故事 4：主题切换（亮色/暗色）（优先级：P2）

作为用户，我希望能切换应用的亮色和暗色主题，以适应不同光线环境和个人偏好。

**Why this priority**: 主题切换是桌面应用的基本 UX 功能，提升视觉舒适度，但不影响功能使用。

**Independent Test**: 从亮色主题切换到暗色主题，验证所有界面元素正确应用暗色配色。

**Acceptance Scenarios**:

1. **Given** 用户在设置中选择"暗色主题"，**When** 应用设置，**Then** 所有界面元素（背景、文字、按钮、边框）切换到暗色配色方案。

2. **Given** 用户选择"跟随系统"，**When** 操作系统切换到暗色模式，**Then** 应用自动跟随切换。

3. **Given** 用户自定义了主题色（强调色），**When** 切换主题，**Then** 自定义颜色在亮/暗主题下都有合适的对比度。

4. **Given** 用户使用自定义主题色，**When** 该颜色在暗色背景下对比度不足（WCAG AA），**Then** 系统发出警告提示。

**Edge Cases**:
- 主题切换应即时生效，无需重启应用
- 自定义颜色需存储为 CSS 兼容格式
- MUI 主题提供者需正确注入暗色/亮色变量

### 用户故事 5：快捷键配置（优先级：P2）

作为高级用户，我希望配置自定义快捷键，以便快速执行常用操作，减少鼠标使用。

**Why this priority**: 快捷键提升效率，对高级用户有价值，但多数用户不会自定义快捷键。

**Independent Test**: 为"检查所有订阅"设置快捷键 Ctrl+Shift+C，按下快捷键，验证触发检查操作。

**Acceptance Scenarios**:

1. **Given** 用户在设置中查看快捷键列表，**When** 打开快捷键配置页面，**Then** 显示所有可配置的操作及当前快捷键。

2. **Given** 用户为"打开设置"重新绑定快捷键为 `Ctrl+,`，**When** 保存并按下 `Ctrl+,`，**Then** 设置对话框打开。

3. **Given** 快捷键 `Ctrl+C` 被"复制"占用，**When** 用户尝试将"检查所有订阅"绑定到 `Ctrl+C`，**Then** 系统提示冲突并拒绝保存。

4. **Given** 用户选择"重置为默认"，**When** 确认操作，**Then** 所有快捷键恢复默认值。

**Edge Cases**:
- 快捷键仅限于应用中注册的操作，不能覆盖系统级快捷键
- 单个操作的快捷键支持组合键：Ctrl/Alt/Shift + 字母/功能键
- 快捷键冲突检测仅检测应用内冲突
- 所有快捷键在应用获得焦点时才有效（防止后台干扰）

## Requirements *(mandatory)*

### Functional Requirements

**订阅级参数覆盖**:
- FR-001: 系统必须支持订阅级覆盖的参数字段：画质、下载路径、输出模板、下载模式
- FR-002: 订阅级覆盖必须在 Subscription 数据模型中序列化
- FR-003: 参数覆盖解析顺序：订阅级 > 全局默认
- FR-004: 系统必须提供"重置为全局"操作一键清除覆盖

**视频质量预设**:
- FR-005: 系统必须提供预定义质量预设：最佳、1080p、720p、480p、360p
- FR-006: 系统必须支持用户创建、编辑、删除自定义质量预设
- FR-007: 每个预设必须包含：名称、视频格式选择器、音频格式选择器
- FR-008: 系统必须验证格式选择器的合法性

**音频格式选择**:
- FR-009: 系统必须提供可选音频格式：m4a、mp3、opus、aac、ogg、自动
- FR-010: 音频格式选择仅对"仅音频"模式生效（视频模式下通过预设配置）
- FR-011: 系统必须在设置页显示音频格式的说明（兼容性、文件大小等）

**主题切换**:
- FR-012: 系统必须支持三种主题模式：亮色、暗色、跟随系统
- FR-013: 主题切换必须即时生效
- FR-014: 系统必须支持用户选择自定义强调色
- FR-015: 主题设置必须持久化

**快捷键配置**:
- FR-016: 系统必须支持默认快捷键：Ctrl+N 添加订阅、Ctrl+Shift+C 检查全部、Ctrl+, 打开设置
- FR-017: 系统必须支持用户自定义快捷键
- FR-018: 快捷键冲突检测必须在保存时执行
- FR-019: 快捷键仅在应用焦点下生效

### Key Entities

- **SubscriptionOverrides**: `{ quality: string | null, download_path: string | null, output_template: string | null, download_mode: string | null }` — 内嵌于 Subscription
- **QualityPreset**: `{ id: uuid, name: string, video_format: string, audio_format: string | null }` — 持久化到 settings.json
- **ThemeConfig**: `{ mode: "light" | "dark" | "system", accent_color: string }` — 持久化到 settings.json
- **Keybinding**: `{ action: string, key: string, modifiers: string[] }` — 持久化到 settings.json

## Success Criteria *(mandatory)*

### Measurable Outcomes

- SC-001: 订阅级覆盖参数优先级解析正确率 100%
- SC-002: 质量预设格式选择器语法正确率 100%（所有预设生成有效的 yt-dlp 参数）
- SC-003: 主题切换响应时间 < 100ms（不引起闪烁）
- SC-004: 快捷键响应延迟 < 50ms
- SC-005: 音频格式选择与 yt-dlp 兼容性 100%（所有可选项生成有效的格式选择器）

## Assumptions

1. MUI 5 主题提供者已支持亮色/暗色切换（通过 `createTheme` + `ThemeProvider`）
2. yt-dlp 格式选择器语法完全参考 yt-dlp 官方文档
3. 快捷键基于 Tauri 的全局快捷键 API（`tauri-plugin-global-shortcut`）或前端 `keydown` 事件
4. 音频格式转换依赖系统安装 ffmpeg，系统只做检测提示，不自动安装
5. 订阅级覆盖字段仅覆盖 PRD 中列出的 4 个参数，不扩展
6. 主题强调色选择器提供 12-16 种预定义颜色 + 自定义色值
