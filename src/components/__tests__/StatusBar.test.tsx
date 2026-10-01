import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import StatusBar from "../StatusBar";
import { getAppState } from "@/lib/tauri";

// 捕获 listen 注册的回调，供测试手动触发事件。
// 用 vi.hoisted 声明，避免 vi.mock 被 hoist 到文件顶部后工厂闭包引用未初始化变量
// （否则报 "Cannot access before initialization"）。
const listeners = vi.hoisted(() => new Map<string, (event: unknown) => void>());

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, cb: (event: unknown) => void) => {
    listeners.set(name, cb);
    return Promise.resolve(() => {
      listeners.delete(name);
    });
  }),
}));

vi.mock("@/lib/tauri", () => ({
  getAppState: vi.fn(),
}));

describe("StatusBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
  });

  it("挂载时拉取并显示上次检查时间", async () => {
    // 重要：删掉轮询后，挂载时这一次初始拉取是「从未」→「有值」的唯一途径
    const iso = "2026-03-05T08:30:00Z";
    vi.mocked(getAppState).mockResolvedValue({ last_check_time: iso });
    // 期望值用与组件相同的表达式计算，避免时区/locale 差异导致换机器失败
    const expected = new Date(iso).toLocaleString("zh-CN");

    render(<StatusBar />);
    // 精确匹配完整文案（getByText 对元素 textContent 归一化后比较），
    // 避免 toContain 之类子串匹配放过「上次检查: <时间>X」这种尾部追加型回归
    await waitFor(() => {
      expect(screen.getByText(`上次检查: ${expected}`)).toBeInTheDocument();
    });
    expect(getAppState).toHaveBeenCalledTimes(1);
  });

  it("last_check_time 为 null 时显示「从未」", async () => {
    // 重要：初始状态与后端无检查记录时的展示语义
    vi.mocked(getAppState).mockResolvedValue({ last_check_time: null });

    render(<StatusBar />);
    // 精确匹配「从未」而非子串匹配，否则 "从未X" 之类的文案回归无法被察觉
    await waitFor(() => {
      expect(screen.getByText("上次检查: 从未")).toBeInTheDocument();
    });
  });

  it("收到 scheduler-check-complete 后重新拉取并更新显示", async () => {
    // 核心用例：删掉轮询后事件订阅是唯一的实时刷新来源。
    // 这条锁住「事件订阅真的注册上了且回调会重新拉取」这个契约。
    const isoA = "2026-03-05T08:30:00Z";
    const isoB = "2026-03-06T09:45:00Z";
    const expectedA = new Date(isoA).toLocaleString("zh-CN");
    const expectedB = new Date(isoB).toLocaleString("zh-CN");

    vi.mocked(getAppState).mockResolvedValue({ last_check_time: isoA });
    render(<StatusBar />);
    // 精确匹配完整文案，同用例 1：子串匹配挡不住尾部追加型回归
    await waitFor(() => {
      expect(screen.getByText(`上次检查: ${expectedA}`)).toBeInTheDocument();
    });

    vi.mocked(getAppState).mockResolvedValue({ last_check_time: isoB });
    await act(async () => {
      listeners.get("scheduler-check-complete")?.({});
    });

    await waitFor(() => {
      expect(screen.getByText(`上次检查: ${expectedB}`)).toBeInTheDocument();
    });
    expect(screen.queryByText(`上次检查: ${expectedA}`)).not.toBeInTheDocument();
  });

  it("completedCount prop 正确渲染", async () => {
    // 重要：已完成数由父组件派生传入，本组件只负责显示
    vi.mocked(getAppState).mockResolvedValue({ last_check_time: null });

    const { unmount } = render(<StatusBar completedCount={3} />);
    expect(screen.getByText("已完成: 3 个视频")).toBeInTheDocument();
    // 等待挂载时那次异步拉取落定，避免 act 警告
    await waitFor(() => {
      expect(screen.getByText("上次检查: 从未")).toBeInTheDocument();
    });
    unmount();

    // 不传时默认 0
    render(<StatusBar />);
    expect(screen.getByText("已完成: 0 个视频")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("上次检查: 从未")).toBeInTheDocument();
    });
  });

  it("订阅 scheduler-check-complete 事件", async () => {
    // 重要：防事件名拼错——拼错则界面静默地永不刷新
    vi.mocked(getAppState).mockResolvedValue({ last_check_time: null });

    render(<StatusBar />);
    await waitFor(() => {
      expect(listeners.has("scheduler-check-complete")).toBe(true);
    });
  });
});
