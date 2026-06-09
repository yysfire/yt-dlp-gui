import { useState, useMemo } from "react";
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  LinearProgress,
  CircularProgress,
  Chip,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  Typography,
  Box,
  Alert,
  FormControlLabel,
  Checkbox,
} from "@mui/material";
import {
  CheckCircle as OkIcon,
  Warning as WarningIcon,
  Error as DeadIcon,
  Delete as DeleteIcon,
  PlayArrow as PlayIcon,
} from "@mui/icons-material";
import type { Subscription, HealthCheckSummary, HealthCheckResult, HealthStatus } from "@/types";
import * as api from "@/lib/tauri";

interface HealthCheckPanelProps {
  open: boolean;
  onClose: () => void;
  subscriptions: Subscription[];
  isChecking: boolean;
  progress: { completed: number; total: number } | null;
  summary: HealthCheckSummary | null;
  onCheckAll: () => Promise<void>;
  onCheckSelected: (ids: string[]) => Promise<void>;
  onClearResults: () => void;
  onRefreshSubscriptions: () => void;
}

const statusConfig: Record<
  HealthStatus,
  { label: string; color: "success" | "warning" | "error"; icon: typeof OkIcon }
> = {
  ok: { label: "正常", color: "success", icon: OkIcon },
  warning: { label: "警告", color: "warning", icon: WarningIcon },
  dead: { label: "失效", color: "error", icon: DeadIcon },
};

/** 健康检查结果行 */
function ResultItem({ result, name }: { result: HealthCheckResult; name: string }) {
  const config = statusConfig[result.status];
  const Icon = config.icon;

  return (
    <ListItem dense>
      <ListItemIcon sx={{ minWidth: 32 }}>
        <Icon color={config.color} fontSize="small" />
      </ListItemIcon>
      <ListItemText
        primary={name || result.url}
        secondary={
          <Typography variant="caption" color="text.secondary">
            {result.detail}
            {result.latency_ms > 0 && ` • ${result.latency_ms}ms`}
          </Typography>
        }
        primaryTypographyProps={{
          variant: "body2",
          noWrap: true,
          sx: { maxWidth: 320 },
        }}
      />
      <Chip label={config.label} color={config.color} size="small" variant="outlined" />
    </ListItem>
  );
}

/**
 * Health check panel: run batch health checks on subscriptions,
 * view results with status breakdown, and batch delete dead subscriptions.
 */
