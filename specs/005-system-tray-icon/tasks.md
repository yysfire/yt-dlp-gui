# Tasks: 系统托盘图标

**Input**: Design documents from `specs/005-system-tray-icon/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. TDD is a constitutional requirement (章程 §I).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Rust backend**: `src-tauri/src/`
- **TypeScript frontend**: `src/`
- **Icons**: `src-tauri/icons/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare tray icon assets and module structure without touching production code.

- [ ] T001 Create tray icon overlay images: download arrow (`src-tauri/icons/tray-downloading.png`) and checking spinner (`src-tauri/icons/tray-checking.png`), based on existing `src-tauri/icons/icon.png`
- [ ] T002 Create placeholder module file `src-tauri/src/services/tray.rs` with `pub mod tray;` declaration in `src-tauri/src/services/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data model and service skeleton that ALL user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T003 [P] Extend `AppSettings` in `src-tauri/src/models/settings.rs`: add `minimize_to_tray: bool` (default `true`), `close_to_tray: bool` (default `true`), `start_in_tray: bool` (default `false`), `scheduler_paused: bool` (default `false`). Write unit tests for serde default values and round-trip serialization.
- [ ] T004 [P] Extend `AppError` enum in `src-tauri/src/utils/error.rs`: add `TrayInitialization(String)`, `TrayNotSupported`, `TrayUpdate(String)` variants with `#[error]` annotations.
- [ ] T005 [P] Extend TypeScript `AppSettings` interface in `src/types/index.ts`: add `minimize_to_tray: boolean`, `close_to_tray: boolean`, `start_in_tray: boolean`, `scheduler_paused: boolean` fields with correct defaults.
- [ ] T006 Implement `TrayState` types in `src-tauri/src/services/tray.rs`: define `TrayStatus` enum (`Idle`, `Downloading { active_count: u32 }`, `Checking`), `TrayState` struct with `status`, `window_visible`, `tray_supported`, `active_downloads`. Write unit tests for status transitions.
- [ ] T007 [P] Add `get_settings` and `update_settings` command support for new tray fields in `src-tauri/src/commands/settings.rs`: ensure serde serializes/deserializes new fields correctly. Write unit test for round-trip.

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - 窗口最小化到系统托盘 (Priority: P1) 🎯 MVP

**Goal**: 用户点击最小化按钮时，窗口隐藏至系统托盘，托盘图标出现，后台任务继续运行。用户左键单击托盘图标恢复窗口。

**Independent Test**: 启动应用 → 点击窗口最小化按钮 → 验证窗口隐藏，托盘图标出现 → 左键单击托盘图标 → 验证窗口恢复显示。

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T008 [P] [US1] Unit test for `TrayService::init()` in `src-tauri/src/services/tray.rs`: verifies TrayIconBuilder configuration, icon assignment, and initial tooltip "yt-dlp 订阅管理器 - 空闲"
- [ ] T009 [P] [US1] Unit test for window visibility toggle logic in `src-tauri/src/services/tray.rs`: verifies `window_visible` flag flips correctly on show/hide

### Implementation for User Story 1

- [ ] T010 [US1] Implement `TrayService::init()` in `src-tauri/src/services/tray.rs`: creates tray icon via `TrayIconBuilder` with app icon, initial tooltip, and left-click toggle handler via `on_tray_icon_event`
- [ ] T011 [US1] Implement `TrayService::show_window()` and `TrayService::hide_window()` in `src-tauri/src/services/tray.rs`: uses `app_handle.get_webview_window("main")` to show/hide
- [ ] T012 [US1] Integrate TrayService into `src-tauri/src/lib.rs::setup()`: initialize tray, register `on_tray_icon_event` callback for left-click show/hide toggle, store tray handle in `AppContext`
- [ ] T013 [US1] Register window minimize event in `src-tauri/src/lib.rs::setup()`: on `WindowEvent::Minimize`, call `hide_window()` if `minimize_to_tray` setting is enabled

**Checkpoint**: User Story 1 complete — 最小化到托盘，左键单击恢复窗口

---

