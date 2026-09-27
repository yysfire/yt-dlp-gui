import { describe, it, expect } from "vitest";
import {
  buildUnifiedVideoList,
  statusPriority,
  formatTime,
  formatDuration,
  formatFileSize,
  formatUploadDate,
} from "../unifiedVideoList";
import type { VideoStatus, VideoInfo, DownloadRecord, DownloadTask } from "@/types";

/**
 * 统一视频列表合并逻辑的接口级测试。
 *
 * 这些用例锁定的是**产品规则**（同一个视频只出现一行、哪个视频排在前面、
 * 显示什么状态与标题），而不是实现细节 —— 规则变了它们就该红。
 */

function makeVideo(overrides: Partial<VideoInfo> = {}): VideoInfo {
  return {
    id: "vid-1",
    title: "Video 1",
    url: "https://example.com/watch?v=vid-1",
    duration: 600,
    upload_date: null,
    epoch: null,
    thumbnail: null,
    ...overrides,
  };
}

function makeRecord(overrides: Partial<DownloadRecord> = {}): DownloadRecord {
  return {
    id: "rec-1",
    subscription_id: "sub-1",
    video_id: "vid-1",
    video_title: "Video 1",
    video_url: "https://example.com/watch?v=vid-1",
    file_path: "/tmp/vid-1.mp4",
    file_size: 1000,
    status: "completed",
    error_message: null,
    downloaded_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function makeTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: "task-1",
    video_id: "vid-1",
    video_url: "https://example.com/watch?v=vid-1",
    video_title: "Video 1",
    subscription_id: "sub-1",
    quality: "1080p",
    status: "waiting",
    progress: null,
    error_message: null,
    created_at: "2026-01-01T00:00:00Z",
    completed_at: null,
    ...overrides,
  };
}

function build(
  videos: VideoInfo[] = [],
  records: DownloadRecord[] = [],
  tasks: DownloadTask[] = [],
) {
  return buildUnifiedVideoList({ videos, records, tasks });
}

describe("buildUnifiedVideoList - 三路合并", () => {
  it("同一个视频在三个来源各出现一次时合并为单条，且保留三者的引用", () => {
    const video = makeVideo();
    const record = makeRecord();
    const task = makeTask();

    const items = build([video], [record], [task]);

    // 每个视频只应占一行，否则详情面板会重复显示同一个视频
    expect(items).toHaveLength(1);
    expect(items[0].channelInfo).toBe(video);
    expect(items[0].downloadInfo).toBe(record);
    expect(items[0].queueTask).toBe(task);
  });

  it("video_id 为空时回退到 url 匹配", () => {
    const url = "https://example.com/watch?v=same";
    // flat-playlist 有时拿不到 video_id，此时 url 是唯一的合并依据
    const items = build(
      [makeVideo({ id: "", url })],
      [makeRecord({ video_id: "", video_url: url })],
    );

    expect(items).toHaveLength(1);
  });

  it("key 不同的视频不会被合并（防止把两个视频并成一个）", () => {
    const items = build(
      [makeVideo({ id: "a", url: "https://example.com/a" })],
      [makeRecord({ video_id: "b", video_url: "https://example.com/b" })],
    );

    expect(items).toHaveLength(2);
  });

  it("同一 key 的多条下载记录只保留第一条", () => {
    const first = makeRecord({ id: "rec-1", video_title: "First", status: "completed" });
    const second = makeRecord({ id: "rec-2", video_title: "Second", status: "failed" });

    const items = build([], [first, second]);

    // 取第一条：重复记录是历史遗留，后者不应悄悄改写标题与状态
    expect(items).toHaveLength(1);
    expect(items[0].downloadInfo).toBe(first);
    expect(items[0].title).toBe("First");
    expect(items[0].status).toBe("completed");
  });

  it("同一 key 的多条队列任务保留最后一条", () => {
    const first = makeTask({ id: "task-1", video_title: "First", status: "waiting" });
    const second = makeTask({ id: "task-2", video_title: "Second", status: "running" });

    const items = build([], [], [first, second]);

    expect(items).toHaveLength(1);
    expect(items[0].queueTask).toBe(second);
    expect(items[0].title).toBe("Second");
  });

  it("同一 key 的多条频道视频保留最后一条", () => {
    const first = makeVideo({ title: "First" });
    const second = makeVideo({ title: "Second" });

    const items = build([first, second]);

    expect(items).toHaveLength(1);
    expect(items[0].channelInfo).toBe(second);
    expect(items[0].title).toBe("Second");
  });
});

