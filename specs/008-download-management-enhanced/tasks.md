---

description: "Task list for 008-download-management-enhanced (P1)"
---

# Tasks: 下载管理增强（P1）

**Input**: Design documents from `/specs/008-download-management-enhanced/`

**Prerequisites**: [plan.md](./plan.md)、[spec.md](./spec.md)、[research.md](./research.md)、[data-model.md](./data-model.md)、[contracts/](./contracts/)、[quickstart.md](./quickstart.md)

> 📌 **决策的唯一真相源是 [decisions.md](./decisions.md)。** 本文件给出可执行的任务拆解；每条规则细节都在 decisions.md（分类器匹配式 §5、状态转移 §6、真值表 §2.4、字段契约 §1）。

**Tests**: 本节**包含测试任务**。依据：项目章程原则 I（TDD）是**不可协商**的，且 spec 的 `Success Criteria` 与 `quickstart.md` 把测试列为验收标准。

**Organization**: 按用户故事分组。`spec.md` 里的故事 3 / 4 / 5 **已划出本次范围**，故不生成阶段。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可与同阶段其它 [P] 任务并行（不同文件、无未完成依赖）
- **[Story]**: `[US1]` / `[US2]`
- ⚠️ **`download_queue.rs` 是热点文件**：涉及它的任务**一律不可并行**，即使标了 [P] 也不要在同文件内并行。

## Path Conventions

单仓库 desktop-app：Rust 在 `src-tauri/src/`，前端在 `src/`。测试与源码同文件（Rust 用 `#[cfg(test)]` 模块；前端用 `__tests__/`）。

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 建立两侧共用的契约基准

- [X] T001 创建契约基准 fixture `src-tauri/tests/fixtures/download_record.json`，内容照 [contracts/download-record.md](./contracts/download-record.md) §3 逐字段照抄（**`status = "completed"` 与 `retry_count = 2` 是有意组合**，用来锁住「成功时保留重试次数」）

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 两个用户故事共用的数据契约与任务骨架

**⚠️ CRITICAL**: 本阶段完成前，任何用户故事都不能开始

- [X] T002 扩展 Rust `DownloadRecord`：新增 `quality: String`（`#[serde(default)]`，空串 = 未知）、`retry_count: u32`（`#[serde(default)]`）、`last_retry_at: Option<String>`（`#[serde(default, skip_serializing_if = "Option::is_none")]`）；**`DownloadRecord::new()` 签名保持不变**，在 `src-tauri/src/models/download.rs`
- [X] T003 [P] 同步 TS 类型：`DownloadRecord` 加同名三字段、`DownloadRecordStatus` 加 `"retrying"`，在 `src/types/index.ts`（照 [contracts/download-record.md](./contracts/download-record.md) §2 的对照表）
- [X] T004 [P] `deduplicate_vec` 的 rank 表插入 `retrying`，新表为 `completed=0, downloading=1, retrying=2, failed=3, paused=4, cancelled=5`、其余 `9`；**同 rank 内取最新 `downloaded_at`**；同文件补单测，在 `src-tauri/src/services/storage.rs`
- [X] T005 契约测试（Rust 侧）：`include_str!` 读 T001 的 fixture，断言 `serde_json::to_value(基准记录) == 解析出的 fixture`；另断言 `error_message = None` 时该键**不出现**（而非 `null`），在 `src-tauri/src/models/download.rs` 的 `#[cfg(test)]`
- [X] T006 [P] 契约测试（前端侧）：读**同一个** fixture，`JSON.parse(...) satisfies DownloadRecord`，并断言 `status` 落在联合类型内；另断言 fixture 含 `quality`/`retry_count`/`last_retry_at` 三键，在 `src/types/__tests__/downloadRecordContract.test.ts`
- [X] T007 `DownloadTask` 新增 `record_id: String` 与 `next_retry_at: Option<String>`，并在 `src/types/index.ts` 同步；Rust 侧在 `src-tauri/src/services/download_queue.rs`
- [X] T008 回写定位改按 `record_id`：完成回写、失败回写、`update_record_status` 全部由 `(video_url, subscription_id)` 过滤改为 `record_id` 精确匹配；**`downloaded_at` 只在真正成功完成时写**（暂停/取消/重试不再改写），在 `src-tauri/src/services/download_queue.rs`

