//! 失败归因与重试策略 —— **纯函数**模块（零 I/O、零全局状态、不改入参）。
//!
//! 与既有 `utils/progress_parser.rs` 同层同性质。所有判据都集中在
//! [`classify_failure`]，循环骨架（`services/download_queue.rs`）只负责接线。
//!
//! 规则的唯一真相源是 `specs/008-download-management-enhanced/decisions.md` §5。
//! 关键约束（实现时不得违反）：
//!
//! 1. **只读最后一条 `ERROR:` 行** —— `WARNING:` 行里会合法出现
//!    `HTTP Error 403`（YouTube 的「这些格式可能产生 403，已跳过」提示），
//!    对整段 stderr 做子串匹配会把「格式不可用」误判成 403。
//! 2. 顺序：退出码短路 → 终态黑名单 → 代理隧道状态码 → HTTP 状态 → 网络白名单 → 兜底 `Unknown`。
//! 3. 代理 407 **不是** `HTTP Error 407`，而是 `Tunnel connection failed: 407`。
//! 4. SOCKS5 不可达**不含 `proxy` 字样**，靠网络白名单兜住。
//! 5. 黑名单**宁窄勿宽**，白名单**宁宽勿窄**（错的方向是安全的：措辞一变退化成
//!    「未知 → 重试」，只多等 210 秒，不会误杀）。

/// 一次失败的可重试性判定。
///
/// 调用方约定：`Retry` **与 `Unknown`** 都执行重试；`NoRetry` 直接判失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    Retry,
    NoRetry,
    Unknown,
}

/// 退避档位表（秒）。**硬编码**，不提供设置项（见 plan.md 的 Complexity Tracking）。
pub const BACKOFF_SECS: [u64; 3] = [30, 60, 120];

/// 单次下载允许的最大重试次数。
pub const MAX_ATTEMPTS: u32 = 3;

/// 一次下载在某一时刻所处的相位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 子进程在跑。
    Running,
    /// 失败后的退避等待（子进程已退出）。
    BackingOff,
    /// 退避期间被用户暂停（计时冻结）。
    Paused,
}

/// 终态黑名单（大小写不敏感的子串）。
///
/// **宁窄勿宽**：`restricted`（太泛，会误伤限流这类本该重试的文案）与
/// `An extractor error has occurred`（yt-dlp 内部 bug 的兜底文案，不是语义终态）
/// 都**明确不收**。
const TERMINAL_MARKERS: &[&str] = &[
    "requested format is not available",
    "unsupported url",
    "this video is drm protected",
    "this video is only available for registered users",
    "private video",
    "video unavailable",
    "this video is not available",
    "sign in to confirm",
    "not available in your country",
];

/// 网络类白名单（大小写不敏感的子串）。
///
/// **宁宽勿窄**：首选 yt-dlp **自有异常类名**（比 urllib3 / requests 的内部措辞抗
/// 版本漂移），其余为常见的连接/DNS/TLS/代理措辞。**不含**
/// `Giving up after N retries`（它只在 WARNING 行出现，且不是所有失败都走 yt-dlp
/// 自身重试路径，用它当补刀前置条件等于静默关掉本功能）。
const NETWORK_MARKERS: &[&str] = &[
    "transporterror",
    "proxyerror",
    "sslerror",
    "certificateverifyerror",
    "failed to establish a new connection",
    "failed to resolve",
    "name or service not known",
    "temporary failure in name resolution",
    "network is unreachable",
    "connection reset by peer",
    "connection aborted",
    "remotedisconnected",
    "incompleteread",
    "read timed out",
    "connect timeout=",
    "timed out",
    "unable to connect to proxy",
    "sockshttpconnection",
];

