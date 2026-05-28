import {
  ListItem,
  ListItemIcon,
  ListItemText,
  Typography,
} from "@mui/material";
import {
  CheckCircle as CompletedIcon,
  Downloading as DownloadingIcon,
  Error as FailedIcon,
  InsertDriveFile as FileIcon,
} from "@mui/icons-material";
import type { DownloadRecord } from "@/types";

interface DownloadRecordItemProps {
  record: DownloadRecord;
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

/** Single download record row with status icon and metadata. */
export default function DownloadRecordItem({
  record,
}: DownloadRecordItemProps) {
  const statusIcon = () => {
    switch (record.status) {
      case "completed":
        return <CompletedIcon fontSize="small" color="success" />;
      case "downloading":
        return <DownloadingIcon fontSize="small" color="info" />;
      case "failed":
        return <FailedIcon fontSize="small" color="error" />;
      default:
        return <FileIcon fontSize="small" />;
    }
  };

  const statusColor = () => {
    switch (record.status) {
      case "completed":
        return "success.main";
      case "downloading":
        return "info.main";
      case "failed":
        return "error.main";
      default:
        return "text.secondary";
    }
  };

  const statusLabel = () => {
    switch (record.status) {
      case "completed":
        return "已完成";
      case "downloading":
        return "下载中";
      case "failed":
        return "失败";
      default:
        return record.status;
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
          <span className="flex items-center gap-2">
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
          </span>
        }
        sx={{ my: 0 }}
      />
    </ListItem>
  );
}
