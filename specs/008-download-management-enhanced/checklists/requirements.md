# Quality Checklist: 下载管理增强

**Feature**: 008-download-management-enhanced
**Phase**: P1 Enhancement
**Last Updated**: 2026-10-02
**范围**: 仅**用户故事 1（智能重复检测增强）+ 用户故事 2（失败自动重试）**。故事 3 / 4 / 5 已划出本次范围（见 spec 的「范围界定」与「超出范围」）。
**依据**: 本清单的每一条都能追溯到 wayfinder 地图（[008 下载管理增强（P1：智能重复检测 + 失败自动重试）](https://github.com/yysfire/yt-dlp-gui/issues/2)）上已关闭的决策 ticket。**实现时逐项勾选**（参照 `specs/007-unified-video-list/checklists/` 的用法）。

## 需求完整性检查

- [ ] 用户故事 1 / 2 的每个验收场景都有可执行的 Given/When/Then，且与 spec 的判定规则不冲突
- [ ] 画质档次全序（`480p < 720p < 1080p < 1440p < 2160p < best`）与判定规则已写明，含「**未知值不参与比较**」「**降级不提示**」
- [ ] 重试判定的**有序**清单已写明：只取最后一条 `ERROR:` 行 → 终态黑名单 → HTTP 白名单（5xx / 408 / 429）→ 网络白名单 → 未知即重试
- [ ] 「**文件缺失优先于可升级**」与「**已在其它订阅下载**」两条 UI 口径已写明
- [ ] 已划出范围的故事（3 / 4 / 5）在 spec 中有明确横幅，且原文保留未删
- [ ] spec 中的字段名与记录契约一致（`quality` / `retry_count` / `last_retry_at`）

## 技术可行性检查

- [ ] `yt-dlp --flat-playlist --dump-json` 的输出中**稳定包含视频 ID** —— 去重与画质判定都建立在此之上
- [ ] 输出文件名模板为 `%(title)s.%(ext)s`、**不含订阅或画质判别符** —— 与「全局去重、同一视频只有一份文件」一致
- [ ] 已核实「失败原因只出现在 stderr 的 `ERROR:` 行」且「退出码无错误类型语义（0 / 1 / 2 / 101）」
- [ ] `DownloadRecord` 新增字段全部 `#[serde(default)]`，旧 `download_records.json` 能正常加载（`quality` 为空表示未知）
- [ ] `deduplicate_vec` 的 rank 表已含 `retrying`（插在 `downloading` 之后）
- [ ] 退避期间 `ActiveTask` 的 `pid` 为 `None`，且所有发信号处都在 `Some(pid)` 分支内

## 用户体验检查

- [ ] 「可升级」徽标带目标档次（如「可升级 1080p」）；`best` 渲染为「最高画质」
- [ ] 「文件缺失」与「已删除」**共用**同一个「重新下载」入口
- [ ] 重试行显示「重试中 (n/3)」与倒计时，且**倒数只在详情面板**（列表视图不加）
- [ ] 「已在其它订阅下载」的行**不提供**重下、**不提供**升级
- [ ] 升级 / 重下走**行内按钮**，与既有 暂停 / 取消 / 重试 同位（不引入悬浮菜单）
- [ ] 「已下载」视图（`DownloadedList`）复用同一套文案与同一命令

## 兼容性与边界检查

- [ ] 旧记录（`quality` 为空）**永不**触发「可升级」
- [ ] 「修改下载目录」后的行为是「显示为文件缺失、需手动重新下载」，且已在文档中告知用户
- [ ] 「删除文件腾空间」与「移动下载目录」都**不会**触发自动重下（检查路径不参与存在性判定）
- [ ] `yt-dlp` 无法启动（路径 / 权限）时**不重试**，且写入明确的失败原因（不再永久卡在「下载中」）
- [ ] 取消能**立即**中断退避，且不会把 `cancelled` 覆盖成 `failed`
- [ ] 暂停订阅**不**中断已入队下载的重试（只让检查跳过该订阅）
- [ ] 退避期间**保持**占用并发名额；`update_max_concurrent` 缩容时排除退避中的任务

## 安全性检查

- [ ] `error_message` **落库前剥离代理凭证**（`user:pass@`）—— 它现在来自真实 stderr，而 `proxy_url` 可能含明文口令，不得写入 `download_records.json` 或显示在 UI
- [ ] `error_message` 截断到 200 字符（与既有 `unparsable_line_error` 的截断口径一致）
- [ ] 本次不新增任何凭证落盘（无认证类改动）

## 测试覆盖检查（自动化）

- [ ] `unifiedVideoList` 的**真值表逐行单测**：可升级 / 同档 / 降级 / 画质未知 / 文件缺失（优先于升级）/ 已删除 / 失败 / 取消 / 下载中 / 重试中 / 无记录
- [ ] 分类器对**调研 §3.1 的实测 stderr 样本**给出期望结论：HTTP 404（generic 与 BiliBili 两条）、DNS 解析失败、代理 407、SOCKS5 不可达、格式不可用
- [ ] 「**只取最后一条 `ERROR:` 行**」的假阳性回归（使用那组同时含 WARNING 假阳性 403 与最终 ERROR 的样本）
- [ ] Rust ↔ TS **契约 fixture 测试**：Rust 侧断言 `to_value` 与 fixture 一致，前端侧 `satisfies DownloadRecord`
- [ ] `deduplicate_vec` 新 rank 表的单测
- [ ] 退避剩余时长与相位判断的**纯函数**单测（`remaining_after(phase, elapsed)` / `phase_of(child, status)` / `should_retry` 组合）

## 人工验收（前提 P11 的边界 —— 只覆盖骨架接线）

> 前提：只抽纯函数、不引入可注入的 runner。因此**循环骨架的接线**没有自动化测试，必须人工过一遍。

- [ ] 制造一次可重试失败（如抓取阶段 HTTP 503）：确认 30 秒后自动重试、最终成功，记录 `completed` 且 `retry_count == 1`
- [ ] 退避等待期间点「暂停」：确认计时冻结；「恢复」后从**剩余时间**继续
- [ ] 退避等待期间点「取消」：确认**立即**中断，记录为 `cancelled`，**没有**被改写成 `failed`
- [ ] 触发一次**不可重试**的失败（如所选格式不存在）：确认**没有**进入退避，直接判 `failed`
- [ ] 把 yt-dlp 路径改成不存在的值：确认记录变为 `failed` 且写明「无法启动」

## 依赖与集成检查

- [ ] 与 `services/ytdlp.rs` 的命令构建兼容（本次不改下载参数）
- [ ] 新命令 `redownload_video` 已在 `lib.rs` 的 `generate_handler!` 注册，且前端封装与 `invoke` 名一致
- [ ] 凡写 `download_records.json` 的路径都调用了 `notify_records_changed`；队列改动 emit `queue-changed`
- [ ] `recover_state` 已把 `retrying` 一并置为 `failed`
- [ ] 既有 `buildUnifiedVideoList` 测试不回归（新增字段是**正交**的，不改变原有排序与合并）
