import { useState, useEffect } from "react";
import { Box, Typography } from "@mui/material";
import * as api from "@/lib/tauri";
import type { AppState } from "@/types";

/** Bottom status bar showing last check time, download count, and scheduler status. */
export default function StatusBar() {
  const [state, setState] = useState<AppState>({
    last_check_time: null,
    total_downloads: 0,
  });

  useEffect(() => {
    const load = async () => {
      try {
        const s = await api.getAppState();
        setState(s);
      } catch {
        // Silently ignore — status bar is informational only
      }
    };
    load();
    // Refresh every 30 seconds
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
  }, []);

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
