import {
  AppBar,
  Toolbar,
  Typography,
  IconButton,
  Tooltip,
} from "@mui/material";
import {
  Add as AddIcon,
  Settings as SettingsIcon,
  Menu as MenuIcon,
} from "@mui/icons-material";

interface TopBarProps {
  onOpenSettings: () => void;
  onOpenAddDialog: () => void;
  onToggleSidebar: () => void;
}

/** Top navigation bar with app title, add and settings buttons. */
export default function TopBar({
  onOpenSettings,
  onOpenAddDialog,
  onToggleSidebar,
}: TopBarProps) {
  return (
    <AppBar
      position="static"
      elevation={0}
      sx={{
        backgroundColor: "background.paper",
        borderBottom: 1,
        borderColor: "divider",
      }}
    >
      <Toolbar variant="dense" sx={{ minHeight: 48 }}>
        <IconButton
          edge="start"
          size="small"
          onClick={onToggleSidebar}
          sx={{ mr: 1 }}
        >
          <MenuIcon fontSize="small" />
        </IconButton>

        <Typography
          variant="subtitle1"
          fontWeight={600}
          sx={{ flexGrow: 1 }}
          noWrap
        >
          yt-dlp 订阅管理器
        </Typography>

        <Tooltip title="添加订阅">
          <IconButton size="small" onClick={onOpenAddDialog} sx={{ mr: 0.5 }}>
            <AddIcon fontSize="small" />
          </IconButton>
        </Tooltip>

        <Tooltip title="设置">
          <IconButton size="small" onClick={onOpenSettings}>
            <SettingsIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Toolbar>
    </AppBar>
  );
}
