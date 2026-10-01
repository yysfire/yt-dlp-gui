# yt-dlp 下载失败的可重试性判别研究

> 目标：为 `src-tauri/src/services/download_queue.rs` 的**自动重试**（最多 3 次，间隔 30/60/120s，白名单 + 未知即重试）提供事实依据 —— 一次 yt-dlp 下载失败在 stderr/stdout/退出码上分别留下什么，哪些特征可以稳定依赖。
>
> 结论必须能支撑一个纯函数分类器：输入「stderr 文本 + 退出码」，输出「可重试 / 不可重试 / 无法判定」。

---

## 0. 结论速览（TL;DR）

1. **最稳的判别特征是 stderr 里的 `HTTP Error <code>:`** —— 由 yt-dlp 自己构造（`networking/exceptions.py`），不依赖第三方库措辞。实测在 `generic` 与 `BiliBili` 两个 extractor 上完全一致，走的是全 extractor 共用的 `urlopen` 通道。
2. **退出码没有错误类型语义**：成功 `0`；任意错误 `1`；命令行参数错误 `2`；用户取消 `101`。**没有为网络错误保留专用退出码**。官方 README **未文档化**退出码，只能从源码推导。
3. **代理 407 不是 `HTTP Error 407`**，而是 `Tunnel connection failed: 407 Proxy Authentication Required`；SOCKS5 不可达甚至完全不含 `proxy` 字样。只按 `HTTP Error (\d{3})` 匹配会**漏判**这两类。
4. **必须只看 `ERROR:` 行**。警告行里会合法出现 `HTTP Error 403` 这种「假阳性」文本（如 YouTube「某些格式可能产生 HTTP Error 403，已跳过」），若对整段 stderr 做子串匹配会误判。
5. **「网络超时」与「格式不可用」可以同现**，但**不可能同时是致命主因**：前者以 `WARNING: ... Retrying (n/N)` 出现，后者是最终 `ERROR:`；若提取阶段真的超时致命，根本到不了格式选择。主因 = 最终 `ERROR:` 行。
6. yt-dlp **自身已经重试**（下载 10 次 / 提取 3 次 / 分片 10 次），应用层再叠加 3 次会放大请求量，建议以 `Giving up after N retries` 作为「yt-dlp 已放弃」的信号再决定是否补刀。

---

## 1. 方法与证据来源

### 1.1 被测对象

| 项 | 值 |
|---|---|
| `which yt-dlp` | `/usr/bin/yt-dlp` |
| `yt-dlp --version` | `2026.08.19` |
| 形态 | 自包含 zipapp（`head -1` 为 `#!/usr/bin/env python3`，其后为 zip） |
| 源码提取 | `python3 -m zipfile` 解包到 `/tmp/ytdlp-src`，逐文件检索 |

交叉校验版本：apt 安装的 `python3-distutils` 模块 `/usr/lib/python3/dist-packages/yt_dlp`，`version.py` 为 `2026.03.17`。第 8 节列出两版本关键文本的一致性核验结果。

### 1.2 实验方法

所有实验都**用应用的真实参数**调用（`-f <format>`、`-o <tmpl>`、`--no-playlist`、`--newline`、`--progress`、`--progress-template`、`--print after_move:filepath`），分别重定向 stdout/stderr 并记录退出码，以复现 App 实际拿到的字节流。

为规避真实站点的不确定性与限流，用本地桩服务器精确构造 HTTP 状态码：

- 桩 HTTP 服务器（`/code/<n>` 返回指定状态码、`/hang` 接受连接但不响应制造读超时）；
- 桩 HTTP 代理（对 `CONNECT` 一律回 407）；
- 桩 HLS 服务器（m3u8 正常、分片回 403）。

> **环境注意**：本机环境预置了 `http_proxy/https_proxy/all_proxy=http(s)://127.0.0.1:6478`，yt-dlp 默认会读取。做直连类实验时必须加 `--proxy ""` 显式关闭，否则会得到「代理返回 502」之类的**伪结果**（这正是最初 `127.0.0.1:1` 被报成 `HTTP Error 502` 的原因）。

### 1.3 一手来源清单

