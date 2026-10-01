/**
 * 变体 A ——「状态旁加徽标」：最小侵入。
 *
 * 结构沿用现有的三列行（状态图标 | 内容 | 操作按钮列），只在**状态文字旁边**
 * 追加一个小徽标，在**操作列**追加一个文字图标按钮。
 * 问的是：不加结构，够不够看得出来？
 */
import { Box, Chip, IconButton, Link, ListItem, Typography } from "@mui/material";
import { Upgrade, Download, WarningAmber } from "@mui/icons-material";
import type { ProtoRow } from "./mockData";
import { ProtoFrame, ProtoList, StatusIcon, statusText, RetryCountdown, qualityLabel } from "./Frame";

function UpgradeBadge({ target }: { target?: string }) {
  return (
    <Chip
      size="small"
      color="primary"
      variant="outlined"
      label={`可升级 ${qualityLabel(target)}`}
      sx={{ height: 18, fontSize: "0.62rem", ml: 0.5 }}
    />
  );
}

export function VariantA({ rows }: { rows: ProtoRow[] }) {
  return (
    <ProtoFrame note="不加结构改动，只在状态文字旁贴徽标、在操作列加文字按钮，够不够看得出来？">
      <Typography variant="subtitle2" color="text.secondary" sx={{ px: 2, pt: 2, pb: 1 }}>
        全部视频 ({rows.length})
      </Typography>
      <ProtoList>
        {rows.map((row) => {
          const st = statusText(row);
          return (
            <ListItem
              key={row.id}
              sx={{ px: 2, borderBottom: 1, borderColor: "divider", opacity: row.status === "deleted" ? 0.5 : 1 }}
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

                <Box sx={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: 0.5 }}>
                  <Typography variant="caption" sx={{ fontSize: "0.65rem", whiteSpace: "nowrap" }}>
                    <Box component="span" sx={{ color: st.color, fontWeight: 500 }}>
                      {st.text}
                    </Box>
                  </Typography>

                  {row.status === "retrying" && (
                    <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "text.secondary" }}>
                      · 还有 <RetryCountdown initial={row.retryInSec ?? 30} />
                    </Typography>
                  )}

                  {row.missing && row.status !== "new" && (
                    <Chip
                      size="small"
                      color="error"
                      variant="outlined"
                      icon={<WarningAmber sx={{ fontSize: 12 }} />}
                      label="文件缺失"
                      sx={{ height: 18, fontSize: "0.62rem" }}
                    />
                  )}

                  {row.upgradeable && !row.missing && <UpgradeBadge target={row.target} />}

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
                {row.status === "retrying" && (
                  <Box
                    sx={{
                      height: 4,
                      borderRadius: 2,
                      mt: 0.5,
                      background: "repeating-linear-gradient(90deg, #f59e0b 0 6px, transparent 6px 12px)",
                      opacity: 0.6,
                    }}
                  />
                )}
              </Box>

              <Box sx={{ width: 40, flexShrink: 0, ml: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
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
                  <IconButton size="small" title="重试" sx={{ color: "warning.main" }}>
                    <Download sx={{ fontSize: 18 }} />
                  </IconButton>
                )}
              </Box>
            </ListItem>
          );
        })}
      </ProtoList>
    </ProtoFrame>
  );
}
