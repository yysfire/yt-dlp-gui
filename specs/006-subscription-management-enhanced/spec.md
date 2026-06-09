# Feature Specification: 订阅管理增强

**Feature Branch**: `006-subscription-management-enhanced`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P1 Enhancement Phase)

## Clarifications

### Session 2026-06-10

- Q: 健康状态是否持久化到 Subscription 实体？ → A: 是，在 Subscription 上持久化 `health_status` 和 `last_health_check` 字段，HealthCheckResult 仅作为运行时中间结果。
- Q: 详情面板视频列表显示条数？ → A: 分页加载，初始显示 10 条，用户可加载更多

## User Scenarios & Testing *(mandatory)*

### 用户故事 1：批量导入订阅（优先级：P1）

作为用户，我希望一次性导入多个订阅源（OPML/TXT/URL列表），以便快速迁移或恢复我的订阅集合，无需逐个手动添加。

**Why this priority**: 批量导入是用户迁移和恢复订阅的核心入口，没有此功能用户需要逐个添加，严重影响使用意愿。

**Independent Test**: 准备一个包含 10 个频道的 OPML 文件，通过导入对话框导入，验证 10 个订阅全部正确创建且频道信息自动解析。

**Acceptance Scenarios**:

1. **Given** 用户有一个包含 5 个 YouTube 频道的 OPML 文件，**When** 用户打开导入对话框并选择该文件，**Then** 系统解析 OPML 文件，列出 5 个频道预览，用户确认后批量创建订阅，每个订阅自动调用 yt-dlp 解析频道信息，完成后显示成功/失败统计。

2. **Given** 用户有一个包含混合 YouTube/Bilibili URL 的 TXT 文件（每行一个 URL），**When** 用户导入该文件，**Then** 系统逐行解析 URL，跳过空行和注释行（`#` 开头），为每个有效 URL 创建订阅。

3. **Given** 用户粘贴了包含 3 个 URL 的文本到导入框，**When** 用户点击导入，**Then** 系统直接解析文本内容，创建对应订阅。

4. **Given** 导入过程中部分 URL 无效或频道已删除，**When** 导入完成，**Then** 系统显示详细结果：成功 N 个、失败 M 个，失败项列出原因（频道不存在、网络错误等）。

**Edge Cases**:
- 导入的频道与已有订阅重复时，跳过并提示用户
- OPML 文件使用非 UTF-8 编码时，自动检测并转换编码
- TXT 文件中包含超长 URL（>2048 字符）时，拒绝并报错
- 导入 100+ 条目时，显示进度条而非阻塞 UI
- 网络断开时，批量解析中途失败不影响已创建的订阅

### 用户故事 2：订阅健康检查（优先级：P1）

作为用户，我希望定期或手动检查我的订阅频道是否仍然可用，以便及时发现并处理失效订阅，保持订阅列表整洁。

**Why this priority**: 订阅失效是常见问题，用户需要主动感知并清理，否则积累大量失效订阅会影响使用体验。

**Independent Test**: 在订阅列表中选择 3 个活跃频道和 2 个已删除频道，运行健康检查，验证系统正确识别活跃与失效频道并显示结果。

**Acceptance Scenarios**:

1. **Given** 用户有 20 个订阅，其中 3 个频道已被删除，**When** 用户点击"检查所有订阅健康状态"，**Then** 系统逐个访问频道 URL，标记每个订阅为"正常""警告（限流）"或"失效"，完成后显示摘要报告。

2. **Given** 健康检查发现失效订阅，**When** 用户在结果面板中查看，**Then** 每个失效项显示具体原因（404 未找到、403 禁止访问、网络超时等）和最后可用日期。

3. **Given** 用户选择 5 个订阅，**When** 用户右键选择"检查选中订阅"，**Then** 仅对这 5 个订阅执行健康检查，不干扰其他订阅。

