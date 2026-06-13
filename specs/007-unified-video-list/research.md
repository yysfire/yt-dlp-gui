# Research Report: DetailPanel 统一视频列表

**Feature**: 007-unified-video-list
**Date**: 2026-06-13

## 研究项

本功能为纯前端重构，无技术不确定项。所有技术选择已由现有架构和设计规格确定。

| 主题 | 决策 | 理由 | 替代方案 |
|------|------|------|----------|
| 合并策略 | 前端 `useMemo` 按 `video_id`/`video_url` 合并 | 数据源均在前端可用；后端已有去重逻辑，前端只需做视觉合并 | 后端合并：需新增命令，增加复杂度，与 KISS 原则冲突 |
| 后台分页 | 递归 `useCallback` + `requestIdRef` 竞态取消 | 复用现有 `getChannelVideos` API 和已有的 `requestIdRef` 模式 | `useEffect` 自动触发：难以控制加载时机和竞态取消 |
| 进度条复用 | 保留 `DownloadProgressBar`，直接嵌入统一条目 | 无变更成本，组件已成熟 | 重新实现内联进度条：违反 DRY |
| Component 移除 | 删除 `DownloadRecordList` + `DownloadRecordItem` | 职责已由 DetailPanel 内部接管，不再有外部引用 | 保留但改为内部使用：保留死代码，违反 YAGNI |
| TypeScript 类型 | 新增 `UnifiedVideoItem` 接口 + `VideoStatus` 联合类型 | 清晰表达合并后的数据结构 | 使用泛型 Map：过度抽象，违反 KISS |

## 结论

无需额外调研。所有技术决策已在设计规格中明确，实施路径清晰。