## Phase 4: User Story 2 - 托盘右键菜单提供快捷操作 (Priority: P1)

**Goal**: 用户右键托盘图标弹出上下文菜单，包含"显示/隐藏主窗口""检查全部订阅更新""暂停/恢复定时检查""退出"。菜单项可动态切换文本。

**Independent Test**: 右键托盘图标 → 验证菜单弹出含所有选项 → 点击"检查全部订阅更新"→ 验证后台开始检查 → 点击"暂停定时检查"→ 验证菜单变为"恢复定时检查" → 点击"退出"→ 验证应用退出。

### Tests for User Story 2 ⚠️

- [ ] T014 [P] [US2] Unit test for menu item texts in `src-tauri/src/services/tray.rs`: verifies "显示主窗口"/"隐藏主窗口" and "暂停/恢复" text switches correctly based on state
- [ ] T015 [P] [US2] Unit test for `TrayService::toggle_scheduler()` in `src-tauri/src/services/tray.rs`: verifies `scheduler_paused` flag toggles and watch notification is sent

### Implementation for User Story 2

- [ ] T016 [US2] Build tray context menu in `src-tauri/src/services/tray.rs::TrayService::init()`: use `MenuBuilder` to create 6 items — "显示主窗口", separator, "检查全部订阅更新", "暂停定时检查", separator, "退出"
- [ ] T017 [US2] Implement `on_menu_event` handler in `src-tauri/src/services/tray.rs`: match menu item IDs and dispatch actions — "show" toggles window, "check_all" calls existing check_all command, "toggle_scheduler" toggles pause, "quit" triggers exit flow
- [ ] T018 [US2] Implement scheduler toggle logic in `src-tauri/src/services/tray.rs::TrayService::toggle_scheduler()`: update `AppSettings.scheduler_paused`, persist via `StorageService::save_settings()`, send `scheduler_notify.send(())` via `AppContext`
- [ ] T019 [US2] Implement `update_menu()` in `src-tauri/src/services/tray.rs`: dynamically switch menu item text for "显示主窗口"/"隐藏主窗口" (based on `window_visible`) and "暂停/恢复定时检查" (based on `scheduler_paused`)
- [ ] T020 [US2] Wire menu events into `src-tauri/src/lib.rs::setup()`: register `on_menu_event` callback on tray icon, delegate to `TrayService` methods

**Checkpoint**: User Stories 1 + 2 complete — 最小化到托盘 + 右键菜单操作

---

## Phase 5: User Story 3 - 关闭窗口时最小化到托盘 (Priority: P2)

**Goal**: 用户点击关闭按钮（X）时，窗口隐藏到托盘而非退出应用。用户可在设置中开启/关闭此行为。

**Independent Test**: 在设置中启用"关闭到托盘" → 点击关闭按钮 → 验证窗口隐藏、托盘图标保持、进程未退出 → 通过托盘菜单恢复窗口 → 关闭"关闭到托盘"设置 → 点击关闭按钮 → 验证应用正常退出。

### Tests for User Story 3 ⚠️

- [ ] T021 [P] [US3] Unit test for `close_requested` handler logic in `src-tauri/src/services/tray.rs`: verifies `api.prevent_close()` is called when `close_to_tray` is enabled
- [ ] T022 [P] [US3] Unit test for `minimize_to_tray` setting toggle in `src-tauri/src/models/settings.rs`: verifies default value and serde persistence

### Implementation for User Story 3

- [ ] T023 [US3] Register `on_window_event` for `CloseRequested` in `src-tauri/src/lib.rs::setup()`: if `close_to_tray` enabled → call `api.prevent_close()` + `hide_window()`; otherwise allow normal exit
- [ ] T024 [US3] Add tray behavior settings section in `src/components/SettingsDialog.tsx`: three Switch components — "最小化到托盘" (default on), "关闭到托盘" (default on), "启动时最小化到托盘" (default off). Wire to `update_settings` invoke.
- [ ] T025 [US3] Extend `src/lib/tauri.ts`: add `getTrayState()` invoke wrapper returning `TrayState` type. Add `TrayState` interface in `src/types/index.ts`.
- [ ] T026 [US3] Implement `start_in_tray` logic in `src-tauri/src/lib.rs::setup()`: after tray initialization, if `start_in_tray` setting is true, call `window.hide()` (avoid splash flash via `visible: false` or post-init hide)

