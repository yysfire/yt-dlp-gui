# Quickstart: DetailPanel 统一视频列表

**Feature**: 007-unified-video-list
**Date**: 2026-06-13

## 验证步骤

### 1. 开发模式启动

```bash
npm run tauri dev
```

### 2. 关键验证点

| 场景 | 预期行为 | 验证方式 |
|------|----------|----------|
| 点击有下载记录的订阅 | 右侧面板显示单一视频列表，无重复条目 | 目视确认 |
| 切换订阅 | 列表清空后显示新订阅视频，无旧数据残留 | 目视确认 |
| 等待后台加载 | 视频列表自动增长，无"加载更多"按钮 | 目视确认 |
| 触发下载 | 对应条目显示进度条和百分比 | 目视确认 |
| 下载完成 | 条目状态变为"已完成"，进度条消失 | 目视确认 |
| 频道失效 | 列表仅显示下载记录，无频道视频加载 | 目视确认 |

### 3. 编译检查

```bash
npx tsc --noEmit    # TypeScript 类型检查
cargo test           # Rust 测试（无变更，保持通过）
```

### 4. 生产构建

```bash
npm run build
```

## 文件清理

实现完成后确认以下文件已移除：

- [ ] `src/components/DownloadRecordList.tsx`
- [ ] `src/components/DownloadRecordItem.tsx`

确认以下文件已变更：

- [ ] `src/components/DetailPanel.tsx` (重写)
- [ ] `src/types/index.ts` (新增类型)

确认以下文件无变更：

- [ ] `src/components/DownloadedList.tsx`
- [ ] `src/components/DownloadedItem.tsx`
- [ ] `src/components/DownloadProgressBar.tsx`
- [ ] `src/components/AppShell.tsx`
- [ ] `src-tauri/` (所有文件)
