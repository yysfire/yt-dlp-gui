# Data Model: 必要设置

**Phase 1** | **Date**: 2026-06-05

## 实体：AppSettings

应用中唯一的设置实体，单例（一份 JSON 文件，一个运行时实例）。

### 字段定义

| 字段 | Rust 类型 | TS 类型 | 默认值 | 约束 | 规格来源 |
|------|----------|---------|--------|------|----------|
| `download_dir` | `String` | `string` | 系统默认下载目录 | 非空字符串，路径存在且可写 | FR-001, FR-002, FR-003 |
| `quality_preset` | `String` | `string` | `"1080p"` | 枚举：`"最高画质"`, `"1080p"`, `"720p"`, `"480p"`, `"仅音频"` | FR-004, Assumptions |
| `proxy_url` | `String` | `string` | `""` | 空或合法 URL（scheme: http/https/socks5/socks5h） | FR-005, Assumptions |
| `max_concurrent_downloads` | `u32` | `number` | `1` | 范围 1-5（含） | FR-006, Assumptions |
| `check_interval_minutes` | `u32` | `number` | `60` | 0（手动）或有效正整数 | FR-007, Assumptions |
| `yt_dlp_path` | `String` | `string` | `"yt-dlp"` | 非空字符串 | (已有字段) |
| `notifications_enabled` | `bool` | `boolean` | `false` | — | (已有字段，非本规格范围) |
| `dark_mode` | `bool` | `boolean` | `false` | — | (已有字段) |
| `cookie_file` | `String` | `string` | `""` | 空或合法文件路径 | (已有字段) |

> **已存在但非本规格范围的字段**：`notifications_enabled`, `dark_mode`, `cookie_file`, `yt_dlp_path` 在当前代码中已存在，无需新增或修改。

### 验证规则

| 字段 | 规则 | 验证位置 | 测试要求 |
|------|------|----------|----------|
| `download_dir` | 非空；路径存在；可写入（`create_dir_all` + 后续写入测试文件） | `settings_validator.rs` (新增 Service) | 单元测试：空路径、不存在、权限不足、正常 |
| `proxy_url` | 若为空则跳过；否则必须是合法 URL，scheme 为 http/https/socks5/socks5h，含 host | `settings_validator.rs` | 单元测试：空、合法 http、合法 socks5、缺 host、非法 scheme |
| `max_concurrent_downloads` | 整数 ≥1 且 ≤5 | 前端 + `update_settings` 命令 | 单元测试：0、1、5、6 |
| `check_interval_minutes` | 0（手动）或 ≥15（有效间隔）| 前端下拉限定 + 后端允许 | 不强制拒绝 <15，但 UI 提示建议 |
| `quality_preset` | 必须为预设列表之一 | 前端下拉 + 后端枚举检查 | 单元测试：合法值、非法值 |

### 状态转换

```
[不存在] ──(首次启动)──→ [默认值] ──(用户修改)──→ [用户配置] ──(持久化)──→ settings.json
                              ↑                                                    │
                              │                                                    │
                              └────────────(重启加载)───────────────────────────────┘

[损坏的 settings.json] ──(启动检测)──→ [默认值]  (FR-009, SC-005)
```

**并发数调整的状态影响**：
```
用户设置 max_concurrent_downloads = N
  │
  ├─ N > active_count → 从等待队列中取出 (N - active_count) 个任务开始下载
  └─ N < active_count → 暂停 (active_count - N) 个任务
       │ 暂停逻辑：按 `percent` 升序排列活跃任务，暂停进度最少的
       │ 暂停语义：通过 DownloadQueue 的 `TaskControlSignal` 发信号
       └ 被暂停任务保留在队列中，等待活跃任务完成后重新获取 slot
```

### 持久化

- **格式**：JSON（serde 序列化）
- **路径**：`~/.yt-dlp-sub-gui/settings.json`
- **读写**：`StorageService::load_settings()` / `StorageService::save_settings()`
- **缺失处理**：返回 `Default::default()`
- **损坏处理**：serde 反序列化失败 → 返回 `Default::default()`（已有 `#[serde(default)]` 容错）

### 类型定义（TypeScript）

```typescript
// src/types/index.ts 扩展现有 AppSettings 接口
interface AppSettings {
  download_dir: string;            // FR-001
  check_interval_minutes: number;  // FR-007
  yt_dlp_path: string;             // (已有)
  quality_preset: string;          // FR-004
  notifications_enabled: boolean;  // (已有，本规格不修改)
  dark_mode: boolean;              // (已有)
  proxy_url: string;               // FR-005
  cookie_file: string;             // (已有)
  max_concurrent_downloads: number; // FR-006 (已有字段，仅扩展上限)
}
```

### 类型定义（Rust）

```rust
// src-tauri/src/models/settings.rs 扩展
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_download_dir")]
    pub download_dir: String,
    #[serde(default = "default_check_interval")]
    pub check_interval_minutes: u32,  // 默认值改为 60
    #[serde(default = "default_yt_dlp_path")]
    pub yt_dlp_path: String,
    #[serde(default = "default_quality_preset")]
    pub quality_preset: String,
    #[serde(default)]
    pub notifications_enabled: bool,
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub cookie_file: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: u32,  // 字段已存在，上限扩展至 5
}

fn default_check_interval() -> u32 { 60 }   // 从 360 改为 60
fn default_max_concurrent() -> u32 { 1 }     // 不变
```
