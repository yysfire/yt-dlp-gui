import {
  List,
  Typography,
  Box,
  CircularProgress,
  Alert,
  IconButton,
  Tooltip,
} from "@mui/material";
import {
  Refresh as RefreshIcon,
  DownloadForOffline as CheckAllIcon,
  FileDownload as ExportIcon,
  FileUpload as ImportIcon,
} from "@mui/icons-material";
import type { Subscription } from "@/types";
import SubscriptionItem from "./SubscriptionItem";

interface SubscriptionListProps {
  subscriptions: Subscription[];
  loading: boolean;
  error: string | null;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onDelete: (id: string) => Promise<void>;
  onTogglePause: (id: string) => Promise<void>;
  onCheckSubscription: (id: string) => Promise<void>;
  onManualCheckAll: () => Promise<void>;
  onRefresh: () => Promise<void>;
  onOpenExport: () => void;
  onOpenImport: () => void;
}

/** Sidebar container rendering the subscription list with toolbar. */
export default function SubscriptionList({
  subscriptions,
  loading,
  error,
  selectedId,
  onSelect,
  onDelete,
  onTogglePause,
  onCheckSubscription,
  onManualCheckAll,
  onRefresh,
  onOpenExport,
  onOpenImport,
}: SubscriptionListProps) {
  return (
    <div className="flex flex-col h-full bg-white dark:bg-gray-800">
      {/* Toolbar */}
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          px: 1.5,
          py: 0.5,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        <Typography variant="body2" fontWeight={600} sx={{ flexGrow: 1 }}>
          订阅列表
        </Typography>
        <Tooltip title="导入">
          <IconButton size="small" onClick={onOpenImport}>
            <ImportIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="导出">
          <IconButton size="small" onClick={onOpenExport}>
            <ExportIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="检查全部">
          <IconButton size="small" onClick={onManualCheckAll}>
            <CheckAllIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="刷新">
          <IconButton size="small" onClick={onRefresh}>
            <RefreshIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {loading && (
          <Box sx={{ display: "flex", justifyContent: "center", py: 4 }}>
            <CircularProgress size={24} />
          </Box>
        )}

        {error && (
          <Alert severity="error" sx={{ m: 1 }} variant="outlined">
            {error}
          </Alert>
        )}

        {!loading && !error && subscriptions.length === 0 && (
          <Box sx={{ p: 3, textAlign: "center" }}>
            <Typography variant="body2" color="text.secondary">
              暂无订阅
            </Typography>
            <Typography variant="caption" color="text.disabled">
              点击右上角 + 添加频道
            </Typography>
          </Box>
        )}

        {!loading && subscriptions.length > 0 && (
          <List disablePadding dense>
            {subscriptions.map((sub) => (
              <SubscriptionItem
                key={sub.id}
                subscription={sub}
                selected={selectedId === sub.id}
                onSelect={() =>
                  onSelect(selectedId === sub.id ? null : sub.id)
                }
                onDelete={() => onDelete(sub.id)}
                onTogglePause={() => onTogglePause(sub.id)}
                onCheck={() => onCheckSubscription(sub.id)}
              />
            ))}
          </List>
        )}
      </div>
    </div>
  );
}
