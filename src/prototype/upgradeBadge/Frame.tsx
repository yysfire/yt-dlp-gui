/** 原型（一次性）：页面外壳 + 共用小件。贴近 DetailPanel 的头部密度，便于判断徽标是否显得拥挤。 */
import { useEffect, useState } from "react";
import { Avatar, Box, Chip, List, Typography } from "@mui/material";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
  PlayCircleOutline as VideoIcon,
  Pause as PauseIcon,
  Cancel as CancelIcon,
  Downloading as DownloadingIcon,
  Replay as ReplayIcon,
  HourglassEmpty as WaitingIcon,
  DeleteOutline as DeletedIcon,
  Autorenew as RetryingIcon,
} from "@mui/icons-material";
import type { ProtoRow } from "./mockData";

export const CHANNEL = {
  name: "技术与生活 Tech & Life",
  avatar: "https://i.pravatar.cc/112?img=12",
  platform: "youtube",
  preset: "1080p",
  completed: 5,
  subscribers: "12.4K",
  videos: 318,
  lastChecked: "2026-10-01 15:20",
};

export function ProtoFrame({ children, note }: { children: React.ReactNode; note: string }) {
  return (
    <div className="flex flex-col min-h-screen">
      <Box sx={{ p: 3, display: "flex", alignItems: "center", gap: 2, borderBottom: 1, borderColor: "divider" }}>
        <Avatar src={CHANNEL.avatar} sx={{ width: 56, height: 56 }}>
          {CHANNEL.name.charAt(0)}
        </Avatar>
        <div className="flex-1 min-w-0">
          <Typography variant="h6" fontWeight={600} noWrap>
            {CHANNEL.name}
          </Typography>
          <div className="flex items-center gap-2 mt-0.5">
            <Chip label={CHANNEL.platform} size="small" variant="outlined" sx={{ height: 20, fontSize: "0.7rem" }} />
            <Chip label={CHANNEL.preset} size="small" variant="outlined" sx={{ height: 20, fontSize: "0.7rem" }} />
          </div>
          <div className="flex items-center gap-2 mt-1">
            <Typography variant="caption" color="text.secondary">
              已完成: {CHANNEL.completed} 个
            </Typography>
            <Typography variant="caption" color="text.secondary">
              · 订阅者: {CHANNEL.subscribers}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              · 视频: {CHANNEL.videos}
            </Typography>
            <Typography variant="caption" color="text.disabled">
              · 上次检查: {CHANNEL.lastChecked}
            </Typography>
          </div>
        </div>
      </Box>

      <Box sx={{ px: 2, py: 1, bgcolor: "action.hover" }}>
        <Typography variant="caption" color="text.secondary">
          这个变体在回答：{note}
        </Typography>
      </Box>

      {children}
    </div>
  );
}

export function ProtoList({ children }: { children: React.ReactNode }) {
  return (
    <List dense disablePadding sx={{ borderTop: 1, borderColor: "divider" }}>
      {children}
    </List>
  );
}

export function StatusIcon({ row }: { row: ProtoRow }) {
  const sx = { fontSize: 18 };
  switch (row.status) {
    case "new":
      return <VideoIcon sx={{ ...sx, color: "text.disabled" }} />;
    case "downloading":
      return <DownloadingIcon sx={{ ...sx, color: "info.main" }} />;
    case "paused":
      return <PauseIcon sx={{ ...sx, color: "warning.main" }} />;
    case "waiting":
      return <WaitingIcon sx={{ ...sx, color: "text.disabled" }} />;
    case "completed":
      return <SuccessIcon sx={{ ...sx, color: "success.main" }} />;
    case "failed":
      return <ErrorIcon sx={{ ...sx, color: "error.main" }} />;
    case "cancelled":
      return <CancelIcon sx={{ ...sx, color: "text.disabled" }} />;
    case "deleted":
      return <DeletedIcon sx={{ ...sx, color: "text.disabled" }} />;
    case "retrying":
      return <RetryingIcon sx={{ ...sx, color: "warning.main" }} />;
  }
}

export function statusText(row: ProtoRow) {
  switch (row.status) {
    case "new":
      return { text: row.meta, color: "text.secondary" };
    case "downloading":
      return { text: `下载中 · ${row.meta}`, color: "text.secondary" };
    case "completed":
      return { text: `已完成 · ${row.meta}`, color: "success.main" };
    case "failed":
      return { text: `失败 · ${row.meta}${row.errorMessage ? ` · ${row.errorMessage}` : ""}`, color: "error.main" };
    case "cancelled":
      return { text: `已取消 · ${row.meta}`, color: "text.disabled" };
    case "deleted":
      return { text: `已删除 · ${row.meta}`, color: "text.disabled" };
    case "retrying":
      return { text: `重试中 (${row.retryCount}/3)`, color: "warning.main" };
  }
}

/** 活的倒数，证明「还有 N 秒」是能跟得上的。 */
export function RetryCountdown({ initial }: { initial: number }) {
  const [sec, setSec] = useState(initial);
  useEffect(() => {
    const t = setInterval(() => setSec((s) => (s > 0 ? s - 1 : initial)), 1000);
    return () => clearInterval(t);
  }, [initial]);
  return <>{sec} 秒</>;
}

export function qualityLabel(preset?: string) {
  if (!preset) return "未知";
  return preset === "best" ? "最高画质" : preset;
}
