# Quickstart: 必要设置

**Phase 1** | **Date**: 2026-06-05

## 开发前准备

```bash
# 确认在项目根目录
cd /home/yys/projects/yt-dlp-gui

# 确认当前 Tauri 开发环境正常
cargo check 2>&1 | tail -5
npx tsc --noEmit 2>&1 | tail -5
```

## 实现顺序（按依赖关系）

### Step 1: 模型层 — `AppSettings` 默认值修改

**文件**: `src-tauri/src/models/settings.rs`

1. 将 `default_check_interval()` 返回值从 `360` 改为 `60`
2. 更新 `max_concurrent_downloads` 的文档注释：上限从 3 改为 5
3. 确认现有 `#[serde(default)]` 容错逻辑正确

**验证**: `cargo test -p yt-dlp-gui -- models::settings`

### Step 2: 验证 Service — `settings_validator.rs`

**文件**: `src-tauri/src/services/settings_validator.rs`（新建）

1. 实现 `validate_proxy_url(input: &str) -> ProxyValidateResult`
2. 实现 `validate_download_path(path: &str) -> PathValidateResult`
3. 编写 `#[cfg(test)]` 单元测试（TDD 先行）

**新增依赖**:
```toml
# Cargo.toml
url = "2"
```

**验证**: `cargo test -p yt-dlp-gui -- settings_validator`

### Step 3: 命令层 — 新增 IPC 命令

**文件**: `src-tauri/src/commands/settings.rs`

1. 新增 `validate_download_path` 命令（调用 Step 2 的 Service）
2. 新增 `validate_proxy_url` 命令（调用 Step 2 的 Service）
3. 在 `update_settings` 中添加并发数变更通知逻辑（通知 DownloadQueue）

**文件**: `src-tauri/src/lib.rs`

1. 在 `generate_handler![]` 中注册新命令

**验证**: `cargo check`

### Step 4: 前端类型 — TypeScript 类型更新

**文件**: `src/types/index.ts`

1. 确认 `AppSettings.max_concurrent_downloads: number` 已存在（可能已存在）
2. 若不存在则添加

### Step 5: 前端封装层

**文件**: `src/lib/tauri.ts`

1. 新增 `validateDownloadPath(path: string)` invoke 封装
2. 新增 `validateProxyUrl(url: string)` invoke 封装

### Step 6: 前端 UI — `SettingsDialog.tsx` 扩展

**文件**: `src/components/SettingsDialog.tsx`

1. 下载路径：文本输入 + "浏览"按钮（调用 Tauri `dialog` 插件选择文件夹）+ 实时验证
2. 画质预设：下拉选择框（复用现有逻辑）
3. 代理地址：文本输入 + 验证状态显示 + 空值提示"留空不使用代理"
4. 并发下载数：滑块（Slider，范围 1-5，步长 1，默认 1）
5. 检查频率：下拉选择（选项：手动、30分钟、每小时、每天；对应值 0、30、60、1440）

### Step 7: 下载队列调度 — FR-010 实现

**文件**: `src-tauri/src/services/download_queue.rs`

1. 跟踪每个活跃任务的 `last_progress_percent`
2. 并发数降低时，按 percent 升序暂停任务
3. 实现 `adjust_concurrency(new_max: u32)` 方法

### Step 8: 下载失败处理 — FR-011, FR-012

**文件**: `src-tauri/src/services/download_queue.rs` 或 `commands/download.rs`

- FR-011: 代理不可用时，任务标记 "failed"，`DownloadRecord.error` = "代理连接失败"
- FR-012: 下载路径不可访问时，回退到 `default_download_dir()`

## 运行验收

```bash
# 完整 Tauri 应用测试
npm run tauri dev

# 验证清单：
# 1. 打开设置 → 修改下载路径 → 浏览选择 → 验证路径
# 2. 选择画质 720p → 下载测试 → 验证文件分辨率
# 3. 输入代理地址 → 格式错误提示
# 4. 设置并发数为 2 → 触发 3 个下载 → 验证最多 2 个同时进行
# 5. 并发数从 2 降到 1 → 验证进度较少的任务暂停
# 6. 修改检查频率 → 重启应用 → 验证设置保持
```

## 测试命令

```bash
# Rust 单元测试
cargo test -p yt-dlp-gui

# TypeScript 类型检查
npx tsc --noEmit

# 生产构建
npm run build
```
