# Tray IPC Contracts

**Phase 1** | **Date**: 2026-06-09

## Overview

托盘功能主要通过 Rust 端实现，前端仅通过 IPC 命令获取/更新配置和接收状态事件。以下为涉及的前后端接口约定。

## Tauri Commands（Rust → 前端）

### `get_tray_state`

获取当前托盘运行时状态。

```
命令: get_tray_state
参数: 无
返回: { window_visible: bool, tray_supported: bool, status: string, active_downloads: number }
```

- `status`: `"idle"` | `"downloading"` | `"checking"`
- `tray_supported`: 桌面环境是否支持托盘
- 用于前端判断是否显示托盘相关 UI 提示

### `get_settings`（扩展）

现有命令，返回 `AppSettings` 包含托盘相关的新字段。

```
命令: get_settings
参数: 无
返回: AppSettings (含 minimize_to_tray, close_to_tray, start_in_tray, scheduler_paused)
```

### `update_settings`（扩展）

现有命令，支持更新托盘配置字段。

```
命令: update_settings
参数: { settings: AppSettings }
返回: AppSettings (更新后的完整设置)
```

- 前端在 `SettingsDialog.tsx` 中通过此命令保存托盘行为开关

## Frontend invoke 封装（`src/lib/tauri.ts` 扩展）

```typescript
// 新增 invoke 调用

/** 获取当前托盘运行时状态 */
export async function getTrayState(): Promise<TrayState> {
  return invoke<TrayState>('get_tray_state');
}

// 扩展现有类型
export interface TrayState {
  window_visible: boolean;
  tray_supported: boolean;
  status: 'idle' | 'downloading' | 'checking';
  active_downloads: number;
}

// AppSettings 扩展字段在 types/index.ts 中已通过接口定义
```

## Backend Events（Rust → 前端推送）

### `tray-state-changed`

托盘状态变化时推送给前端。

```json
{
  "event": "tray-state-changed",
  "payload": {
    "status": "downloading",
    "active_downloads": 3,
    "window_visible": true,
    "tray_supported": true
  }
}
```

- 前端监听此事件更新 UI（如状态栏、设置页面的托盘状态指示）

### 已有的 `download-progress` 事件（不变）

现有的 `download-progress` 事件由 `download.rs` 发出。当所有下载完成时，托盘 Service 通过监听此事件（或其内部计数）来判断何时从 `Downloading` 切换到 `Idle`。

## 现有命令复用（不新增接口）

| 托盘菜单操作 | 复用的现有 Tauri 命令 | 说明 |
|-------------|---------------------|------|
| "检查全部订阅更新" | `check_all` | 现有命令，由托盘菜单事件直接调用 |
| "暂停/恢复定时检查" | 不通过 invoke | 托盘菜单事件直接更新 `AppSettings.scheduler_paused` + 发送 watch 通知 |
| "退出" | 不通过 invoke | 托盘菜单事件直接执行退出流程 |

## 前端状态同步

```
Rust setup() 创建托盘
  │
  ├─ 托盘事件 → Rust 命令 → 结果 → emit 事件 → 前端
  │
  │    例: "检查全部更新" → check_all → download-progress → 前端更新进度
  │                                → records-changed → 前端刷新列表
  │
  └─ 托盘状态变化 → emit "tray-state-changed" → 前端更新状态栏
```

前端**不直接**操作托盘图标、菜单或状态。所有托盘操作通过 Rust 端 `on_menu_event` / `on_tray_icon_event` 回调处理。
