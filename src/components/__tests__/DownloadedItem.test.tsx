import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import DownloadedItem, { resolveStatusVisual } from "../DownloadedItem";
import {
  CheckCircle as CompletedIcon,
  Warning as MissingIcon,
  Error as FailedIcon,
  DeleteOutline as DeletedIcon,
  Downloading as DownloadingIcon,
  Pause as PausedIcon,
  HourglassEmpty as WaitingIcon,
  Cancel as CancelledIcon,
} from "@mui/icons-material";
import type { DownloadRecord } from "@/types";

// 组件只通过 api.openInFolder / api.deleteFile 触达后端，这里整体隔离掉 Tauri。
vi.mock("@/lib/tauri", () => ({
  openInFolder: vi.fn(),
  deleteFile: vi.fn().mockResolvedValue(undefined),
}));

/** 创建一个测试用 DownloadRecord（file_size 默认 0 表示未知） */
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
    ...overrides,
  };
}

/**
 * 定位整行（ListItemButton 渲染为带 .MuiListItemButton-root 的 div），
 * 样式断言（删除线/半透明）挂在这一层。
 */
function getRow(title: string): HTMLElement {
  const row = screen.getByText(title).closest(".MuiListItemButton-root");
  if (!row) throw new Error(`未找到标题为「${title}」的记录行`);
  return row as HTMLElement;
}

describe("DownloadedItem", () => {
  // 状态图标是本组件最关键的输出：它曾因嵌套三元缺少兜底以外的分支，导致
  // downloading/paused/waiting/cancelled 全部错显示为「已完成」对勾。
  describe("状态图标", () => {
    // [status, fileMissing, 期望的 data-testid]
    const iconCases: Array<[DownloadRecord["status"], boolean, string]> = [
      ["deleted", false, "DeleteOutlineIcon"],
      ["completed", false, "CheckCircleIcon"],
      ["completed", true, "WarningIcon"],
      ["failed", false, "ErrorIcon"],
      ["downloading", false, "DownloadingIcon"],
      ["paused", false, "PauseIcon"],
      ["waiting", false, "HourglassEmptyIcon"],
      ["cancelled", false, "CancelIcon"],
    ];

    it.each(iconCases)(
      "status=%s fileMissing=%s 时显示 %s",
      (status, fileMissing, testId) => {
        render(
          <DownloadedItem
            record={makeRecord({ status, video_title: `${status} video` })}
            fileMissing={fileMissing}
          />,
        );
        expect(screen.getByTestId(testId)).toBeInTheDocument();
      },
    );

    it("failed 与 downloading 使用不同图标（防止退化成同一兜底图标）", () => {
      render(
        <>
          <DownloadedItem
            record={makeRecord({ id: "r-fail", video_title: "Failed", status: "failed" })}
          />
          <DownloadedItem
            record={makeRecord({ id: "r-dl", video_title: "Downloading", status: "downloading" })}
          />
        </>,
      );

      const failedRow = getRow("Failed");
      const downloadingRow = getRow("Downloading");

      expect(within(failedRow).getByTestId("ErrorIcon")).toBeInTheDocument();
      expect(within(downloadingRow).getByTestId("DownloadingIcon")).toBeInTheDocument();
      // 负向断言：两者绝不能是同一个图标
      expect(within(failedRow).queryByTestId("DownloadingIcon")).toBeNull();
      expect(within(downloadingRow).queryByTestId("ErrorIcon")).toBeNull();
    });
  });

  describe("deleted 的既有行为", () => {
    it("显示「已删除」Chip，且不渲染「打开文件夹」「删除文件」按钮", () => {
      render(
        <DownloadedItem
          record={makeRecord({ status: "deleted", video_title: "Deleted Video" })}
        />,
      );

      expect(screen.getByText("已删除")).toBeInTheDocument();
      // Tooltip 的 title 会落到子按钮的 aria-label 上，故按可访问名定位。
      expect(screen.queryByRole("button", { name: "打开文件夹" })).toBeNull();
      expect(screen.queryByRole("button", { name: "删除文件" })).toBeNull();
    });

    it("非 deleted 时两个操作按钮都存在", () => {
      render(
        <DownloadedItem
          record={makeRecord({ status: "completed", video_title: "Completed Video" })}
        />,
      );

      expect(screen.queryByText("已删除")).toBeNull();
      expect(screen.getByRole("button", { name: "打开文件夹" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "删除文件" })).toBeInTheDocument();
    });

    it("整行带删除线与半透明样式", () => {
      render(
        <DownloadedItem
          record={makeRecord({ status: "deleted", video_title: "Deleted Video" })}
        />,
      );

      const row = getRow("Deleted Video");
      expect(row).toHaveStyle({ textDecoration: "line-through" });
      expect(row).toHaveStyle({ opacity: 0.5 });
    });
  });
});