| 主题 | 文件:行 |
|---|---|
| HTTP 状态错误消息构造 | `yt_dlp/networking/exceptions.py:57-73` |
| 网络异常基类（Transport/SSL/Proxy/CertVerify） | `yt_dlp/networking/exceptions.py:53-103` |
| `network_exceptions` 元组 | `yt_dlp/networking/exceptions.py:103` |
| 错误/警告输出通道 | `yt_dlp/YoutubeDL.py:1072-1104`（`trouble`）、`1160-1165`（`report_error`）、`1138-1149`（`report_warning`） |
| 退出码 | `yt_dlp/YoutubeDL.py:651,1096-1104,3703-3717`；`yt_dlp/__init__.py:1065-1093` |
| 提取阶段包装 | `yt_dlp/extractor/common.py:865-923`（`_request_webpage`） |
| 下载阶段包装 | `yt_dlp/YoutubeDL.py:3596-3598` |
| 格式不可用 | `yt_dlp/YoutubeDL.py:3049-3061` |
| yt-dlp 自身重试判定 | `yt_dlp/downloader/http.py:148-192`；`yt_dlp/utils/_utils.py:5255-5308` |
| urllib handler 异常映射 | `yt_dlp/networking/_urllib.py:335-443` |
| requests handler 异常映射 | `yt_dlp/networking/_requests.py:335-356,366-392` |
| 代理 tunnel 关键字 | `yt_dlp/networking/_urllib.py:426-434` |
| YouTube 终态原因 | `yt_dlp/extractor/youtube/_video.py:4047-4076` |
| YouTube 登录提示 | `yt_dlp/extractor/youtube/_base.py:699-707` |
| BiliBili 412/352/401 | `yt_dlp/extractor/bilibili.py:232-245,1405-1416` |
| 重试默认值 | `yt_dlp/options.py:1024-1043,1905-1907` |

---

## 2. 输出通道与退出码：两个前置事实

### 2.1 错误只走 stderr；stdout 在 App 的参数下「失败时为空」

- `report_error` → `trouble` → `to_stderr`（`YoutubeDL.py:1072-1104,1160-1165`）。所有 `ERROR:` / `WARNING:` 都在 **stderr**。
- App 传了 `--print after_move:filepath`，而 `--print` **隐含 `--quiet`**，因此 `to_screen` 的常规信息被抑制；stdout 在成功时只有 `after_move` 的文件路径（以及 `--progress-template` 渲染的进度行），**失败时为空**。
- 实测：第 3 节所有失败场景 `*.out` 均为 0 字节，`*.err` 非空。

> 这就是当前 `execute_download_with_control` 只读 stdout 时，永远拿不到失败原因的直接原因。

### 2.2 退出码：没有错误类型语义

```
_download_retcode 初始为 0                              # YoutubeDL.py:651
trouble() 在任意 is_error 错误上把它置 1                # YoutubeDL.py:1096-1104
download() 返回 _download_retcode                        # YoutubeDL.py:3703-3717
_real_main() 正常返回 retcode；DownloadCancelled → 101   # __init__.py:1065-1074
main() 捕获 DownloadError/CookieLoadError/… → _exit(1)   # __init__.py:1077-1082
main() 捕获 OptParseError → _exit(2, ...)                # __init__.py:1092-1093
```

| 退出码 | 含义 | 来源 |
|---|---|---|
| `0` | 全部成功 | `_download_retcode=0` |
| `1` | **任意**错误（提取失败、网络失败、404、格式不可用、下载数据失败…） | `trouble()` 置位；`main()` 对 `DownloadError` 兜底 |
| `2` | 命令行参数解析错误 | `OptParseError` 分支 |
| `101` | 用户/程序取消（`DownloadCancelled`） | `_real_main` 返回值 |

**直接回答「退出码是否有语义」：没有。** yt-dlp 不为「网络错误」保留任何特定退出码；网络失败、404、格式不可用一律是 `1`。退出码只能用于区分「成功 / 失败 / 取消 / 参数错」，**不能**用于分类失败原因。

> 官方 README（拉取自 `master`，2026-10-01）**没有任何 exit/return code 章节**。该表完全由源码推导，属实现细节而非契约 —— 归类为「稳定但非文档化」。

---

## 3. 场景矩阵（实测）

> 除特别注明外，退出码均为 **1**，stdout 均为空。`<extractor>` 表示实际 extractor 名（如 `generic`/`BiliBili`/`youtube`），`<id>` 为视频 id。