/// 剥离 ANSI 转义序列（`ESC [ ... <letter>`）。
///
/// 子进程被 pty 包装或 yt-dlp 彩色输出时，stderr 里会混入颜色码，
/// 不做剥离会让「行首是 `ERROR:`」这类判定失败。
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // 跳过 CSI 序列：ESC [ 参数直至一个终止字母
            if let Some('[') = chars.clone().next() {
                chars.next();
                for cc in chars.by_ref() {
                    if cc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// 剥 ANSI 转义 + 只保留 `ERROR:` 行，返回**去掉 `ERROR:` 标记**后的正文。
///
/// 同时去掉前导空白与 `[debug]` 前缀。保持原始顺序。
///
/// `ERROR:` 必须出现在**行首**（正是指去掉前导空白与 `[debug]` 前缀之后的位置）。
/// 若在行内任意位置匹配子串，文件名或警告里的 `error:` 会被误当成失败判据，
/// 污染「只读最后一条 `ERROR:` 行」这一契约。
pub fn error_lines(stderr: &str) -> Vec<String> {
    let cleaned = strip_ansi(stderr);
    let mut lines = Vec::new();
    for raw in cleaned.lines() {
        let mut line = raw.trim();
        if let Some(rest) = line.strip_prefix("[debug]") {
            line = rest.trim_start();
        }
        // 只认行首的 `ERROR:`（yt-dlp 输出固定大写）。大小写不敏感以防包装层改写。
        if let Some(text) = strip_prefix_ascii_ci(line, "error:") {
            lines.push(text.trim().to_string());
        }
    }
    lines
}

/// ASCII 大小写不敏感地查找子串，返回在 `haystack` 中的**字节下标**。
///
/// 只在 ASCII 上做折叠，因此命中位置的字节边界一定与 `haystack` 对齐
/// （UTF-8 的续字节 ≥ 0x80，不可能等于 ASCII 的字母）。
fn find_ascii_ci(haystack: &str, needle_lower: &str) -> Option<usize> {
    let hay = haystack.as_bytes();
    let needle = needle_lower.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    for i in 0..=(hay.len() - needle.len()) {
        if hay[i..i + needle.len()]
            .iter()
            .zip(needle)
            .all(|(h, n)| h.to_ascii_lowercase() == *n)
        {
            return Some(i);
        }
    }
    None
}

/// 若 `s` 以 `prefix_lower`（ASCII，大小写不敏感）**开头**，返回去掉前缀后的剩余部分。
///
/// 与 `find_ascii_ci` 的区别：只认行首，不做行内子串查找。
fn strip_prefix_ascii_ci<'a>(s: &'a str, prefix_lower: &str) -> Option<&'a str> {
    let prefix = prefix_lower.as_bytes();
    let head = s.as_bytes().get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        // 前缀全为 ASCII，故 prefix.len() 一定是字符边界
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// 在文本中查找 `prefix`（大小写不敏感），随后解析一个十进制整数。
///
/// 用于 `HTTP Error <code>` 与 `Tunnel connection failed: <code>` 两处判据。
/// `HTTP Error` 判据额外要求数字后跟可选的空白与 `:`；隧道判据不要求。
fn code_after_prefix(text: &str, prefix_lower: &str) -> Option<u16> {
    let mut from = 0usize;
    while let Some(rel) = find_ascii_ci(&text[from..], prefix_lower) {
        let start = from + rel + prefix_lower.len();
        let rest = &text[start..];
        let after_ws = rest.trim_start();
        let digits: String = after_ws.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() == 3 {
            if let Ok(code) = digits.parse::<u16>() {
                let tail_trimmed = after_ws[digits.len()..].trim_start();
                let needs_colon = prefix_lower == "http error";
                if !needs_colon || tail_trimmed.starts_with(':') {
                    return Some(code);
                }
            }
        }
        // 继续在后续文本中查找（跳过本次命中的前缀）
        let advance = rel + prefix_lower.len();
        from += advance;
        if from >= text.len() {
            break;
        }
    }
    None
}

/// HTTP 状态码 → 是否可重试：仅 `408` / `429` / `5xx` 重试。
fn status_decision(code: u16) -> RetryDecision {
    if code == 408 || code == 429 || (500..=599).contains(&code) {
        RetryDecision::Retry
    } else {
        RetryDecision::NoRetry
    }
}

/// 把一次失败映射为「可重试 / 不可重试 / 无法判定」。
///
/// `exit_code` 为 `None` 表示拿不到退出码（spawn 失败或被信号终止）。
pub fn classify_failure(stderr: &str, exit_code: Option<i32>) -> RetryDecision {
    match exit_code {
        // spawn 失败（路径 / 权限）或 被信号杀死 —— 都不是网络问题
        None => return RetryDecision::NoRetry,
        // 成功不该进分类器；2 = 命令行参数错误；101 = DownloadCancelled
        Some(0) | Some(2) | Some(101) => return RetryDecision::NoRetry,
        // Some(1) 及其它：交给文本判据
        _ => {}
    }

    let lines = error_lines(stderr);
    let Some(text) = lines.last() else {
        return RetryDecision::Unknown;
    };
    let lower = text.to_lowercase();

    // 1. 终态黑名单
    if TERMINAL_MARKERS.iter().any(|m| lower.contains(m)) {
        return RetryDecision::NoRetry;
    }

    // 2. 代理隧道状态码（407 走这里，而不是 `HTTP Error 407`）
    if let Some(code) = code_after_prefix(text, "tunnel connection failed:") {
        return status_decision(code);
    }

    // 3. HTTP 状态码
    if let Some(code) = code_after_prefix(text, "http error") {
        return status_decision(code);
    }

    // 4. 网络白名单
    if NETWORK_MARKERS.iter().any(|m| lower.contains(m)) {
        return RetryDecision::Retry;
    }

    // 5. 兜底：未知即重试
    RetryDecision::Unknown
}

/// 分类结果 + 已尝试次数 → 是否继续重试。
///
/// `attempts` 是**本次失败之前**已经重试过的次数（即 `retry_count` 的当前值）。
/// `Unknown` 视同 `Retry`。
pub fn should_retry(decision: &RetryDecision, attempts: u32) -> bool {
    matches!(decision, RetryDecision::Retry | RetryDecision::Unknown) && attempts < MAX_ATTEMPTS
}

/// 由「子进程是否存在」与「任务状态」推出当前相位。
///
/// 暂停优先于一切：退避期间暂停时子进程本就不存在，但相位应是 `Paused`（计时冻结）。
pub fn phase_of(child_present: bool, status: &str) -> Phase {
    if status.eq_ignore_ascii_case("paused") {
        Phase::Paused
    } else if child_present {
        Phase::Running
    } else {
        Phase::BackingOff
    }
}

/// 相位感知的退避剩余时长：**暂停相位冻结**（不扣减），其余相位按 `elapsed_secs` 扣减。
///
/// 循环持**剩余时长**（而不是绝对 deadline），因此「暂停冻结 / 恢复续算」天然正确。
pub fn remaining_after(phase: Phase, remaining_secs: u64, elapsed_secs: u64) -> u64 {
    match phase {
        Phase::Paused => remaining_secs,
        _ => remaining_secs.saturating_sub(elapsed_secs),
    }
}

/// 从 stderr 提取一条**给人看的可读摘要**：最后一条 `ERROR:` 行；没有则给兜底文案。
pub fn summarize_failure(stderr: &str) -> String {
    error_lines(stderr)
        .last()
        .cloned()
        .unwrap_or_else(|| "yt-dlp process exited with error".to_string())
}

/// 剥离代理凭证（`user:pass@`）并截断到 200 字符。
///
/// 🔒 `error_message` 现在来自真实 stderr，而 `proxy_url` 可能含明文口令，
/// **不得**写进 `download_records.json` 或显示在 UI。
pub fn sanitize_error_message(message: &str) -> String {
    let stripped = strip_credentials(message);
    stripped.chars().take(200).collect()
}

/// 删除 URL 中 `scheme://user:pass@host` 的 `user:pass@` 部分（仅当 userinfo 含 `:`）。
fn strip_credentials(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < input.len() {
        // 找 `://`
        if bytes[i..].starts_with(b"://") {
            out.push_str("://");
            i += 3;
            // authority 段：直到 `/`、`?`、`#`、空白或行尾
            let auth_start = i;
            let mut j = i;
            while j < input.len() {
                let c = bytes[j];
                if c == b'/' || c == b'?' || c == b'#' || c.is_ascii_whitespace() {
                    break;
                }
                j += 1;
            }
            let authority = &input[auth_start..j];
            if let Some(at) = authority.rfind('@') {
                let userinfo = &authority[..at];
                if userinfo.contains(':') {
                    // 丢弃 userinfo@，保留 host[:port]
                    out.push_str(&authority[at + 1..]);
                    i = j;
                    continue;
                }
            }
            out.push_str(authority);
            i = j;
            continue;
        }
        // 逐字符推进（保证多字节字符安全）
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 分类器：contracts/retry-classifier.md §3 的 12 个实测样本 ──────

    #[test]
    fn sample_http_404_generic_is_no_retry() {
        let stderr = "ERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    #[test]
    fn sample_http_404_bilibili_is_no_retry() {
        let stderr = "ERROR: [BiliBili] 123456: HTTP Error 404: Not Found";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    #[test]
    fn sample_http_503_is_retry() {
        let stderr = "ERROR: unable to download video data: HTTP Error 503: Service Unavailable";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Retry);
    }

    #[test]
    fn sample_http_408_is_retry() {
        let stderr = "ERROR: HTTP Error 408: Request Timeout";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Retry);
    }

    #[test]
    fn sample_http_429_is_retry() {
        let stderr = "ERROR: HTTP Error 429: Too Many Requests";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Retry);
    }

    #[test]
    fn sample_dns_failure_is_retry() {
        let stderr = "ERROR: [generic] Failed to resolve 'www.youtube.com' ([Errno -2] Name or service not known)";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Retry);
    }

    #[test]
    fn sample_proxy_407_tunnel_is_no_retry() {
        // 代理 407 表现为 `Tunnel connection failed: 407`，而不是 `HTTP Error 407`
        let stderr = "ERROR: Unable to download webpage: ('Tunnel connection failed: 407 Proxy Authentication Required')";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    #[test]
    fn sample_socks5_unreachable_is_retry() {
        // SOCKS5 不可达**完全不含 `proxy` 字样**，靠网络白名单兜住
        let stderr = "ERROR: SocksHTTPConnection(host='127.0.0.1', port=1080): Failed to establish a new connection: [Errno 111] Connection refused";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Retry);
    }

    #[test]
    fn sample_format_unavailable_is_no_retry() {
        let stderr = "ERROR: [BiliBili] 123456: Requested format is not available. Use --list-formats for a list of available formats";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    #[test]
    fn sample_warning_false_positive_403_only_reads_last_error_line() {
        // 同一份 stderr：WARNING 行里合法出现 `HTTP Error 403`（假阳性），
        // 真正的失败是最后一条 ERROR 行「格式不可用」。必须判 NoRetry。
        let stderr = concat!(
            "WARNING: Some formats may yield HTTP Error 403: Forbidden; skipping them\n",
            "[download] Destination: video.mp4\n",
            "ERROR: [BiliBili] 123456: Requested format is not available. Use --list-formats for a list of available formats",
        );
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    #[test]
    fn sample_exit_code_none_is_no_retry() {
        assert_eq!(classify_failure("", None), RetryDecision::NoRetry);
        // 即使 stderr 看起来可重试，拿不到退出码也判 NoRetry
        assert_eq!(
            classify_failure("ERROR: HTTP Error 503: Service Unavailable", None),
            RetryDecision::NoRetry
        );
    }

    #[test]
    fn sample_exit_code_2_and_101_are_no_retry() {
        let retryable = "ERROR: HTTP Error 503: Service Unavailable";
        assert_eq!(classify_failure(retryable, Some(2)), RetryDecision::NoRetry);
        assert_eq!(classify_failure(retryable, Some(101)), RetryDecision::NoRetry);
    }

    #[test]
    fn no_error_lines_is_unknown() {
        assert_eq!(classify_failure("", Some(1)), RetryDecision::Unknown);
        assert_eq!(
            classify_failure("[download]   0% of 10MiB", Some(1)),
            RetryDecision::Unknown
        );
    }

    #[test]
    fn unknown_error_text_is_unknown() {
        let stderr = "ERROR: something nobody has seen before";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::Unknown);
    }

    #[test]
    fn ansi_escapes_are_stripped_before_matching() {
        let stderr = "\u{1b}[0;31mERROR:\u{1b}[0m HTTP Error 404: Not Found";
        assert_eq!(classify_failure(stderr, Some(1)), RetryDecision::NoRetry);
    }

    // ── error_lines ────────────────────────────────────────────────

    #[test]
    fn error_lines_keeps_only_error_lines_and_strips_marker() {
        let stderr = concat!(
            "WARNING: a\n",
            "[debug] ERROR: first problem\n",
            "  ERROR: second problem  \n",
            "not an error\n",
        );
        assert_eq!(
            error_lines(stderr),
            vec!["first problem".to_string(), "second problem".to_string()]
        );
    }

    #[test]
    fn error_lines_requires_marker_at_line_start() {
        // 行内出现 `error:` 的行不算错误行——否则文件名 / WARNING 会被误当成失败判据
        let stderr = concat!(
            "[download] Destination: Lecture 3 - Error: Reconstruction.mp4\n",
            "WARNING: cosmetic: error: not a real failure\n",
            "ERROR: real failure\n",
        );
        assert_eq!(error_lines(stderr), vec!["real failure".to_string()]);
    }

    // ── 退避纯函数 ──────────────────────────────────────────────────

    #[test]
    fn phase_of_paused_wins_over_child() {
        assert_eq!(phase_of(true, "paused"), Phase::Paused);
        assert_eq!(phase_of(false, "paused"), Phase::Paused);
    }

    #[test]
    fn phase_of_child_presence() {
        assert_eq!(phase_of(true, "running"), Phase::Running);
        assert_eq!(phase_of(false, "retrying"), Phase::BackingOff);
    }

    #[test]
    fn remaining_after_freezes_when_paused() {
        // 暂停期间计时钟冻结：无论过了多久，剩余时长不变
        assert_eq!(remaining_after(Phase::Paused, 30, 999), 30);
    }

    #[test]
    fn remaining_after_decrements_when_backing_off() {
        assert_eq!(remaining_after(Phase::BackingOff, 30, 12), 18);
        assert_eq!(remaining_after(Phase::Running, 30, 12), 18);
    }

    #[test]
    fn remaining_after_saturates_at_zero() {
        assert_eq!(remaining_after(Phase::BackingOff, 10, 999), 0);
    }

    #[test]
    fn should_retry_treats_unknown_as_retry_and_respects_max() {
        assert!(should_retry(&RetryDecision::Retry, 0));
        assert!(should_retry(&RetryDecision::Unknown, 0));
        assert!(!should_retry(&RetryDecision::NoRetry, 0));

        // 已尝试 3 次即达上限
        assert!(should_retry(&RetryDecision::Retry, 2));
        assert!(!should_retry(&RetryDecision::Retry, 3));
    }

    #[test]
    fn backoff_table_matches_spec() {
        assert_eq!(BACKOFF_SECS, [30, 60, 120]);
        assert_eq!(MAX_ATTEMPTS, 3);
    }

    // ── 脱敏 ────────────────────────────────────────────────────────

    #[test]
    fn sanitize_strips_proxy_credentials() {
        let raw = "Failed to connect via socks5://user:pass@127.0.0.1:1080 (Connection refused)";
        let clean = sanitize_error_message(raw);
        assert!(!clean.contains("user:pass@"), "实际: {}", clean);
        assert!(clean.contains("socks5://127.0.0.1:1080"));
    }

    #[test]
    fn sanitize_strips_credentials_from_all_urls() {
        let raw = "http://a:b@host1 and https://c:d@host2/path";
        let clean = sanitize_error_message(raw);
        assert!(!clean.contains("a:b@"));
        assert!(!clean.contains("c:d@"));
        assert!(clean.contains("http://host1"));
        assert!(clean.contains("https://host2/path"));
    }

    #[test]
    fn sanitize_keeps_urls_without_credentials() {
        let raw = "HTTP Error 404 at https://example.com/watch?v=x";
        assert_eq!(sanitize_error_message(raw), raw);
    }

    #[test]
    fn sanitize_truncates_to_200_chars() {
        let raw = "x".repeat(500);
        assert_eq!(sanitize_error_message(&raw).chars().count(), 200);
    }

    #[test]
    fn sanitize_truncates_on_char_boundary() {
        let raw = "中".repeat(300);
        let clean = sanitize_error_message(&raw);
        assert_eq!(clean.chars().count(), 200);
        assert_eq!(clean, "中".repeat(200));
    }

    #[test]
    fn summarize_failure_prefers_last_error_line() {
        let stderr = "ERROR: first\nERROR: last one";
        assert_eq!(summarize_failure(stderr), "last one");
    }

    #[test]
    fn summarize_failure_falls_back_when_no_error_line() {
        assert_eq!(
            summarize_failure("[download] 0%"),
            "yt-dlp process exited with error"
        );
    }
}
