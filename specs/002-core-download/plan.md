# Implementation Plan: 核心下载功能

**Branch**: `027-subscription-management` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/002-core-download/spec.md`

## Summary

在现有定时检查和视频下载基础上，补充 MVP 缺失的核心下载功能。主要新增：
1. **FIFO 下载队列** — 将检测到的新视频按检测顺序排队，支持可配置并发下载数
2. **实时进度推送** — 通过流式解析 yt-dlp 输出，向前端推送下载百分比、速度和大小
3. **暂停/继续/取消** — 进程级生命周期管理（通过 SIGSTOP/SIGCONT 暂停/继续，SIGKILL 取消并清理部分文件）
4. **失败重试与错误记录** — 在 DownloadRecord 中新增 `error_message` 字段，失败记录允许后续重试

技术方案：Rust 后端 tokio::sync::Semaphore 控制并发数，FIFO 队列由 `VecDeque<DownloadTask>` + `tokio::spawn` 异步任务组成。yt-dlp 子进程通过 `tokio::process::Command` 启用 stdout/stderr 流式输出，解析 yt-dlp 的 `--progress-template` 输出。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18
**Primary Dependencies**: Tauri v2, tokio 1.x (full), serde_json 1.x, chrono 0.4
**Storage**: JSON 文件 (`download_records.json`, `state.json`)
**Testing**: `cargo test`, manual integration testing for yt-dlp subprocess
**Target Platform**: Windows, macOS, Linux 桌面端
**Project Type**: Desktop application (Tauri v2)
**Performance Goals**: 检查响应 < 5s/频道, 下载不阻塞 UI, 内存 < 200MB, 队列任务切换 < 2s
**Constraints**: 依赖 yt-dlp CLI, 无内置下载引擎, 网络中断需优雅降级
**Scale/Scope**: 单用户, 并发下载数 1-3 (可配置), 订阅数 < 100

### 技术不确定项 (NEEDS CLARIFICATION → 见 research.md)

| 项 | 描述 |
|----|------|
| yt-dlp 进度输出格式 | yt-dlp 的 `--progress-template` 在不同版本间输出格式差异 |
| SIGSTOP/SIGCONT 跨平台 | Windows 不支持 POSIX 信号，需要用 Windows API 替代 |
| tokio::process::Command 流式输出 | 异步子进程 stdout/stderr 流式读取的 Buffer 大小与背压处理 |
| 并发控制机制 | Semaphore 与 JoinSet 的配合使用方式 |
| 视频 ID 去重 | yt-dlp `--flat-playlist` JSON 中的 id 字段是否可靠存在 |
| 取消时文件清理 | yt-dlp 生成的部分文件命名模式（`.part`、`.ytdl` 等临时文件） |

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | DownloadQueue 核心逻辑需单元测试 | ⭐ 新增 |
| I. TDD | yt-dlp 进度解析器有测试 | ⭐ 新增 |
| I. TDD | 暂停/继续/取消命令测试 | ⭐ 新增 |
| I. TDD | 现有 download.rs 命令测试 | ✅ 已覆盖 |
| II. 可测试性 | DownloadQueue 通过参数注入数据目录、app_handle | ✅ |
| II. 可测试性 | 进度解析器为纯函数(输入字符串→结构化事件) | ✅ |
| II. 可测试性 | YtDlpService 保持无状态 | ✅ 现有 + 新增 stream 方法 |
| III. KISS | FIFO 队列用 VecDeque + tokio::spawn，不引入复杂调度引擎 | ✅ |
| III. KISS | 进度解析器用简单正则，不引入完整解析器框架 | ✅ |
| III. KISS | JSON 文件记录去重，不引入数据库 | ✅ |
| IV. DRY | 检查逻辑复用现有 `check_and_download` | ✅ |
| IV. DRY | 下载流式执行复用 `download_video` 核心逻辑 | ✅ |
| V. YAGNI | 不实现断点续传、下载优先级、速度限制 | ✅ |
| V. YAGNI | 不实现 `--download-archive`（yt-dlp 原生 archive） | ✅ |
| V. YAGNI | 不实现 WebSocket 替代 Tauri Event 推送 | ✅ |

**Gate Result**: ✅ ALL PASS — 新增项按 TDD/KISS 准则设计

### Phase 1 后重新评估

| 原则 | 检查项 | Phase 1 验证结果 |
|------|--------|------------------|
| I. TDD | progress_parser.rs 纯函数 → 易于单元测试 | ✅ 输入/输出均为基础类型 |
| I. TDD | DownloadQueue 状态机 → 每个状态转换可测试 | ✅ 通过注入 Semaphore 和 JoinSet 模拟并发 |
| I. TDD | models 新增字段 → 序列化往返测试 | ✅ 遵循现有 models/download.rs 测试模式 |
| II. 可测试性 | DownloadQueue::new() 接收 data_dir, app_handle | ✅ 参数注入，无全局状态 |
| II. 可测试性 | 流式下载 download_video_streaming() 通过 BufReader + LineStream | ✅ 可 mock 管道输入 |
| II. 可测试性 | 进度解析器 parse_progress_line(s: &str) → Option<DownloadProgress> | ✅ 纯函数，无副作用 |
| III. KISS | 队列实现: VecDeque + Arc<Semaphore> + Arc<Mutex<JoinSet>> | ✅ 三个标准库组件 |
| III. KISS | 进程暂停: #[cfg(unix)] libc::kill / #[cfg(windows)] kernel32 | ✅ 最小条件编译 |
| IV. DRY | 下载逻辑: download_video_streaming() 复用 download_video() 的参数构建 | ✅ 格式化字符串和参数构建共享函数 |
| IV. DRY | 错误处理: 使用现有 AppError，新增变体而非新错误类型 | ✅ |
| V. YAGNI | 无持久化队列（重启后任务丢失，由 recover_state 恢复为 failed） | ✅ 符合 MVP 范围 |
| V. YAGNI | 无下载优先级、速度限制、断点续传 | ✅ spec 未要求 |

**Phase 1 Re-evaluation**: ✅ ALL PASS — 设计完全符合章程五条原则

## Project Structure

```text
# 需新增/修改的文件

