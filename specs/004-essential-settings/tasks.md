# Tasks: 必要设置

**Input**: Design documents from `specs/004-essential-settings/`

**Prerequisites**: plan.md, spec.md (4 user stories), research.md, data-model.md, contracts/settings-commands.md, quickstart.md

**Tests**: 包含测试任务（项目章程 Section I 要求 TDD）

**Organization**: 按用户故事（User Story）组织，每个故事可独立实现和测试。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 所属用户故事（US1, US2, US3, US4）
- 包含精确文件路径

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 新增依赖、模块声明、命令注册

- [x] T001 在 `src-tauri/Cargo.toml` 中添加 `url = "2"` 依赖
- [x] T002 [P] 在 `src-tauri/src/services/mod.rs` 中声明 `pub mod settings_validator;` 并创建空模块文件 `src-tauri/src/services/settings_validator.rs`
- [x] T003 [P] 在 `src-tauri/src/utils/error.rs` 中新增 `InvalidPath(String)`、`InvalidProxy(String)` 错误变体，并确保 `Display`/`Into<tauri::InvokeError>` 实现正确

---

## Phase 2: Foundational（阻塞性前置任务）

**Purpose**: 核心基础——所有用户故事依赖此阶段完成

**⚠️ CRITICAL**: 用户故事实现必须在此阶段完成后才能开始

- [x] T004 修改 `src-tauri/src/models/settings.rs` 中 `default_check_interval()` 默认值从 `360` 改为 `60`，更新 `max_concurrent_downloads` 文档注释：上限从 3 改为 5
- [x] T005 [P] 在 `src-tauri/src/services/settings_validator.rs` 中编写 `validate_download_path()` 函数测试（`#[cfg(test)]`），覆盖：`test_validate_path_exists_and_writable`（路径存在且可写）、`test_validate_path_created`（mkdir 后可写）、`test_validate_path_unwritable`（权限不足）、`test_validate_empty_path`（空路径）
- [x] T006 [P] 在 `src-tauri/src/services/settings_validator.rs` 中编写 `validate_proxy_url()` 函数测试（`#[cfg(test)]`），覆盖：空字符串、合法 http、合法 socks5、合法 socks5h、缺 host、非法 scheme、格式错误
- [x] T007 在 `src-tauri/src/services/settings_validator.rs` 中实现 `validate_download_path()` - 检查路径存在性，`create_dir_all` 创建目录，写入临时文件验证可写性（通过 T005 的测试）
- [x] T008 在 `src-tauri/src/services/settings_validator.rs` 中实现 `validate_proxy_url()` - 使用 `url::Url::parse()` 解析，验证 scheme 为 http/https/socks5/socks5h，检查 host 非空（通过 T006 的测试）
- [x] T009 在 `src-tauri/src/commands/settings.rs` 中新增 `validate_download_path` Tauri 命令，输入 `{ path: String }`，返回 `PathValidateResult`
- [x] T010 [P] 在 `src-tauri/src/commands/settings.rs` 中新增 `validate_proxy_url` Tauri 命令，输入 `{ url: String }`，返回 `ProxyValidateResult`
- [x] T011 在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册 `validate_download_path` 和 `validate_proxy_url` 命令
- [x] T012 [P] 在 `src/types/index.ts` 中新增 `PathValidateResult` 和 `ProxyValidateResult` 接口定义
- [x] T013 [P] 在 `src/lib/tauri.ts` 中新增 `validateDownloadPath(path: string)` 和 `validateProxyUrl(url: string)` invoke 封装函数
- [x] T018 [P] 在 `src-tauri/src/models/settings.rs` 的 `#[cfg(test)]` 模块中新增测试 `test_corrupt_settings_returns_defaults`：构造损坏的 JSON 字符串，验证 `serde_json::from_str::<AppSettings>()` 反序列化失败后回退到 `Default::default()`（覆盖 FR-009, SC-005）

**Checkpoint**: 基础就绪 — 已可并行开始用户故事实现

