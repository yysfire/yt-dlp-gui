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
  Downloading as DownloadingIcon,
  Pause as PausedIcon,
  HourglassEmpty as WaitingIcon,
  Cancel as CancelledIcon,
  Autorenew as RetryingIcon,
  FileDownload as RedownloadIcon,
} from "@mui/icons-material";
import type { SvgIconComponent } from "@mui/icons-material";
import type { DownloadRecord } from "@/types";
import * as api from "@/lib/tauri";

interface DownloadedItemProps {
  record: DownloadRecord;
  fileMissing?: boolean;
  /** 按订阅当前画质重下这一条记录（「文件缺失」与「已删除」共用） */
  onRedownload?: (recordId: string) => void;
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
 * 把记录状态映射为「图标 + 颜色」。
 *
 * 用 `switch` 并让每个 case 都 return、不写 `default`，以借助 TypeScript 的
 * 穷尽性检查：将来 `DownloadRecord["status"]` 新增成员时，此处会编译报错，
 * 强制作者决定它该显示什么，而不是悄悄落进兜底分支。
 *
 * `fileMissing` 只在 `completed` 分支生效 —— 其余状态下文件本就不该存在，
 * 且后端同步只检查 `completed && file_path 非空` 的记录。
 *
 * 导出该函数是为了让视觉映射规则可在纯函数层被直接测试：`color` 返回的是
 * 语义键字面量而非主题解析值，直接断言它不涉及主题解析，因此不脆。
 */
export function resolveStatusVisual(
  status: DownloadRecord["status"],
  fileMissing: boolean,
): { Icon: SvgIconComponent; color: string } {
  switch (status) {
    case "deleted":
      return { Icon: DeletedIcon, color: "text.disabled" };
    case "completed":
      return fileMissing
        ? { Icon: MissingIcon, color: "warning.main" }
        : { Icon: CompletedIcon, color: "success.main" };
    case "failed":
      return { Icon: FailedIcon, color: "error.main" };
    case "downloading":
      return { Icon: DownloadingIcon, color: "info.main" };
    case "paused":
      return { Icon: PausedIcon, color: "warning.main" };
    case "waiting":
      return { Icon: WaitingIcon, color: "text.disabled" };
    case "cancelled":
      return { Icon: CancelledIcon, color: "text.disabled" };
    case "retrying":
      return { Icon: RetryingIcon, color: "warning.main" };
    default: {
      // 新增状态时此处编译报错，强制登记图标与颜色
      const exhaustive: never = status;
      return exhaustive;
    }
  }
}

/**
 * A single download record row in the downloaded videos list.
 * Shows title, status, file size, download time, and action buttons.
 */
export default function DownloadedItem({
  record,
  fileMissing = false,
  onRedownload,
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

  const { Icon: StatusIcon, color: statusColor } = resolveStatusVisual(
    record.status,
    fileMissing,
  );

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
                <Chip label="文件缺失" size="small" color="warning" sx={{ height: 18, fontSize: "0.6rem" }} />
              )}
            </Stack>
          }
          sx={{ my: 0 }}
        />

        {/* 「文件缺失」与「已删除」共用同一个「重新下载」入口 */}
        {(isDeleted || fileMissing) && onRedownload && (
          <Tooltip title="重新下载">
            <IconButton size="small" onClick={() => onRedownload(record.id)}>
              <RedownloadIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        )}

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
