import type {
  VideoInfo,
  DownloadRecord,
  DownloadTask,
  UnifiedVideoItem,
  UnifiedVideoStatus,
} from "@/types";

/**
 * 统一视频列表的合并逻辑与展示格式化。
 *
 * 这三路数据（频道视频 / 持久化下载记录 / 内存队列任务）各自只描述同一个视频的一个侧面，
 * 详情面板需要把它们合成一行。合并规则（键、状态优先级、标题来源、排序）此前内联在
 * `DetailPanel.tsx` 里，是该组件最容易出 bug 的部分，因此抽成纯函数以便在接口上验证。
 *
 * 本模块是**纯函数**：无 I/O、无状态、不修改入参。
 */

/** 从 yt-dlp epoch 字段或 upload_date (YYYYMMDD) 转为 YYYY-MM-DD 字符串 */
export function formatUploadDate(
  uploadDate: string | null | undefined,
  epoch: number | null | undefined,
): string {
  if (uploadDate && /^\d{8}$/.test(uploadDate)) {
    return `${uploadDate.slice(0, 4)}-${uploadDate.slice(4, 6)}-${uploadDate.slice(6, 8)}`;
  }
  if (epoch != null && epoch > 0) {
    const d = new Date(epoch * 1000);
    return d.toISOString().slice(0, 10);
  }
  return "";
}

/** 将秒数格式化为人类可读的时长字符串（如 "12:34", "1:01:01"） */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  const totalSeconds = Math.round(seconds);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** 格式化文件大小（0 返回空串，由调用方决定占位符） */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/** 格式化 ISO 8601 时间戳为本地化字符串 */
export function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

/** 画质档次全序（数字越大越高）；未知档位返回 `null`，**永不参与比较**。 */
const QUALITY_RANK: Record<string, number> = {
  "480p": 1,
  "720p": 2,
  "1080p": 3,
  "1440p": 4,
  "2160p": 5,
  best: 6,
};

/**
 * 画质档次的比较键。
 *
 * 未知值（空串 / `audio` / 其它串）返回 `null` —— 方向是「宁可漏报，不可对看不懂的
 * 值天天提示」。
 */
export function qualityRank(quality: string | null | undefined): number | null {
  if (!quality) return null;
  return QUALITY_RANK[quality] ?? null;
}

/** 按状态排序优先级（越小越靠前） */
export function statusPriority(status: UnifiedVideoStatus): number {
  switch (status) {
    case "downloading":
      return 0;
    case "retrying":
      return 1;
    case "waiting":
      return 2;
    case "paused":
      return 3;
    case "completed":
      return 4;
    case "new":
      return 5;
    case "cancelled":
      return 6;
    case "failed":
      return 7;
    default:
      return 9;
  }
}

/** 某个视频在三个来源里的原始数据，以及在阶段一里按原顺序演化出的展示字段 */
type Sources = {
  record?: DownloadRecord;
  task?: DownloadTask;
  channel?: VideoInfo;
  /** 标题：按「记录 → 队列 → 频道」的顺序演化，规则见下方各轮注释 */
  title: string;
  /** 链接：只在条目创建时确定，之后不再变化 */
  url: string;
  /** 唯一标识：默认取合并键，频道视频带 id 时改用该 id */
  id: string;
};

/** 合并键：优先 video_id，回退 url */
function makeKey(id: string, url: string): string {
  return id || url;
}

/**
 * 派生用于展示的状态：队列任务最权威（它反映此刻正在发生什么），
 * 其次回退到持久化记录，两者都没有就是尚未下载过的新视频。
 */
function deriveStatus(sources: Sources): UnifiedVideoStatus {
  const task = sources.task;
  if (task) {
    if (task.status === "running") return "downloading";
    if (task.status === "retrying") return "retrying";
    if (task.status === "paused") return "paused";
    if (task.status === "waiting") return "waiting";
    if (task.status === "cancelled") return "cancelled";
    if (task.status === "failed") return "failed";
    if (task.status === "completed") return "completed";
  }
  if (sources.record) {
    // DownloadRecord.status 含 "deleted"（用户删除了已下载的文件）。该值已纳入
    // UnifiedVideoStatus 并在此原样透传；DetailPanel 对 "deleted" 有独立的展示分支
    // （图标 + 删除线 + 「已删除」文字，无操作按钮）。
    return sources.record.status;
  }
  return "new";
}

/**
 * 派生三个**正交**标记（规则按序执行，见 decisions.md §2.4 / data-model §7.1）。
 *
 * 1. 无本订阅记录 → 都 false（`downloadedElsewhere` 单独判：记录属于别的订阅）；
 * 2. 有活跃队列任务 → 都 false；
 * 3. 记录状态非 `completed` → `missing` / `upgradeable` 为 false；
 * 4. `missing == true` → `upgradeable = false`（**缺失优先于升级**）；
 * 5. 记录画质或订阅 preset 未知 → `upgradeable = false`；
 * 6. `rank(订阅) > rank(记录)` → `upgradeable = true`（同档与降级都不提示）。
 */
