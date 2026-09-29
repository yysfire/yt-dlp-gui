import { useState, useEffect, useCallback } from "react";
import { Box, Typography } from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import * as api from "@/lib/tauri";
import type { AppState } from "@/types";

/**
 * Bottom status bar showing last check time, completed download count, and scheduler status.
 *
 * 已完成数由父组件从下载记录派生后传入（单一真相源是下载记录，后端不再维护计数字段）；
 * 本组件只负责从后端取 `last_check_time`。
 */
export default function StatusBar({
  refreshTrigger,
  completedCount = 0,
}: {
  refreshTrigger?: string | null;
  completedCount?: number;
}) {
  const [state, setState] = useState<AppState>({
    last_check_time: null,
  });

  const refresh = useCallback(async () => {
    try {
      const s = await api.getAppState();
      setState(s);
    } catch {
      // Silently ignore — status bar is informational only
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, refreshTrigger]);

  useEffect(() => {
    // 周期性兜底刷新
    const interval = setInterval(refresh, 60_000);
    // 任一次检查完成时立即刷新 —— last_check_time 只在那里被写入
    // （records-changed 与本组件无关：它的 emit 点无一写 last_check_time）
    const unlistenPromise = listen("scheduler-check-complete", () => { refresh(); });
    return () => {
      clearInterval(interval);
      unlistenPromise.then((fn) => fn());
    };
  }, [refresh]);

  const formatTime = (iso: string | null): string => {
    if (!iso) return "从未";
    try {
      const d = new Date(iso);
      return d.toLocaleString("zh-CN");
    } catch {
      return iso;
    }
  };

  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        px: 2,
        py: 0.5,
        borderTop: 1,
        borderColor: "divider",
        backgroundColor: "background.default",
      }}
    >
      <Typography variant="caption" color="text.secondary">
        上次检查: {formatTime(state.last_check_time)}
      </Typography>
      <Typography variant="caption" color="text.secondary">
        已完成: {completedCount} 个视频
      </Typography>
    </Box>
  );
}
