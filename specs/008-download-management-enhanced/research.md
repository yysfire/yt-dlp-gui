# Phase 0 Research: 下载管理增强（P1）

**Date**: 2026-10-02 | **Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

## 0. 本阶段没有派发调研代理

`Technical Context` 中**没有 `NEEDS CLARIFICATION`**。原因：本特性的不确定项已在 wayfinder 地图 [008 下载管理增强（P1：智能重复检测 + 失败自动重试）](https://github.com/yysfire/yt-dlp-gui/issues/2) 的 11 张决策票里逐条解决，其中**唯一的 `research` 票**（yt-dlp 失败输出分类）已由背景代理完成并留下一手依据：

- **一手调研全文**：`docs/research/yt-dlp-failure-classification.md` @ 分支 `origin/research/yt-dlp-retry-classification`（commit `5395461`，35157 字节，含实测 stderr 样本、跨版本核验、置信度与敏感性表）
- **分类器判据的完整匹配式清单**：[重试分类器的终态黑名单与判据优先级](https://github.com/yysfire/yt-dlp-gui/issues/10) 的结论
- **折叠后的实现真相源**：[`decisions.md`](./decisions.md)

因此本文件的作用是**归并**这些已完成的调研，按 Decision / Rationale / Alternatives 记录，供 Phase 1 与实现阶段引用。**不重复调研。**

---

## 1. yt-dlp 失败的可重试性判据

- **Decision**: 顺序判定 `终态黑名单 → HTTP 状态白名单 → 网络白名单 → 未知即重试`；**只对 stderr 中最后一条 `ERROR:` 行匹配**；退出码 `None` / `2` / `101` 一律不可重试。
- **Rationale**: 实测确认失败原因只走 stderr 的 `ERROR:` 行；退出码无错误类型语义（`0` 成功 / `1` 任意错误 / `2` 参数错 / `101` 取消）。主判据 `HTTP Error <code>:` 由 yt-dlp 自己构造、跨 extractor 一致、跨两个版本逐字一致；网络类优先匹配 yt-dlp **自有异常类名**（`TransportError` / `ProxyError` / `SSLError` / `CertificateVerifyError`），比 urllib3/requests 措辞抗漂移。
- **Alternatives considered**:
  - **纯「白名单 + 未知即重试」（原前提 P4）** —— **实测推翻**：它会把 `Requested format is not available`、私享/已删除、需登录、代理 407 这类**无状态码的终态失败**也重试 3 次（白等 210 秒）。故引入终态黑名单。
  - **用 `Giving up after N retries` 作为补刀前置条件** —— **否决**：该信号只出现在 `WARNING:` 行（与「只取 ERROR 行」冲突），且若某类失败没走 yt-dlp 自身的重试路径，信号根本不出现 → **等于静默关掉本功能**。
  - **黑名单收 `restricted`（B 站）/ `An extractor error has occurred`** —— **否决**：前者太泛会误伤「限流」这类本该重试的文案；后者是 yt-dlp 内部 bug 的兜底文案，不是语义终态。
  - **依赖 `DownloadRecord.error_message` 做分类** —— **否决**：它当前被硬编码成「代理连接失败」，不含真实原因。

**完整匹配式清单**：decisions.md §5。**回归样本**：decisions.md §5.7（调研 §3.1 的实测输出，直接当断言输入）。

## 2. 重试的落点与并发槽

- **Decision**: **就地重试** —— 循环放在 `execute_download_with_control` 内部，退避期间**继续持有 semaphore permit**。
- **Rationale**: 符合「失败后 30 秒重试」的语义；不需要给队列调度器加「定时延后」能力，队列保持纯 FIFO。
- **Alternatives considered**:
  - **释放槽位、带 `next_retry_at` 重入队** —— 需要给队列加定时语义，复杂度明显上升；本次失败稀少，占槽代价可接受。
  - **持久化「待重试」状态，下个检查周期再试** —— 属 013 的夜间模式，已出范围。

## 3. 退避的实现机制

- **Decision**: `tokio::select!` + 每个任务一个 `Arc<Notify>`；循环持**剩余时长**（不是绝对 deadline）。
- **Rationale**: 取消要求「立即中断」；持剩余时长让「暂停冻结 / 恢复续算」天然正确，不必累计已过时间。
- **Alternatives considered**: **切片轮询**（每 200ms 醒一次）—— 要么延迟最多 200ms，要么把间隔压小反而更费。

## 4. `ActiveTask` 在退避期间的表示（**安全修复**）

- **Decision**: `pid: u32` → **`Option<u32>`**（与 `child` 同步：`Some` 当且仅当 `child` 存在）；`update_max_concurrent` 缩容时**排除退避中的 entry**。
- **Rationale**: 退避期间 entry 继续存活而子进程早已退出，那个数字**可能已被系统复用给无关进程** —— 对陈旧 PID 发 SIGSTOP 会**挂起用户毫不相干的进程**。用类型让「没有 PID 却发信号」在编译期不可表示，优于加一个需要记得检查的 phase 字段。
- **Alternatives considered**: **显式 `phase: Running | BackingOff` 字段** —— 与 `child.is_none()` 表达同一件事，多一处可以忘记同步的状态。

## 5. 文件存在性的检查落点

- **Decision**: **检查路径不 `stat`**。`seen_ids` / `seen_urls` 语义逐字不变（`status != "failed"` 即算已存在），`deleted` 与「文件缺失」都留在集合内，**不自动重下**。存在性只由周期同步（`spawn_file_sync`，每 5 分钟）负责，前端派生展示，重下由用户点击触发。
- **Rationale**: 若把缺失记录排除出 `seen_ids`（= 自动重下），会触发两个灾难场景 ——（1）用户**删文件腾空间** → 检查一到就自动填满，删一次下一次，无限循环；（2）用户**移动 / 整理下载目录** → 所有 `file_path` 同时失效 → 检查会把**整个库重下一遍**。「缺失」与「失败」不同构：`failed` 没有「用户故意为之」的语义，所以 `failed` **继续**自动重下。
- **Alternatives considered**:
  - **检查时 `stat` 但结果只用于标记** —— 引入第二个存在性写者，会与周期同步得出矛盾结论。
  - **检查时 `stat` 并排除出 seen（自动重下）** —— 上述两个灾难场景。

## 6. 跨订阅去重范围

- **Decision**: **全局去重**（一个 `video_id` ↔ 一条记录 ↔ 一个文件），记录**归属于首次下载它的订阅**；B 的列表把该视频呈现为「已在其它订阅下载」，**不提供重下也不提供升级**。
- **Rationale**: 「一个文件」不是偏好，是**输出模板 `%(title)s.%(ext)s` 没有订阅判别符**的必然结果 —— 订阅 A 与 B 下同一视频本来就写同一个路径。
- **Alternatives considered**:
  - **订阅级去重（每订阅一份）** —— 物理上需要**连模板一起改**（属已出范围的「多文件」特性），本图内不可选。
  - **全局但归属可转移** —— 会让「打开文件夹 / 删除文件」的归属在订阅间漂移，收益不明。

## 7. 「可升级」判定的层次与存储形态

- **Decision**: **前端**派生（`src/lib/unifiedVideoList.ts`）；记录里存**请求的 preset 字符串**（不存实际分辨率、不存扩展名）。
- **Rationale**:
  - 前端派生：数据（记录、订阅 preset、存在性）都在前端；该模块已是纯函数且有测试打底；放后端要新增一个需两侧同步的序列化字段，而「文件存在性」在后端是异步事件，反而要把状态灌回去。
  - 存 preset：若存**实际分辨率**，用户选 1080p 而源最高只有 720p 时，会**永远误报可升级**。
- **Alternatives considered**: 后端返回派生字段 —— 上述同步与状态回灌成本；输出模板加画质判别符以支持多份 —— 已出范围。

## 8. Rust ↔ TS 类型一致性

- **Decision**: **共享 JSON fixture + 两侧各一个测试**（Rust 断言 `to_value` 与 fixture 一致；前端 `satisfies DownloadRecord`），**不加依赖**。
- **Rationale**: 两侧类型是手写断言、零校验机制。fixture 一改，两侧必有一侧变红，能真抓住「改了 Rust 忘了改 TS」。
- **Alternatives considered**:
  - **`ts-rs` / `specta` 生成** —— 为 3 个新字段引入生成链与构建步骤，维护成本不抵收益。
  - **只在 AGENTS.md 登记契约清单** —— 拦不住任何东西，只是文档。

## 9. 旧文件回收的时机

- **Decision**: **只在成功完成时**回收；顺序是**先写记录（新 `file_path` 落库），再送回收站**；路径比较用简单字符串不等。
- **Rationale**: 顺序反过来的话，「回收成功但落库失败」会丢掉旧文件引用，留下一个**记录和磁盘都没有的空洞**；当前顺序最坏只是一个无害的孤儿文件。
- **Alternatives considered**: 先回收再落库 —— 上述空洞；路径规范化处理大小写不敏感 FS —— 这里判的是「要不要回收」而非「是否重复」，误判代价只是留孤儿或早删一个旧文件。

## 10. 可测试性边界（前提 P11）

- **Decision**: 只抽纯函数，**不引入可注入的 runner**；但把边界推到极限 —— `remaining_after(phase, elapsed)` / `phase_of(child, status)` / `should_retry(...)` 全部纯函数化并单测；**人工验收只覆盖循环骨架接线**（5 步）。
- **Rationale**: 判据的正确性（分类、退避档位、相位）都是纯逻辑，可完全覆盖；循环骨架是接线。
- **Alternatives considered**: **引入 runner trait** 换取骨架的自动化测试 —— 会破坏 `download_queue.rs` 既有结构，收益不抵成本；**完全不测** —— 不必要地放弃了三个可纯化的点。
- **章程影响**: 这是对章程原则 I（TDD「所有功能」）的一处**已登记偏差**，见 plan.md 的 Complexity Tracking。
