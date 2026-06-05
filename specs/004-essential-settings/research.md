# Research: 必要设置

**Phase 0** | **Date**: 2026-06-05

## 1. 动态调整 tokio interval

### Decision
**已实现，无需额外开发。**

### Rationale
`lib.rs::setup()` 中已使用 `tokio::sync::watch` + `tokio::time::sleep` + `tokio::select!` 模式实现动态 interval 调整：
- `AppContext` 持有 `watch::Sender<()>`
- 调度器 task 通过 `rx.changed()` 在 `tokio::select!` 中监听变更
- `commands/settings.rs::update_settings()` 检测 `check_interval_minutes` 变更时调用 `scheduler_notify.send(())`
- 收到通知后重新从磁盘读取 `check_interval_minutes`，更新 `interval_mins` 变量

当前唯一问题是 `check_interval_minutes` 的默认值：代码中为 `360`（6 小时），规格要求为 "每小时"（60 分钟）。**需修改** `AppSettings::default()` 中的默认值。

### Alternatives Considered
- 使用 Arc<AtomicU32> 共享 interval 值 → 已排除，因为需要支持从磁盘重载（不仅仅是内存修改）
- 使用 tokio::sync::broadcast → 已排除，watch 对单消费者场景更合适

## 2. 下载进度跟踪（FR-010 实现基础）

### Decision
**复用现有 `ProgressEvent.percent` 机制，无需新增解析逻辑。**

### Rationale
当前 `services/download_queue.rs` 已有完整进度跟踪：
- `DownloadTask` 结构体包含 `progress: Option<ProgressInfo>` 字段
- yt-dlp 通过 `--progress-template` 输出格式化的进度行
- `progress_parser::parse_progress_line()` 解析为 `ProgressEvent { percent: f32 }`
- `download-progress` 事件通过 Tauri `emit` 推送给前端

实现 FR-010（并发数降低时按进度暂停）的路径：
1. 扩展 `DownloadQueue`，维护每个活跃任务的 `last_progress_percent: f32`
2. 降低并发时，`active_tasks` 按 `percent` 升序排列
3. 暂停 `percent` 最低的任务（通过标记 `TaskControlSignal::Pause`）

实现 FR-012（路径不可访问回退到默认路径）的路径：
1. 在 `DownloadQueue::process_pending()` 中，下载前调用 `std::fs::create_dir_all(download_dir)`
2. 若失败，回退到 `default_download_dir()` 并记录日志
3. 通知前端下载路径已变更

实现 FR-011（代理不可用不自动回退）的路径：
1. 当前 `YtDlpService` 已在不可用时原生失败（yt-dlp 返回非 0 退出码）
2. 明确：不在代码中添加"代理失败 → 直连"的自动切换逻辑
3. 任务状态标记为 "failed"，`DownloadRecord.error` 字段记录代理连接错误

### Alternatives Considered
- 新增独立的进度跟踪表 → 已排除，`active_tasks` HashMap 已满足需求
- 使用文件大小排序替代进度百分比 → 已排除（规格明确要求按进度百分比）

## 3. 代理 URL 验证（url crate 兼容性）

### Decision
**引入 `url` crate v2.x 进行代理地址验证，socks5:// 格式兼容。**

### Rationale
`url` crate v2.x 支持 `socks5://` scheme，验证方法：

```rust
use url::Url;

fn validate_proxy_url(input: &str) -> Result<(), String> {
    let url = Url::parse(input).map_err(|e| format!("代理地址格式无效: {}", e))?;
    match url.scheme() {
        "http" | "https" | "socks5" | "socks5h" => {}
        other => return Err(format!("不支持的代理协议: {}", other)),
    }
    if url.host().is_none() {
        return Err("代理地址缺少主机名".to_string());
    }
    Ok(())
}
```

支持的格式示例：
- `http://127.0.0.1:7890`
- `https://proxy.example.com:8443`
- `socks5://127.0.0.1:1080`
- `socks5h://127.0.0.1:1080`

注意：`socks5h://` 表示 SOCKS5 代理负责 DNS 解析（yt-dlp 原生支持）。

### Alternatives Considered
- 仅简单字符串检查 → 已排除，无法捕获格式错误
- 正则表达式 → 已排除，url crate 提供更可靠的解析
- 自定义 parser → 已排除（YAGNI）

## 4. 默认值对齐

### Decision
**修改 `AppSettings::default()` 中的以下默认值以对齐规格：**

| 字段 | 当前代码默认值 | 规格要求默认值 | 变更 |
|------|---------------|---------------|------|
| `check_interval_minutes` | `360`（6小时） | `60`（每小时） | 修改 |
| `max_concurrent_downloads` | `1`（最大值硬编码为 3） | `1`（最大值 5） | 最大上限从 3 改为 5 |

### Rationale
规格 Assumptions 节明确要求默认值对齐。当前代码默认值偏离规格定义，需统一。

### Impact
- `check_interval_minutes` 从 360 改为 60：首次使用体验更合理（每小时检查一次而非 6 小时）
- `max_concurrent_downloads` 上限从 3 改为 5：给予高级用户更多灵活性
- 这两个变更不影响已有用户的配置（仅影响默认值生成，已有 `settings.json` 保持不变）

## 5. "手动"检查频率模式

### Decision
**实现为 `check_interval_minutes = 0` 表示手动模式。**

### Rationale
当前代码中 `check_interval_minutes` 为 `u32`，`0` 不在有效间隔范围内。用 `0` 值表示"暂停自动检查"是简洁的语义映射：
- `check_interval_minutes == 0` → 调度器不执行定时检查（可在 `select!` 循环中跳过 sleep 路径）
- 用户仍可通过 UI 手动触发检查（现有 `check_all` 命令）
- 前端显示为 "手动" 选项

### Alternatives Considered
- 使用 `Option<u32>` → 已排除，增加 serde 复杂度，`0` 值语义足够清晰
- 新增 `bool manual_mode` 字段 → 已排除，冗余字段违反 DRY
