# Phase 0 Research: 基本文件管理

**Feature**: specs/003-basic-file-management
**Date**: 2026-06-04

## 1. 跨平台 open_in_folder

### Decision

通过 Tauri v2 的 `tauri-plugin-shell::ShellExt::open()` API 调用系统默认文件管理器。

### Rationale

- Tauri v2 `shell` plugin 提供 `open(path, opener)` 方法，会自动注册对应的 macOS 沙盒权限（`com.apple.security.files.user-selected.read-only`）
- `tauri-plugin-shell` 已在 `Cargo.toml` 中引入（v2），无需新增依赖
- `open()` 会自动调用各平台的默认文件管理器：Windows → `explorer /select,<path>`、macOS → `open -R <path>`、Linux → `xdg-open <dir>`
- 但 Tauri 的 `open` 对**定位到具体文件**的支持有限——在 macOS 上可通过 `open -R` 定位，在 Windows 上需要 `explorer /select,` 格式，在 Linux 上大部分文件管理器不支持定位到文件
- **折中方案**：对路径取 `Path::parent()` 打开父目录即可满足规格要求（规格接受场景 2 明确：文件不存在时打开到目录级别）

### Alternatives Considered

| 方案 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| Tauri shell `open()` | 统一 API，已引入 | 定位到文件支持不一致 | ✅ 采用 |
| 手动调用系统命令 | 精细控制参数 | 平台差异代码量大，需条件编译 | ❌ 过度设计 |
| `std::process::Command` | 无额外依赖 | 需处理各平台差异 | ❌ 无必要 |

### Implementation Notes

```rust
// 实现方式（pseudocode）
use tauri_plugin_shell::ShellExt;
fn open_in_folder(app: &tauri::AppHandle, file_path: &str) -> Result {
    let path = std::path::Path::new(file_path);
    let dir = path.parent().unwrap_or(path);
    app.shell().open(dir.to_string_lossy(), None)?;
}
```

## 2. 文件同步性能 —— 串行 vs 并发

### Decision

使用 `tokio::task::spawn_blocking` 批量并发检查文件存在性，并发上限 50。

### Rationale

- `std::fs::metadata()` 是同步阻塞 I/O，不能直接在 async 上下文中调用大量文件（会阻塞 tokio 运行时线程）
- 对于 500 个文件，使用 `spawn_blocking` 并发（限制 50 并发）可以显著减少总耗时
- 当前 `Cargo.toml` 已引入 `tokio` with `full` features，`spawn_blocking` 可用
- 500 个文件 * 假设每次 `metadata()` ~5ms = 2500ms 串行 → 并发 50 后约 50ms（满足 <10s 的 SC-004 目标）

### Alternatives Considered

| 方案 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| `spawn_blocking` 批量并发（限 50） | 高性能，满足 SC-004 | 需管理并发数 | ✅ 采用 |
| 串行 `metadata()` | 实现简单 | 500 文件慢（~2.5s 仍满足 10s 但冗余小） | ❌ 不够优化 |
| `tokio::fs::metadata()` | async 原生 | tokio 的 fs 模块本质仍是 spawn_blocking，无优势 | ❌ 无区别 |
| `std::thread::scope` | 无 tokio 依赖 | 与 tokio 运行时混用不推荐 | ❌ 不推荐 |

### Implementation Notes

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

async fn check_files_exist(file_paths: &[String]) -> Vec<bool> {
    let semaphore = Arc::new(Semaphore::new(50));
    let mut handles = Vec::with_capacity(file_paths.len());
    for path in file_paths {
        let path = path.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        handles.push(tokio::task::spawn_blocking(move || {
            let exists = std::fs::metadata(&path).is_ok();
            drop(permit);
            exists
        }));
    }
    // collect results...
}
```

## 3. DownloadRecord 状态扩展 —— 向后兼容

### Decision

无需 serde 枚举技巧——`status` 字段当前是 `String` 类型，直接添加 `"deleted"` 作为新的状态值即可。

### Rationale

- 当前 `DownloadRecord.status` 字段类型是 `pub status: String`（`src-tauri/src/models/download.rs:21`），**不是 Rust 枚举**
- TypeScript 端类型为联合类型 `"downloading" | "completed" | "failed" | "paused" | "cancelled" | "waiting"`（6 种）
- 新增 `"deleted"` 作为第 7 种状态值完全向后兼容——旧 JSON 记录中的任何 `status` 值都能正常反序列化
- **无需 `#[serde(other)]`、`#[serde(untagged)]` 或 `#[serde(default)]` 技巧**

### Alternatives Considered

| 方案 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| 保持 `String` 类型，新加 `"deleted"` | 零破坏，简单 | 无编译时状态检查 | ✅ 采用（遵循现有模式） |
| 改为 Rust enum | 编译时安全 | 破坏现有 JSON 数据，需迁移 | ❌ 过度设计（YAGNI） |
| 新增 `is_deleted: bool` 字段 | 独立标记 | 与 status 语义重叠，数据模型不一致 | ❌ 违反 DRY |

### Implementation Notes

需要更新的文件：
- `src-tauri/src/models/download.rs`: 注释补充 `"deleted"` 状态
- `src-tauri/src/commands/download.rs`: 删除文件后更新 status 为 `"deleted"`
- `src/types/index.ts`: `status` 联合类型追加 `"deleted"`
- `src-tauri/src/services/storage.rs`: 无需修改（`String` 反序列化天然兼容）

## 4. 定时同步 —— 复用 scheduler.rs 还是新 timer

### Decision

复用现有 `SchedulerService`，但为文件同步创建独立的调度器实例。

### Rationale

- `SchedulerService::start(interval_mins, callback)` 是泛型、无状态的——接受任意 `Fn() + Send + 'static` 回调，与当前订阅检查的调度器互不干扰
- 复用可避免重复的 `tokio::time::interval` + `abort` 封装代码
- 创建独立实例允许独立启停：文件同步调度器和订阅检查调度器可分别控制
- 定时间隔建议 5 分钟（`interval_mins: 5`）

### Alternatives Considered

| 方案 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| 复用 SchedulerService | DRY，零新代码 | 无 | ✅ 采用 |
| 新写独立 timer | 完全隔离 | 代码重复 | ❌ 违反 DRY |

### Implementation Notes

```rust
// 启动文件同步定时器
let sync_handle = SchedulerService::start(5, move || {
    // 检查所有下载记录的文件是否存在
    // 标记已删除的记录
    // 发送 events 通知前端
});
// 存入 AppContext 或 State 以便 stop 时使用
```

## 5. 文件删除实现

### Decision

使用 `std::fs::remove_file()` 删除物理文件，然后更新记录 status 为 `"deleted"` 并持久化。

### Rationale

- 根据澄清结果：删除文件 → 保留记录 → 标记为 `"deleted"`（不再提供清除入口）
- `std::fs::remove_file()` 跨平台统一，无需额外依赖
- 删除前先检查文件是否存在，避免错误（文件已手动删除的情况）

### Implementation Notes

```rust
#[tauri::command]
async fn delete_file(id: String, state: tauri::State<'_, AppContext>) -> Result<(), String> {
    let records = StorageService::load_download_records(&state.data_dir)?;
    let record = records.iter().find(|r| r.id == id)
        .ok_or("下载记录不存在")?;
    
    // 尝试删除文件（忽略文件已不存在的情况）
    let path = std::path::Path::new(&record.file_path);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    
    // 更新记录状态
    let mut updated = record.clone();
    updated.status = "deleted".to_string();
    StorageService::update_download_record(&state.data_dir, &updated)?;
    Ok(())
}
```
