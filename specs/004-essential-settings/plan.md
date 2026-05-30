# Implementation Plan: 必要设置

**Branch**: `001-mvp-core-features` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/004-essential-settings/spec.md`

## Summary

实现应用的必要设置功能：全局下载路径设置（含文件夹浏览和路径验证）、yt-dlp 基本参数配置（画质预设选择、代理服务器地址）、并发下载任务数设置（1-5）、定时检查频率设置（手动/30分钟/每小时/每天）。所有设置持久化到 JSON 文件，应用重启后自动恢复，设置损坏时降级为安全默认值。

技术方案：扩展现有 `AppSettings` 结构体，新增 `concurrent_downloads` 字段。路径验证通过 `std::fs::create_dir_all` 测试可写性，代理格式通过 URL 解析验证。并发数调整时通过下载队列 Manager 动态调度（暂停多余任务或启动等待任务）。检查频率调整时更新 tokio `interval` 定时器。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18  
**Primary Dependencies**: Tauri v2, tokio 1.x (full), serde 1.x, MUI 5, Tailwind CSS 3  
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/settings.json`), StorageService 读写  
**Testing**: `cargo test` (Rust), `npx tsc --noEmit` (TypeScript)  
**Target Platform**: Windows, macOS, Linux 桌面端 (Tauri v2)  
**Project Type**: Desktop application (Tauri v2, WebView frontend)  
**Performance Goals**: 设置保存 < 500ms, 路径验证 < 2s, 重启恢复准确率 100%, 并发调整后调度 < 3s  
**Constraints**: 单用户本地应用, 设置文件损坏时必须有降级方案  
**Scale/Scope**: 单用户本地应用, 设置字段数 < 15

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 设置字段验证逻辑 MUST 先有单元测试 | ✅ 路径验证、代理格式验证可独立测试 |
| I. TDD | 设置默认值和降级逻辑 MUST 先有测试 | ✅ `AppSettings::default()` 已有测试，需补降级测试 |
| II. 可测试性 | 设置验证与 Tauri 运行时解耦 | ✅ 验证函数不依赖 State，纯输入输出 |
| II. 可测试性 | 设置持久化通过 StorageService 抽象 | ✅ 已有 `load_settings`/`save_settings` 可 mock |
| III. KISS | 设置文件单一来源 | ✅ 仅 `settings.json`，不引入多文件配置 |
| III. KISS | 路径可写性验证使用 `create_dir_all` + 写测试文件 | ✅ 简单直接 |
| IV. DRY | 画质预设与订阅画质设置共享映射表 | ✅ 复用现有 `quality_preset` 格式字符串映射 |
| IV. DRY | 代理验证逻辑统一 | ✅ URL 解析一次，多处使用 |
| V. YAGNI | 不实现订阅级别的独立下载路径 | ✅ 规格仅要求全局路径 |
| V. YAGNI | 不实现配置文件导入/导出 | ✅ 规格未要求 |

**Gate Result**: ✅ ALL PASS — 无需复杂性论证

## Project Structure

### Documentation (this feature)

```text
specs/004-essential-settings/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (IPC contracts)
│   └── settings-commands.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── commands/
│   ├── settings.rs        # [扩展] 新增 validate_download_path 命令
│   └── mod.rs
├── services/
│   ├── settings_validator.rs  # [新增] 路径验证、代理格式验证
│   ├── storage.rs             # [复用] 设置 JSON 读写
│   └── mod.rs                 # [扩展] 声明新模块
├── models/
│   ├── settings.rs            # [扩展] 新增 concurrent_downloads 字段，AppSettings 扩展
│   └── mod.rs
└── utils/
    └── error.rs               # [扩展] 新增 InvalidPath、InvalidProxy 错误变体

src/
├── components/
│   └── SettingsDialog.tsx     # [扩展] 新增路径浏览、并发数、频率选择等控件
├── lib/
│   └── tauri.ts               # [扩展] 新增 validateDownloadPath invoke
└── types/
    └── index.ts               # [扩展] AppSettings 新增 concurrent_downloads 字段
```

**Structure Decision**: 扩展现有 `AppSettings` 结构体而非创建新的设置实体。新增 `settings_validator.rs` Service 处理路径和代理验证。前端 `SettingsDialog.tsx` 增设下载路径（含文件夹浏览）、并发数滑块、检查频率下拉等控件。设置修改后通过 `update_settings` 命令即时持久化。

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。

待确认的技术风险点（在 Phase 0 research 中验证）：
- **动态调整 tokio interval**: 当前 scheduler 在 `lib.rs::setup()` 中启动一个固定的 interval，频率调整需要重建 task 或使用 `tokio::sync::watch` 通知变更
- **并发数降低的调度策略**: 规格要求"暂停多余任务"，需明确按什么规则选择暂停哪个（FIFO 先暂停队列中等待的，已下载中的记录进度后续继续）
- **代理地址验证**: URL 解析库（url crate）与 socks5:// 格式的兼容性