**Checkpoint**: 数据契约与任务骨架就绪，两个故事可并行开工

---

## Phase 3: User Story 1 - 智能重复检测增强 (Priority: P1) 🎯 MVP

**Goal**: 基于视频 ID 的全局去重 + 基于画质档次的「可升级」派生 + 文件缺失的呈现与用户触发式重下 + 「已在其它订阅下载」的如实呈现

**Independent Test**: 先以 `720p` 下载某视频，再把订阅画质设为 `1080p` 并重新检查 —— 该行标为「可升级 1080p」，点「升级」后重下为 1080p、记录更新，且**只有一份文件**（`quickstart.md` §2.1）

### Tests for User Story 1 ⚠️（先写、必须失败）

- [X] T009 [P] [US1] 真值表单测：逐行覆盖 `upgradeable` / `missing` / `downloadedElsewhere` 的 6 条有序规则与 13 行组合（含「缺失优先于升级」「降级不提示」「未知不提示」「成功时 retry_count 不被归零」），在 `src/lib/__tests__/unifiedVideoList.test.ts`
- [X] T010 [P] [US1] 去重语义回归测试：断言 `seen_ids`/`seen_urls` 仍以 `status != "failed"` 为界，`deleted` 与「文件缺失」**留在集合内**（不自动重下），在 `src-tauri/src/commands/download.rs` 的 `#[cfg(test)]`
- [X] T011 [P] [US1] 画质档次全序单测：`480p < 720p < 1080p < 1440p < 2160p < best`，未知/空串/`audio` 一律不可比较，在 `src/lib/__tests__/unifiedVideoList.test.ts`

### Implementation for User Story 1

- [X] T012 [US1] 扩展 `buildUnifiedVideoList` 签名（加 `qualityPreset` / `missingPaths` / `subscriptionId`）并按序派生 `upgradeable` / `missing` / `downloadedElsewhere`，在 `src/lib/unifiedVideoList.ts`
- [X] T013 [US1] `UnifiedVideoItem` 加三个**正交**字段（`upgradeable` / `missing` / `downloadedElsewhere`），在 `src/types/index.ts`
- [X] T014 [US1] 把 `missingPaths` 从 `DownloadedList` 组件本地状态提升到 `src/App.tsx`（监听 `file-sync-complete` 并汇聚），下发给两个面板
- [X] T015 [US1] `AppShell` 不再只下传 `filteredRecords`：改为下传**全局去重后**的记录 + 当前订阅 id（计数仍只数本订阅），在 `src/components/AppShell.tsx`
- [X] T016 [P] [US1] 新增共享函数 `sync_completed_records(data_dir) -> Vec<FileExistenceResult>`（筛 `completed` + 非空 `file_path` → `check_files_exist` → emit `file-sync-complete`），在 `src-tauri/src/services/file_manager.rs`
- [X] T017 [US1] `sync_file_states`（`src-tauri/src/commands/file_manager.rs`）与 `spawn_file_sync`（`src-tauri/src/services/scheduler.rs`）**都改为调用 T016 的共享函数**（消除两处重复内联）
- [X] T018 [US1] `enqueue_from_video` 改为按 `(video_id, subscription_id)` **upsert 重置**：命中即重置 `status="downloading"` / `error_message=None` / `quality=订阅当前 preset` / `retry_count=0` / `last_retry_at=None`，**保留 `file_path`/`file_size`/`downloaded_at`**；未命中则新建，在 `src-tauri/src/services/download_queue.rs`
- [X] T019 [US1] 入队收口加**幂等守卫**：`queue` 待处理或 `active_tasks` 已有同 `record_id` 任务，或记录状态 ∈ `{waiting, downloading, retrying}` → 不重复入队，在 `src-tauri/src/services/download_queue.rs`
- [X] T020 [US1] `delete_file_to_trash` 由私有改 `pub(crate)`（`src-tauri/src/services/file_manager.rs`）；在 `src-tauri/src/services/download_queue.rs` 实现**旧文件回收**：**仅成功时**、旧 `file_path` 非空且与新路径**字符串不等**时触发，且**先落库新路径再回收**
- [X] T021 [US1] 新增命令 `redownload_video(record_id) -> Result<DownloadRecord, String>`（后端据记录反查订阅、取**订阅当前** `quality_preset`），在 `src-tauri/src/commands/download.rs`
- [X] T022 [US1] 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 注册 `redownload_video`
- [X] T023 [P] [US1] 前端封装 `redownloadVideo(recordId)`，在 `src/lib/tauri.ts`（禁止组件内直接 `invoke`）
- [X] T024 [US1] `DetailPanel`：可升级/缺失/重试行的**左侧 3px 色条**（非强调行用 `transparent` 占位避免行宽跳动）+ 状态旁 chip（「可升级 1080p」/`best` 渲染为「可升级 最高画质」、「文件缺失」、「已在其它订阅下载」）+ 右侧行内按钮（「升级」`title="升级到 1080p"` / 「重新下载」`title="重新下载"`），三个按钮（含失败行的「重试」）**全部改调 `redownloadVideo`**，在 `src/components/DetailPanel.tsx`
- [X] T025 [US1] `DownloadedList` 复用同一套文案与同一命令（「已在其它订阅下载」不提供重下/升级），并移除 `AppShell.tsx` 中 `handleRetry` 的「重查整个订阅」语义，在 `src/components/DownloadedList.tsx` 与 `src/components/AppShell.tsx`
- [X] T026 [US1] 补上**订阅级画质设置入口**（复用已存在的 `update_subscription_quality` 命令与 `useSubscriptions.updateQuality`，把 `DetailPanel.tsx` 频道头部那枚只读画质 chip 变为可编辑），并把死设置 `AppSettings.quality_preset` 接上作为新建订阅的默认画质（替代 `Subscription::new` 里硬编码的 `1080p`），在 `src/components/DetailPanel.tsx` 与 `src-tauri/src/commands/subscription.rs`

