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
cargo test               # 运行所有 Rust 测试（共 237 个单元测试）
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
- `useFilter()` — 订阅列表的筛选与排序（纯前端派生，无后端调用）。**在 `SubscriptionList` 内调用**，筛选/排序状态不提升到 `App`；纯逻辑另导出 `applyFilter` / `applySort` 供直接测试。

不使用 Redux 或 Context。**域数据**（订阅、下载记录、下载进度、健康检查）在 `App.tsx` 汇聚后通过 props 下发；**纯 UI 局部状态**各自持有：主面板视图与选中订阅由 `AppShell` 的 `AppView` 可辨识联合管理（`{ kind: "detail"; subscriptionId } | { kind: "downloads" }`，因此「已下载视图同时有选中项」这种非法组合无法表示），订阅筛选/排序由 `SubscriptionList` 内的 `useFilter` 管理。`AppShell` 接收 19 个 props。

> 注意：把状态下移到局部组件时，要确认该组件不会因折叠/切换而卸载（`SubscriptionList` 就依赖「始终挂载、仅 CSS 隐藏」这一点来保住筛选状态）。

**后端通信**: `src/lib/tauri.ts` 封装了全部 38 个 Tauri `invoke()` 调用，返回类型与 `src/types/index.ts` 一致（类型是手写断言，与 Rust 侧 serde 结构各自维护，没有校验机制，改字段时两边都要动）。前端通过 `listen()` 订阅后端推送事件，事件名分散在多个文件中定义，**没有单一契约清单**（下面这份即事实上的契约，改事件名或载荷时请同步此处）：

- `records-changed` — 下载记录发生变化（**无载荷**）。**唯一订阅者是 `App.tsx`**（`AppShell` 只订 `queue-changed`，`StatusBar` 只订 `scheduler-check-complete`）。所有 emit 统一走 `services/download_queue.rs::notify_records_changed`（纯 emit，零 I/O 零加锁，可在临界区安全调用）；契约：**凡写 `download_records.json` 的路径都必须调用它**
- `download-progress` — 单条下载的进度（带载荷，`useDownloadProgress` 消费）
- `download-complete` — 单个视频下载完成。注意它 emit 于记录落库**之前**，刷新职责实际由 `records-changed` 覆盖，属清理候选
- `queue-changed` — 下载队列状态变化（带载荷）
- `scheduler-check-complete` — **任一次检查完成**：自动调度与托盘「检查全部」经 `run_check_round`，手动「检查全部」经 `check_all_subscriptions`，手动单订阅经 `check_subscription`；均在该次检查的 `last_check_time` 落盘后 emit。语义是「一次检查完成」，**不是**「调度器轮次」
- `subscriptions-updated` — 批量导入/删除后
- `file-sync-complete` — 文件存在性同步结果
- `health-check-progress` / `health-check-complete` — 健康检查
- `import-progress` / `import-complete` — 批量导入
- `tray-state-changed` — 由后端 emit，**当前前端无订阅者**
- `settings-changed` — settings 落盘并同步缓存后（前端保存 **或** 托盘切换 `scheduler_paused`），**带完整 `AppSettings` 载荷**；`App.tsx` 用它同步 `darkMode`（取代原先的 5 秒轮询）。契约：**凡写 settings 的路径都必须 emit 本事件**

使用的 Tauri 插件：`dialog`（文件对话框）、`notification`（桌面通知）、`shell`（打开 URL）。

### Rust 后端（`src-tauri/src/`）

`lib.rs` 定义模块划分、注册命令（38 个，见末尾 `generate_handler!`）、装配全局状态；后台周期任务委托给 `services::scheduler`。

```
commands/               # Tauri IPC 命令处理函数（38 个注册命令）
  subscription.rs       # 订阅增删改查、分组、batch_delete、get_channel_info
  download.rs           # check_subscription / check_all、记录查询、队列控制、get_channel_videos
                        #   以及共享的 check_and_download / run_check_round（被 scheduler 与 tray 调用）
  settings.rs           # get/update 设置、get_app_state、路径与代理校验
  health.rs             # check_all_health / check_selected_health（快照后 spawn 后台任务）
  import_export.rs      # JSON/OPML 导出、三种导入源（OPML 文件 / TXT 文件 / URL 列表）
  file_manager.rs       # 打开所在文件夹、文件存在性检查、删除文件、同步文件状态

services/               # 业务逻辑（多数无状态，通过参数接收路径/配置）
  storage.rs            # JSON 持久化 + 写事务（见「持久化」一节）
  download_queue.rs     # 下载队列：并发控制、子进程生命周期、暂停/恢复/取消（有状态）
                        #   并导出 notify_records_changed —— records-changed 的唯一 emit 入口
  ytdlp.rs              # yt-dlp 命令行封装（解析频道、检查视频、下载视频）
  opml.rs               # OPML 2.0 XML 导入/导出（quick-xml）
  health.rs             # HTTP 健康检查（reqwest）
  file_manager.rs       # 文件存在性/删除，并与记录状态联动
  settings_validator.rs # 下载路径与代理 URL 校验（纯函数）
  tray.rs               # 系统托盘：菜单、状态、图标
  scheduler.rs          # 后台周期任务：spawn_scheduler（自动检查订阅）+ spawn_file_sync
                        #   （文件状态同步）。每轮从磁盘重读设置、遵循 scheduler_paused

models/                 # 数据结构（serde 序列化/反序列化）
  subscription.rs, download.rs, settings.rs, import_export.rs, health.rs, video.rs

utils/
  error.rs              # AppError 枚举（12 个变体，thiserror）
  progress_parser.rs    # yt-dlp 进度行解析（纯函数）
```

