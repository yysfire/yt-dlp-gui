# IPC Contracts: 文件管理命令

**Feature**: specs/003-basic-file-management
**Date**: 2026-06-04

## 新增 Tauri 命令

### 1. `open_in_folder`

打开文件所在文件夹。

```
命令名: open_in_folder
```

**参数**:

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| `file_path` | `String` | ✅ | 文件的完整路径（绝对路径） |

**返回值**: `Result<(), String>`

**行为**:
1. 解析 `file_path` 获取父目录
2. 调用 `tauri-plugin-shell::ShellExt::open()` 打开父目录
3. 文件不存在时仍打开父目录（降级行为）
4. 目录不存在时返回错误

**前端 TypeScript**:

```typescript
// src/lib/tauri.ts
export async function openInFolder(filePath: string): Promise<void> {
  return invoke("open_in_folder", { filePath });
}
```

**错误情况**:

| 错误 | 条件 |
|------|------|
| `"父目录不存在: {path}"` | 文件路径所在目录已被删除 |

---

### 2. `check_file_existence`

批量检查文件是否存在。

```
命令名: check_file_existence
```

**参数**:

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| `file_paths` | `Vec<String>` | ✅ | 要检查的文件路径列表 |

**返回值**: `Result<Vec<FileExistenceResult>, String>`

```rust
#[derive(Serialize)]
struct FileExistenceResult {
    file_path: String,
    exists: bool,
}
```

**行为**:
1. 批量并发检查所有文件的 `std::fs::metadata()`（限 50 并发）
2. 返回每个文件的 `exists: bool` 结果
3. 外部存储断连等 I/O 错误也返回 `exists: false`（不区分错误类型）

**前端 TypeScript**:

```typescript
// src/lib/tauri.ts
export interface FileExistenceResult {
  file_path: string;
  exists: boolean;
}
export async function checkFileExistence(filePaths: string[]): Promise<FileExistenceResult[]> {
  return invoke("check_file_existence", { filePaths });
}
```

---

### 3. `delete_file`

从应用内删除已下载视频文件（保留记录）。

```
命令名: delete_file
```

**参数**:

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| `id` | `String` | ✅ | DownloadRecord 的 id |

**返回值**: `Result<(), String>`

**行为**:
1. 通过 `id` 查找 DownloadRecord
2. 使用 `std::fs::remove_file()` 删除物理文件
3. 文件不存在时不报错（静默成功）
4. 更新记录 `status` 为 `"deleted"`
5. 持久化更新后的记录
6. 发送 `records-changed` 事件通知前端

**前端 TypeScript**:

```typescript
// src/lib/tauri.ts
export async function deleteFile(id: string): Promise<void> {
  return invoke("delete_file", { id });
}
```

**错误情况**:

| 错误 | 条件 |
|------|------|
| `"下载记录不存在"` | 给定 id 的记录未找到 |
| `"删除文件失败: {msg}"` | 文件系统操作失败（如权限不足） |

---

### 4. `sync_file_states`

触发文件状态同步（检查所有记录的文件是否存在）。

```
命令名: sync_file_states
```

**参数**: 无

**返回值**: `Result<Vec<FileExistenceResult>, String>`

**行为**:
1. 加载所有 DownloadRecord（仅 status 为 `"completed"` 的记录）
2. 调用 `check_file_existence` 批量检查
3. 发送 `file-sync-complete` 事件通知前端（携带结果）

**前端 TypeScript**:

```typescript
// src/lib/tauri.ts
export async function syncFileStates(): Promise<FileExistenceResult[]> {
  return invoke("sync_file_states");
}
```

---

## 事件

### `records-changed`（已有）

现有事件，文件删除后触发，前端刷新列表。

**Payload**: `DownloadRecord[]` — 更新后的全部记录

### `file-sync-complete`（新增）

文件状态同步完成后触发，前端更新文件存在性标记。

**Payload**: `FileExistenceResult[]` — 所有文件的检查结果

**前端监听**:

```typescript
// App.tsx
useEffect(() => {
  const unlisten = listen<FileExistenceResult[]>("file-sync-complete", (event) => {
    // 更新下载记录的文件存在性标记
  });
  return () => unlisten.then(fn => fn());
}, []);
```

---

## 前端 `src/lib/tauri.ts` 汇总

```typescript
// 新增部分
export interface FileExistenceResult {
  file_path: string;
  exists: boolean;
}

export async function openInFolder(filePath: string): Promise<void> {
  return invoke("open_in_folder", { filePath });
}

export async function checkFileExistence(filePaths: string[]): Promise<FileExistenceResult[]> {
  return invoke("check_file_existence", { filePaths });
}

export async function deleteFile(id: string): Promise<void> {
  return invoke("delete_file", { id });
}

export async function syncFileStates(): Promise<FileExistenceResult[]> {
  return invoke("sync_file_states");
}
```

## Rust 端注册

在 `lib.rs` 中注册新命令：

```rust
pub fn run() {
    tauri::Builder::default()
        // ... 现有插件和命令 ...
        .invoke_handler(tauri::generate_handler![
            // 现有命令...
            crate::commands::download::open_in_folder,
            crate::commands::download::check_file_existence,
            crate::commands::download::delete_file,
            crate::commands::download::sync_file_states,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```
