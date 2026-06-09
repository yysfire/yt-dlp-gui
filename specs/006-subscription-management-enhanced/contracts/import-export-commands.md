# Tauri 命令契约：导入导出扩展

**Date**: 2026-06-10 | **Spec**: [spec.md](../spec.md)

## 命令列表

### 1. `batch_import_preview`（新增）

解析导入源并返回预览列表，不实际创建订阅。

```
Rust: batch_import_preview(source: ImportSource)
TypeScript: invoke("batch_import_preview", { source })

返回值: ImportPreview
```

**参数 `ImportSource`**:
```rust
enum ImportSource {
    OpmlFile { path: String },
    TxtFile { path: String },
    UrlList { urls: Vec<String> },
}
```

**返回值 `ImportPreview`**:
```typescript
interface ImportPreview {
  total: number;
  items: ImportPreviewItem[];
  duplicates: number;  // 与已有订阅重复的条目数
}

interface ImportPreviewItem {
  url: string;
  title: string | null;      // OPML 中可能有标题
  is_duplicate: boolean;     // 是否与已有订阅 URL 重复
  error: string | null;      // URL 无效等错误
}
```

### 2. `batch_import_execute`（新增）

确认预览后执行实际导入（异步后台任务）。

```
Rust: batch_import_execute(source: ImportSource, skip_duplicates: bool)
TypeScript: invoke("batch_import_execute", { source, skipDuplicates })

返回值: { task_id: string }
```

**行为**:
1. 快照导入项列表
2. `tokio::spawn` 后台任务
3. 对每个 URL：调用 `YtDlpService::parse_channel_info()` 解析频道
4. 检查重复（skip_duplicates=true 则跳过）
5. 创建 Subscription 并保存
6. 通过事件 `import-progress` 推送进度
7. 通过事件 `import-complete` 推送结果

### 3. `cancel_import`（新增）

取消正在进行的导入任务。

```
Rust: cancel_import(task_id: String)
TypeScript: invoke("cancel_import", { taskId })

返回值: void
```

## 事件

### `import-progress`

```typescript
interface ImportProgress {
  task_id: string;
  completed: number;
  total: number;
  current_url: string;
  current_title: string;
}
```

### `import-complete`

```typescript
interface ImportResult {
  task_id: string;
  total: number;
  success: number;
  failed: number;
  skipped: number;  // 跳过的重复项
  errors: { url: string; reason: string }[];
}
```

## TypeScript 类型

```typescript
// src/types/index.ts

export type ImportSourceType = "opml" | "txt" | "url_list";

export interface ImportSourceOpml {
  type: "opml";
  path: string;
}

export interface ImportSourceTxt {
  type: "txt";
  path: string;
}

export interface ImportSourceUrlList {
  type: "url_list";
  urls: string[];
}

export type ImportSource = ImportSourceOpml | ImportSourceTxt | ImportSourceUrlList;

export interface ImportPreview {
  total: number;
  items: ImportPreviewItem[];
  duplicates: number;
}

export interface ImportPreviewItem {
  url: string;
  title: string | null;
  is_duplicate: boolean;
  error: string | null;
}
```

## Frontend Tauri 封装

```typescript
// src/lib/tauri.ts 新增函数

export async function batchImportPreview(
  source: ImportSource
): Promise<ImportPreview> {
  return invoke("batch_import_preview", { source });
}

export async function batchImportExecute(
  source: ImportSource,
  skipDuplicates: boolean
): Promise<{ task_id: string }> {
  return invoke("batch_import_execute", { source, skipDuplicates });
}

export async function cancelImport(taskId: string): Promise<void> {
  return invoke("cancel_import", { taskId });
}
```

## 复用说明

- OPML 解析复用现有 `services/opml.rs` 的 `parse_opml()` 函数
- TXT 解析复用现有导入逻辑中的逐行 URL 解析
- 频道解析复用现有 `YtDlpService::parse_channel_info()`
- 预览阶段不调用 yt-dlp（仅解析源文件），执行阶段才并发调用 `parse_channel_info()`
