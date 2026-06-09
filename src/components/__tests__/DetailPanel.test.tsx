import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import DetailPanel from "../DetailPanel";
import type { Subscription, DownloadRecord } from "@/types";

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
    download_count: 0,
    last_check_status: null,
    last_check_error: null,
    tags: [],
    health_status: null,
    last_health_check: null,
    ...overrides,
  };
}

describe("DetailPanel", () => {
  const emptyRecords: DownloadRecord[] = [];
  const defaultProps = {
    records: emptyRecords,
    queueTasks: [],
    onPauseDownload: vi.fn(),
    onResumeDownload: vi.fn(),
    onCancelDownload: vi.fn(),
    onRetryDownload: vi.fn(),
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
        download_count: 5,
        last_checked_at: "2026-06-10T12:00:00Z",
      });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText(/已下载: 5 个/)).toBeInTheDocument();
      });
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
        expect(screen.getByText(/错误: Network timeout/)).toBeInTheDocument();
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

    it("health_status 为 dead 时不显示下载记录", () => {
      const sub = makeSub({ health_status: "dead" });

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 失效率频道不应显示下载记录区域
      expect(screen.queryByText("暂无下载记录")).toBeNull();
    });
  });

  describe("视频列表分页", () => {
    it("加载完成后显示下载记录区域", async () => {
      const sub = makeSub();

      render(
        <DetailPanel
          subscription={sub}
          {...defaultProps}
        />,
      );

      // 等待异步视频数据加载完成
      await waitFor(() => {
        expect(screen.getByText("暂无下载记录")).toBeInTheDocument();
      });
    });
  });
});
