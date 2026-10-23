# Feature Specification: 文件命名与组织

**Feature Branch**: `009-file-naming-organization`
**Created**: 2026-05-31
**Last Revised**: 2026-10-10
**Status**: Draft（已按 wayfinder 地图 [009 文件命名与组织](https://github.com/yysfire/yt-dlp-gui/issues/15) 的已锁决策重写）
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P1 Enhancement Phase)

> 📌 **本 spec 已被重写。** 初稿是 PRD 自动生成的草稿，其**三条核心假设被实测证伪**。重写依据为 wayfinder 地图上 6 张已关闭的决策票，每条 FR 都可追溯到具体票号。**实现前请先读 [wayfinder 地图 #15](https://github.com/yysfire/yt-dlp-gui/issues/15) 的 Decisions so far。**

---

## 范围界定

**做**：
- 用户故事 1：文件名模板系统（可配置模板 + 实时预览 + 字段校验）
- 用户故事 4：文件夹自动组织（**并入故事 1** —— 组织即模板中的路径分隔符，不另设机制）
- 用户故事 5：文件冲突处理

**不做**（详见「超出范围」）：
- 用户故事 2：视频时长后缀处理
- 用户故事 3：特殊字符清理（**已由 yt-dlp 实现**，不自研）

**与 011 的边界**：`specs/011-settings-system-enhanced/spec.md:131,161` 声明的 `SubscriptionOverrides.output_template` 归 011（**且它目前只是提案、尚未实现**）。本 spec 只管**模板字符串本身的语法/校验/预览/渲染**，**不加存储字段**。

---

## 背景：本 feature 要解决的真实问题

**输出模板当前硬编码**于 `src-tauri/src/services/ytdlp.rs:609`（`output_dir.join("%(title)s.%(ext)s")`），全库无任何可配置入口。

**硬编码模板有一个未被察觉的缺陷**：`%(title)s` 不含 `video_id`，而 **YouTube 上标题重复是常态**（重传、分 P、同曲不同版、教程分集）。**同标题的不同视频会渲染成完全相同的路径**。

**实证**（维护者本机 138 条真实下载记录，已脱敏）：

```
某韩区短视频订阅的 22 条记录（标题为 emoji + 韩文 hashtag 串）
  记录数 = 22 | 唯一 video_id = 21 | 唯一 file_path = 1
```

**21 个不同的视频指向同一个文件名。** 全部 11 组同标题记录中，**11 组均为不同 `video_id`，0 组良性**。今天未造成更大损失，仅因这批记录 **103 条全是 `HTTP Error 403` 失败**（bot 校验）—— 下载一旦成功，21 个视频会静默塌成 1 个文件。

且损坏**完全静默**：`unifiedVideoList.ts:196-197` 的 `missing` 派生在碰撞时**恒为 false**（文件确实存在，装的是别人的内容）→ UI 永不报警。

> ⚠️ **「一个文件」不是本项目的不变量。** 008 曾把「同一视频在库中只有一个文件」写成「输出模板没有订阅判别符的物理必然」，该论证**后半段已被证伪**。正确表述见 008 修订：[修订 008「物理必然性」论证链](https://github.com/yysfire/yt-dlp-gui/issues/21)。**路径独占性**（一个落盘路径同一时刻只属于一个 `video_id`）是一条**独立的、至今未被满足的约束** —— **本 feature 的核心任务就是把它变成可断言的不变式。**

---

## 用户故事 & 验收场景 *(mandatory)*

### 用户故事 1：文件名模板系统（优先级：P1）

作为用户，我希望自定义下载文件的命名格式，以便按照自己的组织习惯命名文件。

**Why this priority**: 文件命名直接影响用户的文件管理体验；且当前硬编码模板正在静默损坏用户数据。

**Independent Test**: 设置模板为 `%(channel)s/%(upload_date)s [%(id)s].%(ext)s`，下载一个视频，验证生成的文件路径符合模板。

**Acceptance Scenarios**:

1. **Given** 用户设置模板为 `%(title)s [%(id)s].%(ext)s`，**When** 下载视频 "Hello World"（video_id `dQw4w9WgXcQ`），**Then** 生成文件名 `Hello World [dQw4w9WgXcQ].mp4`。

2. **Given** 用户设置模板为 `%(channel)s/%(upload_date)s [%(id)s].%(ext)s`，**When** 下载频道 "TechChannel" 发布于 2026-05-15 的视频（video_id `abc123XYZ_0`），**Then** 生成路径 `TechChannel/20260515 [abc123XYZ_0].mp4`。

3. **Given** 用户使用默认模板，**When** 下载视频，**Then** 使用默认命名规则，不要求用户必须配置。

4. **Given** 用户在模板中输入了未知占位符 `%(invalid)s`，**When** 保存模板，**Then** 系统提示警告"未知占位符"，但仍允许保存。

5. **Given** 用户在模板预览中输入模板表达式，**When** 模板改变，**Then** 实时显示预览效果（使用示例数据填充占位符）。

6. **Given** 用户提交的模板**不含 `%(id)s`**，**When** 保存，**Then** 系统**拒绝保存**并说明原因（路径独占性要求）。

7. **Given** 用户提交的模板**不含 `%(ext)s`**，**When** 保存，**Then** 系统**拒绝保存**并说明原因（渲染结果可能无扩展名）。

8. **Given** 用户提交的模板**含 `..`**（如 `../%(title)s.%(ext)s`），**When** 保存，**Then** 系统**拒绝保存**并提示路径遍历风险。

**Edge Cases**:
- 占位符对应值为空时使用空字符串替代（如 `channel` 未知）
- 模板包含路径分隔符 `/` 时，yt-dlp 自动创建子目录（**下载前**发生，无 flag 可关）
- 模板导致路径过长时给出警告（**不做自动截断** —— 截断会破坏路径独占性）
- 模板仅由 `%(id)s.%(ext)s` 组成时合法工作

### 用户故事 4：文件夹自动组织（优先级：P2，**并入故事 1**）

作为用户，我希望下载的文件能按订阅频道自动组织到不同文件夹中。

**Why this priority**: 便利性增强。**实现上它就是模板里的路径分隔符** —— FR-005 本身已规定「模板中的 `/` 自动创建子目录」，故不设独立机制。

**Independent Test**: 模板设为 `%(channel)s/%(title)s [%(id)s].%(ext)s`，从 3 个不同频道各下载一个视频，验证文件被放入对应频道名的子文件夹。

**Acceptance Scenarios**:

1. **Given** 用户模板含 `%(channel)s/` 作为首段，**When** 下载视频，**Then** 文件放入以频道名命名的子文件夹。

2. **Given** 用户模板含 `%(extractor_key)s/` 作为首段，**When** 分别下载 YouTube 与 Bilibili 视频，**Then** 文件分别放入 `Youtube/` 与 `BiliBili/` 子文件夹。

3. **Given** 用户模板同时含 `%(extractor_key)s/%(channel)s/`，**When** 下载视频，**Then** 路径形如 `Youtube/TechReviews/video [id].mp4`。

4. **Given** 用户模板不含任何目录段，**When** 下载视频，**Then** 文件直接放入下载根目录。

5. **Given** 频道名包含 Windows 非法字符（如 `A/B`），**When** 生成目录名，**Then** 由 yt-dlp 净化为全角等价物（`A⧸B`），**我们不自研净化规则**。

**Edge Cases**:
- 频道名为空时，模板渲染出空目录段 —— yt-dlp 会跳过空段
- 目录名与既有文件冲突时，yt-dlp 复用既有目录
- ⚠️ **频道改名会导致路径漂移**：同一 `video_id` 的路径从 `旧名/V.mp4` 变为 `新名/V.mp4`。**这是「按频道组织」的固有代价**，且旧文件在重下时会被回收（Windows 上是**永久删除**，见 FR-018 注）

### 用户故事 5：文件冲突处理（优先级：P2）

作为用户，当下载的文件与已存在文件同名时，我希望系统有明确的处理策略。

**Why this priority**: 冲突是实际使用中的常见问题；「跳过」无法在 yt-dlp 层可靠实现（见下）。

**Independent Test**: 下载一个视频，手动创建同名文件，再次下载同一视频，验证系统按设定策略处理冲突。

**Acceptance Scenarios**:

1. **Given** 用户选择冲突策略为「自动重命名」，**When** 下载时目标路径已存在，**Then** 新文件自动追加序号 `video (1).mp4`、`video (2).mp4`（**序号插在扩展名之前**，由 Rust 侧计算）。

2. **Given** 用户选择冲突策略为「覆盖」，**When** 下载时目标路径已存在，**Then** 旧文件被替换为新内容，**且旧文件不残留**。

3. **Given** 用户选择冲突策略为「跳过」，**When** 下载时目标路径已存在，**Then** 系统**不调用 yt-dlp**，记录标记为 `skipped`。

4. **Given** 文件冲突已发生，**When** 用户查看下载记录详情，**Then** 冲突类型与被跳过的路径在记录详情中显示。

5. **Given** 冲突策略为「自动重命名」，**When** 发生冲突，**Then** 记录详情中**同时显示新旧两个文件的大小**（用于判断「画质不同」还是「真重复」）；其余策略下该字段为空。

6. **Given** 用户点击「重新下载」已完成的视频，**When** 目标路径已存在，**Then** 系统按当前冲突策略处理，**不静默谎报成功**。

**Edge Cases**:
- 「跳过」**仅适用于首次下载**；**重新下载永不跳过** —— 因路径由 `video_id` 决定，重下必然撞上自己
- 同一视频重复下载但画质不同时，**不**产生 `video_1080p.mp4` 这类画质变体文件名（**多文件特性属 008 已划出的范围**）
- 冲突判定的**能力边界**：按**路径字符串**比较，挡不住大小写碰撞（Windows 上 `A.mp4` 与 `a.mp4` 落盘同一文件）—— 但该场景已被 `%(id)s` 硬约束消除（不同 `video_id` → 路径必不同）

---

## 功能需求 *(mandatory)*

### 输出模板

- **FR-001**: 系统必须允许用户配置输出模板，模板语法采用 **yt-dlp 原生语法**（`%(field)s`、`%(a|b|c)s` 备选、`%(upload_date>%Y-%m-%d)s` 日期格式）—— **不是** Python 标准 `%` 格式化
- **FR-002**: 系统必须提供**实时模板预览**，渲染结果随输入即时更新
- **FR-003**: 模板中的**未知占位符**必须在保存时给出警告但允许继续（**因为 yt-dlp 会把未知字段静默渲染成 `NA`、rc=0、不报错**）
- **FR-004**: 模板分为**全局默认模板**与**订阅级覆盖模板**（**存储与优先级链路归 011**；本 spec 只定义模板字符串本身的校验与渲染）
- **FR-005**: 模板中的路径分隔符 `/` 由 yt-dlp 自动创建子目录（**下载前**发生）
- **FR-006**: **硬校验（拒绝保存）**：模板必须含 `%(id)s`
- **FR-007**: **硬校验（拒绝保存）**：模板必须含 `%(ext)s`
- **FR-008**: **硬校验（拒绝保存）**：模板不得含 `..`（**yt-dlp 只 sanitize 字段值，不防模板自身穿越** —— 实测 `-o "trav/../../ESCAPED_%(title)s.%(ext)s"` 真写到目录外）
- **FR-009**: 模板的**白名单字段**限定为：`id` `title` `ext` `upload_date` `timestamp` `duration` `channel` `channel_id` `uploader` `uploader_id` `extractor_key`
- **FR-010**: 以下字段**禁止使用**，各带实测理由：
  - `webpage_url` —— sanitize 后冒号/斜杠变全角，实测渲染成 `https□□e.comv`，作为路径片段是垃圾
  - `autonumber` —— **yt-dlp 的冲突计数器、不是视频属性**；用户写进去会让重命名行为不可预测（实测渲染为 `00001`，5 位补零）
  - `playlist_index` —— `--load-info-json` 路径下**即使数据源有值也返回 `NA`**，会导致预览与实际渲染不一致
  - `group_name` —— 我们自己的订阅字段，yt-dlp 不认识，实测渲染成 `NA`
  - **易变值一律禁入**（`view_count` `like_count` `width` `height` `fps` `filesize` `resolution` `vcodec` `acodec` `format_id`）—— 写进路径意味着**同一视频重下时路径会变**，从而触发旧文件回收
- **FR-011**: 系统**不得**引入 `--compat-options filename-sanitization`（该选项下 `id` 字段走 `is_id=True` 分支，**跳过全角替换改为删除**，整条净化路径改变）
- **FR-012**: 系统**不得**实现 Unicode NFKC 归一化（yt-dlp 默认不做；开 `--restrict-filenames` 会做，但同时把中文与重音全毁 —— `日本語タイトル` → `_.mp4`）
- **FR-013**: 系统**不得**自行实现文件名字符净化（**透传 yt-dlp 即可**）。理由：yt-dlp 默认已在净化（win32 上恒走完整 `sanitize_filename`，非法字符 → 全角等价物）；**自研删除式净化会把碰撞从 1 组放大到 3 组**（`sa<me`/`same`/`sa?me` 全塌成 `same.mp4`）；且自研等于 fork 一份上游逻辑，yt-dlp 升级后会静默不同步
- **FR-014**: **Windows 保留设备名**（CON/PRN/AUX/NUL/COM1-9/LPT1-9）**不需要专门处理** —— 带扩展名的名字（`CON.mp4`）是完全合法的真实文件；**危险形状是「渲染结果无扩展名」**，已由 FR-007 覆盖

### 文件冲突

- **FR-015**: 系统必须提供三种冲突策略：**自动重命名** / **覆盖** / **跳过**
- **FR-016**: 自动重命名的序号由 **Rust 侧计算**，插在**扩展名之前**，格式 `(1)`、`(2)`（**不使用 yt-dlp 的 `%(autonumber)s`** —— `--autonumber` 在 2026.06 已废弃，rc=2；且 `autonumber` 已被 FR-010 禁入）
- **FR-017**: 「跳过」的实现方式是**下载前查询文件系统**，命中已存在则**不调用 yt-dlp**，记录标记为 `skipped`
- **FR-018**: 「覆盖」必须解决**旧文件回收**问题 —— yt-dlp 的 `--force-overwrites` 覆盖后 `old == file_path`，`download_queue.rs:823` 的「`old != file_path`」守卫不成立 ⇒ **旧文件永不回收**，且用户无法回退
- **FR-019**: 「跳过」**仅适用于首次下载**；**重新下载永不跳过**（路径由 `video_id` 决定，重下必然撞上自己）
- **FR-020**: **重新下载必须走与首次下载相同的冲突处理路径** —— 现状是坏的：`redownload_video`（`commands/download.rs:421-479`）对已存在文件**零处理** → yt-dlp 默认跳过（rc=0、stderr 空、mtime 未变）→ `exit_status.success()` → `finalize_success` → 标 `Completed`。**实际什么都没下，记录却谎报成功。**
- **FR-021**: 「跳过」**不是终态** —— `skipped` 记录**可重试**（不进 seen 集合 / `dedup_rank` 排在 `failed` 之前）。理由：复用 `completed` 会让「跳过」与「成功」无法区分；永久跳过需要额外 UI 入口且用户很少主动「永久跳过」。**已知后果：`skipped` 记录会反复出现在列表里。**
- **FR-022**: 冲突信息（冲突类型、被跳过的路径）必须在下载记录详情中可查看
- **FR-023**: 冲突为「自动重命名」时，记录详情须**同时显示新旧两个文件的大小**；其余策略下该字段为空

### 存量数据

- **FR-024**: 模板变更**不触发**存量文件迁移 —— **只对新下载生效**。理由：Windows 上「回收站」是 `file_manager.rs:117` 的 `std::fs::remove_file`（**永久删除、无 undo**）；「改个设置 → 批量删用户文件」比损坏更不可接受。
  - **已确认接受的代价**：用户改模板后老文件**不会自动改名**，库中两种命名长期并存
  - **前置条件**：`%(id)s` 硬约束（FR-006）**必须先于**模板改默认值落地，否则「改模板 → 全库永久删除」会先发生

### 编码（已修复，作为前置条件记录）

- **FR-025**: 所有 yt-dlp 调用**必须**在统一收口（`yt_dlp_command` / `yt_dlp_command_async`）加 `--encoding utf-8`
  - **机制**：yt-dlp 的 stdout 编码跟随父进程代码页，而**落盘文件名始终是正确的 UTF-8 字节** —— 损坏形态是「文件下载对了、记录里的路径是乱的」且**无报错**
  - **实证**：标题 `你好世界` 的 stdout 原始字节为 `c4 e3 ba c3 ca c0 bd e7`（GBK），被 UTF-8 解码成 `□□□□□□□□`；而落盘名是 `e4 bd a0 e5 a5 bd…`（正确）
  - **影响面**：全部非 ASCII（每个汉字变成一个替换符），**不只韩文**；纯 ASCII 不受影响
  - **落点**：5 个调用点**全部读 stdout**（4 处 `String::from_utf8_lossy` + 下载处 `BufReader::lines()`），只改下载点不够
  - **状态**：✅ 已修复（`6ad6704`）

---

## 关键实体

- **FilenameTemplate**: `string` —— yt-dlp 输出模板语法（如 `%(channel)s/%(upload_date)s [%(id)s].%(ext)s`）
  - **必须含 `%(id)s` 与 `%(ext)s`**（FR-006/FR-007）
  - **字段须在白名单内**（FR-009），**禁入四项**（FR-010）
  - **不得含 `..`**（FR-008）
- **ConflictStrategy**: enum `"rename" | "overwrite" | "skip"` —— 全局设置，不按订阅配置
- **RecordStatus（新增变体）**: `Skipped`
  - ⚠️ **须在四处登记**：`models/download.rs` 的变体声明（13-24）/ `as_str`（28-40）/ `parse`（43-55）/ `dedup_rank`（58-68），以及 TS 的 `RECORD_STATUSES`（`src/types/__tests__/downloadRecordContract.test.ts:17-26`）
  - **`dedup_rank` 的 `match` 是穷尽的** —— 新增变体会**编译报错**，强制显式登记（不会静默落 fallback 9）
  - **rank 比较是「最小者胜」**（`ra.cmp(&rb)` 升序 + `group.remove(0)`，与「越大越优先」相反）
  - ⚠️ **`DetailPanel.tsx` 的四处是「非穷尽」的**：图标链 `:483-491`、文案链 `:522-564`、色条 `:68-70`、按钮矩阵 `:651-674` 全是 `item.status === "x"` 链 —— **新增 `skipped` 不会编译报错，只会静默不渲染**，须主动补

---

## Success Criteria *(mandatory)*

### 可执行断言

- **SC-001**: **路径独占性**（本 feature 的核心不变式）：对任意两个不同 `video_id`，其渲染出的路径**必不相同**
  - 实现形态：**条件式**断言 —— 「若渲染结果含该 video 的 id，则不同 `video_id` 必得不同路径」（模板不含 id 时本就无法区分，写成「对任意模板」是假的不变式）
  - **反向护栏**：断言「删除式净化不满足该性质」—— 防止日后有人把 FR-013 实现成删除法
- **SC-002**: 模板预览渲染时间 < 100ms（**纯本地字符串替换，不调 yt-dlp**）
- **SC-003**: 冲突处理 **100% 避免意外的数据覆盖**（**存在性断言**：三种策略各有对应的行为测试，非统计性断言）
- **SC-004**: 自动生成的文件夹名称 100% 符合操作系统要求（**由 yt-dlp 净化保证**，我们不自研）
- **SC-005**: 跨 Windows/macOS/Linux 三平台**路径独占性**成立（依赖 `%(id)s` 硬约束）
- **SC-006**: 模板字面量含 `..` 的输入 **100% 被拒绝**（含 `../`、`..\\`、混合分隔符）
- **SC-007**: 未知占位符在保存时**100% 给出警告**（即使允许保存）

### ❌ 已删除的统计性断言

初稿的以下 SC **不可执行**，已删除（不保留删除线）：
- 「时长后缀检测准确率 > 95%（500 个含时长标注的标题样本）」—— 对应的功能已出范围，且真实语料中该形态**0 例**
- 「特殊字符清理在 1ms 内完成」—— 净化已交由 yt-dlp，我们不实现该逻辑，无从测量

---

## 超出范围

> 以下各项**已明确划出本 feature 范围**，原文保留以便日后需要时重开。

- **用户故事 2「视频时长后缀处理」（FR-006~008 的初稿版）** —— **默认出范围（结论已定，但未毕业进范围）**
  - **机制已证实可靠**：应使用 `--replace-in-metadata FIELDS REGEX REPLACE`（**不是** `--parse-metadata` —— 后者只有 `FROM:TO` 两段、无第三段，且无条件覆盖：源字段缺失时 `evaluate_outtmpl` 渲染成 `NA` 而 `.+` 会匹配它，会把 title 写成 `NA`）
  - **但两条理由使其暂不落地**：① 真实语料 138 条中含该形态标题 **0 例**，价值未证实；② 落地不止加参数，还要改输出模板格式，会变更所有既有文件名约定、旧记录 `file_path` 失效
  - ⚠️ **且「移除」本身会制造碰撞**：实测 `Episode 3: The End (11:30)` 与 `Episode 3: The End` 改写后落到**完全相同的绝对路径**。yt-dlp 的 `sanitize_filename` 开头**已无条件**执行 `[0-9]+(?::[0-9]+)+` → `:` 换 `_`，再做「移除」等于在替换之上叠层删除
  - **回归条件**：若将来用户实际反馈需要，再立实现票；届时**三项须齐做**（`--replace-in-metadata` + `%(id)s` + 参数收敛为常量与回归断言），**只做第一项则结论翻转为 DON'T**
  - 详见 [调研：--parse-metadata 能否可靠改写 title](https://github.com/yysfire/yt-dlp-gui/issues/20) 的 resolution 与 `docs/research/parse-metadata.md`
- **用户故事 3「特殊字符清理」** —— **不做**。yt-dlp 默认已做完整净化（win32 上恒走完整 `sanitize_filename`）；自研删除式会**扩大**损坏面 3 倍（FR-013）。初稿的 FR-009「清理 `<>:"/\|?*`」已改写为「**与 yt-dlp 净化行为一致**」
- **Unicode 归一化（`ＡＢＣ` → `ABC`）** —— **明确不做**（FR-012）
- **多画质变体（`video_1080p.mp4` / `video_720p.mp4` 并存）** —— 属 008 已划出的「多文件」特性，**且会破坏路径独占性**
- **「每订阅各留一份」（同一视频在多个订阅下各存一份）** —— 属 008 已划出的「多文件」特性
- **`--compat-options filename-sanitization`** —— **明确禁止引入**（FR-011）
- **模板变更时的存量文件迁移** —— 明确不做（FR-024）

---

## 实现注意事项

1. **模板预览必须纯本地** —— **不调用 yt-dlp**（用示例数据做字符串替换）。理由：FR-002 要求 < 100ms 实时更新，而调 yt-dlp 子进程远达不到
2. **`--encoding utf-8` 已在 `6ad6704` 修复** —— 实现时**不要**重复添加，也不要移除
3. **`test_build_download_args_exact`**（`ytdlp.rs:963-986`）**逐元素断言整个 argv** —— 改参数必须同步改它
4. **`build_download_args` 是纯函数**（`ytdlp.rs:100-115`），参数构造在 command 构造器层 —— 新增 argv 参数应加在构造器层，且**注意 `tokio::process::Command` 没有 `get_args`**（既有 `EnvCommand` trait 正是为此存在）
5. **设置项落点**：`AppSettings`（`models/settings.rs:5-37`，13 个字段，当前无命名字段）；前端 `SettingsDialog.tsx` 是**单页 Grid、无 Tabs** —— 「文件命名」区自然位置在**下载目录之后、yt-dlp 路径之前**（同一「文件落地」语义群）
6. **模板存哪** —— 归 011（`SubscriptionOverrides.output_template`）或全局 `AppSettings`，**本 spec 不加存储字段**

---

## 追溯

| 本 spec 的内容 | 依据票 |
|---|---|
| FR-001~FR-014（模板机制、两条硬校验、白名单、禁入、净化边界） | [#19 输出模板系统的机制边界](https://github.com/yysfire/yt-dlp-gui/issues/19) |
| FR-015~FR-023（三种冲突策略、`skipped`、重下走统一路径） | [#18 文件冲突策略](https://github.com/yysfire/yt-dlp-gui/issues/18) |
| FR-024（存量迁移 = 只对新下载生效） | [#19](https://github.com/yysfire/yt-dlp-gui/issues/19) |
| FR-025（`--encoding utf-8`） | [#17 缺 --encoding utf-8](https://github.com/yysfire/yt-dlp-gui/issues/17) |
| 「路径独占性」的正确表述 | [#21 修订 008「物理必然性」论证链](https://github.com/yysfire/yt-dlp-gui/issues/21) |
| 时长后缀出范围 + `--replace-in-metadata` | [#20 调研：--parse-metadata](https://github.com/yysfire/yt-dlp-gui/issues/20) |
| 碰撞的真实规模（21 个 `video_id` 塌成 1 个路径） | 地图 [#15](https://github.com/yysfire/yt-dlp-gui/issues/15) 的 Notes |

**相关但不在本 feature 范围**（各自独立立项）：
- [#24 按已锁方案修 video_id 为空绕过去重](https://github.com/yysfire/yt-dlp-gui/issues/24) —— 空 id 可见化 + 统一 URL 去重键 + 历史回填
- [#25 统一 id 字段类型](https://github.com/yysfire/yt-dlp-gui/issues/25) —— 分页路径遇无 id 条目会整行解析失败并中断列表
- [#22 P0 落盘路径无归属校验](https://github.com/yysfire/yt-dlp-gui/issues/22) —— 独立于本 feature，与「路径独占性」同源
