import { Box, Typography, Avatar, Chip, Alert } from "@mui/material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask } from "@/types";
import DownloadRecordList from "./DownloadRecordList";

interface DetailPanelProps {
  subscription: Subscription | null;
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  error?: string | null;
  progressMap?: Map<string, DownloadProgress>;
  onPauseDownload: (videoUrl: string) => void;
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
          progressMap={progressMap}
          onPause={onPauseDownload}
          onCancel={onCancelDownload}
          onRetry={onRetryDownload}
        />
      </div>
    </div>
  );
}
