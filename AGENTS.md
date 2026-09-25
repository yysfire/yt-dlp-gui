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
cargo test               # 运行所有 Rust 测试（共 234 个单元测试）
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
  └── AppShell.tsx            # 双栏布局（侧边栏 + 主面板）+ 视图切换 + 弹窗容器
        ├── TopBar.tsx             # 顶栏：Logo、标题、设置/添加按钮
        ├── SubscriptionList.tsx   # 左侧边栏：订阅列表 + 操作栏
        │     ├── FilterBar.tsx         # 筛选/排序控件
        │     └── SubscriptionItem.tsx  # 单个订阅行
        ├── DetailPanel.tsx        # 主面板：频道信息 + 统一视频列表
        │                          #   （把频道视频/下载记录/队列任务三路合并为一个列表）
        ├── DownloadedList.tsx     # 「已下载」视图
        │     ├── FileSearchBar.tsx
        │     └── DownloadedItem.tsx
        ├── HealthCheckPanel.tsx   # 失效频道健康检查面板
        ├── StatusBar.tsx          # 底部状态栏
        └── 弹窗（由 AppShell 管理状态）:
              ├── AddSubscriptionDialog.tsx
              ├── SettingsDialog.tsx
              ├── ExportDialog.tsx
              └── ImportDialog.tsx
```

**状态管理**: `src/hooks/` 中的五个自定义 Hook：
- `useSubscriptions()` — 订阅列表的增删改查，持有 `Subscription[]` 状态
- `useDownloadRecords()` — 下载记录的获取/检查，持有 `DownloadRecord[]` 状态
- `useDownloadProgress()` — 订阅 `download-progress`，按 video_url 索引的进度表
- `useHealthCheck()` — 健康检查的进度与摘要，订阅两个 health 事件
- `useFilter()` — 订阅列表的筛选与排序（纯前端派生，无后端调用）

不使用 Redux 或 Context。`App.tsx` 是唯一的状态中心，从 Hook 中提升状态并通过 props 向下传递。注意 `AppShell` 因此接收 30 个 props，其中大部分原样转给子组件。

**后端通信**: `src/lib/tauri.ts` 封装了全部 40 个 Tauri `invoke()` 调用，返回类型与 `src/types/index.ts` 一致（类型是手写断言，与 Rust 侧 serde 结构各自维护，没有校验机制，改字段时两边都要动）。前端通过 `listen()` 订阅后端推送事件，事件名分散在多个文件中定义，**没有单一契约清单**：

- `records-changed` — 记录发生任何变化（无载荷，订阅者各自全量重取；下载队列里 emit 十几处）
- `download-progress` — 单条下载的进度（带载荷，`useDownloadProgress` 消费）
- `download-complete` — 单个视频下载完成
- `queue-changed` — 下载队列状态变化（带载荷）
- `scheduler-check-complete` — 后台调度器完成一轮检查
- `subscriptions-updated` — 批量导入/删除后
- `file-sync-complete` — 文件存在性同步结果
- `health-check-progress` / `health-check-complete` — 健康检查
- `import-progress` / `import-complete` — 批量导入
- `tray-state-changed` — 由后端 emit，**当前前端无订阅者**

使用的 Tauri 插件：`dialog`（文件对话框）、`notification`（桌面通知）、`shell`（打开 URL）。

### Rust 后端（`src-tauri/src/`）

`lib.rs` 定义模块划分、注册命令（40 个，见末尾 `generate_handler!`）、装配全局状态并启动后台任务。

```
commands/               # Tauri IPC 命令处理函数（40 个注册命令）
  subscription.rs       # 订阅增删改查、分组、batch_delete、get_channel_info
  download.rs           # check_subscription / check_all、记录查询、队列控制、get_channel_videos
  settings.rs           # get/update 设置、get_app_state、路径与代理校验、start/stop_scheduler
  health.rs             # check_all_health / check_selected_health（快照后 spawn 后台任务）
  import_export.rs      # JSON/OPML 导出、三种导入源（OPML 文件 / TXT 文件 / URL 列表）
  file_manager.rs       # 打开所在文件夹、文件存在性检查、删除文件、同步文件状态

services/               # 业务逻辑（多数无状态，通过参数接收路径/配置）
  storage.rs            # JSON 持久化 + 写事务（见「持久化」一节）
  download_queue.rs     # 下载队列：并发控制、子进程生命周期、暂停/恢复/取消（有状态）
  ytdlp.rs              # yt-dlp 命令行封装（解析频道、检查视频、下载视频）
  opml.rs               # OPML 2.0 XML 导入/导出（quick-xml）
  health.rs             # HTTP 健康检查（reqwest）
  file_manager.rs       # 文件存在性/删除，并与记录状态联动
  settings_validator.rs # 下载路径与代理 URL 校验（纯函数）
  tray.rs               # 系统托盘：菜单、状态、图标
  scheduler.rs          # 定时器封装 —— 注意：真实调度循环内联在 `lib.rs` 的 setup 里，
                        #   本文件的 SchedulerService 当前无调用者

