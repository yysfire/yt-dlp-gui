import { useState, useEffect, useCallback } from "react";
import { Box, Typography } from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import * as api from "@/lib/tauri";
import type { AppState } from "@/types";

/**
 * Bottom status bar showing last check time and completed download count.
 *
 * 已完成数由父组件从下载记录派生后传入（单一真相源是下载记录，后端不再维护计数字段）；
 * 本组件只负责从后端取 `last_check_time`。
 */
export default function StatusBar({
  completedCount = 0,
}: {
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
    // 挂载时拉取一次初始值；此后的更新全部来自 scheduler-check-complete
    refresh();
  }, [refresh]);

  useEffect(() => {
    // 本组件唯一的实时刷新来源：last_check_time 只在两处被写入
    // （run_check_round 与 check_subscription），两处都会在落盘后 emit 本事件，
    // 已由后端契约（落盘后必 emit）保证，故不再需要轮询兜底。
    const unlistenPromise = listen("scheduler-check-complete", () => { refresh(); });
    return () => {
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
