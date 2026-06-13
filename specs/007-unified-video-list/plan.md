# Implementation Plan: DetailPanel 统一视频列表

**Branch**: `007-unified-video-list` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-unified-video-list/spec.md`

## Summary

重构 `DetailPanel.tsx`，将"频道视频"和"下载记录"两个独立渲染区域合并为一个统一视频列表。前端使用 `useMemo` 按 `video_id`/`video_url` 合并 `videoList` + `records` + `queueTasks` 为 `UnifiedVideoItem[]`，后台自动分页加载全部频道视频，移除 `DownloadRecordList` 组件。Rust 后端无需变更。

## Technical Context

**Language/Version**: TypeScript 5.x + Rust 1.x (Tauri v2)
**Primary Dependencies**: React 18, MUI 5, Tailwind CSS 3, serde
**Storage**: JSON 文件 (`~/.yt-dlp-sub-gui/`, 无变更)
**Testing**: cargo test (Rust 单元测试), npx tsc --noEmit (TypeScript 类型检查)
**Target Platform**: Windows / macOS / Linux 桌面端 (Tauri v2)
**Project Type**: desktop-app (Tauri v2 + React frontend)
**Performance Goals**: 打开订阅后 1 秒内看到下载记录, 3 秒内看到合并列表; 进度条更新延迟 ≤1 秒
**Constraints**: 后台分页不阻塞 UI; 切换订阅需取消在途请求
**Scale/Scope**: 单频道最多数千视频; 前端纯渲染层变更, 后端无改动

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查项 | 状态 |
|------|--------|------|
| I. TDD | 实现新逻辑前须编写失败测试 | ⚠ 前端无正式测试框架; 采用 `tsc --noEmit` + Rust 现有测试保持通过 + 手动验证关键交互路径 |
| II. 可测试性 | 合并逻辑封装在 `useMemo` 中, 纯函数可测试; 后台加载封装在 `useCallback` 中 | ✅ |
| III. KISS | 复用现有 `getChannelVideos` API, 复用 `DownloadProgressBar` 组件, JSON 文件存储不变 | ✅ |
| IV. DRY | 合并逻辑集中在一个 `useMemo` 中; 排序逻辑集中在一个函数中 | ✅ |
| V. YAGNI | 不新增过滤标签 UI (未在规格中要求); 不新增虚拟滚动 (当前频道规模不需要) | ✅ |

**Gate Result**: 全部通过。TDD 放宽理由: 本项目前端无测试框架, 遵循 CODEBUDDY.md 中"前端组件 MUST 通过手动测试验证关键交互路径"的指示。

## Project Structure

### Documentation (this feature)

```text
specs/007-unified-video-list/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (N/A — 无外部接口变更)
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── components/
│   ├── DetailPanel.tsx           # 重写：合并逻辑 + 三行三列渲染
│   ├── DownloadRecordList.tsx    # 移除
│   ├── DownloadRecordItem.tsx    # 移除（其渲染逻辑内联到 DetailPanel）
│   └── DownloadProgressBar.tsx   # 保留（复用）
├── types/
│   └── index.ts                  # 新增 UnifiedVideoItem、VideoStatus 类型
└── lib/
    └── tauri.ts                  # 无变更（复用现有 invoke 封装）

src-tauri/
└── src/                          # 无变更
```

**Structure Decision**: 单一前端项目结构（Option 1）。`src/` 下变更仅涉及 `components/` 和 `types/`，后端 `src-tauri/` 无改动。

## Complexity Tracking

> 无章程违规，无需记录。

