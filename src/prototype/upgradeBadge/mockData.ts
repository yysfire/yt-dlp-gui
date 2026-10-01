/**
 * 原型用的假数据。每一行的 `note` 说明「这条在演示什么」，
 * 状态组合直接取自 ticket #4 的 16 行真值表。
 *
 * 注意：`upgradeable` / `missing` 在前端由 buildUnifiedVideoList 派生；
 * 这里为了只考察「长什么样」，直接写成显式字段。
 */
export type ProtoStatus =
  | "new"
  | "downloading"
  | "completed"
  | "failed"
  | "cancelled"
  | "deleted"
  | "retrying";

export interface ProtoRow {
  id: string;
  title: string;
  url: string;
  status: ProtoStatus;
  /** 上传日期 · 时长 */
  meta: string;
  /** 右侧辅助信息 */
  rightMeta?: string;
  /** 下载中进度 */
  percent?: number;
  /** 下载记录里的 quality（空串 = 未知 / 旧记录） */
  quality?: string;
  /** 订阅当前 preset（升级目标） */
  target?: string;
  upgradeable?: boolean;
  missing?: boolean;
  errorMessage?: string;
  retryCount?: number;
  retryInSec?: number;
  /** 这条在演示什么 */
  note: string;
}

export const MOCK_ROWS: ProtoRow[] = [
  {
    id: "r1",
    title: "深入理解 Tauri v2 的事件系统与状态管理",
    url: "https://www.youtube.com/watch?v=aaaaaaaaaaa",
    status: "completed",
    meta: "2026-09-28 · 12:34",
    rightMeta: "412 MB · 9月28日",
    quality: "720p",
    target: "1080p",
    upgradeable: true,
    note: "已完成 720p，订阅已升到 1080p → 可升级",
  },
  {
    id: "r2",
    title: "Rust 异步运行时内部原理（长篇）",
    url: "https://www.youtube.com/watch?v=bbbbbbbbbbb",
    status: "completed",
    meta: "2026-09-27 · 48:02",
    rightMeta: "1.9 GB · 9月27日",
    quality: "1080p",
    target: "best",
    upgradeable: true,
    note: "订阅切到「最高画质」→ 可升级",
  },
  {
    id: "r3",
    title: "用 React 18 重构一个真实项目",
    url: "https://www.youtube.com/watch?v=ccccccccccc",
    status: "completed",
    meta: "2026-09-26 · 21:10",
    rightMeta: "780 MB · 9月26日",
    quality: "1080p",
    target: "1080p",
    note: "同档 → 不提示",
  },
  {
    id: "r4",
    title: "十分钟看懂 WebGPU",
    url: "https://www.youtube.com/watch?v=ddddddddddd",
    status: "completed",
    meta: "2026-09-25 · 10:02",
    rightMeta: "210 MB · 9月25日",
    quality: "1080p",
    target: "720p",
    note: "订阅档次更低（降级）→ 不提示",
  },
  {
    id: "r5",
    title: "（旧记录）三年前的老视频",
    url: "https://www.youtube.com/watch?v=eeeeeeeeeee",
    status: "completed",
    meta: "2026-09-24 · 05:31",
    rightMeta: "96 MB · 9月24日",
    quality: "",
    target: "1080p",
    note: "旧记录无 quality → 永不判升级",
  },
  {
    id: "r6",
    title: "文件被我手动删掉了的那个视频",
    url: "https://www.youtube.com/watch?v=fffffffffff",
    status: "completed",
    meta: "2026-09-23 · 33:00",
    quality: "720p",
    target: "1080p",
    missing: true,
    note: "文件缺失 → 动作是「重新下载」，优先于升级",
  },
  {
    id: "r7",
    title: "用户显式删除过的视频",
    url: "https://www.youtube.com/watch?v=ggggggggggg",
    status: "deleted",
    meta: "2026-09-22 · 07:14",
    quality: "1080p",
    target: "1080p",
    missing: true,
    note: "已删除 → 可重下",
  },
  {
    id: "r8",
    title: "网络抖动、正在自动重试的下载",
    url: "https://www.youtube.com/watch?v=hhhhhhhhhhh",
    status: "retrying",
    meta: "2026-10-01 · 15:20",
    quality: "1080p",
    target: "1080p",
    retryCount: 2,
    retryInSec: 12,
    note: "重试中 (2/3)，12 秒后第 3 次",
  },
  {
    id: "r9",
    title: "正在下载：进度条基线",
    url: "https://www.youtube.com/watch?v=iiiiiiiiiii",
    status: "downloading",
    meta: "2026-10-01 · 44:18",
    percent: 43,
    note: "基线：既有进度条不应被新徽标挤压",
  },
  {
    id: "r10",
    title: "下载失败、等用户处置",
    url: "https://www.youtube.com/watch?v=jjjjjjjjjjj",
    status: "failed",
    meta: "2026-09-30 · 02:00",
    errorMessage: "Request timed out",
    note: "失败 → 现有重试入口",
  },
  {
    id: "r11",
    title: "还没下载过的新视频",
    url: "https://www.youtube.com/watch?v=kkkkkkkkkkk",
    status: "new",
    meta: "2026-09-30 · 16:40",
    note: "新视频 → 无任何标记",
  },
];