function deriveFlags(
  sources: Sources,
  subscriptionId: string | undefined,
  missingPaths: Set<string> | undefined,
  qualityPreset: string | undefined,
): Pick<UnifiedVideoItem, "upgradeable" | "missing" | "downloadedElsewhere"> {
  const record = sources.record;

  // 「已在其它订阅下载」：有记录，但归属不是本订阅。此时不参与本订阅的升级/缺失判定。
  const downloadedElsewhere =
    record != null && subscriptionId != null && record.subscription_id !== subscriptionId;

  // 有活跃队列任务时一切标记关闭（任务本身就是最新事实）
  if (sources.task != null) {
    return { upgradeable: false, missing: false, downloadedElsewhere };
  }

  const isOurs =
    record != null && (subscriptionId == null || record.subscription_id === subscriptionId);
  if (!isOurs || record.status !== "completed") {
    return { upgradeable: false, missing: false, downloadedElsewhere };
  }

  // 文件缺失：纯派生，不落库。只对本订阅的 completed 记录判定。
  const missing =
    record.file_path !== "" && (missingPaths?.has(record.file_path) ?? false);

  if (missing) {
    return { upgradeable: false, missing: true, downloadedElsewhere };
  }

  const recordRank = qualityRank(record.quality);
  const presetRank = qualityRank(qualityPreset);
  const upgradeable =
    recordRank != null && presetRank != null && presetRank > recordRank;

  return { upgradeable, missing: false, downloadedElsewhere };
}

/**
 * 排序用的时间戳：下载完成时间 > 入队时间 > 视频上传日期。
 *
 * 注意 `downloaded_at` 一旦为真值就直接采用（即使解析出来是 NaN 也不回退），
 * 这是既有行为，改成更健壮的解析会改变并列项的相对顺序。
 */
function itemTime(sources: Sources): number {
  if (sources.record?.downloaded_at) {
    return new Date(sources.record.downloaded_at).getTime();
  }
  if (sources.task?.created_at) {
    return new Date(sources.task.created_at).getTime();
  }
  if (sources.channel?.upload_date) {
    const d = sources.channel.upload_date;
    return new Date(
      `${d.slice(0, 4)}-${d.slice(4, 6)}-${d.slice(6, 8)}`,
    ).getTime();
  }
  return 0;
}

/**
 * 合并频道视频、下载记录与队列任务为统一列表。
 *
 * 输出按状态优先级排序（下载中 → 重试中 → 等待 → 暂停 → 已完成 → 新视频 → 已取消 → 失败），
 * 同状态内按时间倒序；时间并列时保持插入顺序（`records → tasks → videos`）。
 *
 * `records` 传入的是**全局去重后**的记录（可能包含其它订阅的归属）：
 * - `subscriptionId` 用于区分「本订阅的记录」与「已在其它订阅下载」；
 * - 不传 `subscriptionId` 时不做归属判定（`downloadedElsewhere` 恒 false），
 *   以兼容只关心合并/排序的调用方。
 */
export function buildUnifiedVideoList({
  videos,
  records,
  tasks,
  qualityPreset,
  missingPaths,
  subscriptionId,
}: {
  videos: VideoInfo[];
  records: DownloadRecord[];
  tasks: DownloadTask[];
  /** 当前订阅的画质 preset；未知（含 undefined / 空串）永不判「可升级」 */
  qualityPreset?: string;
  /** 磁盘上已缺失的文件路径集合（来自 `file-sync-complete`） */
  missingPaths?: Set<string>;
  /** 当前订阅 id；用于判定「已在其它订阅下载」 */
  subscriptionId?: string;
}): UnifiedVideoItem[] {
  // 阶段一：按合并键索引三路来源，并顺带演化展示字段。
  //
  // 遍历顺序固定为 records → tasks → videos，绝不能改成「按参数对象的属性顺序」遍历：
  // 它决定 Map 的插入顺序（排序在时间并列时依赖稳定排序保持该顺序），
  // 也决定标题最终取哪一路。
  const sources = new Map<string, Sources>();

  // 记录：同键首条胜出，且后续记录完全不参与展示字段的演化
  for (const record of records) {
    const key = makeKey(record.video_id, record.video_url);
    const existing = sources.get(key);
    if (existing) {
      if (!existing.record) existing.record = record;
    } else {
      sources.set(key, {
        record,
        title: record.video_title,
        url: record.video_url,
        id: key,
      });
    }
  }

  // 队列任务：同键末条胜出（queueTask 本身无条件覆盖），
  // 但标题只在 video_title 非空时才覆盖 —— 空标题不会抹掉记录或更早任务的标题
  for (const task of tasks) {
    const key = makeKey(task.video_id, task.video_url);
    const existing = sources.get(key);
    if (existing) {
      existing.task = task;
      if (task.video_title) existing.title = task.video_title;
    } else {
      sources.set(key, {
        task,
        title: task.video_title,
        url: task.video_url,
        id: key,
      });
    }
  }

  // 频道视频：同键末条胜出，标题无条件覆盖（频道来源的标题最完整）
  for (const video of videos) {
    const key = makeKey(video.id, video.url);
    const existing = sources.get(key);
    if (existing) {
      existing.channel = video;
      existing.title = video.title;
      if (video.id) existing.id = video.id;
    } else {
      sources.set(key, {
        channel: video,
        title: video.title,
        url: video.url,
        id: key,
      });
    }
  }

  // 阶段二：一次性产出条目并排序。状态与三个标记在这里算一次即可 ——
  // 它们只取决于队列任务与下载记录，与阶段一的轮次无关。
  const indexed = Array.from(sources, ([, src]) => {
    const flags = deriveFlags(src, subscriptionId, missingPaths, qualityPreset);
    return {
      time: itemTime(src),
      item: {
        channelInfo: src.channel,
        downloadInfo: src.record,
        queueTask: src.task,
        id: src.id,
        title: src.title,
        url: src.url,
        status: deriveStatus(src),
        ...flags,
      } satisfies UnifiedVideoItem,
    };
  });

  indexed.sort((a, b) => {
    const pa = statusPriority(a.item.status);
    const pb = statusPriority(b.item.status);
    if (pa !== pb) return pa - pb;
    return b.time - a.time;
  });

  return indexed.map((entry) => entry.item);
}