**Checkpoint**: 用户故事 1 可独立验证（`quickstart.md` §2.1 / §2.2）

---

## Phase 4: User Story 2 - 失败自动重试 (Priority: P1)

**Goal**: 单条下载失败后在 30/60/120 秒后自动重试，最多 3 次；按失败原因决定是否重试；退避期间可暂停/取消且不误伤并发与无关进程

**Independent Test**: 制造一次可重试失败（如抓取阶段 HTTP 503），确认 30 秒后自动重试、最终成功，记录 `completed` 且 `retry_count == 1`（`quickstart.md` §2.3）

### Tests for User Story 2 ⚠️（先写、必须失败）

- [X] T027 [US2] 分类器单测：跑 [contracts/retry-classifier.md](./contracts/retry-classifier.md) §3 的全部 12 个实测样本（404×2 / 503 / 408 / 429 / DNS / 代理 407 / SOCKS5 / 格式不可用 / **含 WARNING 假阳性 403 的那组** / `exit_code=None` / `2` 与 `101`），在 `src-tauri/src/utils/retry_policy.rs` 的 `#[cfg(test)]`
- [X] T028 [US2] 退避与相位纯函数单测：`remaining_after`（暂停冻结 / 恢复续算）、`phase_of`、`should_retry`（`Unknown` 视同 `Retry`），在 `src-tauri/src/utils/retry_policy.rs` 的 `#[cfg(test)]`
- [X] T029 [P] [US2] `error_message` 脱敏单测：含 `socks5://user:pass@host:1080` 的 stderr 摘要落库后**不含** `user:pass@`，且长度 ≤200 字符，在 `src-tauri/src/utils/retry_policy.rs` 的 `#[cfg(test)]`

### Implementation for User Story 2

