import { useMemo } from "react";
import { List, Typography, Box } from "@mui/material";
import type { DownloadRecord, DownloadProgress, DownloadTask } from "@/types";
import DownloadRecordItem from "./DownloadRecordItem";

interface DownloadRecordListProps {
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  progressMap?: Map<string, DownloadProgress>;
  onPause: (videoUrl: string) => void;
  onCancel: (videoUrl: string) => void;
  onRetry: (subscriptionId: string) => void;
}

/** Status sort priority: active ones first */
function statusPriority(status: string): number {
  switch (status) {
    case "running": return 0;
    case "paused": return 1;
    case "waiting": return 2;
    case "downloading": return 3;
    case "failed": return 4;
    case "completed": return 5;
    case "cancelled": return 6;
    default: return 9;
  }
}

/** Merge queue tasks and persistent records, deduplicating by video_url. */
export default function DownloadRecordList({
  records,
  queueTasks,
  progressMap,
  onPause,
  onCancel,
  onRetry,
}: DownloadRecordListProps) {
  const merged = useMemo(() => {
    // Track seen URLs to avoid duplicates
    const seen = new Set<string>();
    const items: (DownloadRecord & { _isQueueTask?: boolean; _taskId?: string })[] = [];

    // Queue tasks first (more real-time)
    for (const task of queueTasks) {
      if (task.status === "completed" || task.status === "cancelled") continue;
      seen.add(task.video_url);
      items.push({
        id: task.id,
        video_id: task.video_id,
        video_title: task.video_title,
        video_url: task.video_url,
        subscription_id: task.subscription_id,
        file_path: "",
        file_size: 0,
        status: task.status === "failed" ? "failed" : task.status === "running" ? "downloading" : task.status === "paused" ? "paused" : "downloading",
        error_message: task.error_message,
        downloaded_at: task.created_at,
        _isQueueTask: true,
        _taskId: task.id,
      });
    }

    // Then persistent records (skip those already covered by queue tasks)
    for (const record of records) {
      if (seen.has(record.video_url)) continue;
      seen.add(record.video_url);
      items.push(record);
    }

    // Sort: active first, then by time descending
    items.sort((a, b) => {
      const pa = statusPriority(a.status);
      const pb = statusPriority(b.status);
      if (pa !== pb) return pa - pb;
      return new Date(b.downloaded_at).getTime() - new Date(a.downloaded_at).getTime();
    });

    return items;
  }, [records, queueTasks]);

  if (merged.length === 0) {
    return (
      <Box sx={{ p: 3, textAlign: "center" }}>
        <Typography variant="body2" color="text.secondary">
          暂无下载记录
        </Typography>
      </Box>
    );
  }

  return (
    <List disablePadding dense>
      {merged.map((item) => (
        <DownloadRecordItem
          key={item.id}
          record={item}
          progress={progressMap?.get(item.video_url) ?? null}
          onPause={item._isQueueTask ? () => onPause(item.video_url) : undefined}
          onCancel={item._isQueueTask ? () => onCancel(item.video_url) : undefined}
          onRetry={item.status === "failed" ? () => onRetry(item.subscription_id) : undefined}
          isQueueTask={item._isQueueTask}
        />
      ))}
    </List>
  );
}