**关键设计规则**：
- Commands 层**不在命令里写业务逻辑** —— 命令只做参数传递、服务调用和序列化返回。注意 `commands/download.rs` 里有一组 `pub(crate)` 的**非命令**函数（`check_and_download`、`SubCheckOutcome`、`apply_check_outcomes`、`run_check_round`），它们是业务逻辑，被命令层与 `services/`（scheduler、tray）共同调用；`services` → `commands` 的引用在 `tray.rs` 已有先例。
- 已知残留重复：`check_all_subscriptions` 与 `run_check_round` 是同一件事的两份实现（设置来源、错误传播、返回值、是否 emit 事件不同）。`check_all_subscriptions` 自带循环，收敛需先决定这些语义的归属。
- Services 层**以无状态为主** —— 通过参数接收路径/配置，不持有全局状态。例外：`download_queue.rs` 与 `tray.rs` 是有状态的（队列、托盘句柄），通过 `AppContext` / `QueueContext` 注入。
- 全局状态通过 `AppContext`（data_dir + `Mutex<AppSettings>` + scheduler watch channel）与 `QueueContext`（下载队列）管理，通过 `tauri::State` 注入。`scheduler_notify`（watch channel）由设置更新与托盘的暂停切换共同 `send`，调度循环消费；**发送端必须保持存活**，否则 `changed()` 立即返回 `Err` 造成忙循环。
- **设置不要用快照冻结**：调度循环每轮从磁盘 `load_settings` 重读，因此改 yt-dlp 路径 / 代理 / Cookie / 下载目录无需重启即生效。新增长期运行的任务时同样按轮读取，不要持有 setup 时的设置快照。
- **settings 变更必须广播**：`update_settings` 与托盘 `handle_toggle_scheduler` 是仅有的两个 settings 写路径，二者都在「落盘成功且缓存已更新」后 emit `settings-changed`（完整 `AppSettings` 载荷）。新增写路径必须同样 emit，且 emit 须在所有应用级锁释放之后——前端不再轮询设置。
- 持有 `MutexGuard` 时，**先克隆值并释放锁，再进行 `await`**，避免跨异步边界持有互斥锁。**特别注意不要持锁跨越阻塞式子进程调用**（如 yt-dlp）。
- **持久化写入只有一个入口**：`StorageService::update_download_records` / `update_subscriptions` / `update_state`（闭包式事务，内部持有全局写锁）。`save_subscriptions` / `save_download_records` / `save_state` 是私有原语，外部不可调用（编译器强制）；`load_*` 不加锁。
  事务闭包内**禁止**：调用 `update_*` / `save_settings`（`std::sync::Mutex` 不可重入，会死锁；debug 构建下会 panic 提示）、获取任何其他应用级锁、做文件 I/O。锁序固定为 `QueueContext.queue → 写锁`，不要引入反向路径。
- **不要用陈旧快照整体覆盖**：跨 `await` 的长流程（`scheduler.rs` 的调度循环、`check_all_subscriptions`、`run_check_round`）一律「边跑边收集结果（`SubCheckOutcome`），收尾按 id 做一次增量事务」。用循环开始时的快照整表 `save` 会 clobber 并发写者（健康检查、单订阅命令）对其它订阅字段的修改。
- **不要新增派生计数字段**：`Subscription` 与 `AppState` 都**不**再持有「已下载数」。唯一真相源是 `download_records.json`，数量由前端从 `records` 派生（口径 = `status === "completed"` 的记录条数，见 `AppShell.tsx` 与 `DetailPanel.tsx`）。历史上该字段有「完成回调增量」与「启动重算」两个语义不同的写者，导致显示不一致——不要重新引入。
- **写操作与事件成对**：凡修改持久化数据的路径，必须在写事务完成后 emit 对应事件 —— 记录改动 → `services/download_queue.rs::notify_records_changed`，队列改动 → `DownloadQueue::emit_queue_changed`，settings 改动 → `settings-changed`。emit 一律是**纯 emit**（零 I/O、零加锁），因此可在任意临界区内安全调用；不要为了构造载荷去读文件或加锁。前端对高频事件做短延时合并（`App.tsx` 的 `scheduleRefresh`，150ms 窗口内只排一次，用 schedule-once 而非 debounce 以免连续事件把刷新无限推迟）。
- 已知死代码（无消费方，待清理）：`useDownloadRecords` 的 `getQueueState` / `getQueue`、`src/lib/tauri.ts` 的 `getQueueState`、后端 `get_queue_state` 命令。

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

前端测试用 vitest + @testing-library/react（`npm test`），源码在 `src/**/__tests__/`。注意 `src/hooks/__tests__/useFilter.test.ts` 的 38 个筛选/排序用例测的是 `useFilter.ts` 里真实导出的 `applyFilter`/`applySort`，另有一组 `renderHook` 用例覆盖 Hook 的状态联动。

### 平台支持

通过 Tauri 的跨平台打包支持 Windows、macOS 和 Linux。提供了三个平台的图标（`icon.ico`、`icon.icns`、`.png` 变体）。

<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
at `specs/007-unified-video-list/plan.md`
<!-- SPECKIT END -->