| # | 场景 | 触发方式 | stderr 关键行（实测，节选） | 稳定判据 | 分类 |
|---|---|---|---|---|---|
| 1 | HTTP 400 | 桩 `/code/400` | `Unable to download webpage: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)` | `HTTP Error 400:` | 不可重试 |
| 1 | HTTP 401 | 桩 `/code/401` | `... HTTP Error 401: Unauthorized ...` | `HTTP Error 401:` | 不可重试 |
| 1 | HTTP 403 | 桩 `/code/403` | `... HTTP Error 403: Forbidden ...` | `HTTP Error 403:` | 不可重试 |
| 1 | HTTP 404 | 桩 `/code/404` / 不存在的 BV 号 | `[BiliBili] 0000000000: Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)` | `HTTP Error 404:` | 不可重试 |
| 1 | HTTP 410 | 桩 `/code/410` | `... HTTP Error 410: Gone ...` | `HTTP Error 410:` | 不可重试 |
| 1 | **HTTP 408** | 桩 `/code/408` | `... HTTP Error 408: Request Timeout ...` | `HTTP Error 408:` | **可重试**（按策略） |
| 1 | **HTTP 429** | 桩 `/code/429` | `... HTTP Error 429: Too Many Requests ...` | `HTTP Error 429:` | **可重试**（按策略） |
| 2 | HTTP 500 | 桩 `/code/500` | `... HTTP Error 500: Internal Server Error ...` | `HTTP Error 5xx:` | 可重试 |
| 2 | HTTP 502 | 桩 `/code/502` | `... HTTP Error 502: Bad Gateway ...` | 同上 | 可重试 |
| 2 | HTTP 503 | 桩 `/code/503` | `... HTTP Error 503: Service Unavailable ...` | 同上 | 可重试 |
| 2 | HTTP 504 | 桩 `/code/504` | `... HTTP Error 504: Gateway Timeout ...` | 同上 | 可重试 |
| 3 | 连接被拒 | `127.0.0.1:1` + `--proxy ""` | `HTTPConnection(host='127.0.0.1', port=1): Failed to establish a new connection: [Errno 111] Connection refused (caused by TransportError("..."))` | `Failed to establish a new connection` / `TransportError` | 可重试 |
| 3 | DNS 解析失败 | `.invalid` 域 + `--proxy ""` | `HTTPSConnection(host='…invalid', port=443): Failed to resolve '…invalid' ([Errno -2] Name or service not known) (caused by TransportError("…"))` | `Failed to resolve` | 可重试 |
| 3 | 读取超时 | 桩 `/hang` + `--socket-timeout 3` | `HTTPConnectionPool(host='127.0.0.1', port=8731): Read timed out. (read timeout=3.0) (caused by TransportError("…"))` | `Read timed out` | 可重试 |
| 3 | 连接超时 | 真实站点抖动 | `'Connection to www.youtube.com timed out. (connect timeout=20.0)'` | `connect timeout=` / `timed out` | 可重试 |
| 3 | TLS 握手失败 | 对纯 HTTP 端口发 `https://` + `--proxy ""` | `[SSL: RECORD_LAYER_FAILURE] record layer failure (_ssl.c:1081) (caused by SSLError('…')); please report this issue …` | `[SSL:` / `SSLError` | 可重试 |
| 3 | 连接重置/不可达 | 真实站点抖动 | `('Connection aborted.', ConnectionResetError(104, 'Connection reset by peer'))`；`[Errno 101] Network is unreachable` | `Connection reset by peer` / `Network is unreachable` | 可重试 |
| 4 | HTTP 代理不可达 | `--proxy http://127.0.0.1:1` | `('Unable to connect to proxy', NewConnectionError("HTTPConnection(host='127.0.0.1', port=1): Failed to establish a new connection: [Errno 111] Connection refused")) (caused by ProxyError("…"))` | `Unable to connect to proxy` / `ProxyError` | 可重试 |
| 4 | **代理 407** | 桩代理回 407 | `('Unable to connect to proxy', OSError('Tunnel connection failed: 407 Proxy Authentication Required')) (caused by ProxyError("…"))` | `Tunnel connection failed: 407` | **不可重试**（属 4xx；注意它**不是** `HTTP Error 407`） |
| 4 | SOCKS5 不可达 | `--proxy socks5://127.0.0.1:1` | `SocksHTTPConnection(host='127.0.0.1', port=8731): Failed to establish a new connection: [Errno 111] Connection refused (caused by TransportError("…"))` | `Failed to establish a new connection`（**无 `proxy` 字样**） | 可重试 |
| 5 | 已删除/私享（YouTube） | 源码推导 | `ERROR: [youtube] <id>: <YouTube 原因串>`（如 `Video unavailable` / `Private video…`） | 见 §6.3，**措辞由 YouTube 提供，不稳** | 不可重试（需黑名单） |
| 5 | 需要登录（YouTube） | 源码推导 | `<原因>. Sign in to confirm you're not a bot…. <cookie 提示>` | `Sign in to confirm`（中置信） | 不可重试 |
| 5 | 地区限制（YouTube） | 源码推导 | `The uploader has not made this video available in your country` + `GeoRestrictedError` | `not available in your country`（中置信） | 不可重试 |
| 5 | 地区限制（BiliBili） | 源码 | `This video is restricted`（`GeoRestrictedError`，`bilibili.py:991-992`） | `restricted` | 不可重试 |
| 5 | 格式不可用 | 合法视频 + `-f no-such-format-id-xyz` | `ERROR: [BiliBili] BV1GJ411x7h7: Requested format is not available. Use --list-formats for a list of available formats` | `Requested format is not available` | 不可重试 |
| 5 | B 站风控 412 | 源码 | `Request is blocked by server (412), please wait and try later.`（`bilibili.py:1405-1407`） | `blocked by server (412)` | 可重试（限流性质）**但策略会落未知→重试** |
| 5 | B 站 -352 | 源码 | `Request is rejected by server (352)`（`bilibili.py:1413-1414`） | 同上 | 同上 |
| 5 | 下载阶段 HTTP 错误 | 源码推导 | `ERROR: unable to download video data: HTTP Error 403: Forbidden`（`YoutubeDL.py:3596-3598`） | `HTTP Error 403:`（同一条正则仍可用） | 同 §3 判定 |
| 5 | HLS 分片 403 | 桩 HLS 分片 403 | `ERROR: The downloaded file is empty`（分片被跳过/重试后体积为 0） | **语义含糊** | 无法判定 → 可重试 |
| 6 | 子进程起不来 | `Command::new(<不存在路径>).spawn()` | **无 stderr、无退出码**；Rust 侧拿 `io::Error(NotFound/PermissionDenied)` | 见 §7 输入建模 | 不可重试（配置问题） |