4. **Given** 健康检查正在进行中，**When** 用户切换到其他视图，**Then** 检查在后台继续，完成后通过通知告知用户。

**Edge Cases**:
- 网络中断时，健康检查失败应以网络错误标记，非频道失效
- 被限流（HTTP 429）时，应暂停检查并建议用户稍后重试
- 频道重定向到新 URL 时，提示用户更新订阅地址
- 同时检查 50+ 订阅时，应分批执行避免触发平台反爬机制
- 健康检查执行期间用户删除订阅：该订阅从待检查队列中静默移除，不产生错误
- 健康检查执行期间用户暂停订阅：仍按原 URL 检查，不影响检查流程（暂停仅控制下载行为）
- 健康检查执行期间用户修改订阅 URL：当轮按启动时的原 URL 完成，下次检查使用新 URL
- 健康检查标记为"失效"后，该订阅的 `enabled` 自动置为 false（暂停调度），用户可在订阅列表中手动重新启用

### 用户故事 3：高级筛选和排序（优先级：P1）

作为拥有大量订阅的用户，我希望通过多种条件筛选和排序订阅，以便快速找到目标频道。

**Why this priority**: 当订阅数量增长到 30+ 时，搜索和筛选是刚需，直接影响日常使用效率。

**Independent Test**: 创建 20 个不同类型的订阅（不同平台、分组、启停状态），使用筛选和排序功能，验证结果准确。

**Acceptance Scenarios**:

1. **Given** 用户有混合 YouTube/Bilibili 的订阅列表，**When** 用户选择平台筛选为"YouTube"，**Then** 列表仅显示 YouTube 频道，Bilibili 频道被隐藏。

2. **Given** 用户有已启用和已暂停的订阅，**When** 用户选择状态筛选为"已启用"，**Then** 仅显示活跃订阅。

3. **Given** 用户给订阅添加了分组，**When** 用户选择分组筛选，**Then** 仅显示该分组下的订阅。

4. **Given** 用户在搜索框中输入频道名称关键字，**When** 输入完成，**Then** 列表实时过滤，高亮匹配文本，按名称字母/拼音排序。

5. **Given** 用户运行过健康检查，列表中存在正常、警告、失效率订阅，**When** 用户选择健康状态筛选为"失效"，**Then** 仅显示 health_status 为 dead 的订阅。

6. **Given** 用户设置排序条件为"最近更新"降序，**When** 应用排序，**Then** 订阅按最近视频发布日期从新到旧排列。

**Edge Cases**:
- 搜索关键字为空时显示全部订阅
- 搜索包含正则特殊字符时进行字面匹配，不触发正则
- 同时应用多个筛选条件时，使用 AND 逻辑
- 筛选条件变化后保留用户选中的订阅状态

### 用户故事 4：订阅详情面板（优先级：P2）

作为用户，我希望点击某个订阅后能查看详细的频道信息，包括频道描述、统计数据。

**Why this priority**: 详情面板帮助用户更好地了解频道，判断是否需要保留订阅，但非核心操作流程。

**Independent Test**: 点击一个 YouTube 订阅，验证详情面板展示频道名称、描述、订阅者数等信息。

**Acceptance Scenarios**:

1. **Given** 用户选中一个订阅，**When** 详情面板加载完成，**Then** 显示频道名称、描述、平台类型、添加日期、最近检查时间。

2. **Given** 订阅的频道有缩略图/头像，**When** 详情面板显示，**Then** 频道缩略图正确加载显示。

3. **Given** 用户选中了一个已失效的订阅，**When** 详情面板显示，**Then** 明确标注"此频道已失效"及失效原因，视频列表显示"无法获取"。

4. **Given** 频道有超过 10 个视频，**When** 用户滚动到视频列表底部，**Then** 自动或手动加载更多视频（分页加载）。

**Edge Cases**:
- 频道信息解析失败时，显示最后已知信息并标注"信息可能过期"
- 详情面板加载过程中用户切换订阅，取消前一个加载请求
- 窗口宽度不足时详情面板自适应收缩

