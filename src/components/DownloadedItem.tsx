import { useState } from "react";
import {
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Typography,
  IconButton,
  Tooltip,
  Snackbar,
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Button,
  Chip,
  Stack,
} from "@mui/material";
import {
  FolderOpen as FolderOpenIcon,
  Delete as DeleteIcon,
  CheckCircle as CompletedIcon,
  Error as FailedIcon,
  DeleteOutline as DeletedIcon,
  Warning as MissingIcon,
} from "@mui/icons-material";
import type { DownloadRecord } from "@/types";
import * as api from "@/lib/tauri";

interface DownloadedItemProps {
  record: DownloadRecord;
  fileMissing?: boolean;
  onDeleted?: () => void;
}

/** Format file size bytes to human-readable string. */
function formatFileSize(bytes: number): string {
  if (bytes === 0) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes as number;
  let unitIdx = 0;
  while (size >= 1024 && unitIdx < units.length - 1) {
    size /= 1024;
    unitIdx++;
  }
  return `${size.toFixed(unitIdx === 0 ? 0 : 1)} ${units[unitIdx]}`;
}

/** Format ISO 8601 timestamp to short date string. */
function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    });
  } catch {
    return "-";
  }
}

/**
 * A single download record row in the downloaded videos list.
 * Shows title, status, file size, download time, and action buttons.
 */
export default function DownloadedItem({
  record,
  fileMissing = false,
  onDeleted,
}: DownloadedItemProps) {
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const isDeleted = record.status === "deleted";

  const handleOpenFolder = async () => {
    try {
      await api.openInFolder(record.file_path);
    } catch (e) {
      setToast(String(e));
    }
  };

  const handleDeleteConfirm = () => {
    setDeleteConfirmOpen(false);
    api.deleteFile(record.id)
      .then(() => onDeleted?.())
      .catch((e) => setToast(String(e)));
  };

  const statusColor = isDeleted
    ? "text.disabled"
    : record.status === "completed"
      ? "success.main"
      : record.status === "failed"
        ? "error.main"
        : fileMissing
          ? "warning.main"
          : "text.secondary";

  const StatusIcon = isDeleted
    ? DeletedIcon
    : record.status === "completed"
      ? (fileMissing ? MissingIcon : CompletedIcon)
      : record.status === "failed"
        ? FailedIcon
        : CompletedIcon;

  return (
    <>
      <ListItemButton
        sx={{
          opacity: isDeleted ? 0.5 : 1,
          textDecoration: isDeleted ? "line-through" : "none",
        }}
      >
        <ListItemIcon sx={{ minWidth: 36 }}>
          <StatusIcon fontSize="small" sx={{ color: statusColor }} />
        </ListItemIcon>

        <ListItemText
          primary={
            <Typography variant="body2" noWrap>
              {record.video_title}
            </Typography>
          }
          secondary={
            <Stack direction="row" spacing={1} alignItems="center">
              <Typography variant="caption" color="text.disabled">
                {formatDate(record.downloaded_at)}
              </Typography>
              {!isDeleted && (
                <Typography variant="caption" color="text.disabled">
                  {formatFileSize(record.file_size)}
                </Typography>
              )}
              {isDeleted && (
                <Chip label="已删除" size="small" sx={{ height: 18, fontSize: "0.6rem" }} />
              )}
              {fileMissing && !isDeleted && (
                <Chip label="文件已缺失" size="small" color="warning" sx={{ height: 18, fontSize: "0.6rem" }} />
              )}
            </Stack>
          }
          sx={{ my: 0 }}
        />

        {!isDeleted && (
          <>
            <Tooltip title="打开文件夹">
              <IconButton size="small" onClick={handleOpenFolder}>
                <FolderOpenIcon fontSize="small" />
              </IconButton>
            </Tooltip>
            <Tooltip title="删除文件">
              <IconButton size="small" onClick={() => setDeleteConfirmOpen(true)}>
                <DeleteIcon fontSize="small" />
              </IconButton>
            </Tooltip>
          </>
        )}
      </ListItemButton>

      {/* Delete confirmation dialog */}
      <Dialog open={deleteConfirmOpen} onClose={() => setDeleteConfirmOpen(false)}>
        <DialogTitle>确认删除</DialogTitle>
        <DialogContent>
          <DialogContentText>
            将删除文件"{record.video_title}"，但保留下载记录以便追溯历史。
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDeleteConfirmOpen(false)}>取消</Button>
          <Button onClick={handleDeleteConfirm} color="error">删除</Button>
        </DialogActions>
      </Dialog>

      {/* Toast */}
      <Snackbar
        open={!!toast}
        autoHideDuration={3000}
        onClose={() => setToast(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert severity="error" onClose={() => setToast(null)} sx={{ width: "100%" }}>
          {toast}
        </Alert>
      </Snackbar>
    </>
  );
}