### 3.1 实测原始样本（供规则回归测试使用）

HTTP 404（跨 extractor 一致性）：

```
ERROR: [generic] 404: Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)
ERROR: [BiliBili] 0000000000: Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)
```

DNS 失败：

```
ERROR: [generic] video: Unable to download webpage: HTTPSConnection(host='this-domain-definitely-does-not-exist-9f8a7b6c.invalid', port=443): Failed to resolve 'this-domain-definitely-does-not-exist-9f8a7b6c.invalid' ([Errno -2] Name or service not known) (caused by TransportError("HTTPSConnection(host='this-domain-definitely-does-not-exist-9f8a7b6c.invalid', port=443): Failed to resolve 'this-domain-definitely-does-not-exist-9f8a7b6c.invalid' ([Errno -2] Name or service not known)"))
```

代理 407（注意：**没有** `HTTP Error 407`）：

```
ERROR: [BiliBili] 1GJ411x7h7: Unable to download webpage: ('Unable to connect to proxy', OSError('Tunnel connection failed: 407 Proxy Authentication Required')) (caused by ProxyError("('Unable to connect to proxy', OSError('Tunnel connection failed: 407 Proxy Authentication Required'))")); please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= , filling out the appropriate issue template. Confirm you are on the latest version using  yt-dlp -U
```

SOCKS5 不可达（注意：**没有 `proxy` 字样**，被判成 `TransportError` 而非 `ProxyError`）：

```
ERROR: [generic] 200: Unable to download webpage: SocksHTTPConnection(host='127.0.0.1', port=8731): Failed to establish a new connection: [Errno 111] Connection refused (caused by TransportError("SocksHTTPConnection(host='127.0.0.1', port=8731): Failed to establish a new connection: [Errno 111] Connection refused"))
```

格式不可用：

```
ERROR: [BiliBili] BV1GJ411x7h7: Requested format is not available. Use --list-formats for a list of available formats
```

yt-dlp 自身重试耗尽（YouTube 网络抖动，**WARNING** 行）：

```
WARNING: [youtube] dQw4w9WgXcQ: Unable to download webpage: ('Connection aborted.', ConnectionResetError(104, 'Connection reset by peer')) (caused by TransportError("…")). Giving up after 3 retries
WARNING: [youtube] HTTPSConnection(host='www.youtube.com', port=443): Failed to establish a new connection: [Errno 101] Network is unreachable. Retrying (1/3)...
ERROR: [youtube] dQw4w9WgXcQ: Unable to download API page: HTTPSConnection(host='www.youtube.com', port=443): Failed to establish a new connection: [Errno 101] Network is unreachable (caused by TransportError("…"))
```

---

## 4. 可编码判据清单

> 每条给出：正则/子串、命中后的分类、置信度、版本/环境敏感性。**执行顺序即优先级**（先终态黑名单，后可重试白名单，最后兜底）。

### 4.1 一级判据：HTTP 状态（最高置信度）

```
(?i)HTTP Error\s+(\d{3})\s*:
```

- 来源：`networking/exceptions.py:63` `msg = f'HTTP Error {response.status}: {response.reason}'`。
- **置信度：高**。**版本敏感性：低**（已在 2026.03.17 与 2026.08.19 两版核验，构造方式逐字一致）。
- 命中后：`code in {408,429}` 或 `500 <= code <= 599` → **可重试**；其余 → **不可重试**。
- 覆盖阶段：提取阶段（`Unable to download webpage: …`）与下载阶段（`unable to download video data: …`）都包含此串，规则无需区分阶段。
- **已知例外（必须补偿）**：
  1. **407 不走这条路径**，见 4.2。
  2. 部分 extractor 会捕获 `HTTPError` 换成友好文案：BiliBili 412 → `Request is blocked by server (412)…`（`bilibili.py:1405-1407`，**不再含 `HTTP Error 412`**）；generic 的 Cloudflare 拦截 → `Got HTTP Error 403 caused by Cloudflare anti-bot challenge…`（`generic.py:838`，**仍含** `HTTP Error 403`）。
  3. `--ignore-errors` 未开启时首个错误即中止，但**非致命子请求失败会以 `WARNING` 形式带 `HTTP Error <code>`**（见 §5.1）。

### 4.2 代理类

