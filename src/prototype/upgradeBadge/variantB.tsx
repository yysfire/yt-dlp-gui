/**
 * 变体 B ——「整行强调 + 动作组」：让值得动手的行自己跳出来。
 *
 * 结构改动：可升级行加左侧主色竖条 + 极淡底色；缺失行左侧变红；重试行左侧变琥珀。
 * 右侧操作列从「一个图标按钮」扩成**一竖排小文字按钮**（升级 / 重新下载 / 暂停 / 取消）。
 * 问的是：值不值得为「可升级」把整行提权？
 */
import { Box, Button, Link, ListItem, Typography, Chip } from "@mui/material";
import { Upgrade, Download, Pause, Cancel } from "@mui/icons-material";
import type { ProtoRow } from "./mockData";
import { ProtoFrame, ProtoList, StatusIcon, statusText, RetryCountdown, qualityLabel } from "./Frame";

function accent(row: ProtoRow) {
  if (row.missing) return { color: "error.main", bg: "rgba(239,68,68,0.06)" };
  if (row.status === "retrying") return { color: "warning.main", bg: "rgba(245,158,11,0.07)" };
  if (row.upgradeable) return { color: "primary.main", bg: "rgba(37,99,235,0.05)" };
  return null;
}

export function VariantB({ rows }: { rows: ProtoRow[] }) {
  return (
    <ProtoFrame note="为值得动手的行整行提权（左侧色条 + 淡底色），操作收进右侧一竖排文字按钮组。">
      <Typography variant="subtitle2" color="text.secondary" sx={{ px: 2, pt: 2, pb: 1 }}>
        全部视频 ({rows.length})
      </Typography>
      <ProtoList>
        {rows.map((row) => {
          const st = statusText(row);
          const a = accent(row);
          return (
            <ListItem
              key={row.id}
              sx={{
                px: 2,
                py: 1,
                borderBottom: 1,
                borderColor: "divider",
                borderLeft: a ? `3px solid` : "3px solid transparent",
                borderLeftColor: a ? a.color : "transparent",
                bgcolor: a ? a.bg : "transparent",
                opacity: row.status === "deleted" ? 0.6 : 1,
              }}
            >
              <Box sx={{ width: 40, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                <StatusIcon row={row} />
              </Box>

              <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 0.3 }}>
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

                <Box sx={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: 0.75 }}>
                  <Typography variant="caption" sx={{ fontSize: "0.65rem", whiteSpace: "nowrap" }}>
                    <Box component="span" sx={{ color: st.color, fontWeight: 500 }}>
                      {st.text}
                    </Box>
                  </Typography>

                  {row.upgradeable && !row.missing && (
                    <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "primary.main", fontWeight: 600 }}>
                      · {row.quality} → {qualityLabel(row.target)}
                    </Typography>
                  )}
                  {row.missing && (
                    <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "error.main", fontWeight: 600 }}>
                      · 文件不在磁盘上
                    </Typography>
                  )}
                  {row.status === "retrying" && (
                    <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "warning.main" }}>
                      · 还有 <RetryCountdown initial={row.retryInSec ?? 30} /> 重试第 {(row.retryCount ?? 0) + 1} 次
                    </Typography>
                  )}

                  <Box sx={{ flex: 1 }} />
                  <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "text.disabled", whiteSpace: "nowrap" }}>
                    {row.rightMeta ?? ""}
                  </Typography>
                </Box>

                {row.status === "downloading" && (
                  <Box sx={{ height: 4, bgcolor: "grey.200", borderRadius: 2, mt: 0.5 }}>
                    <Box sx={{ height: "100%", bgcolor: "info.main", borderRadius: 2, width: `${row.percent ?? 0}%` }} />
                  </Box>
                )}
              </Box>

              <Box
                sx={{
                  width: 104,
                  flexShrink: 0,
                  ml: 1,
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "stretch",
                  gap: 0.5,
                }}
              >
                {row.upgradeable && !row.missing && (
                  <Button size="small" startIcon={<Upgrade sx={{ fontSize: 14 }} />} title={`升级到 ${qualityLabel(row.target)}`} sx={{ fontSize: "0.65rem", py: 0 }}>
                    升级
                  </Button>
                )}
                {row.missing && (
                  <Button size="small" startIcon={<Download sx={{ fontSize: 14 }} />} title="重新下载" sx={{ fontSize: "0.65rem", py: 0 }}>
                    重新下载
                  </Button>
                )}
                {row.status === "downloading" && (
                  <Button size="small" startIcon={<Pause sx={{ fontSize: 14 }} />} title="暂停" sx={{ fontSize: "0.65rem", py: 0 }}>
                    暂停
                  </Button>
                )}
                {(row.status === "downloading" || row.status === "retrying") && (
                  <Button size="small" color="error" startIcon={<Cancel sx={{ fontSize: 14 }} />} title="取消" sx={{ fontSize: "0.65rem", py: 0 }}>
                    取消
                  </Button>
                )}
                {row.status === "failed" && (
                  <Button size="small" color="warning" startIcon={<Download sx={{ fontSize: 14 }} />} title="重试" sx={{ fontSize: "0.65rem", py: 0 }}>
                    重试
                  </Button>
                )}
              </Box>
            </ListItem>
          );
        })}
      </ProtoList>
    </ProtoFrame>
  );
}
