# IPC Contracts: 设置相关命令

**Phase 1** | **Date**: 2026-06-05

## 命令概览

本规格涉及以下 Tauri IPC 命令：

| 命令 | 状态 | 来源 |
|------|------|------|
| `get_settings` | 已有 | `commands/settings.rs` |
| `update_settings` | **扩展** | 新增字段支持 + 验证 |
| `validate_download_path` | **新增** | FR-002 |
| `validate_proxy_url` | **新增** | FR-005 |

## `get_settings`

### 调用

```typescript
// src/lib/tauri.ts
export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings');
}
```

### 返回

```json
{
  "download_dir": "/home/user/Videos/yt-dlp",
  "check_interval_minutes": 60,
  "quality_preset": "1080p",
  "proxy_url": "",
  "max_concurrent_downloads": 1,
  "yt_dlp_path": "yt-dlp",
  "notifications_enabled": false,
  "dark_mode": false,
  "cookie_file": ""
}
```

### 错误处理
- Mutex 锁中毒 → 返回错误字符串

---

## `update_settings`

### 调用

```typescript
// src/lib/tauri.ts
export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>('update_settings', { settings });
}
```

### 输入

完整的 `AppSettings` 对象，前端发送前应已通过 `validate_download_path` 和 `validate_proxy_url` 单独验证。

### 行为

1. **持久化**：`StorageService::save_settings()` 写入 `settings.json`
2. **内存缓存更新**：更新 `AppContext.settings` Mutex
3. **调度器通知**：若 `check_interval_minutes` 变更，触发 `scheduler_notify.send(())`
4. **返回**：更新后的完整 `AppSettings`

### 新增验证逻辑（计划添加）

```
update_settings(input):
  1. 验证 proxy_url 格式（若非空）
  2. 验证 download_dir 可写性（若已变更）
  3. 验证 max_concurrent_downloads 在 1-5 范围内
  4. 持久化
  5. 更新运行时状态
  6. 如有并发数变更 → 通知 DownloadQueue 调整
```

### 错误处理
- `proxy_url` 格式无效 → 返回 `"代理地址格式无效: <详情>"`
- `download_dir` 不可写 → 返回 `"下载路径不可写入: <详情>"`
- `max_concurrent_downloads` 越界 → 返回 `"并发数必须在 1-5 之间"`

---

## `validate_download_path`（新增）

### 目的

在用户浏览选择文件夹后、保存设置前，前端调用此命令验证路径可用性。

### 调用

```typescript
// src/lib/tauri.ts
export async function validateDownloadPath(path: string): Promise<PathValidateResult> {
  return invoke<PathValidateResult>('validate_download_path', { path });
}
```

### 输入

```json
{
  "path": "/home/user/Videos/yt-dlp"
}
```

### 返回

```typescript
interface PathValidateResult {
  valid: boolean;
  writable: boolean;
  exists: boolean;
  error: string | null;   // 不为 null 时描述失败原因
}
```

```json
// 成功
{ "valid": true, "writable": true, "exists": true, "error": null }

// 父目录不存在
{ "valid": false, "writable": false, "exists": false, "error": "路径所在的父目录不存在" }

// 无写入权限
{ "valid": false, "writable": false, "exists": true, "error": "无写入权限" }
```

### 实现

```rust
#[tauri::command]
pub async fn validate_download_path(path: String) -> Result<PathValidateResult, String> {
    let p = Path::new(&path);
    // 1. 检查是否存在
    // 2. 不存在则尝试 create_dir_all
    // 3. 写入临时文件验证可写性
    // 4. 清理临时文件
}
```

### 测试

- 单元测试在 `services/settings_validator.rs`
- 覆盖：存在且可写、不存在但父目录可写、父目录不可写、权限不足

---

## `validate_proxy_url`（新增）

### 目的

用户输入代理地址后，前端调用此命令验证格式合法性。

### 调用

```typescript
// src/lib/tauri.ts
export async function validateProxyUrl(url: string): Promise<ProxyValidateResult> {
  return invoke<ProxyValidateResult>('validate_proxy_url', { url });
}
```

### 输入

```json
{
  "url": "http://127.0.0.1:7890"
}
```

### 返回

```typescript
interface ProxyValidateResult {
  valid: boolean;
  scheme: string | null;   // 识别到的协议
  error: string | null;
}
```

```json
// 成功
{ "valid": true, "scheme": "http", "error": null }
// 空（允许不使用代理）
{ "valid": true, "scheme": null, "error": null }
// 格式错误
{ "valid": false, "scheme": null, "error": "代理地址格式无效: ..." }
// 不支持的协议
{ "valid": false, "scheme": "ftp", "error": "不支持的代理协议: ftp" }
```

### 实现

使用 `url` crate v2.x 解析：
```rust
use url::Url;

pub fn validate_proxy_url(input: &str) -> ProxyValidateResult {
    if input.trim().is_empty() {
        return ProxyValidateResult { valid: true, scheme: None, error: None };
    }
    match Url::parse(input) {
        Ok(url) => {
            let scheme = url.scheme();
            match scheme {
                "http" | "https" | "socks5" | "socks5h" => {
                    if url.host().is_some() {
                        ProxyValidateResult { valid: true, scheme: Some(scheme.to_string()), error: None }
                    } else {
                        ProxyValidateResult { valid: false, scheme: Some(scheme.to_string()), error: "代理地址缺少主机名".to_string() }
                    }
                }
                other => ProxyValidateResult { valid: false, scheme: Some(other.to_string()), error: format!("不支持的代理协议: {}", other) }
            }
        }
        Err(e) => ProxyValidateResult { valid: false, scheme: None, error: format!("代理地址格式无效: {}", e) }
    }
}
```

### 测试

- 空字符串 → 通过
- `http://127.0.0.1:7890` → 通过
- `socks5://127.0.0.1:1080` → 通过
- `socks5h://proxy.local:1080` → 通过
- `not-a-url` → 失败
- `ftp://proxy:21` → 失败（不支持的协议）
- `http://` → 失败（缺 host）
