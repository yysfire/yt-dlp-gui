# Feature Specification: 文件命名与组织

**Feature Branch**: `008-file-naming-organization`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P1 Enhancement Phase)

## User Scenarios & Testing *(mandatory)*

### 用户故事 1：文件名模板系统（优先级：P1）

作为用户，我希望自定义下载文件的命名格式，以便按照自己的组织习惯命名文件（如包含频道名、日期、标题等）。

**Why this priority**: 文件命名直接影响用户的文件管理体验，是下载管理的核心定制能力。

**Independent Test**: 设置模板为 `%(channel)s/%(upload_date)s - %(title)s.%(ext)s`，下载一个视频，验证生成的文件路径和名称符合模板。

**Acceptance Scenarios**:

1. **Given** 用户设置文件名模板为 `%(title)s.%(ext)s`，**When** 下载视频"Hello World"（mp4），**Then** 生成文件名为 `Hello World.mp4`。

2. **Given** 用户设置模板为 `%(channel)s/%(upload_date)s_%(title)s.%(ext)s`，**When** 下载频道"TechChannel"发布于 2026-05-15 的视频，**Then** 生成路径 `TechChannel/20260515_Hello_World.mp4`。

3. **Given** 用户使用默认模板 `%(title)s.%(ext)s`，**When** 下载视频，**Then** 使用默认命名规则，不要求用户必须配置。

4. **Given** 用户在模板中输入了无效的格式占位符 `%(invalid)s`，**When** 保存模板，**Then** 系统提示警告"未知占位符"，但仍允许保存。

5. **Given** 用户在模板预览中输入模板表达式，**When** 模板改变，**Then** 实时显示预览效果（使用示例数据填充占位符）。

**Edge Cases**:
- 占位符对应值为空时（如频道名未知），使用空字符串替代
- 模板包含路径分隔符 `/` 时，自动创建子目录
- 模板导致文件路径超过操作系统最大路径长度（Windows 260 字符）时给出警告
- 模板仅由 `%(ext)s` 或类似极简模板组成时，系统仍应合法工作

### 用户故事 2：视频时长后缀处理（优先级：P2）

作为用户，当视频标题包含时长信息（如"10:30"）时，我希望系统能正确清理或保留这些信息，避免文件名混淆。

**Why this priority**: 视频标题中嵌入时长是常见情况，处理不当会导致文件名格式不佳，但不影响核心下载功能。

**Independent Test**: 下载一个标题含"【10:30】"的视频，验证文件名正确处理了时长标记。

**Acceptance Scenarios**:

1. **Given** 视频标题为"Tutorial Part 1 (15:00)"，**When** 启用"移除时长后缀"选项，**Then** 生成文件名中不包含"(15:00)"部分，如 `Tutorial_Part_1.mp4`。

2. **Given** 视频标题为"10:30 AM Meeting - Discussion"，**When** 启用"移除时长后缀"，**Then** 不误删除时间格式文本"10:30 AM"（仅在括号中的时长模式被移除）。

3. **Given** 视频标题为"Video【时长 08:45】Ending"，**When** 启用"移除时长后缀"，**Then** 移除`【时长 08:45】`标记，保留前后文本。

4. **Given** 用户禁用"移除时长后缀"，**When** 下载视频，**Then** 文件名原样保留时长标记。

**Edge Cases**:
- 时长标注使用全角字符（如"１２：３０"）
- 标题中有多个时长标记，仅移除最后一个
- 时长标记位于标题开头处，移除后标题可能为空

### 用户故事 3：特殊字符清理（优先级：P2）

作为用户，我希望文件名中的特殊字符自动清理或替换，避免文件系统兼容性问题。

**Why this priority**: 特殊字符清理防止文件创建失败，是稳定性功能，但多数视频标题不含特殊字符。

**Independent Test**: 下载标题包含 `<>:"/\|?*` 的视频（模拟），验证文件名中特殊字符被正确替换或移除。

**Acceptance Scenarios**:

1. **Given** 视频标题包含 `<>:"/\|?*` 字符，**When** 系统生成文件名，**Then** 这些字符被替换为下划线 `_` 或全角等效字符。

2. **Given** 视频标题包含前后空格和尾部句点 `" Hello World ."`，**When** 清理后，**Then** 文件名变为 `Hello_World`（去除首尾空格和尾部句点）。

3. **Given** 用户设置中启用了"Unicode 规范化"选项，**When** 标题包含全角英文字母，**Then** 转换为半角（如 `ＡＢＣ` → `ABC`）。

4. **Given** 视频标题完全由特殊字符组成（如 `???`），**When** 清理后结果为空，**Then** 使用视频 ID 作为回退文件名。

**Edge Costs**:
- Windows 保留名称（CON、PRN、AUX、NUL、COM1-COM9、LPT1-LPT9）不应作为文件名
- 文件名不能仅由空格或句点组成
- 中文字符和 Emoji 应保留不变
- 清理后的文件名长度超过 255 字符时截断

### 用户故事 4：文件夹自动组织（优先级：P2）

作为用户，我希望下载的文件能按订阅频道自动组织到不同文件夹中，便于浏览和管理。

**Why this priority**: 文件夹组织提升管理效率，但用户可手动实现，属于便利性增强。

**Independent Test**: 启用"按频道组织"选项，从 3 个不同频道各下载一个视频，验证文件被放入对应频道名的子文件夹。

**Acceptance Scenarios**:

1. **Given** 用户启用"按平台组织"选项，**When** 下载 YouTube 和 Bilibili 视频，**Then** 文件分别放入 `YouTube/` 和 `Bilibili/` 子文件夹。

2. **Given** 用户启用"按频道组织"选项，**When** 从"TechReviews"频道下载视频，**Then** 文件放入 `TechReviews/` 子文件夹。