describe("buildUnifiedVideoList - 状态派生", () => {
  it("队列任务的状态覆盖下载记录的状态", () => {
    // 记录说已完成、队列说正在跑：用户此刻看到的应该是「下载中」
    const items = build(
      [],
      [makeRecord({ status: "completed" })],
      [makeTask({ status: "running" })],
    );

    expect(items[0].status).toBe("downloading");
  });

  it("队列的 running 映射为展示用的 downloading（两套词表）", () => {
    const items = build([], [], [makeTask({ status: "running" })]);

    expect(items[0].status).toBe("downloading");
    expect(items[0].status).not.toBe("running");
  });

  it.each([
    ["waiting", "waiting"],
    ["paused", "paused"],
    ["completed", "completed"],
    ["failed", "failed"],
    ["cancelled", "cancelled"],
  ] as const)("队列状态 %s 原样映射为 %s", (taskStatus, expected) => {
    const items = build([], [], [makeTask({ status: taskStatus })]);

    expect(items[0].status).toBe(expected);
  });

  it("没有队列任务时使用下载记录的状态", () => {
    const items = build([], [makeRecord({ status: "paused" })]);

    expect(items[0].status).toBe("paused");
  });

  it("既无队列也无记录时是新视频", () => {
    const items = build([makeVideo()]);

    expect(items[0].status).toBe("new");
  });

  it("只有记录且状态为 completed 时直接采用记录状态", () => {
    const items = build([], [makeRecord({ status: "completed" })]);

    expect(items[0].status).toBe("completed");
  });

  it("已删除（deleted）的记录原样透传 —— 锁定当前行为，UI 对它没有分支", () => {
    // DownloadRecord.status 可能是 "deleted"（用户删了文件），而 VideoStatus 不含它。
    // 这是既有的类型不安全点，此处显式锁定其运行时输出，防止后续无声漂移。
    const items = build([], [makeRecord({ status: "deleted" })]);

    expect(items[0].status).toBe("deleted");
    // 排序优先级落到 default 分支，排在最后
    expect(statusPriority(items[0].status)).toBe(9);
  });
});

