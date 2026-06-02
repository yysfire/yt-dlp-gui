import { Box, Typography, Avatar, Chip, Alert } from "@mui/material";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask } from "@/types";
import DownloadRecordList from "./DownloadRecordList";

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

interface DetailPanelProps {
  subscription: Subscription | null;
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  error?: string | null;
  progressMap?: Map<string, DownloadProgress>;
  onPauseDownload: (videoUrl: string) => void;
  onResumeDownload: (taskId: string) => void;
  onCancelDownload: (videoUrl: string) => void;
  onRetryDownload: (subscriptionId: string) => void;
}

/** Right-side detail panel showing channel info and download records. */
export default function DetailPanel({
  subscription,
  records,
  queueTasks,
  error,
  progressMap,
  onPauseDownload,
  onResumeDownload,
  onCancelDownload,
  onRetryDownload,
}: DetailPanelProps) {
  if (!subscription) {
    return (
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
          color: "text.disabled",
        }}
      >
        <Typography variant="body2" color="text.secondary">
          选择左侧订阅查看详情
        </Typography>
      </Box>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Channel Header */}
      <Box
        sx={{
          p: 3,
          display: "flex",
          alignItems: "center",
          gap: 2,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        <Avatar
          src={subscription.channel_avatar_url}
          sx={{ width: 56, height: 56 }}
        >
          {subscription.channel_name.charAt(0)}
        </Avatar>
        <div className="flex-1 min-w-0">
          <Typography variant="h6" fontWeight={600} noWrap>
            {subscription.channel_name}
          </Typography>
          <div className="flex items-center gap-2 mt-0.5">
            <Chip
              label={subscription.platform}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            <Chip
              label={subscription.quality_preset}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            {subscription.paused && (
              <Chip
                label="已暂停"
                size="small"
                color="warning"
                variant="outlined"
                sx={{ height: 20, fontSize: "0.7rem" }}
              />
            )}
          </div>
          {/* Per-subscription stats */}
          <div className="flex items-center gap-2 mt-1">
            <Typography variant="caption" color="text.secondary">
              已下载: {subscription.download_count} 个
            </Typography>
            {subscription.last_checked_at && (
              <Typography variant="caption" color="text.disabled">
                · 上次检查: {formatTime(subscription.last_checked_at)}
              </Typography>
            )}
            {subscription.last_check_status === "success" && (
              <SuccessIcon sx={{ fontSize: 14, color: "success.main" }} />
            )}
            {subscription.last_check_status === "failed" && (
              <ErrorIcon sx={{ fontSize: 14, color: "error.main" }} />
            )}
          </div>
          {subscription.last_check_status === "failed" && subscription.last_check_error && (
            <Typography variant="caption" color="error.main" component="div" className="mt-0.5">
              错误: {subscription.last_check_error}
            </Typography>
          )}
          <Typography
            variant="caption"
            color="text.disabled"
            sx={{ mt: 0.5, display: "block" }}
            noWrap
          >
            {subscription.url}
          </Typography>
        </div>
      </Box>

      {/* Download Records */}
      <div className="flex-1 overflow-y-auto">
        {error && (
          <Alert severity="error" sx={{ m: 2 }} variant="outlined">
            {error}
          </Alert>
        )}
        <DownloadRecordList
          records={records}
          queueTasks={queueTasks}
          subscriptionId={subscription.id}
          progressMap={progressMap}
          onPause={onPauseDownload}
          onResume={onResumeDownload}
          onCancel={onCancelDownload}
          onRetry={onRetryDownload}
        />
      </div>
    </div>
  );
}
