import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Typography,
  IconButton,
  Chip,
  Divider,
} from "@mui/material";
import {
  Pause as PauseIcon,
  PlayArrow as ResumeIcon,
  Cancel as CancelIcon,
  Download as DownloadIcon,
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
} from "@mui/icons-material";
import { listen } from "@tauri-apps/api/event";
import type { DownloadTask, DownloadProgress, QueueState } from "@/types";
import * as api from "@/lib/tauri";
import DownloadProgressBar from "./DownloadProgressBar";

/** Status chip color mapping */
function statusColor(status: string): "default" | "info" | "warning" | "success" | "error" {
  switch (status) {
    case "running": return "info";
    case "paused": return "warning";
    case "completed": return "success";
    case "failed": return "error";
    case "cancelled": return "default";
    default: return "default";
  }
}

/** Status label in Chinese */
function statusLabel(status: string): string {
  switch (status) {
    case "waiting": return "等待中";
    case "running": return "下载中";
    case "paused": return "已暂停";
    case "completed": return "已完成";
    case "failed": return "失败";
    case "cancelled": return "已取消";
    default: return status;
  }
}

/**
 * Panel showing the current download queue with statuses,
 * progress bars, and pause/resume/cancel controls.
 */
export default function DownloadQueuePanel() {
  const [expanded, setExpanded] = useState(false);
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [state, setState] = useState<QueueState | null>(null);
  const [progressMap, setProgressMap] = useState<Map<string, DownloadProgress>>(new Map());

  const refresh = useCallback(async () => {
    try {
      const queueTasks = await api.getDownloadQueue();
      const queueState = await api.getQueueState();
      setTasks(queueTasks);
      setState(queueState);
    } catch {
      // Ignore errors on refresh
    }
  }, []);

  useEffect(() => {
    refresh();

    // Listen for queue-changed events
    const unlistenQueue = listen<QueueState>("queue-changed", (event) => {
      setState(event.payload);
      refresh();
    });

    // Listen for progress events
    const unlistenProgress = listen<DownloadProgress>("download-progress", (event) => {
      const progress = event.payload;
      setProgressMap((prev) => {
        const next = new Map(prev);
        next.set(progress.video_url, progress);
        return next;
      });
    });

    // Periodic refresh
    const interval = setInterval(refresh, 3000);

    return () => {
      clearInterval(interval);
      unlistenQueue.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, [refresh]);

  const handlePause = async (id: string) => {
    try {
      await api.pauseDownload(id);
      refresh();
    } catch (e) {
      console.error("Failed to pause:", e);
    }
  };

  const handleResume = async (id: string) => {
    try {
      await api.resumeDownload(id);
      refresh();
    } catch (e) {
      console.error("Failed to resume:", e);
    }
  };

  const handleCancel = async (id: string) => {
    try {
      await api.cancelDownload(id);
      refresh();
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  };

  const activeCount = state?.active_count ?? 0;
  const waitingCount = state?.waiting_count ?? 0;

  return (
    <Box
      sx={{
        borderTop: 1,
        borderColor: "divider",
        bgcolor: "background.paper",
      }}
    >
      {/* Header — clickable to expand */}
      <Box
        onClick={() => setExpanded((v) => !v)}
        sx={{
          display: "flex",
          alignItems: "center",
          px: 2,
          py: 1,
          cursor: "pointer",
          "&:hover": { bgcolor: "action.hover" },
        }}
      >
        {expanded ? <ExpandMoreIcon fontSize="small" /> : <ExpandLessIcon fontSize="small" />}
        <DownloadIcon fontSize="small" sx={{ ml: 1, mr: 0.5 }} />
        <Typography variant="body2" fontWeight={600}>
          下载队列
        </Typography>
        {tasks.length > 0 && (
          <Chip
            label={`${tasks.length} 个任务`}
            size="small"
            sx={{ ml: 1, height: 20 }}
          />
        )}
        <Box sx={{ flex: 1 }} />
        <Typography variant="caption" color="text.secondary">
          {activeCount > 0 ? `${activeCount} 下载中` : "空闲"}
          {waitingCount > 0 ? ` · ${waitingCount} 等待` : ""}
        </Typography>
      </Box>

      {/* Task list — shown when expanded */}
      {expanded && tasks.length > 0 && (
        <>
          <Divider />
          <Box sx={{ maxHeight: 200, overflow: "auto", px: 1, pb: 1 }}>
            {tasks.map((task) => {
              const progress = progressMap.get(task.video_url);
              return (
                <Box
                  key={task.id}
                  sx={{
                    display: "flex",
                    alignItems: "center",
                    gap: 1,
                    py: 0.75,
                    px: 1,
                    borderRadius: 1,
                    "&:hover": { bgcolor: "action.hover" },
                  }}
                >
                  {/* Status chip */}
                  <Chip
                    label={statusLabel(task.status)}
                    size="small"
                    color={statusColor(task.status)}
                    variant={task.status === "running" ? "filled" : "outlined"}
                    sx={{ minWidth: 60, height: 20, fontSize: "0.7rem" }}
                  />

                  {/* Title */}
                  <Typography
                    variant="body2"
                    noWrap
                    sx={{ flex: 1, minWidth: 0 }}
                  >
                    {task.video_title}
                  </Typography>

                  {/* Progress bar for running tasks */}
                  {task.status === "running" && (
                    <Box sx={{ flex: 1, minWidth: 80 }}>
                      <DownloadProgressBar
                        progress={progress ?? null}
                        status={task.status}
                      />
                    </Box>
                  )}

                  {/* Action buttons */}
                  <Box sx={{ display: "flex", gap: 0.5 }}>
                    {task.status === "running" && (
                      <IconButton
                        size="small"
                        onClick={() => handlePause(task.id)}
                        title="暂停"
                      >
                        <PauseIcon fontSize="small" />
                      </IconButton>
                    )}
                    {task.status === "paused" && (
                      <IconButton
                        size="small"
                        onClick={() => handleResume(task.id)}
                        title="继续"
                      >
                        <ResumeIcon fontSize="small" />
                      </IconButton>
                    )}
                    {(task.status === "running" ||
                      task.status === "paused" ||
                      task.status === "waiting") && (
                      <IconButton
                        size="small"
                        onClick={() => handleCancel(task.id)}
                        title="取消"
                      >
                        <CancelIcon fontSize="small" color="error" />
                      </IconButton>
                    )}
                  </Box>
                </Box>
              );
            })}
          </Box>
        </>
      )}
    </Box>
  );
}