models/                 # 数据结构（serde 序列化/反序列化）
  subscription.rs, download.rs, settings.rs, import_export.rs, health.rs, video.rs

utils/
  error.rs              # AppError 枚举（12 个变体，thiserror）
  progress_parser.rs    # yt-dlp 进度行解析（纯函数）
```

**关键设计规则**：
- Commands 层**不包含业务逻辑** —— 仅做参数传递、服务调用和序列化返回。
- Services 层**以无状态为主** —— 通过参数接收路径/配置，不持有全局状态。例外：`download_queue.rs` 与 `tray.rs` 是有状态的（队列、托盘句柄），通过 `AppContext` / `QueueContext` 注入。
- 全局状态通过 `AppContext`（data_dir + `Mutex<AppSettings>` + scheduler watch channel）与 `QueueContext`（下载队列）管理，通过 `tauri::State` 注入。
- 持有 `MutexGuard` 时，**先克隆值并释放锁，再进行 `await`**，避免跨异步边界持有互斥锁。**特别注意不要持锁跨越阻塞式子进程调用**（如 yt-dlp）。
- **持久化写入只有一个入口**：`StorageService::update_download_records` / `update_subscriptions` / `update_state`（闭包式事务，内部持有全局写锁）。`save_subscriptions` / `save_download_records` / `save_state` 是私有原语，外部不可调用（编译器强制）；`load_*` 不加锁。
  事务闭包内**禁止**：调用 `update_*` / `save_settings`（`std::sync::Mutex` 不可重入，会死锁；debug 构建下会 panic 提示）、获取任何其他应用级锁、做文件 I/O。锁序固定为 `QueueContext.queue → 写锁`，不要引入反向路径。
- **不要用陈旧快照整体覆盖**：跨 `await` 的长流程（`lib.rs` 的 scheduler 循环、`check_all_subscriptions`）一律「边跑边收集结果（`SubCheckOutcome`），收尾按 id 做一次增量事务」。用循环开始时的快照整表 `save` 会 clobber 并发下载回调写入的 `download_count` / `total_downloads`。

**`check_and_download()`**（`commands/download.rs`）是检查入口：按 `last_check_time` 换算日期下界 → `yt-dlp --flat-playlist` 取新视频 → 对已有记录去重（`failed` 可重试）→ 逐个交给 `DownloadQueue::enqueue_from_video()` 入队。**真正的下载、进度解析与记录状态落库都在 `download_queue.rs` 的 `execute_download_with_control()` 里**（该函数按 `video_url + subscription_id` 定位记录并做事务写入）。

`check_and_download` 还有一个「无队列时直接建记录」的兜底分支 —— 因为 `QueueContext` 在 `lib.rs` setup 里必然注册，该分支实际上走不到。

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
  → StorageService::load_subscriptions() → 只读预检重复
  → YtDlpService::parse_channel_info() → yt-dlp 子进程（阻塞，不持任何锁）
  → 创建 Subscription { id: uuid, ... }
  → StorageService::update_subscriptions() → 事务内复查重复并追加
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

`StorageService` 负责所有文件 I/O：

- 读：`load_subscriptions()` / `load_download_records()` / `load_state()` / `load_settings()`，**不加锁**（`read_json_array()` 在文件不存在时返回 `[]`）。
- 写：只有 `update_download_records()` / `update_subscriptions()` / `update_state()` 三个闭包式事务（`save_settings()` 是唯一的非事务公开写接口，settings 是整对象替换）。事务语义是「拿写锁 → 读磁盘当前内容 → 交给闭包 → `Ok` 才写回，`Err` 丢弃改动」。
- `write_json()` 采用「写同目录 `*.tmp` + 原子 `rename`」（失败退避重试 3 次），因此读端不会读到被截断的半个文件，也不需要读锁。

**新增写入逻辑时**：加一个 `update_*` 闭包调用，不要新增 `save_*`；如需新增实体类型，按同样模式加 `update_xxx` + 私有 `save_xxx` 并在 `with_write_lock` 下执行。

### 测试

所有 Rust 测试均为源代码文件内的 `#[cfg(test)]` 模块 —— 共 234 个测试函数，分布在 models、services 和 commands 中。测试重点包括序列化往返、默认值、OPML 解析/构建、存储事务语义（提交/回滚/并发无丢失更新）、健康状态更新和画质格式字符串。没有需要实际执行 yt-dlp CLI 的集成测试。

前端测试用 vitest + @testing-library/react（`npm test`），源码在 `src/**/__tests__/`。注意 `src/components/__tests__/DetailPanel.test.tsx` 中有一条断言仍引用已被替换的文案（`暂无下载记录`，实际已是 `暂无视频`），该用例当前是失败的。

### 平台支持

通过 Tauri 的跨平台打包支持 Windows、macOS 和 Linux。提供了三个平台的图标（`icon.ico`、`icon.icns`、`.png` 变体）。

<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
at `specs/007-unified-video-list/plan.md`
<!-- SPECKIT END -->
