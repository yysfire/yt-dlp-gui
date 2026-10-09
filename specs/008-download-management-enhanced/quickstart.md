# Quickstart: 下载管理增强（P1）验证指南

**Feature**: 008-download-management-enhanced | **Plan**: [plan.md](./plan.md)

> 本文只写**怎么验证**。规则细节见 [decisions.md](./decisions.md)，数据结构见 [data-model.md](./data-model.md)，接口形状见 [contracts/](./contracts/)。

---

## 0. 前置条件

| 项 | 要求 |
|---|---|
| 工具链 | Node 18+ / npm、Rust stable（edition 2021） |
| yt-dlp | 已安装且可执行（`yt-dlp --version`）；**端到端手测才需要**，自动化测试**不依赖**它 |
| 已有数据 | 至少 1 个订阅、其中有 ≥1 个视频（端到端手测需要） |

---

## 1. 自动化验证（不需要 yt-dlp）

### 1.1 全量

```bash
# Rust 单元测试（含新增的 retry_policy / 存储 / 队列用例）
cargo test

# 前端类型检查 + 构建（含 fixture 的类型断言）
npx tsc --noEmit

# 前端单测（含统一列表真值表）
npm test
```

**全部必须通过**，且都必须在提交前跑（章程「测试运行」）。

### 1.2 分层验证什么

| 层 | 命令 | 覆盖 |
|---|---|---|
| 分类器 | `cargo test retry_policy` | 4 组判据顺序 + [contracts/retry-classifier.md](./contracts/retry-classifier.md) §3 的 12 个实测样本 + 「只取 ERROR 行」回归 |
| 退避/相位纯函数 | `cargo test retry_policy` | `remaining_after`（暂停冻结 / 恢复续算）、`phase_of`、`should_retry` |
| 存储 | `cargo test storage` | `deduplicate_vec` 新 rank（`retrying` 在 `downloading` 之后）、事务语义 |
| 记录契约 | `cargo test` | `to_value(基准记录)` 与 `src-tauri/tests/fixtures/download_record.json` 完全一致；`error_message = None` 时**不出现**该键 |
| 真值表 | `npm test unifiedVideoList` | `upgradeable` / `missing` / `downloadedElsewhere` 逐行断言（[decisions.md](./decisions.md) §2.4） |
| 前端契约 | `npx tsc --noEmit` | fixture `satisfies DownloadRecord`；`status` 落在联合类型内 |

---

## 2. 端到端手测（需要 yt-dlp + 真实订阅）

启动：`npm run tauri dev`

### 2.1 可升级链路（故事 1）

| 步 | 操作 | 期望 |
|---|---|---|
| 1 | 把某订阅的画质设成 `720p`，触发一次检查，让它下完一个视频 | 该行显示「已完成」 |
| 2 | 把该订阅画质改成 `1080p` | 该行出现**蓝色左竖条** + chip **「可升级 1080p」**，右侧出现按钮「升级」 |
| 3 | 点「升级」 | 该行转为「下载中」→「已完成」；**仍然只有一份文件**；记录里的画质变为 1080p |
| 4 | 再把画质改回 `720p` | **不出现**任何标记（降级不提示） |
| 5 | 找一条本次改动前就存在的旧记录 | **不出现**「可升级」（画质未知） |

### 2.2 文件缺失链路（故事 1）

| 步 | 操作 | 期望 |
|---|---|---|
| 1 | 在文件管理器里**手动删除**某个已下载视频的文件 | — |
| 2 | 等 ≤5 分钟（周期同步）或在「已下载」视图触发一次刷新 | 该行出现**红色左竖条** + chip **「文件缺失」**，按钮变为「重新下载」 |
| 3 | **不做任何操作，等一次检查周期** | 该视频**不会**被自动重新下载（这是有意设计） |
| 4 | 点「重新下载」 | 该行转「下载中」→「已完成」，缺失标记消失 |

### 2.3 自动重试链路（故事 2）

| 步 | 操作 | 期望 |
|---|---|---|
| 1 | 制造一次**可重试**失败（例如临时让代理指向不可达端口，或断网后触发下载） | 该行显示「**重试中 (1/3)**」+ **还有 N 秒**（活的倒计时） |
| 2 | 在退避等待期间点「暂停」 | 倒计时**冻结** |
| 3 | 点「恢复」 | 从**剩余时间**继续倒数 |
| 4 | 恢复网络，让它跑完 | 记录终态 `completed`，`retry_count` **保留**为实际次数 |
| 5 | 制造一次**不可重试**失败（例如把格式设成不存在的值，或对一个已删除视频下载） | **不进入退避**，直接判 `failed`，`error_message` 显示**真实原因** |

### 2.4 骨架接线的人工验收（前提 P11 的边界）

> 这 5 步**没有自动化测试** —— 它们是 `checklists/requirements.md`「人工验收」节的对应项。**必须人工过一遍**。

1. 可重试失败 → 30 秒后自动重试 → 最终成功，且 `retry_count == 1`
2. 退避期间暂停 → 计时冻结；恢复 → 从剩余时间继续
3. 退避期间取消 → **立即**中断；记录为 `cancelled`，**没有**被改写成 `failed`
4. 不可重试失败 → **不进入退避**
5. 把 yt-dlp 路径改成不存在的值 → 记录变 `failed` 且写明「无法启动」（**不再永久卡在「下载中」**）

---

## 3. 回归红线（容易踩坏的地方）

| 红线 | 怎么验 |
|---|---|
| **WARNING 行假阳性** | 用调研 §3.1 那组样本（含 `may yield HTTP Error 403` + 最终 `ERROR: ...Requested format is not available`）跑分类器，必须是 `NoRetry` |
| **陈旧 PID 发信号** | 退避期间点暂停 / 取消 / 在设置里调低并发，**不得**挂起任何无关进程；Rust 侧应由 `Option<u32>` 在编译期挡住 |
| **取消被覆盖成失败** | 退避期间取消后，最终状态必须是 `cancelled` |
| **成功时清 error_message** | 一条「失败 → 重下成功」的记录，`error_message` 必须被清空 |
| **失败重试不再造重复记录** | 同 `video_id` 在 `download_records.json` 中始终只有一条 |
| **代理凭证不落库** | 配一个带认证的代理并制造失败，检查 `download_records.json` 里**不出现** `user:pass@` |
| **既有排序不回归** | `npm test` 里 `buildUnifiedVideoList` 的既有用例全绿（新字段是正交的） |
