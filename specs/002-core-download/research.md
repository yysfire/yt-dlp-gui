# Research: 核心下载功能

**Date**: 2026-05-31 | **Spec**: [spec.md](./spec.md)

## 1. yt-dlp 进度输出解析

### 决策

使用 yt-dlp 的 `--progress-template` 参数自定义进度输出格式，通过 `tokio::process::Command` 的 stdout 流式读取，行级解析。

### 方案

```bash
yt-dlp --progress-template "%(progress.status)s|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._total_bytes_str)s|%(progress._eta_str)s" ...
```

输出示例：
```
downloading|45.2%|2.3MiB/s|125.8MiB|00:02:15
```

### 理由

- yt-dlp 内置 `--progress-template` 提供完整的进度信息，无需额外解析器框架
- 行级文本解析是纯函数，易于单元测试
- 比解析 yt-dlp 默认日志输出更稳定（不受日志格式变更影响）

### 备选方案

| 方案 | 优点 | 缺点 |
|------|------|------|
| `--newline` 模式（每行 JSON） | 结构化输出 | yt-dlp 版本要求高，字段可能不稳定 |
| 解析 stderr 进度条（正则） | 无需额外参数 | 多语言输出格式不一致 |
| `--print` 自定义字段 | 精确控制输出 | 无法实时获取进度，仅下载后输出 |
| **`--progress-template`（选定）** | 稳定格式，独立于日志 | 需要 yt-dlp >= 2023.03 |

---

## 2. 跨平台进程暂停/继续

### 决策

- **Linux/macOS**：使用 `nix::sys::signal::kill(pid, Signal::SIGSTOP)` / `SIGCONT`
- **Windows**：使用 `kernel32::SuspendThread` / `ResumeThread`（通过 `ntapi` crate 或 unsafe FFI）
- **取消**：使用 `Child::kill()` (tokio)，跨平台等效 `SIGKILL`

### 实现方式

```rust
#[cfg(unix)]
fn pause_process(child: &tokio::process::Child) -> Result<()> {
    let pid = child.id().ok_or(...)?;
    unsafe { libc::kill(pid as i32, libc::SIGSTOP); }
    Ok(())
}

#[cfg(windows)]
fn pause_process(child: &tokio::process::Child) -> Result<()> {
    // Use kernel32::SuspendThread on each thread of the process
    // ...
}
```

### 理由

- SIGSTOP/SIGCONT 是 Unix 系统标准方式，yt-dlp 作为子进程会挂起读写操作
- Windows 没有进程级暂停原语，但线程级暂停足以实现目的
- `Child::kill()` 跨平台，用于取消下载

### 备选方案

| 方案 | 优点 | 缺点 |
|------|------|------|
| **信号暂停（选定）** | 真正的暂停，不消耗资源 | Windows 实现稍复杂 |
| 放弃暂停，仅支持取消 | 实现简单 | 不符合 FR-004 |
| 使用 yt-dlp `--abort-on-unavailable-fragment` | yt-dlp 原生错误处理 | 不能真正暂停 |

### 风险

- **Windows 上暂停后继续下载**：yt-dlp 可能因网络超时而失败——这是可接受的降级行为，符合 spec 假设（P2 优先级）
- **部分文件清理**：暂停的下载已有部分文件（`.part`），取消时需清理 `{output_dir}/{title}.*.part` 和 `{output_dir}/{title}.*.ytdl`

---

## 3. FIFO 下载队列与并发控制

### 决策

使用 `tokio::sync::Semaphore` + `VecDeque<DownloadTask>` + `tokio::task::JoinSet` 构建 FIFO 队列。

### 架构

```
检测到新视频 → 创建 DownloadTask → 入队 VecDeque
                                    ↓
                          Semaphore.available() > 0?
                               ↓ YES         ↓ NO
                          tokio::spawn     等待 Semaphore 许可
                          Semaphore.acquire()
                                ↓
                          执行下载（流式）
                                ↓
                          Semaphore.release() → 下一个排队任务启动
```