| 模式 | 分类 | 置信度 | 敏感性 |
|---|---|---|---|
| `(?i)Tunnel connection failed:\s*(\d{3})`（典型 407） | 按 4xx→不可重试 | 高（实测 + `_urllib.py:430` 显式判定） | 低 |
| `Unable to connect to proxy` | 可重试 | 高（实测，`ProxyError` 内部文案） | 中（依赖 urllib3 措辞，可能随 handler 变） |
| `ProxyError`（repr 中的类名） | 可重试 | 中高（yt-dlp 自己的异常类，`exceptions.py:99`） | 中 |
| `SocksHTTPConnection` + `Failed to establish a new connection` | 可重试 | 高（实测） | 中 |

> **结论**：代理超时/握手失败中没有可靠的「状态码」，只能靠上述文案 → 落在「未知→可重试」（正确）。唯一需要显式处理的是 **407**：因为它是明确的 4xx 且重试无意义，但**不会被 4.1 捕获**。

### 4.3 网络类（可重试白名单）

| 特征（子串，建议大小写不敏感） | 含义 | 置信度 | 敏感性 |
|---|---|---|---|
| `Failed to establish a new connection` | 拒绝/不可达 | 高 | 中（urllib3 文案） |
| `Failed to resolve` | DNS 失败 | 高 | 中 |
| `Name or service not known` / `Temporary failure in name resolution` | DNS 失败（OS 文案） | 高 | 低（glibc） |
| `Read timed out` / `connect timeout=` / `timed out` | 超时 | 高 | 中 |
| `Network is unreachable` | 路由不可达 | 高 | 低 |
| `Connection reset by peer` / `Connection aborted` | 连接被重置 | 高 | 中 |
| `RemoteDisconnected` | 服务器断开 | 中高 | 中 |
| `IncompleteRead` / `bytes read, .* more expected` | 传输中断 | 中高 | 中 |
| `[SSL:` / `SSLError` / `CertificateVerifyError` | TLS 失败 | 高 | 中（OpenSSL 文案） |
| `TransportError` / `SSLError` / `ProxyError` / `CertificateVerifyError`（异常类名） | yt-dlp 对网络错误的统一归类 | 高 | 中 |
| `Giving up after \d+ retries` | yt-dlp 自身重试已耗尽（**可重试的强信号**） | 中高 | 中（`_utils.py:5296`） |

> **推荐优先匹配 yt-dlp 自己的异常类名**（`TransportError`/`ProxyError`/`SSLError`/`CertificateVerifyError`），它们是 `networking/exceptions.py` 的稳定 API，比 urllib3/requests 的内部措辞更抗版本漂移。

### 4.4 非网络类终态（不可重试黑名单）

| 特征 | 含义 | 置信度 | 敏感性 |
|---|---|---|---|
| `Requested format is not available` | 所选格式不存在 | 高 | 低（两版本逐字一致，历史久） |
| `Use --list-formats` | 同上（伴随） | 高 | 低 |
| `This video is DRM protected` / `DRM protected` | DRM | 中 | 中 |
| `This video is only available for registered users` / `raise_login_required` 默认文案 | 需登录 | 中 | 中 |
| `Private video` / `Video unavailable` / `This video is not available` | 已删除/私享 | 中 | **高**（YouTube 提供的字符串） |
| `Sign in to confirm` | bot 校验/需登录 | 中 | 中 |
| `not available in your country` / `restricted` | 地区限制 | 中 | 中 |
| `Unsupported URL` | URL 不合法 | 高 | 低 |
| `An extractor error has occurred` / `please report this issue` | yt-dlp 内部 bug（如 B 站坏 BV 号触发 `KeyError('bvid')`） | 中 | 中（措辞可能变；重试通常无用） |

> **风险提示**：YouTube 类失败的原因串（`Video unavailable`、`Private video`、`Sign in to confirm`）由 **YouTube 服务端**给出，随其文案调整而漂移。若把它们放进黑名单，一旦措辞变化会退化为「未知→重试」，属于安全的失败方向（多试几次），不会误伤。

### 4.5 退出码判据

| 退出码 | 建议分类 | 备注 |
|---|---|---|
| `0` | 非失败 | 不应进入分类器 |
| `1` | **不参与分类** | 所有失败共用，无信息量 |
| `2` | 不可重试 | 参数错误 |
| `101` | 不可重试 | `DownloadCancelled`（应用自身取消也走这条语义） |
| `None`（无 code） | 见 §7.1 | 可能是**信号终止**或 **spawn 失败**，需由调用方区分 |

---

## 5. 噪声与陷阱（决定了「怎么取文本」）

### 5.1 警告行里的 `HTTP Error 403` 是假阳性

YouTube extractor 会打印**假设性**的 403 警告（仅当某些格式缺少 PO Token 时）：

```python
# extractor/youtube/_video.py:3200-3205
msg = (
    f'{video_id}: {client_name} client {proto} formats require a GVS PO Token which was not provided. '
    'They will be skipped as they may yield HTTP Error 403. '   # ← 文本里含 "HTTP Error 403"
    ...
)
```

同类：多个 extractor 的测试用例把 `HTTP Error 403/404` 列为 `expected_warnings`。