export default function HealthCheckPanel({
  open,
  onClose,
  subscriptions,
  isChecking,
  progress,
  summary,
  onCheckAll,
  onCheckSelected,
  onClearResults,
  onRefreshSubscriptions,
}: HealthCheckPanelProps) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [selectMode, setSelectMode] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const deadResults = useMemo(
    () => summary?.results.filter((r) => r.status === "dead") ?? [],
    [summary],
  );

  const handleToggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleSelectAll = () => {
    if (selectedIds.size === subscriptions.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(subscriptions.map((s) => s.id)));
    }
  };

  const handleCheckSelected = () => {
    if (selectedIds.size > 0) {
      onCheckSelected(Array.from(selectedIds));
      setSelectMode(false);
    }
  };

  const handleBatchDeleteDead = async () => {
    if (deadResults.length === 0) return;
    setDeleting(true);
    try {
      const ids = deadResults.map((r) => r.subscription_id);
      await api.batchDeleteSubscriptions(ids);
      onRefreshSubscriptions();
      onClearResults();
    } catch (e) {
      console.error("批量删除失效订阅失败:", e);
    } finally {
      setDeleting(false);
    }
  };

  const handleClose = () => {
    if (!isChecking && !deleting) {
      setSelectedIds(new Set());
      setSelectMode(false);
      onClose();
    }
  };

  return (
    <Dialog
      open={open}
      onClose={handleClose}
      maxWidth="sm"
      fullWidth
      disableRestoreFocus
    >
      <DialogTitle sx={{ fontWeight: 600, fontSize: "1.1rem" }}>
        订阅健康检查
      </DialogTitle>

      <DialogContent>
        {/* 操作按钮区 */}
        <Box sx={{ display: "flex", gap: 1, mb: 2, flexWrap: "wrap" }}>
          {!isChecking && !summary && !selectMode && (
            <>
              <Button
                variant="contained"
                size="small"
                onClick={() => {
                  onCheckAll();
                  setSelectMode(false);
                }}
              >
                全量检查
              </Button>
              <Button
                variant="outlined"
                size="small"
                onClick={() => setSelectMode(true)}
              >
                选择性检查
              </Button>
            </>
          )}

          {selectMode && !isChecking && (
            <>
              <Button
                variant="contained"
                size="small"
                disabled={selectedIds.size === 0}
                onClick={handleCheckSelected}
                startIcon={<PlayIcon />}
              >
                检查已选 ({selectedIds.size})
              </Button>
              <Button
                variant="text"
                size="small"
                onClick={() => {
                  setSelectMode(false);
                  setSelectedIds(new Set());
                }}
              >
                取消选择
              </Button>
            </>
          )}

          {summary && !isChecking && (
            <Button
              variant="outlined"
              size="small"
              onClick={() => {
                onClearResults();
                setSelectMode(false);
              }}
            >
              重新检查
            </Button>
          )}
        </Box>

        {/* 进度条 */}
        {isChecking && progress && progress.total > 0 && (
          <Box sx={{ mb: 2 }}>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
              正在检查... ({progress.completed}/{progress.total})
            </Typography>
            <LinearProgress
              variant="determinate"
              value={(progress.completed / progress.total) * 100}
            />
          </Box>
        )}

        {isChecking && (!progress || progress.total === 0) && (
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, mb: 2 }}>
            <CircularProgress size={18} />
            <Typography variant="body2" color="text.secondary">
              正在启动健康检查...
            </Typography>
          </Box>
        )}

        {/* 摘要卡片 */}
        {summary && (
          <Box
            sx={{
              display: "flex",
              gap: 1.5,
              mb: 2,
              p: 1.5,
              bgcolor: "grey.50",
              borderRadius: 2,
              flexWrap: "wrap",
            }}
          >
            <Chip
              label={`总数 ${summary.total}`}
              color="default"
              variant="outlined"
              size="small"
            />
            <Chip
              icon={<OkIcon />}
              label={`正常 ${summary.ok}`}
              color="success"
              size="small"
            />
            <Chip
              icon={<WarningIcon />}
              label={`警告 ${summary.warning}`}
              color="warning"
              size="small"
            />
            <Chip
              icon={<DeadIcon />}
              label={`失效 ${summary.dead}`}
              color="error"
              size="small"
            />
            {summary.duration_ms > 0 && (
              <Typography
                variant="caption"
                color="text.secondary"
                sx={{ alignSelf: "center", ml: "auto" }}
              >
                耗时 {(summary.duration_ms / 1000).toFixed(1)}s
              </Typography>
            )}
          </Box>
        )}

        {/* 选择模式：显示订阅列表 */}
        {selectMode && !isChecking && (
          <Box
            sx={{
              maxHeight: 320,
              overflow: "auto",
              border: "1px solid",
              borderColor: "divider",
              borderRadius: 1,
            }}
          >
            <Box sx={{ px: 1, pt: 0.5 }}>
              <FormControlLabel
                control={
                  <Checkbox
                    size="small"
                    checked={selectedIds.size === subscriptions.length && subscriptions.length > 0}
                    indeterminate={selectedIds.size > 0 && selectedIds.size < subscriptions.length}
                    onChange={handleSelectAll}
                  />
                }
                label={
                  <Typography variant="caption">
                    全选 ({selectedIds.size}/{subscriptions.length})
                  </Typography>
                }
              />
            </Box>
            <List dense>
              {subscriptions.map((sub) => (
                <ListItem
                  key={sub.id}
                  dense
                  sx={{ cursor: "pointer" }}
                  onClick={() => handleToggleSelect(sub.id)}
                >
                  <Checkbox
                    size="small"
                    checked={selectedIds.has(sub.id)}
                    edge="start"
                    tabIndex={-1}
                    disableRipple
                  />
                  <ListItemText
                    primary={sub.channel_name}
                    secondary={sub.url}
                    primaryTypographyProps={{ variant: "body2", noWrap: true }}
                    secondaryTypographyProps={{ variant: "caption", noWrap: true }}
                  />
                </ListItem>
              ))}
            </List>
          </Box>
        )}

        {/* 结果列表 */}
        {summary && summary.results.length > 0 && (
          <Box
            sx={{
              maxHeight: 320,
              overflow: "auto",
              border: "1px solid",
              borderColor: "divider",
              borderRadius: 1,
            }}
          >
            <List dense>
              {summary.results.map((result) => {
                const sub = subscriptions.find(
                  (s) => s.id === result.subscription_id,
                );
                return (
                  <ResultItem
                    key={result.subscription_id}
                    result={result}
                    name={sub?.channel_name ?? ""}
                  />
                );
              })}
            </List>
          </Box>
        )}

        {summary && summary.results.length === 0 && (
          <Alert severity="info" variant="outlined">
            未找到订阅，请先添加订阅后再运行健康检查。
          </Alert>
        )}
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2 }}>
        {deadResults.length > 0 && !isChecking && (
          <Button
            color="error"
            size="small"
            startIcon={deleting ? <CircularProgress size={14} /> : <DeleteIcon />}
            disabled={deleting}
            onClick={handleBatchDeleteDead}
          >
            {deleting ? "删除中..." : `删除 ${deadResults.length} 个失效订阅`}
          </Button>
        )}
        <Box sx={{ flex: 1 }} />
        <Button
          onClick={handleClose}
          disabled={isChecking || deleting}
          size="small"
        >
          关闭
        </Button>
      </DialogActions>
    </Dialog>
  );
}
