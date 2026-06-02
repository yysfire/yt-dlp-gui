import { useState, useEffect, useCallback } from "react";
import { Box, Typography } from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import * as api from "@/lib/tauri";
import type { AppState } from "@/types";

/** Bottom status bar showing last check time, download count, and scheduler status. */
export default function StatusBar({ refreshTrigger }: { refreshTrigger?: string | null }) {
  const [state, setState] = useState<AppState>({
    last_check_time: null,
    total_downloads: 0,
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
    // Periodic refresh as safety net
    const interval = setInterval(refresh, 60_000);
    // Instant refresh on record changes and scheduler completion
    const unlistenRecordsPromise = listen("records-changed", () => { refresh(); });
    const unlistenSchedulerPromise = listen("scheduler-check-complete", () => { refresh(); });
    return () => {
      clearInterval(interval);
      unlistenRecordsPromise.then((fn) => fn());
      unlistenSchedulerPromise.then((fn) => fn());
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
        已下载: {state.total_downloads} 个视频
      </Typography>
    </Box>
  );
}
