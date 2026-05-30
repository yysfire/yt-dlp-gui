import { List, Typography, Box } from "@mui/material";
import type { DownloadRecord, DownloadProgress } from "@/types";
import DownloadRecordItem from "./DownloadRecordItem";

interface DownloadRecordListProps {
  records: DownloadRecord[];
  progressMap?: Map<string, DownloadProgress>;
}

/** Scrollable list of download records for a subscription. */
export default function DownloadRecordList({
  records,
  progressMap,
}: DownloadRecordListProps) {
  const sorted = [...records].sort(
    (a, b) =>
      new Date(b.downloaded_at).getTime() -
      new Date(a.downloaded_at).getTime(),
  );

  if (sorted.length === 0) {
    return (
      <Box sx={{ p: 3, textAlign: "center" }}>
        <Typography variant="body2" color="text.secondary">
          暂无下载记录
        </Typography>
      </Box>
    );
  }

  return (
    <List disablePadding dense>
      {sorted.map((record) => (
        <DownloadRecordItem
          key={record.id}
          record={record}
          progress={progressMap?.get(record.video_url) ?? null}
        />
      ))}
    </List>
  );
}
