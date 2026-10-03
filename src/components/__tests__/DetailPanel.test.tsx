import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import DetailPanel from "../DetailPanel";
import { getChannelVideos } from "@/lib/tauri";
import { formatDuration, formatFileSize } from "@/lib/unifiedVideoList";
import type {
  Subscription,
  DownloadRecord,
  DownloadTask,
  DownloadProgress,
  VideoInfo,
} from "@/types";

// Mock @/lib/tauri - 异步解析为空/默认值
vi.mock("@/lib/tauri", () => ({
  getChannelInfo: vi.fn().mockResolvedValue(null),
  getChannelVideos: vi.fn().mockResolvedValue({
    videos: [],
    total: 0,
    page: 1,
    page_size: 10,
    has_more: false,
  }),
}));

/** 创建一个测试用 Subscription */
function makeSub(overrides: Partial<Subscription> = {}): Subscription {
  return {
    id: "sub-1",
    url: "https://youtube.com/@test",
    platform: "youtube",
    channel_name: "Test Channel",
    channel_avatar_url: "https://example.com/avatar.jpg",
    paused: false,
    quality_preset: "1080p",
    group_name: "未分组",
    created_at: "2026-01-01T00:00:00Z",
    last_checked_at: null,
    last_successful_check_at: null,
    last_check_status: null,
    last_check_error: null,
    tags: [],
    health_status: null,
    last_health_check: null,
    ...overrides,
  };
}

/** 创建一个测试用 DownloadRecord（file_size 默认 0 表示未知，避免意外触发右侧文件大小分支） */
function makeRecord(overrides: Partial<DownloadRecord> = {}): DownloadRecord {
  return {
    id: "rec-1",
    subscription_id: "sub-1",
    video_id: "vid-1",
    video_title: "Video 1",
    video_url: "https://e/v1",
    file_path: "/tmp/v1.mp4",
    file_size: 0,
    status: "completed",
    error_message: null,
    downloaded_at: "2026-06-10T12:00:00Z",
    quality: "1080p",
    retry_count: 0,
    last_retry_at: null,
    ...overrides,
  };
}

/** 创建一个测试用 DownloadTask */
function makeTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: "task-1",
    video_id: "vid-1",
    video_url: "https://e/v1",
    video_title: "Video 1",
    subscription_id: "sub-1",
    quality: "1080p",
    status: "waiting",
    progress: null,
    error_message: null,
    created_at: "2026-06-10T12:00:00Z",
    completed_at: null,
    record_id: "rec-1",
    next_retry_at: null,
    ...overrides,
  };
}

/** 创建一个测试用 VideoInfo（频道视频，仅能由组件内部异步加载获得） */
function makeVideo(overrides: Partial<VideoInfo> = {}): VideoInfo {
  return {
    id: "video-1",
    title: "Channel Video 1",
    url: "https://e/video-1",
    duration: null,
    upload_date: null,
    epoch: null,
    thumbnail: null,
    ...overrides,
  };
}

/**
 * 让下一次 getChannelVideos 调用返回指定视频。
 * 必须满足 has_more=false，否则组件会继续翻页。
 */
function mockChannelVideosOnce(videos: VideoInfo[]): void {
  vi.mocked(getChannelVideos).mockResolvedValueOnce({
    videos,
    total: videos.length,
    page: 1,
    page_size: 10,
    has_more: false,
  });
}

