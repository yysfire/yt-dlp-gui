# CODEBUDDY.md

本文件为 CodeBuddy Code 在此仓库中工作提供指导。

## 项目概述

基于 Tauri v2 的 yt-dlp 桌面端订阅管理器。用户可以订阅 YouTube/Bilibili 频道，应用定期检查新视频并自动下载。

- **前端**: React 18 + TypeScript + MUI 5 + Tailwind CSS 3 + Vite 5
- **后端**: Rust（Tauri v2，tokio 异步运行时）
- **存储**: JSON 文件，存放于 `~/.yt-dlp-sub-gui/`（subscriptions.json、download_records.json、settings.json、state.json）

## 开发命令

```bash
# 前端（Vite 开发服务器，端口 5173）
npm run dev              # 启动 Vite 开发服务器（仅前端）
npm run build            # TypeScript 检查 + Vite 生产构建
npm run preview          # 预览生产构建

# 完整 Tauri 应用
npm run tauri dev        # 启动 Tauri 开发模式（前端 + Rust 后端）
npm run tauri build      # 构建 Tauri 生产二进制文件

# Rust
cargo test               # 运行所有 Rust 测试（7 个文件中共 69 个单元测试）
cargo test -p yt-dlp-gui # 仅运行此包的测试
cargo build              # 仅构建 Rust 后端
cargo check              # 快速编译检查，不生成二进制文件

# TypeScript
npx tsc --noEmit         # 仅类型检查，不输出文件
```

## 架构

### 前端（`src/`）

单页面应用，**无路由** —— 通过状态选择驱动视图切换。

```
App.tsx                      # 根组件：主题提供者、暗色模式、事件监听、状态持有者
  └── AppShell.tsx            # 双栏布局（侧边栏 + 详情面板）+ 弹窗容器
        ├── TopBar.tsx        # 顶栏：Logo、标题、设置/汉堡按钮
        ├── SubscriptionList.tsx  # 左侧边栏：订阅列表 + 操作栏
        │     └── SubscriptionItem.tsx  # 单个订阅行
        ├── DetailPanel.tsx   # 右侧面板：选中订阅详情 + 下载记录
        │     └── DownloadRecordList.tsx
        │           └── DownloadRecordItem.tsx
        ├── StatusBar.tsx     # 底部状态栏
        └── 弹窗（由 AppShell 管理状态）:
              ├── AddSubscriptionDialog.tsx
              ├── SettingsDialog.tsx
              ├── ExportDialog.tsx
              └── ImportDialog.tsx
```

**状态管理**: `src/hooks/` 中的两个自定义 Hook：
- `useSubscriptions()` — 订阅列表的增删改查，持有 `Subscription[]` 状态
- `useDownloadRecords()` — 下载记录的获取/检查，持有 `DownloadRecord[]` 状态

不使用 Redux 或 Context。`App.tsx` 是唯一的状态中心，从 Hook 中提升状态并通过 props 向下传递。

**后端通信**: `src/lib/tauri.ts` 封装了全部 18 个 Tauri `invoke()` 调用，返回类型与 `src/types/index.ts` 一致。前端监听三个后端推送事件：
- `records-changed` — 下载过程中的实时状态更新
- `download-complete` — 单个视频下载完成（仅启用通知时发送）
- `scheduler-check-complete` — 后台调度器完成一轮检查

使用的 Tauri 插件：`dialog`（文件对话框）、`notification`（桌面通知）、`shell`（打开 URL）。

### Rust 后端（`src-tauri/src/`）

`lib.rs`（第 8-9 行，参见模块声明）定义了四层架构：

```
commands/          # Tauri IPC 命令处理函数（18 个命令）
  subscription.rs  # add、delete、get、toggle_pause、update_quality
  download.rs      # check_subscription、check_all、get_records、manual_check_all
  settings.rs      # get、update、get_app_state、start/stop_scheduler
  import_export.rs # export_json、export_opml、batch_import（支持 OPML + TXT + URL 列表）

services/          # 无状态的业务逻辑
  storage.rs       # JSON 文件读写（所有实体类型）
  ytdlp.rs         # yt-dlp 命令行封装（解析频道、检查视频、下载视频）
  opml.rs          # OPML 2.0 XML 导入/导出（quick-xml）
  scheduler.rs     # 可复用的 tokio 定时器

models/            # 数据结构（serde 序列化/反序列化）
  subscription.rs, download.rs, settings.rs, import_export.rs

utils/
  error.rs         # AppError 枚举（Io、Serde、YtDlp、NotFound、Duplicate）
```

