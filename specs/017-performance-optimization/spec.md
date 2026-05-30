# Feature Specification: 性能优化 (Performance Optimization)

**Feature Branch**: `017-performance-optimization`
**Created**: 2026-05-31
**Status**: Draft
**Input**: From PRD v1.0 for yt-dlp subscription manager desktop app (P2 Advanced Features Phase)

## User Scenarios & Testing *(mandatory)*

### US-017-01: 启动优化

应用从双击图标到可交互的冷启动时间显著缩短，大容量数据场景下无明显等待。

**Given** 用户有 200 个订阅、10000 条下载记录
**When** 用户启动应用
**Then** 应用窗口在 1 秒内显示
**And** 侧边栏订阅列表在 2 秒内渲染完成
**And** 启动过程中显示加载骨架屏（skeleton），不出现白屏
**And** 数据加载采用分批/懒加载策略，首屏仅加载关键数据

### US-017-02: 内存优化

应用在后台长时间运行时内存占用维持在合理水平，不出现内存泄漏。

**Given** 应用连续运行 24 小时，执行 10 轮调度检查（涉及 50 个订阅）
**When** 用户查看系统资源监控
**Then** 内存占用稳定在 200MB 以内（不含视频下载缓冲区）
**And** 每轮调度检查后内存恢复到检查前水平（无泄漏）
**And** 下载历史数据仅在需要时加载，完成后释放
**And** 大 JSON 文件读取使用流式处理，不全部载入内存

### US-017-03: 数据库优化

JSON 文件存储在大数据量下保持高效的读写性能，减少 I/O 竞争。

**Given** download_records.json 达到 10MB（约 5000 条记录）
**When** 有新下载记录需要追加写入
**Then** 写入耗时 < 100ms，不阻塞 UI 线程
**And** 读取操作使用文件级缓存（Tauri State 中缓存解析后的数据）
**And** 写入使用原子替换策略：先写临时文件 → rename → 刷新缓存
**And** 支持定期自动压缩：移除碎片、优化 JSON 格式

### US-017-04: 缓存策略

应用对频繁访问的数据（订阅列表、设置、最近下载记录）实现内存缓存，减少文件 I/O。

**Given** 用户频繁切换选中订阅，每次切换需要读取下载记录
**When** 系统实现多层缓存策略
**Then** 订阅列表缓存在 Tauri State 中，仅在增删改时刷新
**And** 最近 100 条下载记录缓存在内存中，旧记录按需加载
**And** 缩略图缓存在文件系统中，首次加载后复用
**And** 设置变更仅写入磁盘，读取时命中内存缓存

### US-017-05: 懒加载

前端组件和列表实现虚拟滚动和懒加载，大列表下保持流畅的用户交互体验。

**Given** 用户有 500 条订阅和 5000 条下载记录
**When** 用户浏览订阅列表和下载记录列表
**Then** 订阅列表使用虚拟滚动，仅渲染可视区域内的 20 个项目
**And** 下载记录分页加载：初始加载最近 50 条，滚动到底部时加载更多
**And** 详情面板中的下载记录图表仅在面板展开时才加载
**And** 弹窗组件懒加载：仅在首次打开时初始化

## Requirements *(mandatory)*

### Functional Requirements

#### FR-017-01: 启动优化
- 系统 SHALL 在 Rust 端异步并行加载 JSON 文件（使用 `tokio::spawn`）
- 系统 SHALL 在数据加载完成前先渲染 UI 骨架屏
- 系统 SHALL 实现启动阶段度量：T1（窗口出现）、T2（数据就绪）、T3（完全可交互）
- 系统 SHALL 将非关键初始化工作延迟到首屏渲染后（如历史记录索引）
- 系统 SHALL 在 Vite 构建中启用代码分割（React.lazy + Suspense）

#### FR-017-02: 内存优化
- 系统 SHALL 限制下载缓冲区大小（通过 yt-dlp `--buffer-size` 参数，默认 16MB）
- 系统 SHALL 在每次调度周期完成后显式释放中间数据（drop 或作用域管理）
- 系统 SHALL 监控内存使用（通过 `sysinfo` crate），超过阈值时报警
- 系统 SHALL 对大型 Vec/String 使用 `shrink_to_fit()` 释放过剩容量
- 系统 SHALL 在 Tauri IPC 中避免大 JSON 字符串复制，使用零拷贝或引用传递

#### FR-017-03: 数据库优化
- 系统 SHALL 使用原子写入策略：`temp_named_file → fs::rename → 更新缓存`
- 系统 SHALL 在 StorageService 中维护已加载数据的 `Arc<RwLock<>>` 缓存
- 系统 SHALL 实现增量写入：仅追加新记录而非重写整个文件（download_records 除外）
- 系统 SHALL 在应用退出时自动压缩 JSON 文件（去掉多余空白）
- 系统 SHALL 将大型 JSON 数组拆分为分段文件（每 1000 条记录一个文件片段）

#### FR-017-04: 缓存策略
- 系统 SHALL 实现 TTL 缓存：settings 缓存 30 秒，subscriptions 缓存 10 秒，records 缓存 5 秒
- 系统 SHALL 在写操作后自动失效对应缓存
- 系统 SHALL 前端使用 `useMemo` 和 `useCallback` 避免不必要的重渲染
- 系统 SHALL 缩略图文件缓存使用 LRU 策略（最多 200 个缩略图文件）
- 系统 SHALL 在前端使用 React.memo 对列表项组件包裹

#### FR-017-05: 懒加载
- 系统 SHALL 在 SubscriptionList 中实现虚拟滚动（使用 MUI 或 react-window）
- 系统 SHALL 在 DownloadRecordList 中实现分页/无限滚动加载
- 系统 SHALL 对弹窗组件使用 React.lazy + Suspense 懒加载
- 系统 SHALL 对图表组件（ECharts）仅在对应面板可见时才初始化
- 系统 SHALL 对图片缩略图使用 Intersection Observer 实现延迟加载

### Key Entities

- **StartupMetrics**: 启动指标，包含 window_visible_ms, data_ready_ms, fully_interactive_ms
- **MemorySnapshot**: 内存快照，包含 resident_set_bytes, virtual_memory_bytes, heap_used_bytes
- **CacheEntry<T>**: 泛型缓存条目，包含 data: T, created_at: Instant, ttl: Duration
- **DataSegment**: 数据分段，包含 segment_index, start_id, end_id, records

## Success Criteria *(mandatory)*

- SC-017-01: 冷启动时间：窗口出现 < 1s，数据就绪 < 2s，完全可交互 < 3s（200 订阅 + 10000 记录）
- SC-017-02: 24 小时运行后内存增长 < 10MB（无泄漏），最大内存 < 200MB
- SC-017-03: JSON 写入耗时 < 100ms（10000 条记录场景），原子写入零数据丢失
- SC-017-04: 缓存命中率 > 90%（正常使用场景），缓存失效响应 < 50ms
- SC-017-05: 500 条订阅列表滚动帧率 ≥ 60fps，5000 条下载记录分页加载延迟 < 200ms

## Assumptions

- JSON 分段存储策略需要确保兼容现有数据结构，平滑迁移
- 缓存策略仅在内存中维护，应用重启后重建缓存
- 性能优化不显著改变现有 API 接口（内部实现优化，外部行为不变）
- 代码分割的 chunk 数量控制在 10 以内，避免过多 HTTP 请求
- 内存监控数据仅用于开发调试和告警，不参与业务逻辑
