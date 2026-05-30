# Implementation Plan: 基本文件管理

**Branch**: `001-mvp-core-features` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/003-basic-file-management/spec.md`

## Summary

实现已下载视频文件的管理功能：浏览已下载视频列表（标题、频道、下载时间、文件大小）、按标题关键词搜索（不区分大小写）、从列表中一键打开文件所在文件夹（跨平台）、文件系统同步检测（标记用户手动删除的文件）。这些功能让用户在下载完成后高效管理本地视频库。

技术方案：基于现有 `DownloadRecord` 实体扩展，新增"文件是否存在"标记字段。搜索在前端过滤实现（500 条记录内性能无压力），文件检测通过 Rust 的 `std::fs::metadata()` 实现，跨平台打开文件夹通过 Tauri 的 `shell` plugin 调用系统命令（`explorer` / `open` / `xdg-open`）。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18  
**Primary Dependencies**: Tauri v2, tokio 1.x (full), serde 1.x, chrono 0.4, MUI 5, Tailwind CSS 3  
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/download_records.json`), StorageService 读写  
**Testing**: `cargo test` (Rust), `npx tsc --noEmit` (TypeScript)  
**Target Platform**: Windows, macOS, Linux 桌面端 (Tauri v2)  
**Project Type**: Desktop application (Tauri v2, WebView frontend)  
**Performance Goals**: 500 条记录加载 < 2s, 搜索响应 < 500ms, 打开文件夹 < 2s, 同步 500 文件 < 10s  
**Constraints**: 单用户本地应用, 文件仅通过路径是否存在判断（不校验内容完整性）  
**Scale/Scope**: 单用户本地应用, 下载记录 < 1000

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 文件存在性检测 MUST 先有单元测试 | ✅ 可通过 mock 文件系统测试 |
| I. TDD | 搜索不区分大小写逻辑 MUST 先有测试 | ✅ 纯函数可独立测试 |
| II. 可测试性 | 文件检测 Service 与 Tauri 运行时解耦 | ✅ 仅依赖 `std::fs`，无需 Tauri 上下文 |
| II. 可测试性 | 前端搜索过滤与 UI 组件分离 | ✅ 搜索过滤函数独立于组件 |
| III. KISS | 搜索在前端内存过滤，不实现后端全文搜索 | ✅ 500 条记录内存过滤完全满足 |
| III. KISS | 文件检测仅检查存在性，不校验大小/哈希 | ✅ 规格明确"以路径存在为依据" |
| IV. DRY | 复用现有 `get_all_download_records` 获取记录 | ✅ 无新增数据获取命令 |
| IV. DRY | 打开文件夹功能统一封装为单一 Tauri 命令 | ✅ `open_in_folder` 使用 shell plugin |
| V. YAGNI | 不实现删除下载记录功能 | ✅ 规格未要求 |
| V. YAGNI | 不实现文件重命名/移动功能 | ✅ 规格未要求 |

**Gate Result**: ✅ ALL PASS — 无需复杂性论证

## Project Structure

### Documentation (this feature)

```text
specs/003-basic-file-management/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (IPC contracts)
│   └── file-commands.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── commands/
│   ├── download.rs        # [扩展] 新增 open_in_folder、check_file_existence 命令
│   └── mod.rs
├── services/
│   ├── file_manager.rs    # [新增] 文件存在性检测、路径解析
│   ├── storage.rs         # [复用] 下载记录加载
│   └── mod.rs             # [扩展] 声明新模块
├── models/
│   ├── download.rs        # [扩展] 新增 file_exists 字段
│   └── mod.rs
└── utils/
    └── error.rs           # [扩展] 新增文件不存在错误变体

src/
├── components/
│   ├── DownloadManager.tsx     # [复用] 已下载视频列表在此页面内
│   ├── DownloadedVideoList.tsx # [新增] 已下载视频列表（含搜索框、打开文件夹按钮）
│   ├── DownloadedVideoItem.tsx # [新增] 单个已下载视频行（标题/频道/大小/时间/状态）
│   └── FileSearchBar.tsx       # [新增] 搜索框组件（实时筛选）
├── hooks/
│   └── useDownloadRecords.ts   # [扩展] 新增搜索过滤、文件同步触发
├── lib/
│   └── tauri.ts                # [扩展] 新增 openInFolder、checkFilesExist invoke
└── types/
    └── index.ts                # [扩展] DownloadRecord 新增 file_exists 字段
```

**Structure Decision**: 在现有 `download.rs` 命令文件中扩展文件管理相关命令。新增 `file_manager.rs` Service 处理文件系统操作（存在性检测、打开文件夹）。搜索功能纯前端实现，使用 `useMemo` 对 `DownloadRecord[]` 按搜索词过滤。文件同步在应用启动时执行一次 + 用户手动触发。

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。

待确认的技术风险点（在 Phase 0 research 中验证）：
- **跨平台 open_in_folder**: 需确认 Tauri shell plugin 的 `open` API 在不同平台对文件夹路径的处理，以及是否支持定位到具体文件（而非仅打开目录）
- **文件同步性能**: 500 个文件的批量检测需评估是串行检查还是使用 `tokio::task::spawn_blocking` 并发检查
- **DownloadRecord 向后兼容**: 新增 `file_exists` 字段需处理旧记录缺失该字段的 JSON 反序列化（使用 `#[serde(default)]`）
