# 008 P1 决策记录

**Feature**: `008-download-management-enhanced`
**折叠自**: wayfinder 地图 [008 下载管理增强（P1：智能重复检测 + 失败自动重试）](https://github.com/yysfire/yt-dlp-gui/issues/2) 的 11 张决策票（全部已关闭）
**折叠时间**: 2026-10-02

> 本文件把地图上**分散在各 ticket 结论评论里**的决策折叠进仓库，供 `/speckit-plan` 与实现阶段直接读取。
>
> - **实现阶段以本文件为准。** 各节末尾的「依据」给出原始 ticket 与一手源，需要完整讨论脉络时再点进去。
> - 地图与各 ticket 从此退为**历史记录**，不再更新。
> - ⚠️ 本文件**不是** spec-kit 的产物，`/speckit-plan` 不会改写它（它只生成 `plan.md` / `research.md` / `data-model.md` / `contracts/` / `quickstart.md`）。

---

## 0. 范围

**做**：用户故事 1（智能重复检测增强）+ 用户故事 2（失败自动重试）。

**不做**：故事 3（下载限速）、故事 4（代理认证）、故事 5（音频模式）；格式偏好设置；同一视频并存多画质变体（多文件 / 输出模板带画质）；夜间模式 / 时间窗口重试 / 断点续传（属 013）。详见第 11 节。

---

## 1. 记录契约

### 1.1 `DownloadRecord` 新增字段

| 字段 | 类型 | serde |
|---|---|---|
| `quality` | `String` | `#[serde(default)]`，**空串 = 未知** |
| `retry_count` | `u32` | `#[serde(default)]` |
| `last_retry_at` | `Option<String>`（ISO 8601） | `#[serde(default, skip_serializing_if = "Option::is_none")]` |

- **不存**实际分辨率、**不存**文件扩展名（扩展名从 `file_path` 后缀派生）。
- 存**请求的 preset** 而非实际分辨率：若存实际分辨率，用户选 1080p 而源最高只有 720p 时，会**永远误报可升级**。
- **`DownloadRecord::new()` 签名不变**（仍四参）—— `quality` 由调用方在构造后赋值。生产调用点只有 2 处（`services/download_queue.rs:245` 活的；`commands/download.rs:263` 是无队列兜底、实际走不到）。
- `DownloadTask` 另加 `record_id: String` 与 `next_retry_at: Option<String>`（见第 6、7 节）。

### 1.2 写入与重置时机

| 字段 | 写入 | 归零 / 清理 |
|---|---|---|
| `quality` | **入队时** | 重下时覆盖为新 preset |
| `retry_count` | 每次重试尝试前 +1 | **新一轮下载开始时归零**（入队 / 重下）；**成功时保留**（spec 场景 2 要求「重试 2 次后记录为 2」） |
| `last_retry_at` | 每次重试尝试前 | 新一轮下载开始时清 `None` |
| `error_message` | 失败时写（真实 stderr 摘要） | **成功时清空**（修掉「失败过、后来重下成功却一直挂着旧错误」） |
| `downloaded_at` | **只在真正成功完成时写** | 暂停 / 取消 / 重试不再改写它 |

### 1.3 `deduplicate_vec` 的 rank 表

**去重键与整体策略不改，只扩 rank 表**（前提 P1 那句「`deduplicate_vec` 不改」指的是**键**）。

`completed=0, downloading=1, retrying=2, failed=3, paused=4, cancelled=5`，其余（`waiting` / `deleted` / 未知）落 `9`。同状态内仍取最新 `downloaded_at`。

不登记 `retrying` 会让它落兜底 9（最低），同 `video_id` 有多条历史记录时，**正在重试的记录会被 `failed` 挤掉**。

### 1.4 Rust ↔ TS 一致性

两侧类型是**手写断言、无校验机制**。兜底方案：**共享 JSON fixture + 两侧各一个测试**，不加依赖。

- 基准文件 `src-tauri/tests/fixtures/download_record.json`（目录已存在但为空），内容为字段填满的 `DownloadRecord`。
- Rust 侧 `include_str!` 读同一文件，断言 `serde_json::to_value(基准记录) == 解析出的 fixture`。
- 前端侧 vitest 用 `node:fs` 读**同一个文件**，`satisfies DownloadRecord` 做编译期校验（`npm run build` 的 `tsc --noEmit` 覆盖）。
- 改 fixture 时两侧必有一侧变红。

### 1.5 兼容性

- **向后**：三个新字段全部 `#[serde(default)]`，旧 `download_records.json` 正常加载；`quality` 空 → 永不判升级；`retry_count = 0`；`last_retry_at = None`。**不做数据迁移**。
- **向前**：`DownloadRecord` 无 `deny_unknown_fields`，旧版本读到新字段静默忽略；代价是新字段在旧版本下一次写入时丢失。**接受**，不引入版本号。

### 1.6 派生计数口径（两套并存，不得互相替换）

- **记录总数** = `records.length`（含 `deleted`）→ 侧边栏徽标、「已下载」视图标题。
- **已完成数** = `status === "completed"` → 状态栏、详情面板头部。
- `retrying` **计入**总数、**不计入**已完成数。

**依据**：[DownloadRecord 扩展的序列化契约与前端类型同步](https://github.com/yysfire/yt-dlp-gui/issues/5)（前提 P7）

---

## 2. 画质档次与「可升级」判定

### 2.1 全序

`480p < 720p < 1080p < 1440p < 2160p < best`

- `best`（无高度上限）**高于** `2160p`。
- **未知值永不参与比较**（其它串 / 空串 / `audio` 一律视为未知）。方向：**宁可漏报，不可对看不懂的值天天提示**。
- 取值域现状：后端只认 `best / 2160p / 1440p / 720p / 480p`，其余落 1080p（`services/ytdlp.rs:526-533`）；界面只给 `best / 1080p / 720p / 480p / audio`。因此 `1440p` / `2160p` 只能由 API 或手改文件产生。

### 2.2 判定输入

`record.quality`、`record.status`、`subscription.quality_preset`、`missing`（文件是否存在）。**`AppSettings.quality_preset` 不参与判定**（它只是新建订阅的默认值）。

### 2.3 派生落点：前端

`src/lib/unifiedVideoList.ts`，签名扩为：

```ts
buildUnifiedVideoList({ videos, records, tasks, qualityPreset, missingPaths, subscriptionId })
```

`UnifiedVideoItem` 新增三个**正交**字段：`upgradeable: boolean`、`missing: boolean`、`downloadedElsewhere: boolean`。**不新增状态值、不动 `statusPriority` 的语义**（`retrying` 除外，见第 6 节）。

「文件缺失」的数据管道：把 `missingPaths` 从 `DownloadedList` 的组件本地状态**提升到 `App.tsx`**（它已是 `file-sync-complete` 的天然汇聚点），再下发给 `DetailPanel` 与 `DownloadedList`。

### 2.4 真值表

规则**按序**执行：

1. 无 `record` → 三个标记都 false
2. 有活跃 `queueTask` → 都 false
3. `record.status != "completed"` → 都 false
4. `missing == true` → `missing = true`，`upgradeable = false`（**缺失优先于升级**）
5. `record.quality` 未知 或 订阅 preset 未知 → `upgradeable = false`
6. `rank(订阅) > rank(记录)` → `upgradeable = true`；`==` 或 `<` → false（**降级不提示**）

| record.status | record.quality | 订阅 preset | missing | 展示 | upgradeable |
|---|---|---|---|---|---|
| completed | `720p` | `1080p` | false | 已完成 +「可升级 1080p」 | **true** |
| completed | `720p` | `best` | false | 已完成 +「可升级 最高画质」 | **true** |
| completed | `2160p` | `best` | false | 已完成 + 可升级 | **true** |
| completed | `720p` | `1080p` | **true** | 「文件缺失」+ 重新下载 | false |
| completed | `1080p` | `1080p` | false | 已完成 | false |
| completed | `1080p` | `720p` | false | 已完成（降级） | false |
| completed | `best` | `2160p` | false | 已完成（降级） | false |
| completed | （空） | `1080p` | false | 已完成（画质未知） | false |
| completed | `720p` | （空/未知） | false | 已完成（不判） | false |
| completed | `audio` | `1080p` | false | 已完成（未知档） | false |
| failed / cancelled / downloading / paused / retrying / waiting | 任意 | 任意 | — | 对应状态 | false |
| deleted | `720p` | `1080p` | true | 已删除 + 重新下载 | false |
| （无 record） | — | — | — | 新视频 / 等待中 | false |

**多订阅同一视频**：判定**只看本订阅的记录**（`DetailPanel` 收到的记录已按 `subscription_id` 过滤）。

### 2.5 前提 P12：订阅级画质设置的 UI 进入 P1

- 复用已存在的 `update_subscription_quality` 命令（`commands/subscription.rs:128`）与 `useSubscriptions.updateQuality` **接上 UI** —— 现状**没有任何组件调用它**，故事 1 场景 2「用户升级画质设置」因此无法触达。
- 把死设置 `AppSettings.quality_preset`（后端**从不读取**）接上，作为新建订阅的默认画质，替代 `Subscription::new` 里硬编码的 `1080p`。

**依据**：[画质档次全序与「可升级」判定的真值表](https://github.com/yysfire/yt-dlp-gui/issues/4)

---

## 3. 去重范围与归属

- **全局去重**：一个 `video_id` ↔ 一条记录 ↔ **一个文件**。
- **物理必然性**：输出模板是 `%(title)s.%(ext)s`（`services/ytdlp.rs:522`），**没有订阅判别符** —— 订阅 A 与 B 下同一视频本来就写**同一个路径**。「每订阅各留一份」必须连模板一起改（属已出范围的「多文件」特性）。
- **归属**：`subscription_id` 是记录的**归属** —— 归属于**首次下载它的订阅**。
- **B 的列表**：该视频显示为「**已在其它订阅下载**」，**不提供重下、不提供升级**（否则与第 2 节「判定只看本订阅记录」冲突）。视觉复用第 8 节的 chip 惯用法。
- **启动时的 `deduplicate_records()` 保留**（`lib.rs:78`，事务内持久化）：全局模型下「同 `video_id` 多条」本就不该存在。**已知限制**：两条记录若指向不同 `file_path`，被删的那条会留下**孤儿文件**（只记录、不处理）。
- **upsert 键维持 `(video_id, subscription_id)`**：`subscription_id` 是归属，带它能防「重试 B 的失败记录时误把 A 的完成记录重置掉」。在全局 `seen_ids` 下 B 根本走不到入队，所以不影响全局唯一。

**依据**：[跨订阅同一视频的去重范围](https://github.com/yysfire/yt-dlp-gui/issues/11)

---

## 4. 文件存在性

### 4.1 枢轴结论：检查路径**不 `stat`**

`check_and_download` 的 `seen_ids` / `seen_urls` 构造**逐字不变**（`status != "failed"` 即算「已存在」）—— `deleted` 与「文件缺失」的记录都**留在集合内**，**不自动重下**。

存在性只由**周期同步一条路径**负责：`spawn_file_sync`（每 5 分钟）→ `file-sync-complete` → 前端派生 `missing` → 用户点「重新下载」。

### 4.2 为什么不自动重下

1. 用户**手动删文件腾空间** → 检查一到就自动填满，删一次下一次，**无限循环**；
2. 用户**移动 / 整理下载目录** → 所有 `file_path` 同时失效 → 检查会把**整个库重下一遍**，磁盘翻倍；
3. 带宽在无征询下被消耗。

**「缺失」与「失败」不同构**：`failed` 无「用户故意为之」的语义，所以 `failed` **继续**自动重下（继续被排除出 seen 集合）。

顺带确认既有的正确语义：`cancelled` 与 `paused` 都留在 seen 集合 → 不会在下次检查被自动重入队（符合 spec 故事 2「手动取消后不自动重试」）。

### 4.3 更新版去重判定（伪代码）

```text
seen_ids  = { r.video_id  | r.status != "failed" && !r.video_id.is_empty() }
seen_urls = { r.video_url | r.status != "failed" }

for video in videos:
    vid = video.id or ""
    if vid != "" and vid in seen_ids:  continue
    if vid == "" and video.url in seen_urls:  continue
    enqueue_from_video(...)          # 见第 7 节：upsert 重置
```

### 4.4 「文件缺失」纯派生，不落库

记录状态保持 `completed`，**不新增持久化标记**。理由：一次 `stat` 的结论随时会变（文件放回来、外接盘重挂），落库就要连带设计「恢复」的状态往返。

### 4.5 存在性实现收敛成一处

新增共享函数 `services/file_manager.rs::sync_completed_records(data_dir) -> Vec<FileExistenceResult>`（筛 `completed` + 非空 `file_path` → `check_files_exist` → emit `file-sync-complete`）。`spawn_file_sync` 与 `commands/file_manager.rs::sync_file_states` **都改为调它**（现状是两处各自内联了一遍，必然漂移）。两个入口都保留：周期任务后台兜底、命令供前端主动刷新。

### 4.6 「更改下载目录」明确划出范围

不做路径重定位。文档写明：修改 `download_dir`（或整体移动目录）后旧记录的 `file_path` 失效，会显示为「文件缺失」，需**手动**重新下载。

**依据**：[检查路径里的文件存在性校验与 seen 集合语义](https://github.com/yysfire/yt-dlp-gui/issues/9)（**修订前提 P5**）

---

## 5. 重试分类器

### 5.1 契约（纯函数，零 I/O）

```rust
pub enum RetryDecision { Retry, NoRetry, Unknown }

/// 剥 ANSI 转义 + 只保留 ERROR 行（去前导空白与 [debug] 前缀）
pub fn error_lines(stderr: &str) -> Vec<String>;

pub fn classify_failure(stderr: &str, exit_code: Option<i32>) -> RetryDecision;
```

调用方：`Retry` **与 `Unknown`** 都执行重试；`NoRetry` 直接判失败。

### 5.2 判定顺序

```text
match exit_code:
  None      -> NoRetry     # spawn 失败（路径/权限）或 被信号杀死
  Some(0)   -> NoRetry     # 成功不该进分类器
  Some(2)   -> NoRetry     # 命令行参数错误
  Some(101) -> NoRetry     # DownloadCancelled
  _         -> continue    # Some(1) 及其它交给文本

lines = error_lines(stderr);  if empty -> Unknown
text  = lines.last()                       # 最后一条 ERROR 行即权威原因

1. text 命中 TERMINAL  -> NoRetry
2. text 命中 (?i)tunnel connection failed:\s*(\d{3})  -> status_decision(code)
3. text 命中 (?i)http error\s+(\d{3})\s*:             -> status_decision(code)
4. text 命中 NETWORK   -> Retry
5. 兜底                -> Unknown

status_decision(code) = Retry if code in {408,429} or 500..=599 else NoRetry
```

**⚠️ 只取最后一条 `ERROR:` 行不是优化，是必须。** `WARNING:` 行里会**合法出现** `HTTP Error 403`（YouTube 的「这些格式可能产生 403，已跳过」提示），对整段 stderr 做子串匹配会把「格式不可用」误判成 403。

### 5.3 `TERMINAL` 黑名单（大小写不敏感）

`Requested format is not available` · `Unsupported URL` · `This video is DRM protected` · `This video is only available for registered users` · `Private video` · `Video unavailable` · `This video is not available` · `Sign in to confirm` · `not available in your country`

- `Use --list-formats` 不必单列（总与 `Requested format is not available` 同行）。
- **明确不收**：`restricted`（太泛，会误伤限流这类**本该重试**的文案）、`An extractor error has occurred`（yt-dlp 内部 bug 的**兜底文案**，不是语义终态；收进去等于把一大类未知错误判死）。
- YouTube 三条词由服务端提供、版本敏感，但**错的方向是安全的**：措辞一变就退化成「未知 → 重试」，只多等 210 秒，不会误杀。

### 5.4 `NETWORK` 白名单

**首选**：`TransportError` · `ProxyError` · `SSLError` · `CertificateVerifyError`（yt-dlp **自有异常类名**，比 urllib3 / requests 的内部措辞抗版本漂移）。

其余：`Failed to establish a new connection` · `Failed to resolve` · `Name or service not known` · `Temporary failure in name resolution` · `Network is unreachable` · `Connection reset by peer` · `Connection aborted` · `RemoteDisconnected` · `IncompleteRead` · `Read timed out` · `connect timeout=` · `timed out` · `Unable to connect to proxy` · `SocksHTTPConnection`

- **不收** `Giving up after N retries`（见 5.5）。
- **代理 407 的缺口**：它**不是** `HTTP Error 407`，而是 `Tunnel connection failed: 407`，必须靠第 2 步识别。
- **SOCKS5 不可达更极端**：实测**完全不含 `proxy` 字样**（被判成 `TransportError`），靠白名单兜住。

### 5.5 `Giving up after N retries` **不作**补刀前置条件

它只出现在 `WARNING:` 行（与「只取 ERROR 行」冲突，需破例）；更致命的是：若某类失败没走 yt-dlp 自身的重试路径，该信号**根本不出现**，应用层就永远不补刀 —— **等于静默关掉本功能**。

放大代价已评估并接受：提取阶段走 `--extractor-retries`（默认 3），最坏约 4 × 4 ≈ 16 次请求分散在 4 分多钟里。日后若成问题，单独下调 `--retries` 即可，不需要现在引入这条耦合。

### 5.6 B 站 412 / 352 不单列

落到「未知 → 重试」，结果与单列白名单**相同**；单列只多一处会随站点文案漂移的字符串。

### 5.7 回归样本（直接当断言输入）

调研文档 §3.1 的实测样本：HTTP 404（`generic` 与 `BiliBili` 两条）· DNS 失败 · **代理 407**（验证「无 `HTTP Error 407`」缺口）· **SOCKS5 不可达**（验证「无 `proxy` 字样」）· 格式不可用 · **yt-dlp 自行重试耗尽那组**（同一份 stderr 里既有 WARNING 假阳性 403、又有最终 ERROR —— 验证「只取 ERROR 行」真的生效）。

一手依据：`docs/research/yt-dlp-failure-classification.md` @ 分支 `origin/research/yt-dlp-retry-classification`。

**依据**：[重试分类器的终态黑名单与判据优先级](https://github.com/yysfire/yt-dlp-gui/issues/10)（**定稿并替代前提 P4**）

---

## 6. 重试循环与并发 / 暂停 / 取消

### 6.1 `ActiveTask` 的表示与 **PID 安全**

```rust
struct ActiveTask {
    child: Option<tokio::process::Child>,  // 退避期间为 None
    pid: Option<u32>,                      // 与 child 同步：Some 当且仅当 child 存在
    task: DownloadTask,                    // status 反映当前阶段
    last_progress_percent: f32,
    notify: Arc<tokio::sync::Notify>,      // 打断退避
}
```

**`pid: u32` → `Option<u32>` 是安全修复，不是风格偏好。** 退避期间 entry 继续存活而子进程早已退出，那个数字**可能已被系统复用给无关进程** —— 对陈旧 PID 发 SIGSTOP 会**挂起用户毫不相干的进程**。

**所有发信号处必须在 `Some(pid)` 分支内**：`pause` / `resume` / `pause_by_url` / `cancel` / `update_max_concurrent`。其中 **`update_max_concurrent` 缩容时按进度挑任务暂停的逻辑，必须排除 `child.is_none()`（退避中）的 entry**。

### 6.2 退避实现：`tokio::select!` + `Notify`

```text
loop:
    select! { sleep(remaining) => {}, notify.notified() => {} }

    if active_tasks 里已无本 entry:  return          # 取消：不回写任何东西
    if task.status == Paused:        等下一次 notify 再 continue（remaining 不变）
    if attempt >= 3:                 写 failed（最后一条 ERROR 行摘要）; return
    attempt += 1; 写 retry_count / last_retry_at / status=retrying
    remaining = BACKOFF[attempt]                      # 30 / 60 / 120
    重新 spawn → 回到正常执行
```

- 循环持**剩余时长**（不是绝对 deadline），暂停冻结 / 恢复续算天然正确。
- **取消后循环绝不回写记录**：`cancel` 已写过 `cancelled` + 「Cancelled by user」，循环若再写一次会把取消**覆盖成失败**。
- 硬编码常量：最多 3 次，间隔 30 / 60 / 120 秒（**不提供设置项**，`RetryConfig` 实体已删除）。

### 6.3 状态转移表

| 触发 | `DownloadRecord.status` | `ActiveTask` |
|---|---|---|
| 入队 | `downloading` | 无（任务在 `queue` 里，`Waiting`） |
| 开始执行 | `downloading` | `Running`，`child=Some`，`pid=Some` |
| 失败 · 可重试 · 未达上限 | **`retrying`** | `BackingOff`：`child=None`，`pid=None`，`next_retry_at=Some` |
| 退避到点 → 再次 spawn | `retrying` | `BackingOff` → `Running` |
| 重试成功 | `completed` | entry 移除 |
| 达上限仍失败 | `failed` | entry 移除 |
| **不可重试**的失败 | `failed` | entry 移除（**不经过退避**） |
| **spawn 失败** | `failed` | entry 移除（补写，见 6.4） |
| 取消（`Running`） | `cancelled` | entry 移除 + kill child |
| 取消（`BackingOff`） | `cancelled` | entry 移除 + `notify`；循环醒来发现 entry 不存在 → 退出且不回写 |
| 暂停任务（`Running`） | `paused` | `Running` + SIGSTOP |
| 暂停任务（`BackingOff`） | `paused` | 冻结 `remaining`，等恢复 |
| 恢复任务（`Running`） | `downloading` | `Running` + SIGCONT |
| 恢复任务（`BackingOff`） | `retrying` | 用剩余 `remaining` 重新计时 |
| 应用退出（任何在途） | `failed` | — |
| **暂停订阅** | **不变** | **不变** —— 暂停只让检查跳过该订阅，**不中断**已入队下载（前提 P10） |

`statusPriority`（前端排序）插入 `retrying`：`downloading=0, retrying=1, waiting=2, paused=3, completed=4, new=5, cancelled=6, failed=7`。

### 6.4 spawn 失败与失败归因

- **spawn 失败**（yt-dlp 路径不存在 / 无执行权限）：现状是 `download_video_spawn` 失败后**直接 `return`，连 `failed` 都不写**，记录永久停在 `downloading`。**必须补**：判为**不可重试**，写 `failed` + `error_message = "无法启动 yt-dlp：<io error>"`。
- **`error_message` 改为从最后一条 `ERROR:` 行提取的可读摘要**，截断 **200 字符**（与既有 `unparsable_line_error` 口径一致）。取代现状「只要配了代理就写『代理连接失败』」的硬编码猜测。
- **分类结果只用于控制流，不落库。**
- 🔒 **落库前必须剥离代理凭证**（`user:pass@` 部分）—— `error_message` 现在来自真实 stderr，而 `proxy_url` 可能含明文口令，不得写进 `download_records.json` 或显示在 UI。

### 6.5 stderr 读取

stderr 现在是 `Stdio::piped()` 却**从未读取** —— 管道写满（约 64 KiB）会让 yt-dlp **阻塞在写 stderr 上**，表现为「下载卡死」。必须与 stdout **并发**读（两个独立 task）；**只保留尾部 64 KiB**（超出丢弃最早的），让内存有界。循环结束后把 stderr 与 `ExitStatus::code()` 一起交给 `classify_failure`。

### 6.6 退出 / 重启

`recover_state` 把 `downloading` / `paused` **一并纳入 `retrying`**，全部置 `failed` + `error_message = "Application restarted"`。**不主动恢复重试。**

顺带好处：置成 `failed` 后，下次检查会自动重新入队（`seen_ids` 排除 `failed`），由第 7 节的 upsert 复用同一条记录 —— **不需要任何新机制**。

**依据**：[重试循环与 pause/cancel/并发槽的交互契约](https://github.com/yysfire/yt-dlp-gui/issues/7)

---

## 7. 命令契约与复用记录

### 7.1 命令

```rust
#[tauri::command]
pub async fn redownload_video(
    record_id: String,
    queue_ctx: State<'_, QueueContext>,
    state: State<'_, AppContext>,
) -> Result<DownloadRecord, String>
```

- 后端据 `record_id` 查记录 → 查其订阅 → 取**订阅当前的** `quality_preset` → 走统一入队收口。
- **不区分「升级」与「重新下载」** —— 二者行为完全一致，差别只在 UI 展示理由。
- 错误：记录 / 订阅不存在 → `AppError::NotFound`；队列未初始化 → `"Download queue not initialized"`。

### 7.2 入队路径：`enqueue_from_video` 改为 **upsert 重置**

键 = `(video_id, subscription_id)`。命中 → 在 `update_download_records` 事务内**重置**；未命中 → 新建。

重置：`status → "downloading"`、`error_message → None`、`quality → 订阅当前 preset`、`retry_count → 0`、`last_retry_at → None`；**保留 `file_path` / `file_size`**（旧路径还要用于回收）与 **`downloaded_at`**（完成时才更新）。

**顺带修掉的既有 bug**：`seen_ids` 排除 `failed`，而 `enqueue_from_video` 只新建 —— 所以「失败 → 下次检查重试」今天会给同一 `video_id` **再插一条新记录**。upsert 之后检查路径不再制造重复。

### 7.3 回写定位改用 `record_id`

`DownloadTask` 新增 `record_id: String`；完成回写、失败回写、`update_record_status` **全部按 `record_id` 精确定位**，删除 `(video_url, subscription_id)` 过滤（它现在会匹配**所有**同键记录）。`downloaded_at` **只在真正成功完成时写**。

### 7.4 旧文件回收

- **只在成功完成时**回收；失败 / 取消**绝不动**任何文件。
- **顺序：先写记录（新 `file_path` 落库），再 `delete_file_to_trash(旧路径)`。** 反过来的话，「回收成功但落库失败」会丢掉旧文件引用，留下一个记录和磁盘都没有的空洞。
- 条件：旧 `file_path` 非空 **且** 与新路径**字符串不等**。
- `services/file_manager.rs::delete_file_to_trash` 由私有改 `pub(crate)`。

### 7.5 幂等守卫放在入队这一个收口

入队前若「`queue` 待处理或 `active_tasks` 里已有同 `record_id` 的任务」**或**「记录状态 ∈ `{waiting, downloading, retrying}`」→ **不重复入队**，直接返回当前记录。放这里，检查路径与命令路径拿到的是**同一份**保证。

### 7.6 前端调用点

- `src/lib/tauri.ts` 加 `redownloadVideo(recordId)`。
- `DetailPanel` 的**「升级」**、**「重新下载」**，以及**失败行的「重试」**——**全部改调它**（行内按钮只重下这一条）。「重新检查整个订阅」只保留在工具栏。
- `DownloadedList` 复用同一封装与文案。
- 事件：写记录 → `notify_records_changed`；入队 → `emit_queue_changed`。

**依据**：[升级/重下的命令契约与复用记录的入队路径](https://github.com/yysfire/yt-dlp-gui/issues/8)

---

## 8. UI 形态

- **行容器**：`ListItem` 左 3px 竖条 —— 可升级 `primary.main` / 缺失·已删除 `error.main` / 重试中 `warning.main`；非强调行用 `transparent` 占位，**避免行宽跳动**。配套极淡底色（暗色值实现时另调）。
- **徽标用 chip**（不新增状态值）：可升级显示「**可升级 1080p**」（`best` → 「可升级 最高画质」）；缺失显示「**文件缺失**」；B 的列表显示「**已在其它订阅下载**」。缺失与可升级**互斥，缺失优先**。
- **文案**：chip 带目标档次；按钮短 —— 「**升级**」/「**重新下载**」。
- **动作入口**：留在现有右侧操作列，与既有 暂停 / 取消 / 重试 **同位**（行内按钮），**不引入悬浮菜单**。按钮 `title` 用原生属性（`升级到 1080p` / `重新下载`），便于 `getByTitle` 断言。
- **重试行**：状态文字「重试中 (2/3)」+ 紧随「· 还有 12 秒」（活倒数）。**倒数只在详情面板**，列表视图不加（避免每行挂一个计时器）。
- **`DownloadedList`** 复用同一套文案与同一命令。

原型素材（含未采用的变体 C）：`origin/prototype/upgrade-badge` @ `b9c33cf`。

**依据**：[详情面板「可升级」徽标与升级入口的原型](https://github.com/yysfire/yt-dlp-gui/issues/6)

---

## 9. 前提（standing decisions）

| # | 决定 |
|---|---|
| P1 | **替换模型**：一个 `video_id` ↔ 一条记录 ↔ 一个文件；去重范围全局；记录归属首次下载它的订阅。 |
| P2 | **就地重试**：循环在 `execute_download_with_control` 内部，退避期间**持有并发槽**。 |
| P3 | **重试参数硬编码**：3 次 / 30·60·120 秒；`RetryConfig` 实体已删除。 |
| P4 | 分类判据 —— 见第 5 节（**已被 #10 定稿替代**）。 |
| P5 | 文件存在性 —— 见第 4 节（**已被 #9 修订**）。 |
| P6 | **状态集**：只有「已下载 / 可升级」两种派生状态；「文件缺失」「已删除」是事实性标记。 |
| P7 | 记录字段 —— 见第 1 节。 |
| P8 | **复用记录**：升级 / 重下复用同一条记录，不删旧建新。 |
| P9 | **重试期状态**：新增 `retrying`；前端加 `UnifiedVideoStatus` 与 `statusPriority` 一项。 |
| P10 | **控制语义**：取消立即中断；暂停任务冻结计时钟；暂停订阅**不**中断已入队下载。 |
| P11 | **可测试性**：只抽纯函数，**不引入 runner trait**；循环骨架靠人工验证。 |
| P12 | **订阅级画质设置进 P1** —— 见 2.5。 |

---

## 10. 验收与测试落点

| 原 SC | 处置 |
|---|---|
| 重复检测准确率 > 99% | **删除**统计性断言 → `unifiedVideoList` 的**真值表逐行单测**（第 2.4 节的 13 行 + `missing` 优先 + `downloadedElsewhere`） |
| 重试使成功率提升 20% | **删除**统计性断言 → **分类器对第 5.7 节实测样本的 Rust 单测** |
| 限速偏差 < 10%（故事 3） | **删除**（已出范围） |
| 代理成功率 95%（故事 4） | **删除**（已出范围） |
| 音频体积小 80%（故事 5） | **删除**（已出范围） |
| —— | **新增** Rust↔TS 契约 fixture 测试（第 1.4 节） |

**原则**：统计性断言一律换成**存在性断言** —— 仓库**没有任何跑真实 yt-dlp 的集成测试**，百分比既不可复现也不可判定。

**P11 的边界（把能纯化的都纯化）**：额外抽出纯函数 `remaining_after(phase, elapsed)`、`phase_of(child, status)`、`should_retry(...)`；**人工验收只覆盖骨架接线**（spawn / `select!` / 被唤醒后分派），共 5 步，见 `checklists/requirements.md` 的「人工验收」节。

完整可勾选清单：`checklists/requirements.md`。

**依据**：[008 P1 的验收可执行口径](https://github.com/yysfire/yt-dlp-gui/issues/12)

---

## 11. 超出范围

- 用户故事 3 下载限速 / 故事 4 代理认证与「测试连接」/ 故事 5 音频模式 —— 各自独立推进。
- 「格式偏好」设置（属故事 5；P1 内没有格式偏好，所以「格式差异导致的可重下载」不成立）。
- 同一视频并存多画质变体（多文件 / 输出模板带画质）。
- 013 的夜间模式 / 时间窗口重试 / 断点续传。
- 更改下载目录后的路径重定位（见 4.6）。
- 代理密码的 Base64 存储等旧清单条目（随故事 4 一并出范围）。