describe("buildUnifiedVideoList - 标题/标识/链接的优先级", () => {
  it("多条队列任务时，队列任务取末条但标题取最后一个非空值", () => {
    // 队列里同一个视频可能先后出现多条任务，后一条的标题可能还没拿到（空串）。
    // 此时 queueTask 字段应更新为最新任务，但标题不该被空串抹掉。
    const withTitle = makeTask({ id: "task-1", video_title: "Known Title" });
    const emptyTitle = makeTask({ id: "task-2", video_title: "" });

    const items = build([], [], [withTitle, emptyTitle]);

    expect(items).toHaveLength(1);
    expect(items[0].queueTask).toBe(emptyTitle);
    expect(items[0].title).toBe("Known Title");
  });

  it("频道视频的标题无条件覆盖队列与记录的标题", () => {
    const items = build(
      [makeVideo({ title: "Channel Title" })],
      [makeRecord({ video_title: "Record Title" })],
      [makeTask({ video_title: "Task Title" })],
    );

    expect(items[0].title).toBe("Channel Title");
  });

  it("没有频道视频时，队列任务的标题覆盖记录的标题", () => {
    const items = build(
      [],
      [makeRecord({ video_title: "Record Title" })],
      [makeTask({ video_title: "Task Title" })],
    );

    expect(items[0].title).toBe("Task Title");
  });

  it("队列任务标题为空串时回退到记录标题", () => {
    const items = build(
      [],
      [makeRecord({ video_title: "Record Title" })],
      [makeTask({ video_title: "" })],
    );

    expect(items[0].title).toBe("Record Title");
  });

  it("只有队列任务且标题为空串时标题就是空串", () => {
    const items = build([], [], [makeTask({ video_title: "" })]);

    expect(items[0].title).toBe("");
  });

  it("频道视频标题为空串时仍覆盖（无条件覆盖的边界）", () => {
    // 反直觉但确实是既有行为：频道来源的标题不做真值判断
    const items = build(
      [makeVideo({ title: "" })],
      [makeRecord({ video_title: "Record Title" })],
    );

    expect(items[0].title).toBe("");
  });

  it("url 取最早出现的来源（记录优先于队列）", () => {
    const items = build(
      [makeVideo({ url: "https://example.com/v" })],
      [makeRecord({ video_url: "https://example.com/r" })],
      [makeTask({ video_url: "https://example.com/t" })],
    );

    expect(items[0].url).toBe("https://example.com/r");
  });

  it("没有记录时 url 取队列任务", () => {
    const items = build(
      [makeVideo({ url: "https://example.com/v" })],
      [],
      [makeTask({ video_url: "https://example.com/t" })],
    );

    expect(items[0].url).toBe("https://example.com/t");
  });

  it("只有频道视频时 url 取频道视频的链接", () => {
    const items = build([makeVideo({ url: "https://example.com/v" })]);

    expect(items[0].url).toBe("https://example.com/v");
  });

  it("记录的 url 为空串时不会回退到队列任务（空串是有效值）", () => {
    const items = build(
      [],
      [makeRecord({ video_url: "" })],
      [makeTask({ video_url: "https://example.com/t" })],
    );

    expect(items[0].url).toBe("");
  });

  it("记录没有 video_id 时以 url 作为标识", () => {
    const items = build([], [makeRecord({ video_id: "", video_url: "https://example.com/r" })]);

    expect(items[0].id).toBe("https://example.com/r");
  });

  it("频道视频 id 与记录 video_id 相同时标识即该 id", () => {
    const items = build(
      [makeVideo({ id: "shared-id" })],
      [makeRecord({ video_id: "shared-id" })],
    );

    expect(items).toHaveLength(1);
    expect(items[0].id).toBe("shared-id");
  });

  it("频道视频 id 为空串时不会被写成空标识", () => {
    // 若把 id 直接取成 channel.id，这里会退化为空串，React 的 key 也就失去了意义
    const url = "https://example.com/same";
    const items = build(
      [makeVideo({ id: "", url })],
      [makeRecord({ video_id: "", video_url: url })],
    );

    expect(items).toHaveLength(1);
    expect(items[0].id).toBe(url);
  });
});