3. **Given** 用户同时启用"按平台组织"和"按频道组织"，**When** 下载视频，**Then** 文件路径为 `YouTube/TechReviews/video.mp4`。

4. **Given** 用户禁用所有自动组织，**When** 下载视频，**Then** 文件直接放入下载根目录。

**Edge Cases**:
- 文件夹名称包含特殊字符时，与文件名使用相同的清理规则
- 频道名称为空时使用频道 URL 的后缀（如频道 ID）
- 文件夹名称冲突时追加后缀如 `_2`
- 模板和自动组织同时使用时，优先模板（模板比组织规则更精细）

### 用户故事 5：文件冲突处理（优先级：P2）

作为用户，当下载的文件与已存在文件同名时，我希望系统有明确的处理策略，避免文件被意外覆盖或下载失败。

**Why this priority**: 文件冲突是实际使用中经常遇到的问题，影响数据安全性。

**Independent Test**: 下载一个视频，手动创建同名文件，再次下载同一视频，验证系统按设定策略处理冲突。

**Acceptance Scenarios**:

1. **Given** 用户选择冲突策略为"自动重命名"，**When** 下载时目标路径存在同名文件，**Then** 新文件自动追加序号如 `video (1).mp4`、`video (2).mp4`。

2. **Given** 用户选择冲突策略为"覆盖"，**When** 下载时目标路径存在同名文件，**Then** yt-dlp 使用 `--force-overwrites` 参数覆盖旧文件。

3. **Given** 用户选择冲突策略为"跳过"，**When** 下载时目标路径存在同名文件，**Then** 下载被跳过，记录标记为"skipped (duplicate file)"。

4. **Given** 文件冲突已发生，**When** 用户查看下载记录，**Then** 冲突信息在记录详情中显示，附带两个文件的大小对比。

**Edge Cases**:
- 文件名冲突但文件内容不同（不同视频恰好同名），不应判定为重复下载
- "自动重命名"策略导致文件路径超过系统最大长度时，使用视频 ID 作为短文件名
- 同一视频重复下载但画质不同时，自动重命名为 `video_1080p.mp4`、`video_720p.mp4`

## Requirements *(mandatory)*

### Functional Requirements

**文件名模板系统**:
- FR-001: 系统必须支持 yt-dlp 标准输出模板语法 `%(key)s` 格式
- FR-002: 系统必须提供模板预览功能，使用示例数据展示效果
- FR-003: 模板中未知占位符必须在保存时给出警告但允许继续
- FR-004: 模板分为全局默认模板和订阅级覆盖模板
- FR-005: 模板中路径分隔符 `/` 应自动创建对应子目录

**视频时长后缀处理**:
- FR-006: 系统必须检测并移除常见的时长标注格式：`(HH:MM:SS)`、`【时长 HH:MM:SS】`、`- Duration: HH:MM:SS`
- FR-007: 时长移除必须是可配置的开关选项
- FR-008: 时长检测不能误删正常包含时间的标题内容

**特殊字符清理**:
- FR-009: 系统必须清理 Windows/macOS/Linux 三平台不兼容的文件名字符：`<>:"/\|?*`
- FR-010: 系统必须清理文件名首尾空格和尾部句点
- FR-011: 清理后结果为空时必须使用视频 ID 作为回退文件名
- FR-012: Windows 保留设备名称（CON、PRN、AUX、NUL、COM1-COM9、LPT1-LPT9）必须避免使用

**文件夹自动组织**:
- FR-013: 系统必须提供"按平台组织"和"按频道组织"两种自动组织选项
- FR-014: 自动组织选项必须支持同时启用（叠加效果）
- FR-015: 文件夹名称中的特殊字符必须与文件名使用相同的清理规则

**文件冲突处理**:
- FR-016: 系统必须提供三种冲突策略：自动重命名、覆盖、跳过
- FR-017: 自动重命名必须使用递增序号：`(1)`、`(2)` 格式
- FR-018: 覆盖策略必须通过 `--force-overwrites` 传递 yt-dlp
- FR-019: 冲突信息必须在下载记录中可查看

### Key Entities

- **FilenameTemplate**: `string`（yt-dlp 输出模板语法，如 `%(channel)s/%(upload_date)s_%(title)s.%(ext)s`）
- **ConflictStrategy**: enum `"rename" | "overwrite" | "skip"`
- **OrganizeMode**: enum `"none" | "by_platform" | "by_channel" | "by_both"`

## Success Criteria *(mandatory)*

### Measurable Outcomes

- SC-001: 模板预览渲染时间 < 100ms
- SC-002: 时长后缀检测准确率 > 95%（500 个含时长标注的标题样本）
- SC-003: 特殊字符清理在 1ms 内完成（单个文件名）
- SC-004: 文件冲突处理 100% 避免意外的数据覆盖
- SC-005: 自动生成的文件夹名称 100% 符合操作系统文件系统要求
- SC-006: 文件名处理结果跨 Windows/macOS/Linux 三平台兼容

## Assumptions

1. yt-dlp 输出模板语法遵循 Python 标准 `%` 格式化（yt-dlp 基于 youtube-dl）
2. 文件名清理在 Rust 后端完成（在调用 yt-dlp 前构建输出模板）
3. 时长后缀格式主要来自 YouTube 自动添加的 `(11:30)` 和 Bilibili 的 `【时长 10:30】`
4. 文件冲突处理策略为全局设置，不按订阅配置
5. 模板占位符列表与 yt-dlp 文档保持一致，使用已知的可用占位符白名单
6. 文件路径最大长度限制：Windows 260 字符、Linux 4096 字符、macOS 1024 字符
