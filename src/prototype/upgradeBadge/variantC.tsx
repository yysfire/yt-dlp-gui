/**
 * 变体 C ——「元信息标签条」：把状态从文字降级成一枚标签。
 *
 * 结构改动最大：**去掉状态图标列**，标题下面那条线变成一条 chip 带
 * （状态 / 画质变化 / 缺失 / 重试倒数 / 日期时长 / 文件大小），
 * 右侧只留**一个**随语境换图标的按钮。
 * 问的是：信息密度换辨识度，划不划算？
 */
import { Box, Chip, IconButton, Link, ListItem, Typography } from "@mui/material";
import { Upgrade, Download, Pause, Cancel, Autorenew, WarningAmber } from "@mui/icons-material";
import type { ProtoRow } from "./mockData";
import { ProtoFrame, ProtoList, RetryCountdown, qualityLabel } from "./Frame";

function statusChip(row: ProtoRow) {
  const base = { height: 20, fontSize: "0.62rem" } as const;
  switch (row.status) {
    case "completed":
      return <Chip size="small" color="success" variant="outlined" label="已完成" sx={base} />;
    case "downloading":
      return <Chip size="small" color="info" variant="outlined" label="下载中" sx={base} />;
    case "retrying":
      return (
        <Chip
          size="small"
          color="warning"
          variant="outlined"
          icon={<Autorenew sx={{ fontSize: 12 }} />}
          label={`重试中 ${row.retryCount}/3 · 还有`}
          sx={base}
        />
      );
    case "failed":
      return <Chip size="small" color="error" variant="outlined" label="失败" sx={base} />;
    case "cancelled":
      return <Chip size="small" variant="outlined" label="已取消" sx={base} />;
    case "deleted":
      return <Chip size="small" variant="outlined" label="已删除" sx={base} />;
    case "new":
      return <Chip size="small" variant="outlined" label="新视频" sx={base} />;
  }
}

export function VariantC({ rows }: { rows: ProtoRow[] }) {
  return (
    <ProtoFrame note="去掉状态图标列，状态/画质/缺失/倒数全塞进一条 chip 带，右侧只留一个语境化按钮。">
      <Typography variant="subtitle2" color="text.secondary" sx={{ px: 2, pt: 2, pb: 1 }}>
        全部视频 ({rows.length})
      </Typography>
      <ProtoList>
        {rows.map((row) => (
          <ListItem
            key={row.id}
            sx={{
              px: 2,
              py: 1,
              borderBottom: 1,
              borderColor: "divider",
              alignItems: "flex-start",
              opacity: row.status === "deleted" ? 0.55 : 1,
            }}
          >
            <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 0.6 }}>
              <Typography
                variant="body2"
                noWrap
                component={Link}
                href={row.url}
                target="_blank"
                rel="noopener noreferrer"
                underline="hover"
                color="inherit"
                sx={{ fontSize: "0.8rem", lineHeight: 1.3, ...(row.status === "deleted" && { textDecoration: "line-through" }) }}
              >
                {row.title}
              </Typography>

              <Box sx={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: 0.5 }}>
                {statusChip(row)}
                {row.status === "retrying" && (
                  <Typography variant="caption" sx={{ fontSize: "0.62rem", color: "warning.main", mr: 0.5 }}>
                    <RetryCountdown initial={row.retryInSec ?? 30} />
                  </Typography>
                )}

                {row.quality && (
                  <Chip size="small" variant="outlined" label={`画质 ${row.quality}`} sx={{ height: 20, fontSize: "0.62rem" }} />
                )}

                {row.upgradeable && !row.missing && (
                  <Chip
                    size="small"
                    color="primary"
                    label={`可升级 → ${qualityLabel(row.target)}`}
                    sx={{ height: 20, fontSize: "0.62rem" }}
                  />
                )}

                {row.missing && (
                  <Chip
                    size="small"
                    color="error"
                    variant="outlined"
                    icon={<WarningAmber sx={{ fontSize: 12 }} />}
                    label="文件缺失"
                    sx={{ height: 20, fontSize: "0.62rem" }}
                  />
                )}

                {row.errorMessage && (
                  <Chip size="small" color="error" variant="outlined" label={row.errorMessage} sx={{ height: 20, fontSize: "0.62rem" }} />
                )}

                <Typography variant="caption" sx={{ fontSize: "0.62rem", color: "text.disabled" }}>
                  {row.meta}
                  {row.rightMeta ? ` · ${row.rightMeta}` : ""}
                </Typography>
              </Box>

              {row.status === "downloading" && (
                <Box sx={{ height: 4, bgcolor: "grey.200", borderRadius: 2 }}>
                  <Box sx={{ height: "100%", bgcolor: "info.main", borderRadius: 2, width: `${row.percent ?? 0}%` }} />
                </Box>
              )}
            </Box>

            <Box sx={{ width: 40, flexShrink: 0, ml: 1, display: "flex", justifyContent: "center", pt: 0.5 }}>
              {row.upgradeable && !row.missing && (
                <IconButton size="small" color="primary" title={`升级到 ${qualityLabel(row.target)}`}>
                  <Upgrade sx={{ fontSize: 18 }} />
                </IconButton>
              )}
              {row.missing && (
                <IconButton size="small" color="primary" title="重新下载">
                  <Download sx={{ fontSize: 18 }} />
                </IconButton>
              )}
              {row.status === "failed" && (
                <IconButton size="small" sx={{ color: "warning.main" }} title="重试">
                  <Download sx={{ fontSize: 18 }} />
                </IconButton>
              )}
              {row.status === "downloading" && (
                <IconButton size="small" title="暂停">
                  <Pause sx={{ fontSize: 18 }} />
                </IconButton>
              )}
              {(row.status === "downloading" || row.status === "retrying") && (
                <IconButton size="small" sx={{ color: "error.main" }} title="取消">
                  <Cancel sx={{ fontSize: 18 }} />
                </IconButton>
              )}
            </Box>
          </ListItem>
        ))}
      </ProtoList>
    </ProtoFrame>
  );
}
