# Phase 0: Research — 订阅管理增强

**Date**: 2026-06-10 | **Plan**: [plan.md](./plan.md)

## 决策记录

### 1. HTTP 客户端选型

**Decision**: 直接依赖 `reqwest` 作为 Rust 端 HTTP 客户端

**Rationale**:
- `reqwest` 已是 Tauri v2 的传递依赖（在 `Cargo.lock` 中存在），升级为直接依赖几乎不增加编译时间
- `tauri-plugin-http` 将请求代理到前端 WebView 的 fetch()，引入不必要的 IPC 往返开销
- 健康检查是纯协议级操作（HEAD 请求），不需要浏览器环境
- 与现有 Services 层无状态函数模式一致

**Alternatives considered**:
- `tauri-plugin-http`: 适合前端发起的 HTTP 请求，不适合后端高并发场景
- `ureq`: 同步阻塞库，不适合 tokio 异步运行时

### 2. 健康检查并发策略

**Decision**: 使用 `tokio::sync::Semaphore(3)` 控制并发，不按平台分组冷却

**Rationale**:
- 3 并发满足 SC-002 性能目标（50 个订阅，30 秒内）
- 正常情况每批 1-3 秒，50/3=17 批，总计约 20-30 秒
- 被限流（429）时使用指数退避重试（最多 2 次：2s + random(0..1s)、4s + random(0..2s)）
- 超时设置：连接超时 5 秒，总超时 10 秒

**Alternatives considered**:
- 平台感知冷却间隔（同 YouTube URL 之间 2-3 秒冷却）：增加复杂度但实际改善有限，且可能导致极端场景超时
- 高阶并发（5+）：增加被平台反爬/限流风险

### 3. 并发修改处理

**Decision**: 快照隔离（Snapshot Isolation）

**Rationale**:
- 健康检查开始前快照订阅列表和设置
- 整个检查过程基于快照进行
- 检查期间用户删除/暂停/修改 URL 不影响当前检查
- 前端收到 `health-check-complete` 事件时，若对应订阅已不在列表中则丢弃结果

**Alternatives considered**:
- 实时同步（检查期间锁定修改操作）：严重影响用户体验
- 增量结果推送：增加复杂度，收益有限

### 4. 前端筛选状态管理

**Decision**: `useState` 提升到 `App.tsx`

**Rationale**:
- 组件卸载/重挂时不丢失状态（App.tsx 在整个会话期间常驻）
- 应用关闭后自动丢失，满足"不跨会话恢复"
- 遵循现有数据流模式（状态中心在 App.tsx）
- `useRef` 不触发重渲染，无法驱动 UI 更新
- `sessionStorage` 过度设计（webview 可能被挂起后重建）

**Alternatives considered**:
- `useReducer`：对复杂状态更友好，但筛选状态足够简单，useState 即可
- `sessionStorage`：浏览器级会话，在 Tauri webview 中不可靠

### 5. Hook 设计

**Decision**: 单个 `useFilter` hook（筛选 + 排序合一）

**Rationale**:
- 筛选和排序在 FR-012 至 FR-014 中高度耦合
- 拆分增加 useMemo 多层传递和依赖链复杂度
- 输入：`Subscription[]`；输出：`{ filtered, sorted, filter, setFilter, sort, setSort }`

### 6. 搜索实现

**Decision**: `String.prototype.includes()` + JSX 递归高亮

**Rationale**:
- 字面匹配天然安全，不触发正则（满足边缘情况要求）
- 匹配范围：`channel_name` + `url`
- 纯前端实现，无 Tauri IPC 调用
- 性能：O(n*m) 字符串操作，500 条目远低于 200ms 约束

### 7. 详情面板视频列表

**Decision**: `yt-dlp --flat-playlist --playlist-end 10`，支持分页加载更多

**Rationale**:
- 初始加载 10 条满足 SC-004（< 1 秒）
- `--playlist-end` 限制 yt-dlp 输出量
- 分页通过 `--playlist-start` + `--playlist-end` 实现
- 前端通过 Tauri invoke 传参请求指定页

### 8. 失效订阅自动处理

**Decision**: 健康检查完成后，将 `health_status === "dead"` 的订阅 `enabled` 自动置为 false

**Rationale**:
- 避免调度器继续对失效频道执行 yt-dlp 调用
- 用户可通过订阅列表手动重新启用
- 与章程 KISS 原则一致（最小化代码修改）