---

## Phase 3: User Story 1 - 全局下载路径设置 (Priority: P1) 🎯 MVP

**Goal**: 用户可设置全局下载保存路径，支持文件夹浏览选择，保存前验证路径有效性和可写性，下载时路径不可用则自动回退

**Independent Test**: 打开设置 → 点击"浏览"选择文件夹 → 验证有效路径 → 保存 → 重启 → 验证路径持久化

### Implementation for User Story 1

- [x] T014 [US1] 扩展 `src/components/SettingsDialog.tsx`：下载路径区域 — 文本输入框 + "浏览"按钮（调用 `@tauri-apps/plugin-dialog` 的 `open` 选择文件夹）
- [x] T015 [US1] 扩展 `src/components/SettingsDialog.tsx`：路径选择后调用 `validateDownloadPath()` 验证，显示验证结果（有效/不可写/不存在），无效时禁用保存按钮
- [x] T016 [US1] 在 `src-tauri/src/services/download_queue.rs` 中实现 FR-012 路径回退逻辑：下载前调用 `create_dir_all` 检查路径，失败时回退到 `default_download_dir()`，通过 `emit` 通知前端路径变更
- [x] T017 [US1] 在 `src-tauri/src/commands/settings.rs` 的 `update_settings` 中添加 `download_dir` 变更时的可写性验证

**Checkpoint**: User Story 1 完整可用 — 路径浏览、验证、持久化、回退均通过手动测试

---

## Phase 4: User Story 2 - 基本 yt-dlp 参数设置 (Priority: P2)

**Goal**: 用户可设置画质偏好和代理服务器地址，代理不可用时任务标记失败不自动回退直连

**Independent Test**: 设置画质为 720p → 下载验证 → 配置代理 → 代理不可用时验证任务失败不回退

### Tests for User Story 2 ⚠️

- [x] T019 [P] [US2] 在 `src-tauri/src/services/settings_validator.rs` 新增单元测试：`test_validate_proxy_empty`（空字符串通过）、`test_validate_proxy_http`（合法 HTTP）、`test_validate_proxy_socks5`（合法 SOCKS5）、`test_validate_proxy_invalid`（非法格式）、`test_validate_proxy_no_host`（缺 host）、`test_validate_proxy_unsupported_scheme`（不支持的协议）

### Implementation for User Story 2

- [x] T020 [US2] 扩展 `src/components/SettingsDialog.tsx`：画质预设下拉选择框（复用现有组件或增强，选项：最高画质/1080p/720p/480p/仅音频）
- [x] T021 [US2] 扩展 `src/components/SettingsDialog.tsx`：代理地址文本输入框（placeholder："留空不使用代理"），失焦时调用 `validateProxyUrl()` 实时验证，显示验证状态图标
- [x] T022 [US2] 扩展 `src/components/SettingsDialog.tsx`：代理输入非法时禁用保存按钮，显示具体错误提示
- [x] T023 [US2] 在 `src-tauri/src/services/download_queue.rs` 中实现 FR-011 代理失败处理：下载子进程因代理不可用失败时，任务标记 "failed"，`DownloadRecord.error` 记录 "代理连接失败"，不自动重试为直连

**Checkpoint**: User Story 1 + 2 完整可用 — 画质、代理设置独立验证通过

---

## Phase 5: User Story 3 - 并发任务数设置 (Priority: P3)

**Goal**: 用户可设置并发下载数（1-5），降低并发数时按进度暂停最少进度的任务

**Independent Test**: 设置并发数为 2 → 触发 3 个下载 → 验证最多 2 个同时 → 降到 1 → 验证进度最少任务暂停

### Tests for User Story 3 ⚠️

- [x] T024 [P] [US3] 在 `src-tauri/src/services/download_queue.rs` 的 `#[cfg(test)]` 模块新增测试：`test_adjust_concurrency_increase`（并发提升时启动等待任务）、`test_adjust_concurrency_decrease_by_progress`（按进度排序暂停最少进度任务）、`test_adjust_concurrency_completes_under_3s`（模拟队列验证 `adjust_concurrency()` 在 3 秒内返回，覆盖 SC-004）

