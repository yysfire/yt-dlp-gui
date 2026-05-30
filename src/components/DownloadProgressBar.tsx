import { Box, Typography, LinearProgress } from "@mui/material";
import type { DownloadProgress } from "@/types";

interface DownloadProgressBarProps {
  progress: DownloadProgress | null;
  status: string;
}

/** Formats bytes into a human-readable string. */
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  const i = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/**
 * Displays download progress with a progress bar, percentage, speed, size, and ETA.
 * Shown during `running` state; hidden for other states.
 */
export default function DownloadProgressBar({
  progress,
  status,
}: DownloadProgressBarProps) {
  if (status !== "running" && status !== "downloading") {
    return null;
  }

  const percent = progress?.percent ?? 0;
  const speed = progress?.speed ?? "";
  const downloaded = progress?.downloaded_bytes ?? 0;
  const total = progress?.total_bytes ?? 0;
  const eta = progress?.eta ?? "";

  return (
    <Box sx={{ width: "100%", mt: 0.5 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <Box sx={{ flex: 1 }}>
          <LinearProgress
            variant="determinate"
            value={Math.min(percent, 100)}
            sx={{
              height: 6,
              borderRadius: 3,
              "& .MuiLinearProgress-bar": {
                borderRadius: 3,
              },
            }}
          />
        </Box>
        <Typography variant="body2" sx={{ minWidth: 45, textAlign: "right" }}>
          {percent.toFixed(1)}%
        </Typography>
      </Box>
      <Box
        sx={{
          display: "flex",
          justifyContent: "space-between",
          mt: 0.25,
          px: 0.5,
        }}
      >
        <Typography variant="caption" color="text.secondary">
          {formatBytes(downloaded)}
          {total > 0 ? ` / ${formatBytes(total)}` : ""}
        </Typography>
        {speed && (
          <Typography variant="caption" color="text.secondary">
            {speed}
          </Typography>
        )}
        {eta && (
          <Typography variant="caption" color="text.secondary">
            {eta}
          </Typography>
        )}
      </Box>
    </Box>
  );
}