**若对整个 stderr 做 `HTTP Error (\d{3})` 子串匹配，会把这类警告当成真实失败码**，从而可能把一个「格式不可用」误判成 403（不可重试）——在本例中恰好方向相同，但在别的组合里（如警告 404 + 致命超时）会把它误判成**不可重试**。

### 5.2 正确取文本的方法

1. 只保留**以 `ERROR:` 开头的行**（管道下 yt-dlp 不加 ANSI 颜色；稳妥起见可先剥 `\x1b\[[0-9;]*m`）。
2. 取**最后一条 `ERROR:` 行**作为权威原因；如需更全，可拼接全部 `ERROR:` 行。
3. 若**没有任何 `ERROR:` 行**（例如仅 spawn 失败）→ 走 §7.1 的输入建模。
4. `WARNING:` 行仅用于辅助信号（`Giving up after N retries` 表示可重试），**不用于判定致命原因**。

---

## 6. 三个具体问题的回答

### 6.1 判别 HTTP 状态，最稳的是 stderr 里的 `HTTP Error <code>` 吗？不同 extractor 是否一致？

**是，这是最稳的单条特征**，理由：

- 它由 yt-dlp 自己在 `networking/exceptions.py:63` 构造，**不经过任何第三方库的措辞**；
- 实测 `generic` 与 `BiliBili` 两个 extractor 输出的 `HTTP Error <code>: <reason>` 完全同构；
- YouTube 走的是同一条 `urlopen` 通道（`extractor/common.py:906-920`）。

**但不是 100% 一致**，必须知道的三个缺口：

1. **407 从不以 `HTTP Error 407` 出现**（代理隧道失败被单独处理，`_urllib.py:426-434`）。
2. **部分 extractor 会吞掉原始 HTTPError 换成友好文案**：BiliBili 412（`bilibili.py:1405-1407`）就丢掉了 `HTTP Error 412`；generic 的 Cloudflare 403（`generic.py:838`）则保留了。
3. HTTP 错误可能出现在 `WARNING` 行（非致命子请求），与最终致命原因不同（§5.1）。

**结论**：把 `HTTP Error (\d{3}):`（限定在 `ERROR:` 行内）作为**主判据**，同时为 407 与 B 站 412 做显式补偿。

### 6.2 「网络超时」与「格式不可用」是否可能同时出现在一次失败里？谁是主因？

**会同时出现，但不会同时是致命主因。**

- 同一份 stderr 里完全可能出现：前面若干条 `WARNING: … Retrying (n/3)…` / `Giving up after 3 retries`（网络超时），最后一条 `ERROR: Requested format is not available`。
- **不可能**两者都是致命主因：格式选择发生在提取**之后**（`YoutubeDL.py:3049-3061`）；若提取阶段被网络超时致命中断，根本执行不到格式选择。
- 反之，格式不可用的报错前面**常伴随**关于「某些格式可能产生 HTTP Error 403」的警告（§5.1），这是最容易被误判的组合。

**主因判定 = 最终 `ERROR:` 行**。对本例，主因是「格式不可用」→ **不可重试**；若只看警告里的超时字样就会误判为可重试。**所以分类器必须先做「只取 ERROR 行」这一步**，否则 §6.2 与 §5.1 两个陷阱都会踩中。

### 6.3 退出码是否有语义（是否为「网络错误」保留特定退出码）？

**没有。** 见 §2.2：`_download_retcode` 只有 `0/1`，`trouble()` 对任意错误置 `1`；网络失败、404、格式不可用全是 `1`。`101` 仅表示取消，`2` 仅表示参数错误。官方 README 未文档化退出码。

因此**分类必须基于 stderr 文本**，退出码只能用于：排除成功（0）、识别取消（101）、排除参数错误（2）。

---

## 7. 面向纯函数分类器的建议

### 7.1 输入建模（重要：先解决 `None` 的二义性）

题面要求输入为「stderr 文本 + 退出码」。但 `None`（无退出码）在实际中有**两种**来源，语义相反：

| 情形 | 触发 | 判别 | 建议分类 |
|---|---|---|---|
| spawn 失败 | `Command::new(path).spawn()` 返回 `Err` | 根本拿不到 `Child`，无 stderr | **不可重试**（路径/权限配置问题，重试无意义） |
| 被信号杀死 | App 主动 `start_kill()` 取消 | `child.wait()` 成功，但 Unix 下 `ExitStatus::code()==None` | **不可重试**（用户取消） |

两者当前都会被压成 `Option::None`。建议分类器签名区分，或至少由调用方在「取消」路径上显式短路（现有代码已通过 `status != "cancelled"` 避免覆盖，但**spawn 失败分支完全没写记录**，见 §7.4）。

### 7.2 分类器伪代码（可直接编码）

