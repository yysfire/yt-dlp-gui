/** 原型（一次性）：底部浮动变体切换条。故意做得显眼，避免被当成设计的一部分。 */
import { Box, IconButton, Typography } from "@mui/material";
import { ChevronLeft, ChevronRight, DarkMode, LightMode } from "@mui/icons-material";

export function PrototypeSwitcher({
  variants,
  current,
  labels,
  onChange,
  dark,
  onToggleDark,
}: {
  variants: string[];
  current: string;
  labels: Record<string, string>;
  onChange: (v: string) => void;
  dark: boolean;
  onToggleDark: () => void;
}) {
  const i = variants.indexOf(current);
  const step = (d: number) => onChange(variants[(i + d + variants.length) % variants.length]);

  return (
    <Box
      sx={{
        position: "fixed",
        bottom: 16,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 9999,
        display: "flex",
        alignItems: "center",
        gap: 1,
        px: 1,
        py: 0.5,
        borderRadius: 999,
        bgcolor: "rgba(17,24,39,0.92)",
        color: "#fff",
        boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
        border: "1px dashed rgba(255,255,255,0.45)",
      }}
    >
      <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.6)", pl: 1, pr: 0.5 }}>
        原型
      </Typography>
      <IconButton size="small" onClick={() => step(-1)} sx={{ color: "#fff" }} title="上一个变体">
        <ChevronLeft fontSize="small" />
      </IconButton>
      <Typography variant="caption" sx={{ minWidth: 190, textAlign: "center", fontWeight: 600 }}>
        {current}（{labels[current]}）
      </Typography>
      <IconButton size="small" onClick={() => step(1)} sx={{ color: "#fff" }} title="下一个变体">
        <ChevronRight fontSize="small" />
      </IconButton>
      <Box sx={{ width: 1, height: 20, bgcolor: "rgba(255,255,255,0.25)", mx: 0.5 }} />
      <IconButton size="small" onClick={onToggleDark} sx={{ color: "#fff" }} title="切换暗色">
        {dark ? <LightMode fontSize="small" /> : <DarkMode fontSize="small" />}
      </IconButton>
    </Box>
  );
}