### Implementation for User Story 3

- [x] T025 [US3] 扩展 `src/components/SettingsDialog.tsx`：并发数滑块（Slider，范围 1-5，步长 1，标签显示当前值）
- [x] T026 [US3] 前端输入验证：滑块天然限制范围，额外添加输入框直接修改时的越界校验（1-5）
- [x] T027 [US3] 在 `src-tauri/src/services/download_queue.rs` 中增强 `update_max_concurrent()` 方法：增加时添加信号量许可；减少时按 `percent` 升序暂停进度最少的活跃任务
- [x] T028 [US3] 在 `src-tauri/src/services/download_queue.rs` 中维护每个活跃任务的 `last_progress_percent: f32`：`ActiveTask` 新增字段，`download-progress` 事件时更新
- [x] T029 [US3] 在 `src-tauri/src/commands/settings.rs` 的 `update_settings` 中检测 `max_concurrent_downloads` 变更，通过 `app_handle.state::<QueueContext>()` 调用 `queue.update_max_concurrent()`
- [x] T030 [US3] `QueueContext` 已在 `lib.rs` 中 `app.manage()`，`update_settings` 通过 `AppHandle` 参数访问

**Checkpoint**: User Story 1+2+3 完整可用 — 并发调整独立验证

---

## Phase 6: User Story 4 - 检查频率设置 (Priority: P2)

**Goal**: 用户可设置定时检查频率（手动/30分钟/每小时/每天），频率变更通知调度器立即生效

**Independent Test**: 设置频率为每 30 分钟 → 修改为每小时 → 验证调度器间隔变化 → 切换到手动 → 验证自动检查停止

### Tests for User Story 4 ⚠️

- [x] T031 [P] [US4] 在 `src-tauri/src/models/settings.rs` 的 `#[cfg(test)]` 模块新增测试：`test_default_check_interval_is_60`（验证默认值为 60）
- [x] T032 [P] [US4] 在 `src-tauri/src/commands/settings.rs` 的 `#[cfg(test)]` 模块中新增测试

### Implementation for User Story 4

- [x] T033 [US4] 扩展 `src/components/SettingsDialog.tsx`：检查频率下拉选择（选项：手动/30分钟/每小时/每天，对应值：0/30/60/1440）
- [x] T034 [US4] 前端检查频率选择变更时直接更新 `AppSettings.check_interval_minutes`，通过 `updateSettings()` 保存
- [x] T035 [US4] 在 `src-tauri/src/lib.rs` 的调度器 loop 中处理 `check_interval_minutes == 0`（手动模式）：`select!` 中移除 sleep 分支，仅保留 `rx.changed()`，暂停自动检查
- [x] T036 [US4] 在 `src-tauri/src/lib.rs` 的调度器 loop 中处理手动模式退出：`check_interval_minutes > 0` 恢复为正常间隔定时检查

**Checkpoint**: 全部 4 个 User Story 均可独立测试

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 跨用户故事的质量保障

- [x] T037 [P] 运行 `cargo test -p yt-dlp-gui` 确认所有 Rust 测试通过
- [x] T038 [P] 运行 `npx tsc --noEmit` 确认 TypeScript 类型检查通过
- [ ] T039 按 `specs/004-essential-settings/quickstart.md` 中"运行验收"清单手动验证全部功能
- [ ] T040 [P] 在 `src/components/SettingsDialog.tsx` 中验证所有输入控件的错误/空状态/加载状态均已覆盖
- [x] T041 [P] 在 `src-tauri/src/services/settings_validator.rs` 中补充边界测试：路径含空格/Unicode 字符
- [x] T042 [P] 在 `src-tauri/src/models/settings.rs` 的 `#[cfg(test)]` 模块中新增测试 `test_settings_persist_across_reload`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖 — 可立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成 — **阻塞所有用户故事**
- **User Stories (Phase 3-6)**: 全部依赖 Foundational 完成
  - US1 (P1) → US2 (P2) → US3 (P3) → US4 (P2) 按优先级顺序执行
  - 或全部并行执行（如有多人）