## Requirements *(mandatory)*

### Functional Requirements

**批量导入导出**:
- FR-001: 系统必须支持 OPML 2.0 格式的导入和导出
- FR-002: 系统必须支持 TXT 格式（每行一个 URL）的导入
- FR-003: 系统必须支持从剪贴板粘贴 URL 列表导入
- FR-004: 导入前必须预览待添加项目并允许用户取消
- FR-005: 导出必须包含订阅所有字段
- FR-006: 批量导入必须支持后台异步执行并显示实时进度

**订阅健康检查**:
- FR-007: 系统必须提供全量健康检查和选择性健康检查两种模式
- FR-008: 系统必须对每个订阅执行 HTTP 可达性验证
- FR-009: 健康检查结果必须分类为：正常、警告、失效，并持久化到对应 Subscription 的 `health_status` 和 `last_health_check` 字段
- FR-010: 系统必须在 UI 中展示健康检查摘要和详细报告
- FR-011: 系统必须在健康检查完成后自动暂停所有失效订阅的调度（自动将 `enabled` 置为 false），用户可从健康检查结果中批量删除失效订阅或手动重新启用

**高级筛选和排序**:
- FR-012: 系统必须提供平台、状态、分组、健康状态、关键字五种筛选维度
- FR-013: 筛选条件必须支持组合使用（AND 逻辑）
- FR-014: 系统必须支持按名称、添加日期、最近更新排序
- FR-015: 搜索必须支持实时过滤和高亮匹配
- FR-016: 系统必须保留用户筛选偏好（会话内持久化）

**订阅详情面板**:
- FR-017: 详情面板必须显示频道基本信息和统计数据
- FR-018: 详情面板必须随选中订阅变化实时更新
- FR-019: 失效订阅必须在详情面板明确标注
- FR-020: 详情面板视频列表支持分页加载，初始加载 10 条，用户可加载更多

### Key Entities

- **SubscriptionTag**: `{ id: uuid, name: string, color: string }` — 持久化到 subscriptions.json 的 tags 数组中
- **HealthCheckResult**: `{ subscription_id: string, status: "ok"|"warning"|"dead", reason: string, checked_at: datetime }` — 运行时中间结果，不单独持久化；其 `status` 和 `checked_at` 会持久化到对应 Subscription 的 `health_status` 和 `last_health_check` 字段
- **Subscription 健康字段**: `health_status: "ok"|"warning"|"dead"|null`、`last_health_check: datetime|null` — 持久化到 subscriptions.json 中每个订阅对象
- **ImportResult**: `{ total: number, success: number, failed: number, errors: {url: string, reason: string}[] }` — 不持久化，运行中生成

## Success Criteria *(mandatory)*

### Measurable Outcomes

- SC-001: 批量导入 50 个订阅在 60 秒内完成（含频道解析）
- SC-002: 健康检查 50 个订阅在 30 秒内完成
- SC-003: 搜索/筛选响应时间 < 200ms（本地过滤）
- SC-004: 详情面板加载时间 < 1 秒
- SC-005: 导入失败率 < 5%（网络正常情况下）
- SC-006: 用户一键删除所有失效订阅耗时 < 2 秒

## Assumptions

1. 用户设备上已安装 yt-dlp 并可正常调用
2. OPML 2.0 格式遵循 RSS 2.0 命名空间规范（`xmlUrl`、`htmlUrl`、`title`、`type` 属性）
3. 频道信息解析复用 MVP 阶段的 `YtDlpService::parse_channel_info()`
4. 详情面板的视频列表数据通过 yt-dlp `--flat-playlist` 获取，初始显示最近 10 条，支持分页加载更多
5. 筛选条件仅在当前会话内持久化，不需要跨会话恢复
6. 健康检查使用 HTTP HEAD 请求辅以 yt-dlp 验证，优先轻量检查