describe("buildUnifiedVideoList - 排序", () => {
  it("按状态优先级排序：下载中 → 等待 → 暂停 → 已完成 → 新视频 → 已取消 → 失败", () => {
    const items = build(
      [
        makeVideo({ id: "new" }),
        makeVideo({ id: "failed" }),
        makeVideo({ id: "completed" }),
        makeVideo({ id: "cancelled" }),
        makeVideo({ id: "downloading" }),
        makeVideo({ id: "waiting" }),
        makeVideo({ id: "paused" }),
      ],
      [
        makeRecord({ video_id: "completed", status: "completed" }),
        makeRecord({ video_id: "failed", status: "failed" }),
        makeRecord({ video_id: "cancelled", status: "cancelled" }),
        makeRecord({ video_id: "paused", status: "paused" }),
      ],
      [
        makeTask({ video_id: "downloading", status: "running" }),
        makeTask({ video_id: "waiting", status: "waiting" }),
      ],
    );

    expect(items.map((i) => i.status)).toEqual([
      "downloading",
      "waiting",
      "paused",
      "completed",
      "new",
      "cancelled",
      "failed",
    ]);
  });

  it("同一状态内按时间倒序（新的在前）", () => {
    const older = makeRecord({
      video_id: "older",
      video_url: "https://example.com/older",
      downloaded_at: "2026-01-01T00:00:00Z",
    });
    const newer = makeRecord({
      video_id: "newer",
      video_url: "https://example.com/newer",
      downloaded_at: "2026-06-01T00:00:00Z",
    });

    const items = build([], [older, newer]);

    expect(items.map((i) => i.id)).toEqual(["newer", "older"]);
  });

  it("有时间来源时优先用下载完成时间，而不是入队时间", () => {
    // A 的下载完成时间是 2026-01、入队时间是 2026-12（更晚）；B 只有入队时间 2026-06。
    // 两者状态相同（都是 downloading）故可比时间。若实现误用入队时间，
    // A 会被排在 B 前面 —— 顺序反转即暴露。
    const aRecord = makeRecord({
      video_id: "a",
      video_url: "https://example.com/a",
      downloaded_at: "2026-01-01T00:00:00Z",
    });
    const aTask = makeTask({
      video_id: "a",
      video_url: "https://example.com/a",
      status: "running",
      created_at: "2026-12-01T00:00:00Z",
    });
    const bTask = makeTask({
      video_id: "b",
      video_url: "https://example.com/b",
      status: "running",
      created_at: "2026-06-01T00:00:00Z",
    });

    const items = build([], [aRecord], [aTask, bTask]);

    expect(items.map((i) => i.id)).toEqual(["b", "a"]);
  });

  it("上传日期（YYYYMMDD）参与排序", () => {
    const older = makeVideo({ id: "older", url: "https://example.com/o", upload_date: "20250101" });
    const newer = makeVideo({ id: "newer", url: "https://example.com/n", upload_date: "20260301" });

    const items = build([older, newer]);

    expect(items.map((i) => i.id)).toEqual(["newer", "older"]);
  });

  it("时间全部缺失时保持插入顺序（稳定排序 + 插入顺序）", () => {
    const a = makeVideo({ id: "a", url: "https://example.com/a" });
    const b = makeVideo({ id: "b", url: "https://example.com/b" });
    const c = makeVideo({ id: "c", url: "https://example.com/c" });

    const items = build([a, b, c]);

    expect(items.map((i) => i.id)).toEqual(["a", "b", "c"]);
  });

  it("时间并列时依插入顺序，跨三源的插入顺序固定为 记录 → 队列 → 频道", () => {
    // 记录与队列两条时间都落 0 且状态相同（failed），故顺序完全由插入顺序决定；
    // 频道来源那条是 new（优先级更高）落在最前。
    // 若实现改成按参数对象的属性顺序遍历（videos 在前），前两条的顺序会反转。
    const rec = makeRecord({
      video_id: "from-record",
      video_url: "https://example.com/r",
      status: "failed",
      downloaded_at: "",
    });
    const task = makeTask({
      video_id: "from-task",
      video_url: "https://example.com/t",
      status: "failed",
      created_at: "",
    });
    const video = makeVideo({ id: "from-video", url: "https://example.com/v" });

    const items = build([video], [rec], [task]);

    expect(items.map((i) => i.id)).toEqual([
      "from-video",
      "from-record",
      "from-task",
    ]);
  });
});

describe("buildUnifiedVideoList - 输出形状", () => {
  it("三路都为空时返回空列表", () => {
    expect(build()).toEqual([]);
  });

  it("每个条目都显式带有三个来源字段（值可为 undefined）", () => {
    const items = build([makeVideo()]);

    expect(Object.keys(items[0]).sort()).toEqual([
      "channelInfo",
      "downloadInfo",
      "id",
      "queueTask",
      "status",
      "title",
      "url",
    ]);
    expect(items[0].channelInfo).toBeDefined();
    expect(items[0].downloadInfo).toBeUndefined();
    expect(items[0].queueTask).toBeUndefined();
  });
});

