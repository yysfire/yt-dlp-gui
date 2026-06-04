# Quickstart: 基本文件管理

**Feature**: specs/003-basic-file-management
**Date**: 2026-06-04

## 实现顺序

```
Phase 0 (research.md) 已完成 → Phase 1 (data-model + contracts) 已完成 → tasks.md (下一步)
```

## 开发环境

```bash
# 前端开发（Vite 热更新，端口 5173）
npm run dev

# 完整 Tauri 应用（含 Rust 后端）
npm run tauri dev

# Rust 测试
cargo test

# TypeScript 类型检查
npx tsc --noEmit
```

## 变更清单

按实现顺序排列：

### 1. 类型定义更新

| 文件 | 变更 |
|------|------|
| `src/types/index.ts` | `DownloadRecord.status` 联合类型追加 `"deleted"`；新增 `FileExistenceResult` 接口 |
| `src-tauri/src/models/download.rs` | `status` 字段注释追加 `"deleted"` 状态说明 |

### 2. Rust Service 层

| 文件 | 变更 |
|------|------|
| `src-tauri/src/services/file_manager.rs` | **[新增]** `check_files_exist()` — 并发文件存在性检测；`open_in_folder()` — 跨平台打开文件夹；`delete_file_and_update_record()` — 删除文件并更新记录；`start_sync_timer()` — 启动定时同步 |

### 3. Rust 命令层

| 文件 | 变更 |
|------|------|
| `src-tauri/src/commands/download.rs` | 新增 `open_in_folder`、`check_file_existence`、`delete_file`、`sync_file_states` 四个 `#[tauri::command]` |

### 4. Rust 注册

| 文件 | 变更 |
|------|------|
| `src-tauri/src/lib.rs` | `invoke_handler` 中注册 4 个新命令 |
| `src-tauri/src/services/mod.rs` | 声明 `file_manager` 模块 |

### 5. 前端 invoke 封装

| 文件 | 变更 |
|------|------|
| `src/lib/tauri.ts` | 新增 `openInFolder()`、`checkFileExistence()`、`deleteFile()`、`syncFileStates()` |

### 6. 前端 UI 组件

| 文件 | 变更 |
|------|------|
| `src/components/AppShell.tsx` | 左侧边栏新增"已下载"导航项，视图切换状态管理 |
| `src/components/DownloadedList.tsx` | **[新增]** 文件管理主视图（搜索框 + 列表 + 操作栏） |
| `src/components/DownloadedItem.tsx` | **[新增]** 单条下载记录行（标题/频道/大小/状态/操作按钮） |
| `src/components/FileSearchBar.tsx` | **[新增]** 搜索框组件 |

### 7. 前端 Hook

| 文件 | 变更 |
|------|------|
| `src/hooks/useDownloadRecords.ts` | 新增搜索过滤 `useMemo`、文件同步触发、删除操作、定时同步事件监听 |

## 测试策略

### Rust 单元测试（TDD 顺序）

1. `file_manager.rs`:
   - `test_check_files_exist_batch` — 批量文件检测
   - `test_check_files_exist_missing` — 缺失文件处理
   - `test_open_in_folder_valid_path` — 正常路径（需 mock shell）
   - `test_delete_file_and_update_status` — 删除文件 + 状态更新
   - `test_delete_file_already_deleted` — 文件已不存在时的删除

2. `download.rs` (新增测试):
   - `test_download_record_status_deleted_serde` — `"deleted"` 状态序列化/反序列化

### TypeScript

- `npx tsc --noEmit` 无类型错误
- 搜索过滤函数单独测试（`useMemo` 逻辑可提取为纯函数）

## 验收场景对照

| 规格场景 | 实现要点 |
|----------|----------|
| US1: 查看列表 | DownloadedList + DownloadedItem 组件，`get_all_download_records` 获取数据 |
| US2: 打开文件夹 | DownloadedItem 中的"打开文件夹"按钮 → `openInFolder()` |
| US3: 搜索 | FileSearchBar → `useMemo` 客户端过滤 |
| US4: 同步检测 | 启动时 + 手动刷新 + 定时器（SchedulerService，5 分钟） |
| FR-009: 文件删除 | DownloadedItem 中的"删除"按钮 → `deleteFile()` |
| FR-010: 定时同步 | `SchedulerService::start(5, sync_callback)` |

## 关键约束

- 文件删除后**不提供清除记录入口**（数据模型保持简单）
- 搜索**仅内存过滤**，不发送后端请求
- 文件检测**仅检查存在性**（`metadata()`），不校验内容
- 下载记录中 status=`"deleted"` 的在列表中**灰色显示**或**带标记**