- [X] T030 [US2] 新增模块 `src-tauri/src/utils/retry_policy.rs`：`RetryDecision` 枚举、`error_lines`（剥 ANSI + 只留 `ERROR:` 行）、`classify_failure`（退出码短路 → 终态黑名单 → `Tunnel connection failed:` → `HTTP Error <code>:` → 网络白名单 → `Unknown`）、`BACKOFF_SECS = [30,60,120]`、`MAX_ATTEMPTS = 3`、`remaining_after`、`phase_of`、`should_retry`；**逐条匹配式照抄 [decisions.md](./decisions.md) §5.3/§5.4，黑名单宁窄勿宽、白名单宁宽勿窄**
- [X] T031 [US2] 在 `src-tauri/src/utils/retry_policy.rs` 加 `sanitize_error_message`（剥离 `user:pass@` 凭证 + 截断到 200 字符），并在 `src-tauri/src/utils/mod.rs` 导出新模块
- [X] T032 [US2] `execute_download_with_control` 改为**并发读 stderr**（第二个 tokio task 累积文本，**只保留尾部 64 KiB**），循环结束后把 stderr 与 `ExitStatus::code()` 交给 `classify_failure`，在 `src-tauri/src/services/download_queue.rs`
- [X] T033 [US2] `ActiveTask`：`pid` 由 `u32` 改 **`Option<u32>`**、新增 `notify: Arc<tokio::sync::Notify>`；**所有**发信号处（`pause`/`resume`/`pause_by_url`/`cancel`/`update_max_concurrent`）收进 `Some(pid)` 分支；`update_max_concurrent` 缩容挑选时**排除 `child.is_none()`（退避中）的 entry**，在 `src-tauri/src/services/download_queue.rs`
- [X] T034 [US2] 在 `execute_download_with_control` 内实现**退避循环**：`tokio::select!`（`sleep(remaining)` vs `notify.notified()`）＋持**剩余时长**；暂停 → 冻结并等恢复；恢复 → 用剩余时长续算；**取消 → 发现 entry 已移除即退出且不回写任何记录**，在 `src-tauri/src/services/download_queue.rs`
- [X] T035 [US2] 失败分流接入分类器：`Retry`/`Unknown` 且在 `MAX_ATTEMPTS` 内 → 写 `status="retrying"` + `retry_count += 1` + `last_retry_at` 后进入退避；`NoRetry` 或达上限 → 写 `failed` + `sanitize_error_message(最后一条 ERROR 行)`；**spawn 失败**（`download_video_spawn` 报错）补写 `failed` + `"无法启动 yt-dlp：<io error>"`（现状是直接 return、记录永久卡在下载中），在 `src-tauri/src/services/download_queue.rs`
- [X] T036 [US2] 字段维护：`retry_count` / `last_retry_at` 在**新一轮下载开始时归零**（T018 的 upsert 重置已覆盖）、**成功时保留**；成功时清 `error_message`，在 `src-tauri/src/services/download_queue.rs`
- [X] T037 [P] [US2] `statusPriority` 插入 `retrying`，新序为 `downloading=0, retrying=1, waiting=2, paused=3, completed=4, new=5, cancelled=6, failed=7`，在 `src/lib/unifiedVideoList.ts`
- [X] T038 [US2] `DetailPanel` 重试行显示「重试中 (2/3)」+「· 还有 12 秒」（**活倒数**，数据源为 `queueTask.next_retry_at`，本地计时器驱动；**只在详情面板**，列表视图不加），在 `src/components/DetailPanel.tsx`
- [X] T039 [US2] `recover_state` 把 `retrying` 一并置为 `failed` + `error_message = "Application restarted"`（不主动恢复重试），在 `src-tauri/src/commands/download.rs`

**Checkpoint**: 用户故事 2 可独立验证（`quickstart.md` §2.3）

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T040 [P] 更新 `AGENTS.md` 中受影响的章节：事件契约、去重语义（全局 + 归属）、`retrying` 状态、重试与错误归因、`decisions.md` 的位置与其「实现期唯一真相源」地位
- [ ] T041 跑 `quickstart.md` §2.4 的 **5 步骨架接线人工验收**（不可自动化，前提 P11 的代价）
- [ ] T042 逐项勾选 `specs/008-download-management-enhanced/checklists/requirements.md`
- [X] T043 全量检查：`cargo test` && `npm test` && `npx tsc --noEmit` 全绿；再跑 `quickstart.md` §3 的「回归红线」逐条确认

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖
- **Foundational (Phase 2)**: 依赖 Setup —— **阻塞两个故事**
- **US1 / US2 (Phase 3 / 4)**: 均依赖 Foundational；两者之间**无相互依赖**，可并行
- **Polish (Phase 5)**: 依赖两个故事完成