describe("DetailPanel", () => {
  const emptyRecords: DownloadRecord[] = [];
  const defaultProps = {
    records: emptyRecords,
    queueTasks: [],
    missingPaths: new Set<string>(),
    onPauseDownload: vi.fn(),
    onResumeDownload: vi.fn(),
    onCancelDownload: vi.fn(),
    onRedownload: vi.fn(),
    onUpdateQuality: vi.fn(),
  };

  describe("未选择订阅时", () => {
    it("显示提示文本", () => {
      render(
        <DetailPanel
          subscription={null}
          {...defaultProps}
        />,
      );
      expect(screen.getByText("选择左侧订阅查看详情")).toBeInTheDocument();
    });
  });

  describe("选择有效订阅时", () => {
    it("加载完成后显示频道名称和基本信息", async () => {
      const sub = makeSub({
        channel_name: "My Channel",
        platform: "youtube",
        quality_preset: "720p",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 等待异步加载完成
      await waitFor(() => {
        expect(screen.getByText("My Channel")).toBeInTheDocument();
      });

      expect(screen.getByText("youtube")).toBeInTheDocument();
      expect(screen.getByText("720p")).toBeInTheDocument();
    });

    it("显示暂停状态的标签", async () => {
      const sub = makeSub({ paused: true });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("已暂停")).toBeInTheDocument();
      });
    });

    it("显示下载统计", async () => {
      const sub = makeSub({
        last_checked_at: "2026-06-10T12:00:00Z",
      });
      // 已完成数现在由 records 派生（口径：处于 completed 的记录条数）
      const completedRecords: DownloadRecord[] = Array.from(
        { length: 5 },
        (_, i) => ({
          id: `rec-${i}`,
          subscription_id: "sub-1",
          video_id: `vid-${i}`,
          video_title: `Video ${i}`,
          video_url: `https://youtu.be/${i}`,
          file_path: `/tmp/${i}.mp4`,
          file_size: 1000,
          status: "completed",
          error_message: null,
          downloaded_at: "2026-06-10T12:00:00Z",
          quality: "1080p",
          retry_count: 0,
          last_retry_at: null,
        }),
      );

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          records={completedRecords}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText(/已完成: 5 个/)).toBeInTheDocument();
      });
    });

    it("deleted 记录显示「已删除」且不计入已完成数、无操作按钮", async () => {
      const sub = makeSub();
      const deletedRecords: DownloadRecord[] = [
        {
          id: "rec-del",
          subscription_id: "sub-1",
          video_id: "vid-del",
          video_title: "Deleted Video",
          video_url: "https://youtu.be/del",
          file_path: "/tmp/del.mp4",
          file_size: 1000,
          status: "deleted",
          error_message: null,
          downloaded_at: "2026-06-10T12:00:00Z",
          quality: "1080p",
          retry_count: 0,
          last_retry_at: null,
        },
      ];

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          records={deletedRecords}
        />,
      );

      // 命中新增的「已删除」元信息 span
      await waitFor(() => {
        expect(screen.getByText("已删除")).toBeInTheDocument();
      });
      // deleted 不计入 completed 口径
      expect(screen.getByText(/已完成: 0 个/)).toBeInTheDocument();
      // 无操作按钮（重试按钮只在 failed 时出现）
      expect(screen.queryByTitle("重试")).toBeNull();

      // 回归：已删除的视频必须以「删除线 + 半透明」明确区别于正常条目。
      // 这曾是真实缺陷 —— 记录只剩标题、看不出已删除状态。样式是唯一的视觉线索，
      // 因此必须连同文案一起锁住，避免将来只留文案却丢掉样式。
      const row = screen.getByText("Deleted Video").closest("li")!;
      expect(within(row).getByText("Deleted Video")).toHaveStyle({
        textDecoration: "line-through",
      });
      expect(row).toHaveStyle({ opacity: 0.5 });
      // 日期回退：无频道信息时用 downloaded_at 补出日期
      expect(row.textContent).toContain(
        `已删除 · ${new Date("2026-06-10T12:00:00Z").toLocaleDateString("zh-CN")}`,
      );
    });

    it("显示检查失败的提示", async () => {
      const sub = makeSub({
        last_check_status: "failed",
        last_check_error: "Network timeout",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("错误: Network timeout")).toBeInTheDocument();
      });
    });
  });

  describe("频道已失效率", () => {
    it("health_status 为 dead 时显示失效率提示", () => {
      const sub = makeSub({
        health_status: "dead",
        last_health_check: "2026-06-10T10:00:00Z",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 失效率提示应该立即显示（同步检查，不需要等待异步加载）
      expect(screen.getByText(/此频道已失效/)).toBeInTheDocument();
    });

    it("health_status 为 dead 时不渲染视频列表空态", () => {
      const sub = makeSub({ health_status: "dead" });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 失效率频道不应渲染视频列表区域（空态文案为「暂无视频」）
      expect(screen.queryByText("暂无视频")).toBeNull();
    });
  });

  describe("视频列表分页", () => {
    it("无视频时显示空列表提示", async () => {
      const sub = makeSub();

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 等待异步视频数据加载完成
      await waitFor(() => {
        expect(screen.getByText("暂无视频")).toBeInTheDocument();
      });
    });

    it("翻页失败时显示错误，且不丢弃已加载的第一页", async () => {
      // 后台翻页循环原先是 `catch { break }` 静默中断：后端已改为解析失败即报错，
      // 若这里继续吞掉，第二页起就表现为「列表莫名少了一截」而毫无提示。
      vi.mocked(getChannelVideos)
        .mockResolvedValueOnce({
          videos: [
            makeVideo({ id: "vid-p1", title: "Page 1 Video", url: "https://e/p1" }),
          ],
          total: 2,
          page: 1,
          page_size: 10,
          has_more: true,
        })
        .mockRejectedValueOnce(new Error("第 2 页解析失败"));

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Error: 第 2 页解析失败")).toBeInTheDocument();
      });
      expect(screen.getByText("Page 1 Video")).toBeInTheDocument();
    });
  });

  // 统一视频列表是详情面板信息密度最高的区域：8 种状态各有独立的图标、文案、
  // 按钮组合。此前测试几乎不断言列表行内容，导致「已删除记录只剩标题」这类缺陷
  // 只能靠人工发现。以下用例锁定每一行的可观察输出。
  describe("统一视频列表渲染", () => {
    /** 按标题定位到对应的列表行（ListItem 渲染为 <li>），避免跨行 testid/title 撞车 */
    async function findRow(title: string): Promise<HTMLElement> {
      const text = await screen.findByText(title);
      const row = text.closest("li");
      if (!row) throw new Error(`未找到标题为「${title}」的列表行`);
      return row;
    }

    /**
     * 构造覆盖全部 8 种状态的混合数据。
     * - 7 种可由 records + queueTasks 直接派生；
     * - new 必须由频道视频（videoList）产生，故调用方需先用 mockChannelVideosOnce 注入 videos。
     */
    function makeStatusFixtures() {
      return {
        videos: [makeVideo({ id: "vid-new", title: "New Video", url: "https://e/new" })],
        records: [
          makeRecord({
            id: "r-dl",
            video_id: "vid-dl",
            video_title: "Downloading Video",
            video_url: "https://e/dl",
            status: "downloading",
          }),
          makeRecord({
            id: "r-done",
            video_id: "vid-done",
            video_title: "Completed Video",
            video_url: "https://e/done",
            status: "completed",
            file_size: 1024,
            downloaded_at: "2026-06-10T12:00:00Z",
          }),
          makeRecord({
            id: "r-fail",
            video_id: "vid-fail",
            video_title: "Failed Video",
            video_url: "https://e/fail",
            status: "failed",
            error_message: "boom",
            downloaded_at: "2026-06-10T12:00:00Z",
          }),
          makeRecord({
            id: "r-del",
            video_id: "vid-del",
            video_title: "Deleted Video",
            video_url: "https://e/del",
            status: "deleted",
            file_size: 1000,
            downloaded_at: "2026-06-10T12:00:00Z",
          }),
        ],
        tasks: [
          makeTask({
            id: "t-dl",
            video_id: "vid-dl",
            video_title: "Downloading Video",
            video_url: "https://e/dl",
            status: "running",
          }),
          makeTask({
            id: "t-pause",
            video_id: "vid-pause",
            video_title: "Paused Video",
            video_url: "https://e/pause",
            status: "paused",
          }),
          makeTask({
            id: "t-wait",
            video_id: "vid-wait",
            video_title: "Waiting Video",
            video_url: "https://e/wait",
            status: "waiting",
          }),
          makeTask({
            id: "t-cancel",
            video_id: "vid-cancel",
            video_title: "Cancelled Video",
            video_url: "https://e/cancel",
            status: "cancelled",
          }),
        ],
      };
    }

    it("8 种状态各渲染对应的状态图标", async () => {
      // 图标是用户判断「这个视频现在处于什么阶段」的首要视觉线索。状态图标与
      // 频道头部/操作按钮的图标存在 testid 撞车，因此必须先按标题定位到行再断言。
      const { videos, records, tasks } = makeStatusFixtures();
      mockChannelVideosOnce(videos);

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={records}
          queueTasks={tasks}
        />,
      );

      const expectedIcons: Array<[string, string]> = [
        ["New Video", "PlayCircleOutlineIcon"],
        ["Downloading Video", "DownloadingIcon"],
        ["Paused Video", "PauseIcon"],
        ["Waiting Video", "HourglassEmptyIcon"],
        ["Completed Video", "CheckCircleIcon"],
        ["Failed Video", "ErrorIcon"],
        ["Cancelled Video", "CancelIcon"],
        ["Deleted Video", "DeleteOutlineIcon"],
      ];

      for (const [title, testId] of expectedIcons) {
        const row = await findRow(title);
        expect(within(row).getByTestId(testId)).toBeInTheDocument();
      }
    });

    it("各状态的状态文案正确（含日期回退与失败原因）", async () => {
      // 状态文案是图标之外的文字兜底；日期只来自频道视频，缺失时回退到
      // downloaded_at。硬编码日期会因时区/语言设置而脆，故在测试内计算期望值。
      const { videos, records, tasks } = makeStatusFixtures();
      mockChannelVideosOnce(videos);
      const longDate = new Date("2026-06-10T12:00:00Z").toLocaleDateString("zh-CN");

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={records}
          queueTasks={tasks}
        />,
      );

      // waiting/paused/cancelled 的元信息就是纯状态词（无日期/时长），因此要求
      // 锚定精确匹配；用 toContain 子串断言的话，措辞被污染成「等待中X」也抓不到。
      expect(within(await findRow("Waiting Video")).getByText(/^等待中$/)).toBeInTheDocument();
      expect(within(await findRow("Paused Video")).getByText(/^已暂停$/)).toBeInTheDocument();
      expect(within(await findRow("Cancelled Video")).getByText(/^已取消$/)).toBeInTheDocument();
      expect((await findRow("Completed Video")).textContent).toContain(`已完成 · ${longDate}`);
      expect((await findRow("Failed Video")).textContent).toContain(
        `失败 · boom · ${longDate}`,
      );
      expect((await findRow("Deleted Video")).textContent).toContain(`已删除 · ${longDate}`);

      // downloading 的「下载中」在元信息与右侧辅助列各出现一次，getByText 会因
      // 多匹配报错，故用 getAllByText 锁住「两处都渲染」这一点。
      const downloadingRow = await findRow("Downloading Video");
      expect(within(downloadingRow).getAllByText("下载中")).toHaveLength(2);
    });

    it("「全部视频 (N)」的计数是合并去重后的条目数", async () => {
      // 计数曾与侧边栏「记录总数」口径混淆；这里通过 2 条 record + 1 条 task（各自
      // 唯一 key，不会合并）验证它确实是统一列表的长度，而非任何单一来源的长度。
      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={[
            makeRecord({ id: "r-a", video_id: "vid-a", video_title: "Video A", video_url: "https://e/a" }),
            makeRecord({ id: "r-b", video_id: "vid-b", video_title: "Video B", video_url: "https://e/b" }),
          ]}
          queueTasks={[
            makeTask({ id: "t-c", video_id: "vid-c", video_title: "Video C", video_url: "https://e/c" }),
          ]}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("全部视频 (3)")).toBeInTheDocument();
      });
      // 标题的 (3) 只是个数字，必须用实际渲染行数证明「3 = 统一列表长度」。
      // 该页面只有统一列表会渲染 <li>（频道头部是 div/Chip，其它 ListItem 不存在），
      // 故 listitem 角色计数应恰为 3。
      expect(screen.getAllByRole("listitem")).toHaveLength(3);
    });

    it("操作按钮组合矩阵符合状态规则", async () => {
      // 按钮给错会直接导致误操作（例如对已删除项提供重试、对无任务项提供取消）。
      // 暂停/取消都要求存在 queueTask（内存队列），重试只要求 failed。
      const { videos, records, tasks } = makeStatusFixtures();
      mockChannelVideosOnce(videos);

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={records}
          queueTasks={tasks}
        />,
      );

      const downloadingRow = await findRow("Downloading Video");
      expect(within(downloadingRow).getByTitle("暂停")).toBeInTheDocument();
      expect(within(downloadingRow).getByTitle("取消")).toBeInTheDocument();

      const waitingRow = await findRow("Waiting Video");
      expect(within(waitingRow).queryByTitle("暂停")).toBeNull();
      expect(within(waitingRow).getByTitle("取消")).toBeInTheDocument();

      const pausedRow = await findRow("Paused Video");
      expect(within(pausedRow).queryByTitle("暂停")).toBeNull();
      expect(within(pausedRow).getByTitle("取消")).toBeInTheDocument();

      // failed 记录没有队列任务，仍应给出重试
      const failedRow = await findRow("Failed Video");
      expect(within(failedRow).getByTitle("重试")).toBeInTheDocument();
      expect(within(failedRow).queryByTitle("取消")).toBeNull();

      for (const title of ["Completed Video", "Deleted Video", "New Video"]) {
        const row = await findRow(title);
        expect(within(row).queryByTitle("暂停")).toBeNull();
        expect(within(row).queryByTitle("取消")).toBeNull();
        expect(within(row).queryByTitle("重试")).toBeNull();
      }
    });

    it("进度条只在「下载中且该条目有下载记录」时出现，宽度取自 progressMap", async () => {
      // 进度条宽度必须来自 progressMap（以 video_url 为键）；且仅当条目同时有
      // 下载记录（downloadInfo）时才渲染，纯队列任务没有进度条。
      const record = makeRecord({
        id: "r-dl",
        video_id: "vid-dl",
        video_title: "Downloading With Record",
        video_url: "https://e/dl",
        status: "downloading",
      });
      const taskWithRecord = makeTask({
        id: "t-dl",
        video_id: "vid-dl",
        video_title: "Downloading With Record",
        video_url: "https://e/dl",
        status: "running",
      });
      const taskOnly = makeTask({
        id: "t-dl2",
        video_id: "vid-dl2",
        video_title: "Downloading No Record",
        video_url: "https://e/dl2",
        status: "running",
      });
      const progressMap = new Map<string, DownloadProgress>([
        [
          "https://e/dl",
          {
            task_id: "t-dl",
            video_url: "https://e/dl",
            percent: 42,
            speed: "1MiB/s",
            downloaded_bytes: 42,
            total_bytes: 100,
            eta: "00:10",
          },
        ],
      ]);

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={[record]}
          queueTasks={[taskWithRecord, taskOnly]}
          progressMap={progressMap}
        />,
      );

      const rowWithRecord = await findRow("Downloading With Record");
      expect(within(rowWithRecord).getByTestId("download-progress-fill")).toHaveStyle({
        width: "42%",
      });

      const rowNoRecord = await findRow("Downloading No Record");
      expect(within(rowNoRecord).queryByTestId("download-progress-fill")).toBeNull();
    });

    it("completed 右侧显示文件大小+短日期，failed 右侧只显示短日期，file_size=0 不显示大小", async () => {
      // 右侧辅助列依赖 downloadInfo：completed 展示体积与完成日期，failed 只展示日期。
      // file_size=0 表示大小未知，此时不应渲染空的「 · 日期」形式。
      const completedRowRecord = makeRecord({
        id: "r-size",
        video_id: "vid-size",
        video_title: "Size Video",
        video_url: "https://e/size",
        status: "completed",
        file_size: 1024,
        downloaded_at: "2026-06-10T12:00:00Z",
      });
      const zeroSizeRecord = makeRecord({
        id: "r-zero",
        video_id: "vid-zero",
        video_title: "Zero Size Video",
        video_url: "https://e/zero",
        status: "completed",
        file_size: 0,
        downloaded_at: "2026-06-10T12:00:00Z",
      });
      const failedRecord = makeRecord({
        id: "r-fail",
        video_id: "vid-fail",
        video_title: "Right Failed Video",
        video_url: "https://e/fail",
        status: "failed",
        error_message: "x",
        file_size: 0,
        downloaded_at: "2026-06-10T12:00:00Z",
      });
      const shortDate = new Date("2026-06-10T12:00:00Z").toLocaleDateString("zh-CN", {
        month: "short",
        day: "numeric",
      });

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          records={[completedRowRecord, zeroSizeRecord, failedRecord]}
        />,
      );

      const sizeRow = await findRow("Size Video");
      expect(sizeRow.textContent).toContain(`${formatFileSize(1024)} · ${shortDate}`);

      // 短日期是右侧列独有的格式：失败行不应带文件大小
      const failedRow = await findRow("Right Failed Video");
      expect(failedRow.textContent).toContain(shortDate);
      expect(failedRow.textContent).not.toMatch(/\d+(\.\d+)?\s?(B|KB|MB|GB)/);

      const zeroRow = await findRow("Zero Size Video");
      expect(zeroRow.textContent).not.toMatch(/\d+(\.\d+)?\s?(B|KB|MB|GB)/);
    });

    it("new 状态的行显示频道视频的日期与时长，且无任何操作按钮", async () => {
      // new 是唯一只来自频道视频来源的状态，其日期/时长完全依赖 channelInfo；
      // 尚未下载的视频不应出现暂停/取消/重试中的任何一个。
      mockChannelVideosOnce([
        makeVideo({
          id: "vid-new",
          title: "Channel Video",
          url: "https://e/new",
          upload_date: "20260101",
          duration: 600,
        }),
      ]);

      render(<DetailPanel subscription={makeSub()} {...defaultProps} />);

      const row = await findRow("Channel Video");
      expect(within(row).getByTestId("PlayCircleOutlineIcon")).toBeInTheDocument();
      // 期待 "2026-01-01 · 10:00"，两个片段都按格式化函数计算以保证一致
      expect(row.textContent).toContain(
        `2026-01-01 · ${formatDuration(600)}`,
      );
      expect(within(row).queryByTitle("暂停")).toBeNull();
      expect(within(row).queryByTitle("取消")).toBeNull();
      expect(within(row).queryByTitle("重试")).toBeNull();
    });

    it("空态与列表标题互斥：无数据只显示空态，不渲染「全部视频」标题", async () => {
      // 「全部视频 (N)」只在 N>0 时渲染，避免出现「全部视频 (0)」这种空洞标题；
      // 无数据时必须只保留空态提示。
      render(<DetailPanel subscription={makeSub()} {...defaultProps} />);

      await waitFor(() => {
        expect(screen.getByText("暂无视频")).toBeInTheDocument();
      });
      expect(screen.queryByText(/全部视频/)).toBeNull();
    });

    it("cancelled 状态不拼时长（即使频道视频带 duration）", async () => {
      // 这是与其它状态不一致的既有行为：cancelled 文案只取状态词（可有日期），
      // 不追加时长。此用例把它显式锁住，防止有人「顺手统一」而改变展示。
      mockChannelVideosOnce([
        makeVideo({
          id: "vid-cancel",
          title: "Cancelled With Duration",
          url: "https://e/cancel",
          upload_date: null,
          epoch: null,
          duration: 600,
        }),
      ]);

      render(
        <DetailPanel
          subscription={makeSub()}
          {...defaultProps}
          queueTasks={[
            makeTask({
              id: "t-cancel",
              video_id: "vid-cancel",
              video_title: "",
              video_url: "https://e/cancel",
              status: "cancelled",
            }),
          ]}
        />,
      );

      const row = await findRow("Cancelled With Duration");
      expect(row.textContent).toContain("已取消");
      expect(row.textContent).not.toContain(formatDuration(600));
    });
  });

  // 新增交互（spec 008）：可升级 / 文件缺失 / 已在其它订阅下载 / 订阅级画质设置。
  // 这些是 US1 的核心可观察路径，纯函数层已有真值表，这里锁定组件接线（章程原则 I）。
  describe("新增交互（可升级 / 文件缺失 / 已在他处 / 画质设置）", () => {
    /** 按标题定位列表行（<li>） */
    async function findRow(title: string): Promise<HTMLElement> {
      const text = await screen.findByText(title);
      const row = text.closest("li");
      if (!row) throw new Error(`未找到标题为「${title}」的列表行`);
      return row;
    }

    it("可升级：chip 与按钮 title 带目标档次，点击调用 onRedownload(record.id)", async () => {
      const onRedownload = vi.fn();
      const sub = makeSub({ quality_preset: "1080p" });
      const record = makeRecord({ quality: "720p", video_title: "Upgrade Me" });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          records={[record]}
          onRedownload={onRedownload}
        />,
      );

      const row = await findRow("Upgrade Me");
      expect(within(row).getByText("可升级 1080p")).toBeInTheDocument();

      await userEvent.click(within(row).getByTitle("升级到 1080p"));
      expect(onRedownload).toHaveBeenCalledTimes(1);
      expect(onRedownload).toHaveBeenCalledWith("rec-1");
    });

    it("可升级：best 渲染为「最高画质」", async () => {
      const sub = makeSub({ quality_preset: "best" });
      const record = makeRecord({ quality: "720p", video_title: "Best Upgrade" });

      render(
        <DetailPanel subscription={sub} {...defaultProps} records={[record]} />,
      );

      const row = await findRow("Best Upgrade");
      expect(within(row).getByText("可升级 最高画质")).toBeInTheDocument();
      expect(within(row).getByTitle("升级到 最高画质")).toBeInTheDocument();
    });

    it("文件缺失：显示「文件缺失」、只提供「重新下载」，点击调用 onRedownload(record.id)", async () => {
      const onRedownload = vi.fn();
      const sub = makeSub({ quality_preset: "1080p" });
      const record = makeRecord({
        quality: "720p",
        video_title: "Gone Video",
        file_path: "/tmp/gone.mp4",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          records={[record]}
          missingPaths={new Set(["/tmp/gone.mp4"])}
          onRedownload={onRedownload}
        />,
      );

      const row = await findRow("Gone Video");
      expect(within(row).getByText("文件缺失")).toBeInTheDocument();
      // 缺失优先于升级：不得出现升级按钮
      expect(within(row).queryByTitle("升级到 1080p")).toBeNull();

      await userEvent.click(within(row).getByTitle("重新下载"));
      expect(onRedownload).toHaveBeenCalledWith("rec-1");
    });

    it("已在其它订阅下载：显示 chip，且不提供升级 / 重新下载", async () => {
      const sub = makeSub({ id: "sub-1", quality_preset: "1080p" });
      const record = makeRecord({
        subscription_id: "sub-OTHER",
        quality: "720p",
        video_title: "Other Sub Video",
      });

      render(
        <DetailPanel subscription={sub} {...defaultProps} records={[record]} />,
      );

      const row = await findRow("Other Sub Video");
      expect(within(row).getByText("已在其它订阅下载")).toBeInTheDocument();
      expect(within(row).queryByTitle("升级到 1080p")).toBeNull();
      expect(within(row).queryByTitle("重新下载")).toBeNull();
    });

    it("订阅级画质下拉：选择后调用 onUpdateQuality(id, value)", async () => {
      const onUpdateQuality = vi.fn();
      const sub = makeSub({ id: "sub-1", quality_preset: "1080p" });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          onUpdateQuality={onUpdateQuality}
        />,
      );

      // 面板内只有一个 combobox（订阅级画质下拉）
      await userEvent.click(screen.getByRole("combobox"));
      await userEvent.click(await screen.findByRole("option", { name: "720p" }));

      expect(onUpdateQuality).toHaveBeenCalledWith("sub-1", "720p");
    });

    it("失败行「重试」只重下这一条记录（调用 onRedownload(record.id)）", async () => {
      const onRedownload = vi.fn();
      const sub = makeSub();
      const record = makeRecord({
        status: "failed",
        video_title: "Retry Video",
        error_message: "boom",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
          records={[record]}
          onRedownload={onRedownload}
        />,
      );

      const row = await findRow("Retry Video");
      await userEvent.click(within(row).getByTitle("重试"));
      expect(onRedownload).toHaveBeenCalledWith("rec-1");
    });
  });
});