/**
 * 这一组刻意绕过 DOM 直接测纯函数：`resolveStatusVisual` 的 `color` 是我们自己的
 * 语义键字面量（如 "success.main"），不是主题解析后的具体色值，所以直接断言它
 * 既稳定又不涉及主题解析 —— 不同于 DOM 层断言颜色需要用 `toHaveStyle` 拿到
 * 主题解析值（如 rgba(0, 0, 0, 0.38)），一改主题就碎。
 *
 * 曾有变异测试证明这里的颜色映射完全无保护：把 paused 的 color 从 warning.main
 * 改成 success.main，12 条 DOM 测试全部通过。
 */
describe("resolveStatusVisual（状态视觉映射的纯函数契约）", () => {
  // [status, fileMissing, 期望的 color 语义键]
  const colorCases: Array<[DownloadRecord["status"], boolean, string]> = [
    ["deleted", false, "text.disabled"],
    ["completed", false, "success.main"],
    ["completed", true, "warning.main"],
    ["failed", false, "error.main"],
    ["downloading", false, "info.main"],
    ["paused", false, "warning.main"],
    ["waiting", false, "text.disabled"],
    ["cancelled", false, "text.disabled"],
  ];

  it.each(colorCases)(
    "status=%s fileMissing=%s 时 color 为 %s",
    (status, fileMissing, color) => {
      expect(resolveStatusVisual(status, fileMissing).color).toBe(color);
    },
  );

  // [status, fileMissing, 期望的 Icon 组件]
  const iconComponentCases: Array<
    [DownloadRecord["status"], boolean, typeof CompletedIcon]
  > = [
    ["deleted", false, DeletedIcon],
    ["completed", false, CompletedIcon],
    ["completed", true, MissingIcon],
    ["failed", false, FailedIcon],
    ["downloading", false, DownloadingIcon],
    ["paused", false, PausedIcon],
    ["waiting", false, WaitingIcon],
    ["cancelled", false, CancelledIcon],
  ];

  it.each(iconComponentCases)(
    "status=%s fileMissing=%s 时 Icon 为对应的 MUI 图标组件",
    (status, fileMissing, Icon) => {
      expect(resolveStatusVisual(status, fileMissing).Icon).toBe(Icon);
    },
  );

  it("fileMissing 只影响 completed，其余状态完全不受其影响", () => {
    const nonCompleted: DownloadRecord["status"][] = [
      "deleted",
      "failed",
      "downloading",
      "paused",
      "waiting",
      "cancelled",
    ];
    for (const status of nonCompleted) {
      const present = resolveStatusVisual(status, false);
      const missing = resolveStatusVisual(status, true);
      expect(missing.Icon).toBe(present.Icon);
      expect(missing.color).toBe(present.color);
    }

    // completed 是唯一例外：缺失与存在必须给出不同图标与不同颜色
    const ok = resolveStatusVisual("completed", false);
    const gone = resolveStatusVisual("completed", true);
    expect(gone.Icon).not.toBe(ok.Icon);
    expect(gone.color).not.toBe(ok.color);
  });

  it("状态之间的颜色不退化：有意义的分组保持区分，兜底分组保持一致", () => {
    // 防退化：这些分支的颜色语义不同，不能被顺手写成同一个兜底值
    expect(resolveStatusVisual("downloading", false).color).not.toBe(
      resolveStatusVisual("completed", false).color,
    );
    expect(resolveStatusVisual("failed", false).color).not.toBe(
      resolveStatusVisual("paused", false).color,
    );

    // 有意分组：deleted/waiting/cancelled 共用 text.disabled
    const disabledGroup = [
      resolveStatusVisual("deleted", false).color,
      resolveStatusVisual("waiting", false).color,
      resolveStatusVisual("cancelled", false).color,
    ];
    expect(new Set(disabledGroup).size).toBe(1);
    expect(disabledGroup[0]).toBe("text.disabled");
  });
});
