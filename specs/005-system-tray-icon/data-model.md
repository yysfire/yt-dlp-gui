# Data Model: 系统托盘图标

**Phase 1** | **Date**: 2026-06-09

## 实体：AppSettings（扩展）

在现有 `AppSettings` 结构体中新增托盘行为配置字段。

### 新增字段

| 字段 | Rust 类型 | TS 类型 | 默认值 | 约束 | 规格来源 |
|------|----------|---------|--------|------|----------|
| `minimize_to_tray` | `bool` | `boolean` | `true` | — | FR-010, Assumptions |
| `close_to_tray` | `bool` | `boolean` | `true` | — | FR-011, Assumptions |
| `start_in_tray` | `bool` | `boolean` | `false` | — | FR-012, Assumptions |
| `scheduler_paused` | `bool` | `boolean` | `false` | 由托盘菜单切换，需持久化 | FR-005 (调度器状态需跨启动保持) |

> `scheduler_paused` 用于跟踪托盘菜单"暂停/恢复"操作的持久化状态。当前代码可能已有等效字段或机制；若有用现有机制，需研究确认。

### 完整 AppSettings 结构（扩展后）

```rust
// src-tauri/src/models/settings.rs
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // --- 已有字段 ---
    #[serde(default = "default_download_dir")]
    pub download_dir: String,
    #[serde(default = "default_check_interval")]
    pub check_interval_minutes: u32,
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
    pub max_concurrent_downloads: u32,

    // --- 新增托盘字段 ---
    #[serde(default)]
    pub minimize_to_tray: bool,       // 默认 true (serde bool 默认 false，需自定义)
    #[serde(default)]
    pub close_to_tray: bool,          // 默认 true
    #[serde(default)]
    pub start_in_tray: bool,          // 默认 false
    #[serde(default)]
    pub scheduler_paused: bool,       // 默认 false
}
```

### TypeScript 类型扩展

```typescript
// src/types/index.ts
interface AppSettings {
  // --- 已有字段 ---
  download_dir: string;
  check_interval_minutes: number;
  yt_dlp_path: string;
  quality_preset: string;
  notifications_enabled: boolean;
  dark_mode: boolean;
  proxy_url: string;
  cookie_file: string;
  max_concurrent_downloads: number;

  // --- 新增托盘字段 ---
  minimize_to_tray: boolean;   // 默认 true
  close_to_tray: boolean;      // 默认 true
  start_in_tray: boolean;      // 默认 false
  scheduler_paused: boolean;   // 默认 false
}
```

## 实体：TrayState（运行时，不持久化）

托盘图标的运行时状态，存储在 `services/tray.rs` 的 Service 中。

### 字段定义

| 字段 | Rust 类型 | 说明 |
|------|----------|------|
| `status` | `TrayStatus` 枚举 | 空闲 / 下载中 / 检查中 |
| `window_visible` | `bool` | 主窗口当前是否可见 |
| `tray_supported` | `bool` | 桌面环境是否支持托盘 |
| `active_downloads` | `u32` | 当前活跃下载数（仅 Downloading 状态时有效） |

### TrayStatus 枚举

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TrayStatus {
    Idle,
    Downloading { active_count: u32 },
    Checking,
}
```

### 状态转换图

```
                    ┌──────────┐
        启动时       │   Idle    │
     ──────────────→ │          │
                    └─────┬────┘
                          │
          ┌───────────────┼───────────────┐
          │ 开始下载       │ 调度器检查     │
          ▼               │ 开始           ▼
   ┌──────────────┐      │        ┌───────────┐
   │ Downloading  │      │        │ Checking  │
   │ {active: N}  │      │        │           │
   └──────┬───────┘      │        └─────┬─────┘
          │              │              │
          │ 全部下载完成   │              │ 检查完成
          ▼              │              ▼
     ┌──────────┐        │        ┌──────────┐
     │   Idle   │ ◄──────┘        │   Idle   │
     └──────────┘                 └──────────┘
```

## 新增错误类型

```rust
// src-tauri/src/utils/error.rs 扩展
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // --- 已有变体 ---
    // ...

    // --- 新增变体 ---
    #[error("托盘图标初始化失败: {0}")]
    TrayInitialization(String),

    #[error("当前桌面环境不支持系统托盘")]
    TrayNotSupported,

    #[error("托盘图标更新失败: {0}")]
    TrayUpdate(String),
}
```

## 验证规则

| 字段 | 规则 | 验证位置 | 测试要求 |
|------|------|----------|----------|
| `minimize_to_tray` | 布尔值，无额外约束 | 无需后端验证 | 无需额外测试 |
| `close_to_tray` | 布尔值，无额外约束 | 无需后端验证 | 无需额外测试 |
| `start_in_tray` | 布尔值，无额外约束 | 无需后端验证 | 无需额外测试 |
| `scheduler_paused` | 布尔值，无额外约束 | 无需后端验证 | 无需额外测试 |
| TrayState 转换 | Idle → Downloading/Checking; Downloading/Checking → Idle | services/tray.rs | 单元测试：合法/非法状态转换 |

## 持久化

- **托盘配置字段**：作为 `AppSettings` 的一部分，通过 `StorageService::save_settings()` / `load_settings()` 读写
- **运行时状态**：不持久化，每次启动重新初始化
- **图标文件**：编译时嵌入二进制，位于 `src-tauri/icons/tray-*.png`