```text
enum RetryDecision { Retry, NoRetry, Unknown }

fn classify(stderr: &str, exit_code: Option<i32>) -> RetryDecision {
    // 0) 取消 / 参数错误 / spawn 失败
    match exit_code {
        None      => return NoRetry,   // spawn 失败 或 信号终止（见 §7.1，由调用方区分）
        Some(101) => return NoRetry,   // DownloadCancelled
        Some(2)   => return NoRetry,   // 参数错误
        _ => {}
    }

    // 1) 只取 ERROR 行；无 ERROR 行则退回整段（极少见）
    let text = error_lines(stderr);          // 见 §7.3
    if text.is_empty() { return Unknown; }

    // 2) 终态黑名单（最高优先级，必须早于 HTTP 白名单，
    //    以免被前导 WARNING 中的 "HTTP Error 403" 误导；见 §5.1）
    if matches_terminal(&text) { return NoRetry; }

    // 3) 代理 407（不是 HTTP Error 407）
    if let Some(code) = first_match(&text, r"(?i)Tunnel connection failed:\s*(\d{3})") {
        return if code == 408 || code == 429 || (500..=599).contains(&code) { Retry } else { NoRetry };
    }

    // 4) HTTP 状态（主判据）
    if let Some(code) = first_match(&text, r"HTTP Error\s+(\d{3})\s*:") {
        return if code == 408 || code == 429 || (500..=599).contains(&code) { Retry } else { NoRetry };
    }

    // 5) 网络类白名单
    if matches_retryable_network(&text) { return Retry; }

    // 6) 兜底：未知 → 可重试（符合既定策略）
    Unknown
}

// 调用方：decision == Retry || decision == Unknown  →  执行重试
```

### 7.3 正则/子串表（实现用）

```text
error_lines:      (?m)^\s*(?:\[debug\]\s*)?ERROR:\s*(.+)$        // 取全部，权威取最后一条

TERMINAL (NoRetry):
  "Requested format is not available"
  "Use --list-formats"
  "Unsupported URL"
  "This video is DRM protected"
  "This video is only available for registered users"
  "Private video"
  "Video unavailable"
  "This video is not available"
  "Sign in to confirm"
  "not available in your country"
  "restricted"
  "An extractor error has occurred"

HTTP_STATUS:      (?i)HTTP Error\s+(\d{3})\s*:
PROXY_TUNNEL:     (?i)Tunnel connection failed:\s*(\d{3})

RETRYABLE_NETWORK (Retry):
  "TransportError"                           // yt-dlp 网络异常类名
  "ProxyError"
  "CertificateVerifyError"
  "SSLError"
  "Failed to establish a new connection"
  "Failed to resolve"
  "Name or service not known"
  "Temporary failure in name resolution"
  "Network is unreachable"
  "Connection reset by peer"
  "Connection aborted"
  "RemoteDisconnected"
  "IncompleteRead"
  "Read timed out"
  "connect timeout="
  "timed out"
  "Unable to connect to proxy"
  "SocksHTTPConnection"
  "Giving up after " + " retries"
```

### 7.4 与现有代码的落点提示（本次不改代码）

- 失败分支 `download_queue.rs:541-576` 现在**从不读 stderr**，且把 `ctx.proxy` 非空一律写成「代理连接失败」（`557-561`）。改造需要：`child.stderr.take()` → 并发读取（与 stdout 进度读取并行）→ 把 stderr 与 `ExitStatus::code()` 交给 `classify`。
- **spawn 失败分支（`379-384`）直接 `return`，不更新任何记录**：表现为记录永远停在非终态、前端看不到失败。若要做自动重试，这个分支必须先补上（并按 §7.1 判为不可重试）。
- 重试计数建议落在 `DownloadRecord`（新增字段）或内存队列，研究范围内不展开。
- **叠加放大提醒**：yt-dlp 自身对下载有 `--retries`（默认 **10**）、分片 `--fragment-retries`（默认 **10**）、提取 `--extractor-retries`（默认 **3**）、YouTube 另有 3 次 `RetryManager`（`options.py:1024-1043,1905-1907`）。应用层 3 次重试会让单条视频的最坏请求数成倍增长。若要「yt-dlp 放弃后再补刀」，可用 `Giving up after \d+ retries` 作为前置条件。

---

## 8. 置信度与版本敏感性总表

| 判据 | 置信度 | 版本敏感性 | 依据 |
|---|---|---|---|
| `HTTP Error <code>:` 文本 | **高** | **低** | 两版本逐字一致（`exceptions.py:63`）+ 实测 generic/BiliBili 一致 |
| 退出码 `0/1/2/101` | 高（实现）/ 中（不保证） | 低 | `__init__.py`、`YoutubeDL.py`；README 未文档化 |
| `Requested format is not available` | 高 | 低 | 两版本一致（`YoutubeDL.py:3059`），历史久 |
| `Tunnel connection failed: 407` | 高 | 低 | 实测 + `_urllib.py:430` |
| `TransportError/ProxyError/SSLError/CertificateVerifyError` | 高 | 中 | yt-dlp 自有异常类；类名重命名会失效 |
| DNS/超时/拒绝的**内部措辞** | 中高 | 中 | 来自 urllib3/requests/OS，随 handler 与库版本变 |
| SOCKS5 不可达**无 `proxy` 字样** | 高 | 中 | 实测（被判 `TransportError`） |
| `Giving up after N retries` | 中高 | 中 | `_utils.py:5296`，两版本一致 |
| YouTube 终态原因串（Video unavailable / Private video / Sign in to confirm） | 中 | **高** | 字符串由 YouTube 提供（`_video.py:4047-4076`） |
| BiliBili 412/352 文案 | 中 | 中高 | `bilibili.py:1405-1416`，站点相关 |
| `The downloaded file is empty` | 低（语义含糊） | 中 | 实测（HLS 分片 403 掩盖后的结果） |