### 阶段内顺序

- 测试任务**必须先写并确认失败**（章程原则 I）
- `download_queue.rs` 内的任务**串行**执行（T007→T008→T018→T019→T020→T032→T033→T034→T035→T036）
- 模型/类型先于服务，服务先于命令，命令先于 UI

### 跨故事的文件冲突（**不可并行**）

| 文件 | 被谁碰 |
|---|---|
| `src/types/index.ts` | T003、T007、T013 |
| `src/lib/unifiedVideoList.ts` | T012、T037 |
| `src/components/DetailPanel.tsx` | T024、T038 |
| `src/App.tsx` / `src/components/AppShell.tsx` | T014、T015、T025 |
| `src-tauri/src/services/download_queue.rs` | T007、T008、T018、T019、T020、T032–T036 |

---

## Parallel Example: User Story 1

```bash
# 先并行写三组测试（不同文件，互不冲突）：
Task: "真值表单测 in src/lib/__tests__/unifiedVideoList.test.ts"
Task: "去重语义回归测试 in src-tauri/src/commands/download.rs"
Task: "画质档次全序单测 in src/lib/__tests__/unifiedVideoList.test.ts"
# 注意：T009 与 T011 同文件 → 实为串行

# 实现期可并行的两组（不同文件）：
Task: "sync_completed_records in src-tauri/src/services/file_manager.rs"      # T016
Task: "redownloadVideo in src/lib/tauri.ts"                                   # T023
```

---

## Implementation Strategy

### MVP First（仅用户故事 1）

1. Phase 1 Setup → 2. Phase 2 Foundational（**阻塞项，不可跳过**）→ 3. Phase 3 US1 → 4. **停下按 `quickstart.md` §2.1/§2.2 独立验证** → 5. 可交付/演示

### Incremental Delivery

1. Setup + Foundational → 契约就绪
2. **+ US1** → 独立验证 → 交付（MVP）
3. **+ US2** → 独立验证 → 交付
4. + Polish

> 两个故事都是 P1，但 US1 不依赖 US2 的任何东西，**US2 是纯增量**。

---

## Notes

- **TDD**：每个故事先写测试并确认失败，再实现（章程原则 I，不可协商）
- `[P]` 只在**不同文件**时成立；同一文件的任务必须串行
- `decisions.md` 是实现期的唯一真相源；本文件与 `contracts/` 若与之冲突，**以 `decisions.md` 为准**
- 退避循环骨架**没有自动化测试**（前提 P11 的已知代价），由 T041 的 5 步人工验收兜底
- 每个任务或逻辑分组完成后提交

---

## Phase 6: Convergence

**来源**：`/speckit.converge` 在 `/speckit.implement` 完成后对 spec / plan / decisions / 现有任务的核对结果。**仅追加，未改动任何既有任务。**

- [X] T044 为 `DetailPanel` 的新增交互补 React Testing Library 用例：可升级 chip 文案（含 `best` →「可升级 最高画质」）、「文件缺失」/「已在其它订阅下载」chip、点击「升级」/「重新下载」/失败行「重试」调用 `onRedownload(record.id)`、画质下拉触发 `onUpdateQuality(id, value)`；并为 `DownloadedItem` 的「重新下载」按钮补一条点击断言（`src/components/__tests__/DetailPanel.test.tsx`、`src/components/__tests__/DownloadedItem.test.tsx`）per Constitution I / US1-AC2 / US1-AC6 / US1-AC7 (partial)
- [X] T045 关闭「spawn 成功 → 注册进 `active_tasks`」之间的取消窗口：在 `download_video_spawn` 上启用 `kill_on_drop(true)`（或在 `download_queue.rs` 的 `None => return` 分支先 `child.start_kill()`），确保取消在**任何时刻**都能中断下载、不遗留孤儿 yt-dlp 进程 per FR-012 (partial)
- [X] T046 为强调行补 decisions §8 要求的「极淡底色」（可升级 / 文件缺失·已删除 / 重试中，暗色值另调），与左 3px 色条配套（`src/components/DetailPanel.tsx`）per decisions §8 / plan: UI 形态 (partial)
