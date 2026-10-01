/**
 * 原型（一次性，throwaway）—— 回答的问题：
 *   「详情面板里『可升级 / 文件缺失 / 重试中』长什么样、动作入口放哪？」
 *
 * 三个**结构不同**的变体，同一页用 ?variant=A|B|C 切换，底部浮动条切换。
 *
 * 为什么不是「就地改 DetailPanel」：仓库无路由，而真实 DetailPanel 依赖 Tauri 后端
 * （`invoke`），只跑 `npm run dev` 会拿不到数据。所以这里用**假数据 + 相同的
 * MUI 主题/密度**搭一个独立原型页 —— 取舍是「能一眼跑起来」优先于「贴着真页面」。
 *
 * 运行：npm run dev  然后打开
 *   http://localhost:5173/prototype/upgrade-badge.html?variant=A
 *
 * 本文件及 src/prototype/** 都只存在于分支 prototype/upgrade-badge，**不进主干**。
 */
import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider, createTheme, CssBaseline } from "@mui/material";
import "../../index.css";
import { MOCK_ROWS } from "./mockData";
import { PrototypeSwitcher } from "./PrototypeSwitcher";
import { VariantA } from "./variantA";
import { VariantB } from "./variantB";
import { VariantC } from "./variantC";

const lightTheme = createTheme({
  palette: {
    mode: "light",
    primary: { main: "#2563eb" },
    background: { default: "#f9fafb", paper: "#ffffff" },
  },
  typography: { fontFamily: '"Inter", "Roboto", "Helvetica", "Arial", sans-serif' },
  shape: { borderRadius: 8 },
  components: { MuiButton: { styleOverrides: { root: { textTransform: "none" } } } },
});

const darkTheme = createTheme({
  palette: {
    mode: "dark",
    primary: { main: "#60a5fa" },
    background: { default: "#111827", paper: "#1f2937" },
  },
  typography: { fontFamily: '"Inter", "Roboto", "Helvetica", "Arial", sans-serif' },
  shape: { borderRadius: 8 },
  components: { MuiButton: { styleOverrides: { root: { textTransform: "none" } } } },
});

const VARIANTS = ["A", "B", "C"] as const;
type VariantKey = (typeof VARIANTS)[number];

function readVariant(): VariantKey {
  const v = new URLSearchParams(window.location.search).get("variant");
  return (VARIANTS as readonly string[]).includes(v ?? "") ? (v as VariantKey) : "A";
}

function Root() {
  const [variant, setVariant] = React.useState<VariantKey>(readVariant);
  const [dark, setDark] = React.useState(false);

  const go = React.useCallback((next: VariantKey) => {
    setVariant(next);
    const url = new URL(window.location.href);
    url.searchParams.set("variant", next);
    window.history.replaceState(null, "", url.toString());
  }, []);

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = document.activeElement;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || (el as HTMLElement).isContentEditable)) return;
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      const i = VARIANTS.indexOf(variant);
      const d = e.key === "ArrowRight" ? 1 : -1;
      go(VARIANTS[(i + d + VARIANTS.length) % VARIANTS.length]);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [variant, go]);

  return (
    <>
      {variant === "A" && <VariantA rows={MOCK_ROWS} />}
      {variant === "B" && <VariantB rows={MOCK_ROWS} />}
      {variant === "C" && <VariantC rows={MOCK_ROWS} />}
      <PrototypeSwitcher
        variants={VARIANTS as unknown as string[]}
        current={variant}
        labels={{ A: "状态旁加徽标", B: "整行强调 + 动作组", C: "元信息标签条" }}
        onChange={(v) => go(v as VariantKey)}
        dark={dark}
        onToggleDark={() => setDark((d) => !d)}
      />
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider theme={dark ? darkTheme : lightTheme}>
      <CssBaseline />
      <Root />
    </ThemeProvider>
  </React.StrictMode>,
);
