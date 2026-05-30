# Quickstart: 核心下载功能开发

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## 前置条件

1. **Rust 工具链**：`rustc >= 1.75`，`cargo >= 1.75`
2. **Node.js**：`node >= 18`，`npm >= 9`
3. **yt-dlp CLI**：最新稳定版（2024+，支持 `--progress-template`）
4. **系统工具**：
   - Linux: libwebkit2gtk-4.1-dev, libgtk-3-dev, libayatana-appindicator3-dev
5. **项目已克隆**：`git clone <repo> && cd yt-dlp-gui`

## 快速开始

```bash
# 安装前端依赖
npm install

# Tauri 开发模式（前端 + Rust 后端热更新）
npm run tauri dev

# 仅前端开发
npm run dev

# Rust 测试
cargo test

# Rust 编译检查
cargo check
```

## 开发工作流

### TDD 循环（章程要求）

```
1. 读取当前 spec/plan/data-model 理解需求
2. 编写失败的测试用例（Rust: #[cfg(test)] 模块）
3. 运行测试确认失败
4. 实现最小代码使测试通过
5. 运行全部测试确保无回归
6. 如果需要，重构代码
7. 提交前运行 cargo test && npx tsc --noEmit
```

### 本规格的开发顺序（按依赖关系）

```
Phase 1: 数据模型 + 进度解析器（无外部依赖）
  ├── 1.1 DownloadRecord 新增 video_id, error_message 字段
  ├── 1.2 DownloadTask + TaskStatus + DownloadProgress 定义
  ├── 1.3 progress_parser.rs 实现 + 单元测试
  └── 1.4 AppSettings 新增 max_concurrent_downloads

Phase 2: 下载队列（依赖 Phase 1）
  ├── 2.1 DownloadQueue 核心实现 (VecDeque + Semaphore + JoinSet)
  ├── 2.2 流式下载 download_video_streaming()
  ├── 2.3 队列事件推送 (download-progress, queue-changed)
  └── 2.4 集成测试：多任务 FIFO 排队

Phase 3: 进程控制（依赖 Phase 2）
  ├── 3.1 pause_download / resume_download (跨平台)
  ├── 3.2 cancel_download (含文件清理)
  └── 3.3 单元测试：状态转换正确性

Phase 4: 前端集成（依赖 Phase 2-3）
  ├── 4.1 DownloadQueuePanel 组件
  ├── 4.2 DownloadProgressBar 组件
  ├── 4.3 useDownloadProgress hook
  └── 4.4 tauri.ts 新增 API 封装

Phase 5: 状态恢复 + 边界情况（依赖 Phase 1-3）
  ├── 5.1 recover_state() 启动时恢复
  ├── 5.2 文件存在性检查
  ├── 5.3 网络中断超时处理
  └── 5.4 磁盘空间不足检测
```

## 关键文件路径

```text
src-tauri/src/
├── models/download.rs           # 修改：新增字段
├── models/settings.rs           # 修改：新增 max_concurrent_downloads
├── services/ytdlp.rs            # 修改：新增流式下载
├── services/download_queue.rs   # 新增：FIFO 队列
├── utils/progress_parser.rs     # 新增：进度解析
├── commands/download.rs         # 修改：新增 pause/resume/cancel 命令
└── lib.rs                       # 修改：注册新命令，队列初始化

src/
├── types/index.ts               # 修改：新增类型
├── lib/tauri.ts                 # 修改：新增 invoke 封装
├── hooks/useDownloadRecords.ts  # 修改：新增方法
├── hooks/useDownloadProgress.ts # 新增：进度事件 hook
├── components/DownloadQueuePanel.tsx  # 新增：队列面板
└── components/DownloadProgressBar.tsx # 新增：进度条组件
```

## 测试重点

### Rust 单元测试

1. **progress_parser.rs**：解析正常进度行、解析异常格式、解析空输入
2. **download_queue.rs**：FIFO 顺序、信号量并发限制、状态转换正确性
3. **models/download.rs**：序列化/反序列化往返、默认值、状态转换逻辑

### 手动集成测试

1. 添加订阅 → 手动检查 → 观察队列 → 观察进度 → 确认下载完成
2. 暂停下载 → 等待 10s → 继续下载 → 确认文件完整性
3. 取消下载 → 确认部分文件被清理
4. 多订阅同时检查 → 确认 FIFO 顺序 + 并发限制
5. 模拟网络中断（断开 WiFi）→ 确认失败状态 + error_message

## 常见问题

### yt-dlp 进度模板格式问题

如果 yt-dlp 版本 < 2023.03，`--progress-template` 不可用。回退方案：使用 yt-dlp `--progress` 标志 + 解析 stdout 进度条。

```bash
# 检查版本
yt-dlp --version

# 确认 progress-template 支持
yt-dlp --progress-template "%(progress.status)s" --help
```

### SIGSTOP 在 Linux 容器中的问题

如果运行在 Docker 中，SIGSTOP 可能被容器运行时忽略。测试时使用真实桌面环境。

### 同时运行多个 cargo test

```bash
# 排除依赖 yt-dlp 进程的测试
cargo test -- --skip ytdlp
```
