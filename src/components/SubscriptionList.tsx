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
  MonitorHeart as HealthCheckIcon,
} from "@mui/icons-material";
import type { Subscription, FilterState, SortState } from "@/types";
import SubscriptionItem from "./SubscriptionItem";
import FilterBar from "./FilterBar";

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
  onOpenHealthCheck: () => void;
  onUpdateGroup: (id: string, groupName: string) => Promise<void>;
  /** 筛选和排序状态（从 useFilter hook 输出） */
  filter: FilterState;
  onFilterChange: (partial: Partial<FilterState>) => void;
  sort: SortState;
  onSortChange: (sort: SortState) => void;
  filteredSubscriptions: Subscription[];
  filteredCount: number;
}

/** Sidebar container rendering the subscription list with toolbar and filter bar. */
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
  onOpenHealthCheck,
  onUpdateGroup,
  filter,
  onFilterChange,
  sort,
  onSortChange,
  filteredSubscriptions,
  filteredCount,
}: SubscriptionListProps) {
  return (
    <div className="flex flex-col h-full bg-white dark:bg-gray-800">
      {/* Title row */}
      <Box
        sx={{
          px: 1.5,
          py: 0.5,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        <Typography variant="body2" fontWeight={600} textAlign="center">
          订阅列表
        </Typography>
        {/* Controls row */}
        <Box sx={{ display: "flex", alignItems: "center", justifyContent: "center", mt: 0.5 }}>
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
          <Tooltip title="健康检查">
            <IconButton size="small" onClick={onOpenHealthCheck}>
              <HealthCheckIcon fontSize="small" />
            </IconButton>
          </Tooltip>
          <Tooltip title="刷新">
            <IconButton size="small" onClick={onRefresh}>
              <RefreshIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        </Box>
      </Box>

      {/* Filter bar */}
      <FilterBar
        filter={filter}
        onFilterChange={onFilterChange}
        sort={sort}
        onSortChange={onSortChange}
        subscriptions={subscriptions}
        count={filteredCount}
      />

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
              {subscriptions.length === 0 ? "暂无订阅" : "无匹配结果"}
            </Typography>
            <Typography variant="caption" color="text.disabled">
              {subscriptions.length === 0 ? "点击右上角 + 添加频道" : "调整筛选条件"}
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
                keyword={filter.keyword}
              />
            ))}
          </List>
        )}
      </div>
    </div>
  );
}