**Checkpoint**: User Stories 1 + 2 + 3 complete — 完整的托盘行为（最小化/关闭到托盘 + 设置）

---

## Phase 6: User Story 4 - 托盘图标状态指示 (Priority: P3)

**Goal**: 托盘图标根据应用状态（空闲/下载中/检查中）显示不同的 overlay 徽标和 tooltip 文本。

**Independent Test**: 触发下载任务 → 验证托盘图标切换到下载 overlay 徽标，tooltip 显示"正在下载 N 个视频" → 下载完成 → 验证图标恢复为空闲状态。

### Tests for User Story 4 ⚠️

- [ ] T027 [P] [US4] Unit test for `TrayState::set_status()` in `src-tauri/src/services/tray.rs`: verifies status transition rules (Idle → Downloading, Idle → Checking, Downloading → Idle, Checking → Idle), rejects invalid transitions
- [ ] T028 [P] [US4] Unit test for tooltip text generation in `src-tauri/src/services/tray.rs`: verifies "yt-dlp 订阅管理器 - 空闲", "正在下载 3 个视频", "正在检查订阅更新..." correct format strings

### Implementation for User Story 4

- [ ] T029 [US4] Implement `TrayService::set_status()` in `src-tauri/src/services/tray.rs`: updates `TrayState.status`, calls `tray.set_icon()` to switch between `tray-idle.png` / `tray-downloading.png` / `tray-checking.png`, calls `tray.set_tooltip()` with generated text
- [ ] T030 [US4] Implement icon loading in `src-tauri/src/services/tray.rs`: load `tray-idle.png`, `tray-downloading.png`, `tray-checking.png` at init time via `tauri::image::Image::from_bytes()` with `include_bytes!()`
- [ ] T031 [US4] Integrate status updates with download events in `src-tauri/src/services/tray.rs`: when a download starts, call `set_status(Downloading { active_count })`; when last download completes, call `set_status(Idle)`. Hook into existing download progress/complete event emission points in `src-tauri/src/commands/download.rs`.
- [ ] T032 [US4] Integrate status updates with scheduler check events in `src-tauri/src/services/tray.rs`: when scheduler begins a check cycle, call `set_status(Checking)`; when check cycle ends, call `set_status(Idle)`. Hook into scheduler state in `src-tauri/src/lib.rs`.
- [ ] T033 [US4] Emit `tray-state-changed` event to frontend in `src-tauri/src/services/tray.rs::set_status()`: use `app_handle.emit("tray-state-changed", TrayState)` to notify frontend of status changes
- [ ] T034 [US4] Add frontend listener for `tray-state-changed` event in `src/App.tsx` or `src/components/StatusBar.tsx`: display current tray state (idle/downloading/checking) in status bar

**Checkpoint**: All 4 user stories independently functional — 托盘图标完整功能

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Platform compatibility, logging, error handling, and final validation.