src-tauri/src/
├── commands/
│   └── download.rs            # [修改] 新增 pause/resume/cancel 命令，重构为队列驱动
├── services/
│   ├── ytdlp.rs               # [修改] 新增 download_video_streaming() 流式下载
│   ├── download_queue.rs      # [新增] FIFO 下载队列管理
│   └── scheduler.rs           # [修改] 连接调度器到 DownloadQueue
├── models/
│   ├── download.rs            # [修改] DownloadRecord 新增 error_message, paused_at
│   └── settings.rs            # [修改] AppSettings 新增 max_concurrent_downloads
├── utils/
│   ├── error.rs               # [修改] 新增 ProcessKill 错误变体
│   └── progress_parser.rs     # [新增] yt-dlp 进度输出解析器
└── lib.rs                     # [修改] 注册新命令，启动 DownloadQueue

src/
├── components/
│   ├── DownloadRecordItem.tsx  # [修改] 合并队列任务和持久化记录，统一暂停/取消/重试按钮
│   └── DownloadProgressBar.tsx# [新增] 单个下载进度条组件
├── hooks/
│   ├── useDownloadRecords.ts   # [修改] 新增 pause/resume/cancel API 封装
│   └── useDownloadProgress.ts  # [新增] 实时进度事件监听器
├── lib/tauri.ts                # [修改] 新增 pause/resume/cancel invoke 调用
└── types/index.ts              # [修改] DownloadRecord 新增字段，新增 DownloadProgress
```

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。
>
> 新增复杂度说明：
> - **download_queue.rs**：引入 VecDeque + Semaphore 的组合来管理 FIFO + 并发控制，这是满足 FR-002 和 FR-009 的最小复杂度方案。
> - **progress_parser.rs**：独立的进度解析模块，因为 stream 模式下的行级解析逻辑足够独立，且方便单元测试（纯函数）。
> - **SIGSTOP/SIGCONT 跨平台**：Windows 上使用 `kernel32::SuspendThread/ResumeThread`，Unix 上使用 `kill(pid, SIGSTOP/SIGCONT)`。这是平台差异带来的必要复杂度。

## Phase 0: Research

详见 [research.md](./research.md)
- 决议了 yt-dlp 进度输出解析方案
- 决议了跨平台进程暂停方案
- 决议了 FIFO 队列与信号量设计
- 决议了视频去重的 URL vs ID 策略

## Phase 1: Design

详见以下文件：
- [data-model.md](./data-model.md) — 实体定义、字段说明、状态转换
- [contracts/](./contracts/) — Tauri 命令接口约定与 Tauri Event 格式
- [quickstart.md](./quickstart.md) — 开发快速入门指南
