import { useState, useMemo } from "react";
import {
  List,
  Typography,
  Box,
  CircularProgress,
  Alert,
  IconButton,
  Tooltip,
  FormControl,
  Select,
  MenuItem,
} from "@mui/material";
import {
  Refresh as RefreshIcon,
  DownloadForOffline as CheckAllIcon,
  FileDownload as ExportIcon,
  FileUpload as ImportIcon,
} from "@mui/icons-material";
import type { Subscription } from "@/types";
import { GROUPS } from "@/types";
import SubscriptionItem from "./SubscriptionItem";

const ALL_GROUPS = ["全部", ...GROUPS];

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
  onUpdateGroup: (id: string, groupName: string) => Promise<void>;
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
  onUpdateGroup,
}: SubscriptionListProps) {
  const [groupFilter, setGroupFilter] = useState("全部");

  const filteredSubscriptions = useMemo(() => {
    if (groupFilter === "全部") return subscriptions;
    if (groupFilter === "未分组") return subscriptions.filter((s) => s.group_name === "未分组");
    return subscriptions.filter((s) => s.group_name === groupFilter);
  }, [subscriptions, groupFilter]);

  return (
    <div className="flex flex-col h-full bg-white dark:bg-gray-800">
      {/* Toolbar */}
      <Box
        sx={{
          px: 1.5,
          py: 0.5,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        {/* Title row */}
        <Typography variant="body2" fontWeight={600} textAlign="center">
          订阅列表
        </Typography>
        {/* Controls row */}
        <Box sx={{ display: "flex", alignItems: "center", justifyContent: "center", mt: 0.5 }}>
          <FormControl size="small" sx={{ minWidth: 80, mr: 0.5 }}>
            <Select
              value={groupFilter}
              onChange={(e) => setGroupFilter(e.target.value)}
              displayEmpty
              sx={{ fontSize: "0.75rem" }}
            >
              {ALL_GROUPS.map((g) => (
                <MenuItem key={g} value={g} dense sx={{ fontSize: "0.75rem" }}>
                  {g}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
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

        {!loading && !error && filteredSubscriptions.length === 0 && (
          <Box sx={{ p: 3, textAlign: "center" }}>
            <Typography variant="body2" color="text.secondary">
              暂无订阅
            </Typography>
            <Typography variant="caption" color="text.disabled">
              点击右上角 + 添加频道
            </Typography>
          </Box>
        )}

        {!loading && filteredSubscriptions.length > 0 && (
          <List disablePadding dense>
            {filteredSubscriptions.map((sub) => (
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
                onUpdateGroup={onUpdateGroup}
              />
            ))}
          </List>
        )}
      </div>
    </div>
  );
}