describe("statusPriority", () => {
  it("下载中排最前、失败排最后", () => {
    expect(statusPriority("downloading")).toBeLessThan(statusPriority("waiting"));
    expect(statusPriority("failed")).toBeGreaterThan(statusPriority("cancelled"));
  });

  it("七种状态的优先级严格递增", () => {
    const order: VideoStatus[] = [
      "downloading",
      "waiting",
      "paused",
      "completed",
      "new",
      "cancelled",
      "failed",
    ];

    const priorities = order.map(statusPriority);
    expect([...priorities].sort((a, b) => a - b)).toEqual(priorities);
  });
});

describe("formatDuration", () => {
  it("超过一小时显示 h:mm:ss", () => {
    expect(formatDuration(3661)).toBe("1:01:01");
  });

  it("一小时以内显示 m:ss", () => {
    expect(formatDuration(754)).toBe("12:34");
    expect(formatDuration(30)).toBe("0:30");
    expect(formatDuration(0)).toBe("0:00");
  });

  it("秒数四舍五入到整秒", () => {
    expect(formatDuration(59.6)).toBe("1:00");
    expect(formatDuration(3599.6)).toBe("1:00:00");
  });

  it("非有限数或负数返回空串（时长未知时不显示）", () => {
    expect(formatDuration(NaN)).toBe("");
    expect(formatDuration(Infinity)).toBe("");
    expect(formatDuration(-Infinity)).toBe("");
    expect(formatDuration(-1)).toBe("");
  });
});

describe("formatFileSize", () => {
  it("0 字节返回空串（由调用方决定占位符）", () => {
    expect(formatFileSize(0)).toBe("");
  });

  it("小于 1KB 时以字节显示且不带小数", () => {
    expect(formatFileSize(1023)).toBe("1023 B");
  });

  it("按 1024 进位并保留一位小数", () => {
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(1536)).toBe("1.5 KB");
    expect(formatFileSize(1024 * 1024)).toBe("1.0 MB");
    expect(formatFileSize(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("超过 GB 后继续以 GB 为单位递增（单位封顶）", () => {
    expect(formatFileSize(5 * 1024 ** 4)).toBe("5120.0 GB");
  });
});

describe("formatUploadDate", () => {
  it("YYYYMMDD 转换为带连字符的日期", () => {
    expect(formatUploadDate("20260115", null)).toBe("2026-01-15");
  });

  it("没有 YYYYMMDD 时回退到 epoch（UTC 日期）", () => {
    expect(formatUploadDate(null, 1735689600)).toBe("2025-01-01");
    expect(formatUploadDate(undefined, 1735689600)).toBe("2025-01-01");
  });

  it("YYYYMMDD 优先于 epoch", () => {
    expect(formatUploadDate("20260115", 1735689600)).toBe("2026-01-15");
  });

  it("非 8 位的 upload_date 不参与，回退 epoch", () => {
    expect(formatUploadDate("2026-01", 1735689600)).toBe("2025-01-01");
  });

  it("两者都缺失或 epoch 非正数时返回空串", () => {
    expect(formatUploadDate(null, null)).toBe("");
    expect(formatUploadDate(undefined, undefined)).toBe("");
    expect(formatUploadDate(null, 0)).toBe("");
    expect(formatUploadDate(null, -1)).toBe("");
  });
});

describe("formatTime", () => {
  it("委托给 Date 的本地化格式", () => {
    const iso = "2026-06-10T12:00:00Z";
    // 不硬编码格式化结果，避免 CI 的语言/时区差异导致假失败
    expect(formatTime(iso)).toBe(new Date(iso).toLocaleString("zh-CN"));
  });

  it("非法输入不抛异常", () => {
    expect(() => formatTime("not-a-date")).not.toThrow();
  });
});