**跨版本核验结论**：`2026.03.17`（apt）与 `2026.08.19`（zipapp）在以下位置**逐字一致** —— `HTTP Error {status}: {reason}`、`Requested format is not available…`、`unable to download video data:`、`Giving up after … retries`、`tunnel connection failed`、以及 `downloader/http.py` 的 `err.status < 500 or err.status >= 600 → 不重试 / 否则 RetryDownload` 判定。因此本报告的关键判据不绑定单一补丁版本。

---

## 9. 与既定重试策略的一致性审查

既定策略「能解析 HTTP 状态则仅 5xx/408/429 重试；解析不出则一律重试」在下述场景**会与直觉冲突**，需要在实现时明确取舍（本报告只陈述事实与影响）：

| 场景 | 策略输出 | 实际是否值得重试 | 说明 |
|---|---|---|---|
| `Requested format is not available` | 无状态码 → **重试** | ❌ 不值得 | 必须靠 §4.4 黑名单才能挡住 |
| YouTube `Video unavailable` / `Private video` | 无状态码 → **重试** | ❌ 不值得 | 同上，且措辞不稳 |
| 需登录 / 地区限制 | 无状态码 → **重试** | ❌ 不值得 | 同上 |
| 代理 407 | 无 `HTTP Error` → **重试** | ❌ 不值得 | 需 §4.2 显式识别 |
| B 站 412/352 | 无 `HTTP Error` → **重试** | ✅ 值得 | 属限流，重试合理（策略「意外地」正确） |
| HLS 分片 403 掩盖 | 无状态码 → **重试** | ⚠️ 存疑 | 可能只是分片级失败，重试成本低 |
| DNS/超时/TLS/代理握手 | 无状态码 → **重试** | ✅ 值得 | 策略目标场景 |

> **建议**：保留「未知 → 可重试」作为兜底，但**在其之前插入一个高置信度终态黑名单**（至少 `Requested format is not available`、`Unsupported URL`、`Private video` / `Video unavailable` / `Sign in to confirm`）。这偏离了题面「白名单 + 未知即重试」的字面表述，但若不插入，第 5 类「非网络类终态失败」将全部被重试 3 次——与题面把它们单列为「终态」的意图相矛盾。此点请以决策者判断为准。

---

## 10. 局限与未验证项

1. **未跑真实成功下载**：成功路径的 stdout 形态（仅 `after_move:filepath`）来自既有代码契约与设计文档，未在本轮重新实证。
2. **YouTube 终态串未实测**：受本机到 YouTube 的网络抖动影响（反复 `Connection reset by peer` / `Network is unreachable`），`Video unavailable` / `Private video` / `Sign in to confirm you're not a bot` 三类**仅由源码推导**，见 §4.4 的敏感性提示。
3. **HTTP handler 的选取未穷举**：本机未安装 `curl_cffi` / `websockets`，实测走的是 `requests`（与部分 `urllib`）handler。用户机器上若安装了 `curl_cffi`，**网络类内部措辞可能不同**（`_curlcffi.py` 映射 Curl 错误码），但 yt-dlp 自己的 `TransportError/ProxyError/SSLError` 包装层不变 —— 这也是建议优先匹配这些类名的原因。
4. **下载阶段的实测**：`unable to download video data: …` 由源码给出（`YoutubeDL.py:3596-3598`），本地 HLS 桩未能稳定复现该前缀（分片 403 被 `--skip-unavailable-fragments` 默认吞掉，最终表现为 `The downloaded file is empty`）。但其中 `HTTP Error <code>` 子串与提取阶段同源，正则仍然适用。
5. **未覆盖**：`--cookies` 失效、`--impersonate` 缺失、PO Token、ffmpeg 后处理失败（`PostProcessingError`）等与本次重试策略无关的失败类型。

---

## 11. 一句话交付

**分类器应以「stderr 中最后一条 `ERROR:` 行」为输入，用 `HTTP Error (\d{3}):` 做主判据（5xx/408/429 重试，其余 4xx 不重试），显式补 `Tunnel connection failed: 407`，用终态黑名单挡掉 `Requested format is not available` 等无状态码的终态失败，用网络白名单兜住 DNS/超时/TLS/代理握手，剩余未知一律落到「可重试」。退出码不参与原因分类。**
