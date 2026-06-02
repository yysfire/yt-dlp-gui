import {
  ListItem,
  ListItemIcon,
  ListItemText,
  Typography,
  IconButton,
  Box,
} from "@mui/material";
import {
  CheckCircle as CompletedIcon,
  Downloading as DownloadingIcon,
  Error as FailedIcon,
  InsertDriveFile as FileIcon,
  Pause as PauseIcon,
  Replay as ReplayIcon,
  Cancel as CancelIcon,
  PlayArrow as ResumeIcon,
  HourglassEmpty as WaitingIcon,
} from "@mui/icons-material";
import type { DownloadRecord, DownloadProgress } from "@/types";
import DownloadProgressBar from "./DownloadProgressBar";

interface DownloadRecordItemProps {
  record: DownloadRecord & { _isQueueTask?: boolean; _taskId?: string };
  progress?: DownloadProgress | null;
  onPause?: () => void;
  onResume?: () => void;
  onCancel?: () => void;
  onRetry?: () => void;
  isQueueTask?: boolean;
}

/** Formats a file size in bytes to a human-readable string. */
function formatFileSize(bytes: number): string {
  if (bytes === 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

/** Formats an ISO date string to a short locale string. */
function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

/** Single download record row with status icon, progress, and action buttons. */
export default function DownloadRecordItem({
  record,
  progress,
  onPause,
  onResume,
  onCancel,
  onRetry,
  isQueueTask,
}: DownloadRecordItemProps) {
  const statusIcon = () => {
    switch (record.status) {
      case "completed":
        return <CompletedIcon fontSize="small" color="success" />;
      case "downloading":
        return <DownloadingIcon fontSize="small" color="info" />;
      case "paused":
        return <PauseIcon fontSize="small" color="warning" />;
      case "failed":
        return <FailedIcon fontSize="small" color="error" />;
      case "cancelled":
        return <CancelIcon fontSize="small" />;
      case "waiting":
        return <WaitingIcon fontSize="small" color="disabled" />;
      default:
        return isQueueTask ? <WaitingIcon fontSize="small" color="disabled" /> : <FileIcon fontSize="small" />;
    }
  };

  const statusColor = () => {
    switch (record.status) {
      case "completed": return "success.main";
      case "downloading": return "info.main";
      case "paused": return "warning.main";
      case "failed": return "error.main";
      case "cancelled": return "text.disabled";
      case "waiting": return "text.disabled";
      default: return "text.secondary";
    }
  };

  const statusLabel = () => {
    switch (record.status) {
      case "completed": return "已完成";
      case "downloading": return isQueueTask ? "下载中" : "下载中";
      case "paused": return "已暂停";
      case "failed": return "失败";
      case "cancelled": return "已取消";
      case "waiting": return "等待中";
      default: return record.status;
    }
  };

  return (
    <ListItem
      sx={{
        borderBottom: 1,
        borderColor: "divider",
        "&:last-child": { borderBottom: 0 },
      }}
    >
      <ListItemIcon sx={{ minWidth: 36 }}>{statusIcon()}</ListItemIcon>
      <ListItemText
        primary={
          <Typography variant="body2" noWrap>
            {record.video_title}
          </Typography>
        }
        secondary={
          <Box component="span" sx={{ display: "flex", alignItems: "center", gap: 1 }}>
            <Typography variant="caption" color={statusColor()}>
              {statusLabel()}
            </Typography>
            {record.file_size > 0 && (
              <Typography variant="caption" color="text.disabled">
                {formatFileSize(record.file_size)}
              </Typography>
            )}
            <Typography variant="caption" color="text.disabled">
              {formatDate(record.downloaded_at)}
            </Typography>
          </Box>
        }
        sx={{ my: 0 }}
      />
      {(record.status === "downloading" || record.status === "paused") && progress && (
        <DownloadProgressBar progress={progress} status={record.status} />
      )}
      <Box sx={{ display: "flex", gap: 0.5, ml: 1, flexShrink: 0 }}>
        {record.status === "downloading" && onPause && (
          <IconButton size="small" onClick={onPause} title="暂停">
            <PauseIcon fontSize="small" />
          </IconButton>
        )}
        {record.status === "paused" && onResume && (
          <IconButton size="small" onClick={onResume} title="继续">
            <ResumeIcon fontSize="small" />
          </IconButton>
        )}
        {(record.status === "downloading" || record.status === "paused" || isQueueTask) && onCancel && (
          <IconButton size="small" onClick={onCancel} title="取消">
            <CancelIcon fontSize="small" color="error" />
          </IconButton>
        )}
        {record.status === "failed" && onRetry && (
          <IconButton size="small" onClick={onRetry} title="重试">
            <ReplayIcon fontSize="small" color="warning" />
          </IconButton>
        )}
      </Box>
    </ListItem>
  );
}