### 核心数据结构

```rust
pub struct DownloadQueue {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    semaphore: Arc<Semaphore>,
    tasks: Arc<Mutex<JoinSet<()>>>,
    app_handle: AppHandle,
}

pub struct DownloadTask {
    pub id: String,
    pub video_url: String,
    pub video_title: String,
    pub subscription_id: String,
    pub quality: String,
    pub status: TaskStatus,  // Waiting, Running, Paused, Completed, Failed, Cancelled
    pub progress: Option<DownloadProgress>,
    pub error_message: Option<String>,
}
```

### 理由

- `VecDeque` 天然支持 FIFO（`push_back` + `pop_front`）
- `Semaphore` 控制并发数，简单且高效（tokio 原生支持）
- `JoinSet` 管理异步任务生命周期，支持优雅取消（`join_next()` + `abort_all()`）
- 所有组件都是 tokio 标准库，无需额外依赖

### 备选方案

| 方案 | 优点 | 缺点 |
|------|------|------|
| **VecDeque + Semaphore（选定）** | 简单，无额外依赖 | 无持久化队列 |
| `tokio::sync::mpsc` channel | 天然 FIFO | 需要单独的 worker 任务管理 |
| `crossbeam::deque` | 无锁，高性能 | 引入额外依赖，过度设计 |
| 数据库持久化队列 | 应用重启后恢复 | MVP 不需要，违反 YAGNI |

---

## 4. 视频重复检测策略

### 决策

使用 yt-dlp `--flat-playlist --dump-json` 输出中的 `id` 字段进行去重，**回退到 `url`** 当 `id` 缺失时。

### 理由

- `id` 字段是平台原生视频 ID（如 YouTube 的 `dQw4w9WgXcQ`），与 URL 中的视频 ID 一致
- URL 格式可能因短链接、嵌入链接等不同但指向同一视频
- `id` 在 `--flat-playlist` 模式下通常存在，但某些平台可能不提供
- 现有代码使用 `video_url` 去重，替换为 `video_id` + `video_url` 双重检查

### 实现

```rust
struct VideoInfo {
    pub id: Option<String>,   // 平台视频 ID（优先用于去重）
    pub title: String,
    pub url: String,           // 回退去重
    pub upload_date: Option<String>,
}
```

### 去重逻辑

```
已有的 DownloadRecord 建立 HashSet<video_id> + HashSet<video_url>
新视频先查 video_id HashSet (if Some) → 再查 video_url HashSet
任一命中 → 跳过
```

### 当前 vs 目标

| 项目 | 当前 | 目标 |
|------|------|------|
| 去重字段 | `video_url` | `video_id`（优先）+ `video_url`（回退） |
| 重试策略 | 无（所有记录跳过） | 失败记录允许重试 |
| 文件检查 | 无 | 检查记录对应的文件是否存在 |

---

## 5. 应用重启后状态恢复

### 决策

应用重启时：
1. 读取 `download_records.json` 中状态为 `downloading` 的记录 → 标记为 `failed`
2. 定时检查器基于 `state.json` 中的 `last_check_time` 计算剩余时间

### 理由

- yt-dlp 子进程在应用关闭时被操作系统终止，已下载的部分文件无效
- 将未完成的下载标记为 `failed` 允许用户在下次检查时重试（FR-010）
- 文件清理在取消操作时执行，重试时 yt-dlp 会覆盖同名文件
- 不保留暂停状态——暂停的下载在进程结束后同样变为无效

### 实现

```rust
fn recover_state(data_dir: &Path) -> Result<()> {
    let mut records = StorageService::load_download_records(data_dir)?;
    for record in records.iter_mut() {
        if record.status == "downloading" || record.status == "paused" {
            record.status = "failed".to_string();
            record.error_message = Some("Application restarted".to_string());
        }
    }
    StorageService::save_download_records(data_dir, &records)?;
    Ok(())
}
```