**关键设计规则**：
- Commands 层**不包含业务逻辑** —— 仅做参数传递、服务调用和序列化返回。
- Services 层**无状态** —— 通过参数接收路径/配置，不持有全局状态。
- 全局状态通过 `AppContext` 结构体管理（data_dir + `Mutex<AppSettings>`），通过 `tauri::State` 注入。
- 持有 `MutexGuard` 时，**先克隆值并释放锁，再进行 `await`**，避免跨异步边界持有互斥锁。

**`check_and_download()`**（`commands/download.rs`）是核心算法：对已有记录去重 → 创建"downloading"记录 → 发送事件 → 通过 yt-dlp CLI 下载 → 更新记录为"completed"/"failed" → 发送事件。

### yt-dlp 命令行调用

应用通过子进程（`std::process::Command`）调用 yt-dlp，有以下三种调用模式：

| 用途 | 命令 |
|------|------|
| 解析频道信息 | `yt-dlp --dump-json --playlist-items 1 <url>` |
| 检查新视频 | `yt-dlp --flat-playlist --dump-json --playlist-end 5 --dateafter <YYYYMMDD> <url>` |
| 下载视频 | `yt-dlp -f <format> -o <template> --no-playlist --print after_move:filepath <url>` |

画质预设映射为格式字符串（如 "1080p" → `bestvideo[height<=1080]+bestaudio/best[height<=1080]`）。代理（`--proxy`）和 Cookie（`--cookies`）仅非空时才传入。

### 数据流程：添加订阅

```
用户输入 URL → AddSubscriptionDialog → useSubscriptions.addSubscription()
  → invoke("add_subscription") → commands/subscription.rs::add_subscription()
  → StorageService::load_subscriptions() → 检查重复
  → YtDlpService::parse_channel_info() → yt-dlp 子进程
  → 创建 Subscription { id: uuid, ... } → StorageService::save_subscriptions()
  → 返回 Subscription → Hook 更新本地状态 → UI 刷新
```

### 命名约定

| 上下文 | 约定 | 示例 |
|--------|------|------|
| Rust 文件名 | `snake_case` | `download_record.rs` |
| Rust 结构体/枚举 | `PascalCase` | `DownloadRecord`, `AppError` |
| Rust 函数 | `snake_case` | `add_subscription` |
| Tauri 命令 | `snake_case`（与前端 `invoke` 字符串一致） | `add_subscription` |
| TypeScript 组件 | `PascalCase` | `SubscriptionList.tsx` |
| TypeScript Hook/工具 | `camelCase` | `useSubscriptions.ts` |
| TypeScript 接口 | `PascalCase` | `Subscription`, `DownloadRecord` |

### 持久化

数据以 JSON 文件存储在操作系统标准应用数据目录（通过 `app.path().app_data_dir()` 解析）：
- `~/.yt-dlp-sub-gui/subscriptions.json`
- `~/.yt-dlp-sub-gui/download_records.json`
- `~/.yt-dlp-sub-gui/settings.json`
- `~/.yt-dlp-sub-gui/state.json`

`StorageService` 负责所有文件 I/O：`read_json()`、`read_json_array()`（文件不存在时返回 `[]`）、`write_json()`（创建父目录，格式化输出）。

### 测试

所有测试均为源代码文件内的 `#[cfg(test)]` 模块 —— 共 69 个测试函数，分布在 models 和 services 中。测试重点包括序列化往返、默认值、OPML 解析/构建、存储 CRUD 和画质格式字符串。没有需要实际执行 yt-dlp CLI 的集成测试。

### 平台支持

通过 Tauri 的跨平台打包支持 Windows、macOS 和 Linux。提供了三个平台的图标（`icon.ico`、`icon.icns`、`.png` 变体）。

<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
at `specs/004-essential-settings/plan.md`
<!-- SPECKIT END -->