- **Polish (Phase 7)**: 依赖所有用户故事完成

### User Story Dependencies

- **US1 (P1 - 下载路径)**: Foundational 完成后即可开始，无其他故事依赖
- **US2 (P2 - yt-dlp 参数)**: Foundational 完成后即可开始，可与 US1 并行（UI 同文件需协调）
- **US3 (P3 - 并发数)**: Foundational 完成后即可开始，需与 US1/US2 协调 `SettingsDialog.tsx` 修改
- **US4 (P2 - 检查频率)**: Foundational 完成后即可开始，无其他故事依赖

### Within Each User Story

- 测试（T019, T024, T031-T032）MUST 先编写并确认失败
- 模型/Service 层 → 命令层 → 前端 UI
- 核心实现 → 前端集成 → 手动验证

### Parallel Opportunities

- **Phase 1**: T002 和 T003 可并行
- **Phase 2**: T005 和 T006（测试）可并行；T009 和 T010（命令）可并行；T012 和 T013（前端）可并行
- **Phase 3-6 测试**: T019 [US2]、T024 [US3]、T031-T032 [US4] 全部可并行（不同文件）
- **Phase 7**: T037、T038、T040 可并行
- **跨故事并行**: US1-US4 的 Rust 测试和实现可由不同开发者并行（但需协调 `SettingsDialog.tsx` 的修改）

---

## Parallel Example: Phase 2 (Foundational)

```bash
# 并行启动 Rust 测试编写：
Task: "T005 [P] 编写 validate_download_path 测试 in settings_validator.rs"
Task: "T006 [P] 编写 validate_proxy_url 测试 in settings_validator.rs"

# 并行启动命令注册：
Task: "T009 新增 validate_download_path 命令 in commands/settings.rs"
Task: "T010 [P] 新增 validate_proxy_url 命令 in commands/settings.rs"

# 并行启动前端封装：
Task: "T012 [P] 新增 types in src/types/index.ts"
Task: "T013 [P] 新增 invoke wrappers in src/lib/tauri.ts"
```

## Parallel Example: User Story Tests

```bash
# 所有 User Story 的测试任务可一次性并行启动：
Task: "T019 [P] [US2] 代理验证单元测试 in settings_validator.rs"
Task: "T019 [P] [US2] 代理验证单元测试 in settings_validator.rs"
Task: "T024 [P] [US3] 并发调整单元测试 in download_queue.rs"
Task: "T031 [P] [US4] 默认值测试 in settings.rs"
Task: "T032 [P] [US4] 调度通知测试 in settings.rs"
```

---

## Implementation Strategy

### MVP First (仅 User Story 1)

1. 完成 Phase 1: Setup（T001-T003）
2. 完成 Phase 2: Foundational（T004-T013）
3. 完成 Phase 3: User Story 1（T014-T017）
4. **停止并验证**: 独立测试 User Story 1
5. 此时已可用的功能：全局下载路径设置（浏览 + 验证 + 持久化 + 开机恢复）

### Incremental Delivery

1. Setup + Foundational → 基础就绪
2. 添加 US1 → 独立测试 → MVP 就绪
3. 添加 US2 → 画质 + 代理设置可用
4. 添加 US3 → 并发控制可用
5. 添加 US4 → 检查频率设置可用
6. Phase 7 Polish → 质量验证通过

### 注意事项

- `src/components/SettingsDialog.tsx` 被 US1-US4 修改，多人并行时需拆分或协调合并
- `src-tauri/src/services/download_queue.rs` 被 US1(FR-012) 和 US3(FR-010) 修改，建议 US1 先完成
- `src-tauri/src/lib.rs` 被 US3(T030) 和 US4(T035-T036) 修改，注意合并顺序
