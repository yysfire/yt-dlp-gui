# Implementation Plan: 核心下载功能

**Branch**: `027-subscription-management` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/002-core-download/spec.md`

## Summary

实现自动定时检查和视频下载功能。核心包括：tokio 定时调度器按周期检查已启用订阅、FIFO 下载队列管理、下载进度实时推送、暂停/继续/取消下载操作、基于视频 URL 的重复下载避免。

技术方案：Rust 后端 tokio::interval 定时器驱动检查，check_and_download() 为核心算法，Tauri 事件系统推送实时状态到前端。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18
**Primary Dependencies**: Tauri v2, tokio 1.x (full), serde_json 1.x, chrono 0.4
**Storage**: JSON 文件 (`download_records.json`, `state.json`)
**Testing**: `cargo test`, manual integration testing for yt-dlp subprocess
**Target Platform**: Windows, macOS, Linux 桌面端
**Project Type**: Desktop application (Tauri v2)
**Performance Goals**: 检查响应 < 5s/频道, 下载不阻塞 UI, 内存 < 200MB
**Constraints**: 依赖 yt-dlp CLI, 无内置下载引擎, 网络中断需优雅降级
**Scale/Scope**: 单用户, 并发下载数 ≤ 3, 订阅数 < 100

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | download.rs commands 有单元测试 | ✅ 已覆盖 |
| I. TDD | check_and_download 核心算法有测试 | ✅ 测试覆盖流程 |
| II. 可测试性 | YtDlpService 通过参数注入依赖 | ✅ 路径/代理/cookie 均为参数 |
| II. 可测试性 | Tauri Event 隔离可在测试中 mock | ✅ 事件通过 app_handle 参数传入 |
| III. KISS | FIFO 简单队列而非复杂调度 | ✅ |
| III. KISS | JSON 文件记录去重而非数据库索引 | ✅ |
| IV. DRY | 单个订阅检查和批量检查共享 check_and_download | ✅ |
| V. YAGNI | MVP 不用实现下载速度限制/断点续传 | ✅ P1 阶段功能 |

**Gate Result**: ✅ ALL PASS

## Project Structure

```text
# 已实现的源文件
src-tauri/src/
├── commands/download.rs    # check_subscription, check_all_subscriptions, check_and_download
├── services/ytdlp.rs       # check_new_videos(), download_video()
├── services/storage.rs     # load/save download_records, state
├── services/scheduler.rs   # SchedulerService (可复用)
├── models/download.rs      # DownloadRecord
├── models/settings.rs      # AppSettings (check_interval)
└── lib.rs                  # tokio::spawn 后台调度器

src/
├── hooks/useDownloadRecords.ts  # records state management
├── lib/tauri.ts                 # check_subscription, check_all_subscriptions
└── types/index.ts               # DownloadRecord interface
```

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。