- [ ] T035 [P] Implement tray environment detection in `src-tauri/src/services/tray.rs::TrayService::init()`: wrap `TrayIconBuilder::build()` in error handling; on failure set `tray_supported = false`, log warning, skip all tray operations
- [ ] T036 [P] Implement tray operation logging (FR-016) in `src-tauri/src/services/tray.rs`: use `log::info!` for icon create/destroy and status switches, `log::info!` for menu item clicks, `log::warn!` for unsupported environment, `log::error!` for icon load failures
- [ ] T037 Implement platform-specific icon sizes for macOS in `src-tauri/src/services/tray.rs`: use 18x18 @1x / 36x36 @2x icon versions for macOS menu bar; on non-macOS use standard icon
- [ ] T038 [P] Run `cargo test` and verify all Rust tests pass (target: 69+ existing + new tray tests)
- [ ] T039 [P] Run `npx tsc --noEmit` and verify TypeScript compilation passes
- [ ] T040 Run `npm run tauri dev` and manually verify tray icon appears and functions on the current platform per quickstart.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion (T001 icon assets needed before T006 TrayService) - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational phase completion
- **User Story 2 (Phase 4)**: Depends on US1 completion (T012 provides tray handle used by menu)
- **User Story 3 (Phase 5)**: Depends on US1 + US2 completion (needs tray handle + settings infrastructure)
- **User Story 4 (Phase 6)**: Depends on US1 completion (needs tray handle for set_icon/set_tooltip). Can start in parallel with US2/US3.
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational — No dependencies on other stories
- **User Story 2 (P2, but priority P1)**: Depends on US1 for tray handle — US2 adds menu layer on top of US1's tray creation
- **User Story 3 (P2)**: Depends on US1 (tray handle, hide_window) + US2 (settings infrastructure)
- **User Story 4 (P3)**: Depends on US1 for tray handle. Independent of US2/US3. Can start in parallel with US2.

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- TrayService methods before lib.rs integration
- Rust backend validation before frontend work
- Story complete with cargo test passing before moving to next priority

### Parallel Opportunities

- **Phase 2**: T003 (AppSettings Rust), T004 (AppError), T005 (TypeScript types) can all run in parallel (3 different files)
- **Phase 3**: T008 (init test) and T009 (toggle test) can run in parallel
- **Phase 4**: T014 (menu text test) and T015 (scheduler test) can run in parallel
- **Phase 6**: T027 (status transition test) and T028 (tooltip test) can run in parallel
- **Phase 7**: T035 (env detection), T036 (logging), T037 (macOS icons), T038 (cargo test), T039 (tsc) can all run in parallel

---

## Parallel Example: User Story 1

```bash
# Step 1: Launch all tests for User Story 1 together:
Task: "T008 [P] [US1] Unit test for TrayService::init() in src-tauri/src/services/tray.rs"
Task: "T009 [P] [US1] Unit test for window visibility toggle in src-tauri/src/services/tray.rs"

# Step 2: After tests fail, implement core service:
Task: "T010 [US1] Implement TrayService::init() in src-tauri/src/services/tray.rs"

# Step 3: Then integrate (sequential):
Task: "T011 [US1] Implement show_window/hide_window"
Task: "T012 [US1] Integrate into lib.rs::setup()"
Task: "T013 [US1] Register minimize event in lib.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T002)
2. Complete Phase 2: Foundational (T003-T007) — **CRITICAL: blocks all stories**
3. Complete Phase 3: User Story 1 (T008-T013)
4. **STOP and VALIDATE**: `cargo test` passes, `npm run tauri dev` shows tray icon with left-click show/hide
5. Demo: 最小化到托盘 + 左键单击恢复窗口

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → 最小化到托盘 (MVP!)
3. Add US2 → 右键菜单快捷操作 (扩展 P1)
4. Add US3 → 关闭到托盘 + 设置页面
5. Add US4 → 状态指示 (overlay + tooltip)
6. Polish → 平台兼容性 + 日志

### Sequential Strategy (Single Developer)

Since US2 depends on US1 for the tray handle, and US3 depends on US1+US2:

```
T001-T007 (Setup + Foundation)
  → T008-T013 (US1: 最小化到托盘)
    → T014-T020 (US2: 右键菜单)
      → T021-T026 (US3: 关闭到托盘)
T027-T034 (US4: 状态指示) — 可与 US2/US3 并行进行
  → T035-T040 (Polish)
```

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- TDD cycle: Write test → verify FAIL → implement → verify PASS → commit
- Each user story is independently completable and testable after its phase
- Constitution requirements: all new public functions MUST have unit tests (§I), services MUST remain stateless (§II)
- Reuse existing `StorageService` for settings persistence (do not create new storage abstractions)
- Reuse existing scheduler watch channel for pause/resume (do not create new notification mechanism)
- Avoid: code unrelated to tray feature, speculative abstractions, changes to unrelated components
