import { useState } from "react";
import {
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Avatar,
  Menu,
  MenuItem,
  ListItemSecondaryAction,
  Chip,
  Typography,
  IconButton,
} from "@mui/material";
import {
  MoreVert as MoreIcon,
  Pause as PauseIcon,
  PlayArrow as ResumeIcon,
  YouTube as YouTubeIcon,
  SmartDisplay as BilibiliIcon,
  Language as OtherIcon,
} from "@mui/icons-material";
import type { Subscription } from "@/types";

interface SubscriptionItemProps {
  subscription: Subscription;
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onTogglePause: () => void;
  onCheck: () => void;
}

/** Platform icon mapping. */
function PlatformIcon({ platform }: { platform: string }) {
  switch (platform) {
    case "youtube":
      return <YouTubeIcon fontSize="small" sx={{ color: "#FF0000" }} />;
    case "bilibili":
      return <BilibiliIcon fontSize="small" sx={{ color: "#00A1D6" }} />;
    default:
      return <OtherIcon fontSize="small" />;
  }
}

/** Single subscription row in the sidebar list. */
export default function SubscriptionItem({
  subscription,
  selected,
  onSelect,
  onDelete,
  onTogglePause,
  onCheck,
}: SubscriptionItemProps) {
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
  const menuOpen = Boolean(anchorEl);

  const handleMenuOpen = (e: React.MouseEvent<HTMLElement>) => {
    e.stopPropagation();
    setAnchorEl(e.currentTarget);
  };

  const handleMenuClose = () => setAnchorEl(null);

  return (
    <>
      <ListItemButton
        selected={selected}
        onClick={onSelect}
        sx={{
          opacity: subscription.paused ? 0.55 : 1,
          "&.Mui-selected": {
            bgcolor: "primary.50",
          },
        }}
      >
        <ListItemIcon sx={{ minWidth: 40 }}>
          <Avatar
            src={subscription.channel_avatar_url}
            sx={{ width: 28, height: 28 }}
          >
            {subscription.channel_name.charAt(0)}
          </Avatar>
        </ListItemIcon>

        <ListItemText
          primary={
            <Typography variant="body2" noWrap>
              {subscription.channel_name}
            </Typography>
          }
          secondary={
            <span className="flex items-center gap-1">
              <PlatformIcon platform={subscription.platform} />
              <Typography variant="caption" color="text.disabled">
                {subscription.platform}
              </Typography>
            </span>
          }
          sx={{ my: 0 }}
        />

        {subscription.paused && (
          <Chip
            label="已暂停"
            size="small"
            variant="outlined"
            sx={{ mr: 1, height: 20, fontSize: "0.65rem" }}
          />
        )}

        <ListItemSecondaryAction>
          <IconButton edge="end" size="small" onClick={handleMenuOpen}>
            <MoreIcon fontSize="small" />
          </IconButton>
        </ListItemSecondaryAction>
      </ListItemButton>

      {/* Context Menu */}
      <Menu
        anchorEl={anchorEl}
        open={menuOpen}
        onClose={handleMenuClose}
        anchorOrigin={{ vertical: "center", horizontal: "right" }}
        transformOrigin={{ vertical: "center", horizontal: "left" }}
      >
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onTogglePause();
          }}
          dense
        >
          {subscription.paused ? (
            <ResumeIcon fontSize="small" sx={{ mr: 1 }} />
          ) : (
            <PauseIcon fontSize="small" sx={{ mr: 1 }} />
          )}
          {subscription.paused ? "恢复" : "暂停"}
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onCheck();
          }}
          dense
        >
          <PauseIcon fontSize="small" sx={{ mr: 1 }} />
          手动检查
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleMenuClose();
            onDelete();
          }}
          dense
          sx={{ color: "error.main" }}
        >
          删除
        </MenuItem>
      </Menu>
    </>
  );
}
