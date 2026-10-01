# Contract: 重试策略（纯函数，Rust）

**Feature**: 008-download-management-enhanced | **Plan**: [../plan.md](../plan.md)

模块：`src-tauri/src/utils/retry_policy.rs`（与既有 `utils/progress_parser.rs` 同层同性质：无状态、纯函数、可单测）

---

## 1. 公开接口

```rust
pub enum RetryDecision { Retry, NoRetry, Unknown }

/// 剥 ANSI 转义 + 只保留 `ERROR:` 行（去前导空白与 `[debug]` 前缀）
pub fn error_lines(stderr: &str) -> Vec<String>;

/// 把一次失败映射为「可重试 / 不可重试 / 无法判定」
pub fn classify_failure(stderr: &str, exit_code: Option<i32>) -> RetryDecision;

/// 退避档位表（硬编码常量，不在设置中暴露）
pub const BACKOFF_SECS: [u64; 3] = [30, 60, 120];
pub const MAX_ATTEMPTS: u32 = 3;

/// 暂停 / 恢复后剩余的退避时长
pub fn remaining_after(phase: Phase, elapsed_secs: u64) -> u64;

/// 由子进程与任务状态推出当前相位
pub fn phase_of(child_present: bool, status: &str) -> Phase;

/// 分类结果 + 已尝试次数 → 是否继续重试
pub fn should_retry(decision: &RetryDecision, attempts: u32) -> bool;
```

**调用方约定**：`Retry` **与 `Unknown`** 都执行重试；`NoRetry` 直接判失败。

---

## 2. 判定契约

判定顺序与逐条匹配式见 [../decisions.md](../decisions.md) §5.2–§5.4。要点（**实现时不得违反**）：

1. **只读最后一条 `ERROR:` 行**。`WARNING:` 行里会合法出现 `HTTP Error 403`（YouTube 的「这些格式可能产生 403，已跳过」提示），对整段 stderr 做子串匹配会误判。
2. 顺序：退出码短路 → 终态黑名单 → 代理隧道状态码 → HTTP 状态 → 网络白名单 → 兜底 `Unknown`。
3. 代理 407 **不是** `HTTP Error 407`，而是 `Tunnel connection failed: 407`。
4. SOCKS5 不可达**不含 `proxy` 字样**，靠网络白名单兜住。
5. `Giving up after N retries` **不作**前置条件。
6. 黑名单**宁窄勿宽**，白名单**宁宽勿窄**。

---

## 3. 回归样本（测试输入，直接来自实测）

| 样本 | 期望 |
|---|---|
| `ERROR: [generic] ... HTTP Error 404: Not Found ...` | `NoRetry` |
| `ERROR: [BiliBili] ... HTTP Error 404: Not Found ...` | `NoRetry` |
| `ERROR: ... HTTP Error 503: Service Unavailable ...` | `Retry` |
| `ERROR: ... HTTP Error 408: Request Timeout ...` | `Retry` |
| `ERROR: ... HTTP Error 429: Too Many Requests ...` | `Retry` |
| `ERROR: ... Failed to resolve '...' ([Errno -2] Name or service not known) ...` | `Retry` |
| `ERROR: ... 'Tunnel connection failed: 407 Proxy Authentication Required') ...` | `NoRetry` |
| `ERROR: ... SocksHTTPConnection(...): Failed to establish a new connection ...` | `Retry` |
| `ERROR: [BiliBili] ...: Requested format is not available. Use --list-formats ...` | `NoRetry` |
| 同一份 stderr：含 `WARNING: ... may yield HTTP Error 403` **与**最终 `ERROR: ...Requested format is not available` | `NoRetry`（验证「只取 ERROR 行」） |
| `exit_code = None`（spawn 失败 或 信号终止） | `NoRetry` |
| `exit_code = Some(2)` / `Some(101)` | `NoRetry` |

**完整一手样本**：`docs/research/yt-dlp-failure-classification.md` §3.1 @ 分支 `origin/research/yt-dlp-retry-classification`。

---

## 4. 不变量

1. 纯函数：**零 I/O、零全局状态、不改入参**。
2. `classify_failure` 的返回值**只用于控制流**，**不落库**。
3. 退避参数**硬编码**，不经 `AppSettings`（见 plan.md 的 Complexity Tracking）。
4. 调用方**不得**用 `error_message` 做分类输入（该字段是给人看的摘要）。
