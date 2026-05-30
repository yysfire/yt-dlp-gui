# Implementation Plan: 基本界面

**Branch**: `001-mvp-core-features` | **Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-basic-ui/spec.md`

## Summary

实现应用的基本界面框架：标签页式主窗口布局（订阅管理、下载管理、设置三个标签页）、各功能页面布局、系统托盘图标及右键菜单（显示主窗口/检查全部/退出）、亮色和暗色主题切换。这是整个应用的基础交互框架，所有 MVP 功能依托于此呈现。

技术方案：重构现有 `AppShell.tsx` 的双栏布局为标签页式布局，使用 MUI `Tabs` 组件实现标签切换。各标签页通过状态保持（不卸载组件），系统托盘通过 Tauri v2 的 `tauri::tray::TrayIconBuilder` API 实现。暗色主题复用现有 MUI 主题系统（基于 `AppSettings.dark_mode` 字段，默认跟随系统主题）。

## Technical Context

**Language/Version**: Rust 2021 edition + TypeScript 5.x / React 18  
**Primary Dependencies**: Tauri v2, tokio 1.x (full), MUI 5, Tailwind CSS 3  
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/settings.json`), theme 偏好存储在 AppSettings 中  
**Testing**: `cargo test` (Rust), `npx tsc --noEmit` (TypeScript)  
**Target Platform**: Windows, macOS, Linux 桌面端 (Tauri v2)  
**Project Type**: Desktop application (Tauri v2, WebView frontend)  
**Performance Goals**: 启动到主窗口 < 3s, 标签切换 < 300ms, 布局 800px-2560px 正常, 托盘菜单 < 500ms, 主题切换 < 500ms  
**Constraints**: 最小窗口 800x600, 交互目标 ≥ 24x24px, 标签页保持状态不卸载  
**Scale/Scope**: 单用户本地应用，3 个标签页

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 前端组件 MUST 通过手动测试验证 | ✅ 布局和交互在开发中可即时验证 |
| I. TDD | 主题切换 MUST 通过视觉回归测试 | ✅ 手动切换亮暗模式验证 |
| II. 可测试性 | 标签页组件通过 Props 解耦 | ✅ 各 TabPanel 独立组件，无共享状态 |
| II. 可测试性 | 主题逻辑通过 AppSettings.dark_mode 驱动 | ✅ 单一数据源 |
| III. KISS | 不引入 React Router | ✅ MUI Tabs 组件足够，无路由需求 |
| III. KISS | 主题复用 MUI ThemeProvider | ✅ 已有 dark mode 基础设施 |
| IV. DRY | 页面状态保持统一机制 | ✅ 所有标签页通过 `display: none` 保持挂载 |
| IV. DRY | 托盘菜单复用已有命令 | ✅ 检查全部 → `check_all_subscriptions`，显示窗口 → `show()` |
| V. YAGNI | 不实现侧边栏式导航 | ✅ 规格明确"顶部水平标签栏" |
| V. YAGNI | 不实现多语言支持 | ✅ 规格明确"默认为中文，后续可扩展" |

**Gate Result**: ✅ ALL PASS — 无需复杂性论证

## Project Structure

### Documentation (this feature)

```text
specs/005-basic-ui/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output (N/A for UI feature)
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (IPC contracts)
│   └── tray-commands.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/src/
├── lib.rs                    # [扩展] TrayIconBuilder 配置（图标、菜单、事件处理）
├── main.rs                   # [不变]
├── commands/
│   └── mod.rs                # [不变]
├── models/
│   └── settings.rs           # [不变] dark_mode 字段已存在
└── icons/                    # [确认] 托盘图标文件存在

src/
├── App.tsx                     # [重构] 控制 Tab 索引状态，传递主题设置
├── components/
│   ├── AppShell.tsx            # [重构] 改为 Tabs + TabPanel 布局
│   ├── TopBar.tsx              # [扩展] 新增标签页导航栏（取代原汉堡菜单）
│   ├── TabNavigation.tsx       # [新增] MUI Tabs 标签页导航
│   ├── SubscriptionPage.tsx    # [新增] 订阅管理标签页内容（整合 SubscriptionList + DetailPanel）
│   ├── DownloadPage.tsx        # [新增] 下载管理标签页内容（整合 DownloadManager）
│   ├── SettingsPage.tsx        # [新增] 设置标签页内容（原 SettingsDialog 改为页面）
│   ├── SubscriptionList.tsx    # [合并] 订阅列表整合到 SubscriptionPage 内
│   ├── DownloadManager.tsx     # [新/复用] 从 002 功能引入的下载管理页面
│   └── StatusBar.tsx           # [扩展] 新增托盘状态指示器
├── hooks/
│   ├── useSubscriptions.ts     # [不变]
│   └── useDownloadRecords.ts   # [不变]
├── lib/
│   └── tauri.ts                # [扩展] 新增 toggleWindow、quit 等窗口操作
└── types/
    └── index.ts                # [不变] 无新增实体
```

**Structure Decision**: 从现有的双栏布局（侧边栏 + 详情面板）重构为标签页式布局。这是结构性变更——现有的 `SubscriptionList` + `DetailPanel` 合并到 `SubscriptionPage`；`SettingsDialog` 对话框改为 `SettingsPage` 全页面；新增 `DownloadPage` 整合队列和历史记录。托盘功能在 `lib.rs` 的 `setup()` 阶段配置，菜单事件通过 Tauri 的 `on_menu_event` 回调处理。

## Complexity Tracking

> 无违规项 — 所有 Constitution Check 通过。

待确认的技术风险点（在 Phase 0 research 中验证）：
- **标签页架构重构**: 当前 AppShell 是双栏（左侧列表 + 右侧详情），重构为标签页后 DetailPanel 的概念被弱化。需评估是否保留订阅项的展开/详情交互，还是全部改为页面内操作。
- **SettingsDialog → SettingsPage**: 当前设置是弹窗，改为页面后各设置表单需要即时保存（而非弹窗的"保存按钮"模式），这涉及 UX 行为变更。
- **系统托盘 Linux 兼容性**: 某些桌面环境（如 Wayland）对托盘支持有限，需评估 Tauri v2 tray API 的 Linux 兼容性。
- **关闭按钮行为**: 规格假设"关闭是最小化到托盘"，需在 Tauri 配置中设置 `close_requested` 事件处理，阻止默认关闭行为。
