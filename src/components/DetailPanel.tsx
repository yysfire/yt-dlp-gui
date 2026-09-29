import { useState, useEffect, useRef, useMemo } from "react";
import { Box, Typography, Avatar, Chip, Alert, Skeleton, List, ListItem, Link, IconButton } from "@mui/material";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
  HeartBroken as DeadIcon,
  PlayCircleOutline as VideoIcon,
  Pause as PauseIcon,
  Cancel as CancelIcon,
  Downloading as DownloadingIcon,
  Replay as ReplayIcon,
  HourglassEmpty as WaitingIcon,
  DeleteOutline as DeletedIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, ChannelInfo, VideoInfo } from "@/types";
import { getChannelInfo, getChannelVideos } from "@/lib/tauri";
import {
  buildUnifiedVideoList,
  formatDuration,
  formatFileSize,
  formatTime,
  formatUploadDate,
} from "@/lib/unifiedVideoList";

interface DetailPanelProps {
  subscription: Subscription | null;
  records: DownloadRecord[];
  queueTasks: DownloadTask[];
  error?: string | null;
  progressMap?: Map<string, DownloadProgress>;
  onPauseDownload: (videoUrl: string) => void;
  onResumeDownload: (taskId: string) => void;
  onCancelDownload: (videoUrl: string) => void;
  onRetryDownload: (subscriptionId: string) => void;
}

/** Right-side detail panel showing channel info and download records. */
export default function DetailPanel({
  subscription,
  records,
  queueTasks,
  error,
  progressMap,
  onPauseDownload,
  onCancelDownload,
  onRetryDownload,
}: DetailPanelProps) {
  // 异步加载状态
  const [channelInfo, setChannelInfo] = useState<ChannelInfo | null>(null);
  const [videoList, setVideoList] = useState<VideoInfo[]>([]);
  const [_videoPage, setVideoPage] = useState(1);
  const [_hasMore, setHasMore] = useState(false);
  const [_totalVideos, setTotalVideos] = useState(0);
  const [loadingChannel, setLoadingChannel] = useState(false);
  const [loadingVideos, setLoadingVideos] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // 用于取消上一个请求
  const requestIdRef = useRef(0);

  // 当订阅变化时加载频道信息和视频列表
  useEffect(() => {
    if (!subscription) {
      setChannelInfo(null);
      setVideoList([]);
      setVideoPage(1);
      setHasMore(false);
      setTotalVideos(0);
      setLoadError(null);
      return;
    }

    // 如果频道已失效率，不发起请求
    if (subscription.health_status === "dead") {
      setChannelInfo(null);
      setVideoList([]);
      setLoadError("此频道已失效");
      setLoadingChannel(false);
      setLoadingVideos(false);
      return;
    }

    // 递增 requestId 以取消上一个正在进行的请求
    const currentRequestId = ++requestIdRef.current;

    const loadData = async () => {
      setLoadError(null);
      setLoadingChannel(true);
      setLoadingVideos(true);

      // 加载频道信息
      try {
        const info = await getChannelInfo(subscription.id);
        if (currentRequestId !== requestIdRef.current) return;
        setChannelInfo(info);
      } catch (e) {
        if (currentRequestId !== requestIdRef.current) return;
        console.warn("Failed to load channel info:", e);
      }
      setLoadingChannel(false);

      // 加载视频列表第一页
      let hasMoreVideos = false;
      try {
        const result = await getChannelVideos(subscription.id, 1, 10);
        if (currentRequestId !== requestIdRef.current) return;
        setVideoList(result.videos);
        setVideoPage(1);
        setHasMore(result.has_more);
        setTotalVideos(result.total);
        hasMoreVideos = result.has_more;
      } catch (e) {
        if (currentRequestId !== requestIdRef.current) return;
        setLoadError(String(e));
      }
      setLoadingVideos(false);

      // 后台自动加载剩余页面（闭包变量跟踪页码，无需额外 useEffect）
      if (currentRequestId === requestIdRef.current && hasMoreVideos) {
        let page = 2;
        while (currentRequestId === requestIdRef.current) {
          try {
            const res = await getChannelVideos(subscription.id, page, 10);
            if (currentRequestId !== requestIdRef.current) return;
            setVideoList((prev) => [...prev, ...res.videos]);
            setVideoPage(page);
            setHasMore(res.has_more);
            setTotalVideos(res.total);
            if (!res.has_more) break;
            page++;
          } catch {
            break;
          }
        }
      }
    };

    loadData();
  }, [subscription?.id]);

  const isDead = subscription?.health_status === "dead";

  // 合并频道视频、下载记录、队列任务为统一列表
  const unifiedItems = useMemo(() => {
    try {
      return buildUnifiedVideoList({ videos: videoList, records, tasks: queueTasks });
    } catch (e) {
      console.error("[DetailPanel] buildUnifiedVideoList failed:", e);
      return [];
    }
  }, [videoList, records, queueTasks]);

  // 已完成数从 records 派生（唯一真相源是下载记录，不再由后端维护计数字段）。
  // 口径：当前仍处于 completed 的记录条数。必须放在下方提前返回之前。
  const completedCount = useMemo(
    () => records.filter((r) => r.status === "completed").length,
    [records],
  );

  if (!subscription) {
    return (
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: "100%",
          color: "text.disabled",
        }}
      >
        <Typography variant="body2" color="text.secondary">
          选择左侧订阅查看详情
        </Typography>
      </Box>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Channel Header */}
      <Box
        sx={{
          p: 3,
          display: "flex",
          alignItems: "center",
          gap: 2,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        {loadingChannel ? (
          <Skeleton variant="circular" width={56} height={56} />
        ) : (
          <Avatar
            src={channelInfo?.thumbnail_url || subscription.channel_avatar_url}
            sx={{ width: 56, height: 56 }}
          >
            {subscription.channel_name.charAt(0)}
          </Avatar>
        )}
        <div className="flex-1 min-w-0">
          {loadingChannel ? (
            <Skeleton variant="text" width={200} height={32} />
          ) : (
            <Typography variant="h6" fontWeight={600} noWrap>
              {channelInfo?.channel_name || subscription.channel_name}
            </Typography>
          )}
          <div className="flex items-center gap-2 mt-0.5">
            <Chip
              label={subscription.platform}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            <Chip
              label={subscription.quality_preset}
              size="small"
              variant="outlined"
              sx={{ height: 20, fontSize: "0.7rem" }}
            />
            {subscription.paused && (
              <Chip
                label="已暂停"
                size="small"
                color="warning"
                variant="outlined"
                sx={{ height: 20, fontSize: "0.7rem" }}
              />
            )}
            {isDead && (
              <Chip
                icon={<DeadIcon sx={{ fontSize: 14 }} />}
                label="已失效"
                size="small"
                color="error"
                sx={{ height: 20, fontSize: "0.7rem" }}
              />
            )}
          </div>
          {/* Per-subscription stats */}
          <div className="flex items-center gap-2 mt-1">
            <Typography variant="caption" color="text.secondary">
              已完成: {completedCount} 个
            </Typography>
            {channelInfo?.subscriber_count && (
              <Typography variant="caption" color="text.secondary">
                · 订阅者: {channelInfo.subscriber_count}
              </Typography>
            )}
            {channelInfo?.video_count != null && (
              <Typography variant="caption" color="text.secondary">
                · 视频: {channelInfo.video_count}
              </Typography>
            )}
            {subscription.last_checked_at && (
              <Typography variant="caption" color="text.disabled">
                · 上次检查: {formatTime(subscription.last_checked_at)}
              </Typography>
            )}
            {subscription.last_check_status === "success" && (
              <SuccessIcon sx={{ fontSize: 14, color: "success.main" }} />
            )}
            {subscription.last_check_status === "failed" && (
              <ErrorIcon sx={{ fontSize: 14, color: "error.main" }} />
            )}
          </div>
          {channelInfo?.description && (
            <Typography
              variant="caption"
              color="text.secondary"
              component="div"
              className="mt-0.5 line-clamp-2"
            >
              {channelInfo.description}
            </Typography>
          )}
          {subscription.last_check_status === "failed" && subscription.last_check_error && (
            <Typography variant="caption" color="error.main" component="div" className="mt-0.5">
              错误: {subscription.last_check_error}
            </Typography>
          )}
          <Typography
            variant="caption"
            color="text.disabled"
            sx={{ mt: 0.5, display: "block" }}
            noWrap
          >
            {subscription.url}
          </Typography>
        </div>
      </Box>

      <div className="flex-1 overflow-y-auto">
        {/* 频道已失效率提示 */}
        {isDead && (
          <Alert severity="error" sx={{ m: 2 }} variant="outlined" icon={<DeadIcon />}>
            此频道已失效，无法获取视频列表。
            {subscription.last_health_check && (
              <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
                上次健康检查: {formatTime(subscription.last_health_check)}
              </Typography>
            )}
          </Alert>
        )}

        {/* 加载错误 */}
        {error && !isDead && (
          <Alert severity="error" sx={{ m: 2 }} variant="outlined">
            {error}
          </Alert>
        )}

        {loadError && !isDead && !error && (
          <Alert severity="warning" sx={{ m: 2 }} variant="outlined">
            {loadError}
          </Alert>
        )}

        {/* 视频列表区域（仅非失效率频道） */}
        {!isDead && (
          <>
            {loadingVideos && videoList.length === 0 ? (
              <div className="p-4 space-y-2">
                {Array.from({ length: 5 }).map((_, i) => (
                  <Skeleton key={i} variant="rounded" height={48} />
                ))}
              </div>
            ) : (
              <>
                {/* 统一视频列表 */}
                {unifiedItems.length > 0 && (
                  <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
                    <Typography
                      variant="subtitle2"
                      color="text.secondary"
                      sx={{ px: 2, pt: 2, pb: 1 }}
                    >
                      全部视频 ({unifiedItems.length})
                    </Typography>
                    <List dense disablePadding>
                      {unifiedItems.map((item) => (
                        <ListItem
                          key={item.id}
                          sx={{
                            px: 2,
                            borderBottom: 1,
                            borderColor: "divider",
                            opacity: item.status === "deleted" ? 0.5 : 1,
                          }}
                        >
                          {/* 第 1 列：状态图标（垂直居中） */}
                          <Box sx={{ width: 40, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                            {item.status === "new" && <VideoIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "downloading" && <DownloadingIcon sx={{ fontSize: 18, color: "info.main" }} />}
                            {item.status === "paused" && <PauseIcon sx={{ fontSize: 18, color: "warning.main" }} />}
                            {item.status === "waiting" && <WaitingIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "completed" && <SuccessIcon sx={{ fontSize: 18, color: "success.main" }} />}
                            {item.status === "failed" && <ErrorIcon sx={{ fontSize: 18, color: "error.main" }} />}
                            {item.status === "cancelled" && <CancelIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                            {item.status === "deleted" && <DeletedIcon sx={{ fontSize: 18, color: "text.disabled" }} />}
                          </Box>

                          {/* 第 2 列：三行内容 */}
                          <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 0.3 }}>
                            {/* 第 1 行：标题 */}
                            <Typography
                              variant="body2"
                              noWrap
                              component={Link}
                              href={item.url}
                              target="_blank"
                              rel="noopener noreferrer"
                              underline="hover"
                              color="inherit"
                              sx={{
                                fontSize: "0.8rem",
                                lineHeight: 1.3,
                                ...(item.status === "deleted" && { textDecoration: "line-through" }),
                              }}
                            >
                              {item.title}
                            </Typography>

                            {/* 第 2 行：左(状态+元信息) · 右(辅助信息) */}
                            <Box sx={{ display: "flex", alignItems: "center" }}>
                              <Typography variant="caption" sx={{ fontSize: "0.65rem", whiteSpace: "nowrap" }}>
                                {(() => {
                                  const date = formatUploadDate(item.channelInfo?.upload_date, item.channelInfo?.epoch);
                                  return (
                                    <>
                                      {item.status === "new" && (
                                        <>
                                          {date}
                                          {date && item.channelInfo?.duration != null && " · "}
                                          {item.channelInfo?.duration != null && formatDuration(item.channelInfo.duration)}
                                        </>
                                      )}
                                      {item.status === "waiting" && `等待中${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "downloading" && `下载中${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "paused" && `已暂停${date ? ` · ${date}` : ""}${item.channelInfo?.duration != null ? ` · ${formatDuration(item.channelInfo.duration)}` : ""}`}
                                      {item.status === "completed" && (
                                        <>
                                          <Box component="span" sx={{ color: "success.main", fontWeight: 500 }}>已完成</Box>
                                          {date && ` · ${date}`}
                                          {item.channelInfo?.duration != null && ` · ${formatDuration(item.channelInfo.duration)}`}
                                          {!item.channelInfo && item.downloadInfo && ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                      {item.status === "failed" && (
                                        <>
                                          <Box component="span" sx={{ color: "error.main", fontWeight: 500 }}>失败</Box>
                                          {date && ` · ${date}`}
                                          {item.channelInfo?.duration != null && ` · ${formatDuration(item.channelInfo.duration)}`}
                                          {item.downloadInfo?.error_message && ` · ${item.downloadInfo.error_message}`}
                                          {!item.channelInfo && item.downloadInfo && ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                      {item.status === "cancelled" && `已取消${date ? ` · ${date}` : ""}`}
                                      {item.status === "deleted" && (
                                        <>
                                          <Box component="span" sx={{ color: "text.disabled", fontWeight: 500 }}>已删除</Box>
                                          {date && ` · ${date}`}
                                          {!item.channelInfo && item.downloadInfo &&
                                            ` · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN")}`}
                                        </>
                                      )}
                                    </>
                                  );
                                })()}
                              </Typography>
                              <Box sx={{ flex: 1 }} />
                              <Typography variant="caption" sx={{ fontSize: "0.65rem", color: "text.disabled", whiteSpace: "nowrap" }}>
                                {item.downloadInfo && item.status === "downloading" && `下载中`}
                                {item.downloadInfo && item.status === "completed" && item.downloadInfo.file_size > 0 && `${formatFileSize(item.downloadInfo.file_size)} · ${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}`}
                                {item.downloadInfo && item.status === "failed" && `${new Date(item.downloadInfo.downloaded_at).toLocaleDateString("zh-CN", { month: "short", day: "numeric" })}`}
                              </Typography>
                            </Box>

                            {/* 第 3 行：进度条（仅下载中） */}
                            {item.status === "downloading" && item.downloadInfo && (
                              <Box sx={{ height: 4, bgcolor: "grey.200", borderRadius: 2, mt: 0.5 }}>
                                <Box
                                  sx={{
                                    height: "100%",
                                    bgcolor: "info.main",
                                    borderRadius: 2,
                                    transition: "width 0.3s",
                                    width: `${Math.min(
                                      progressMap?.get(item.downloadInfo.video_url)?.percent ?? 0,
                                      100
                                    )}%`,
                                  }}
                                />
                              </Box>
                            )}
                          </Box>

                          {/* 第 3 列：操作按钮（垂直居中） */}
                          <Box
                            sx={{
                              width: 40,
                              flexShrink: 0,
                              ml: 1,
                              display: "flex",
                              flexDirection: "column",
                              alignItems: "center",
                              justifyContent: "center",
                              gap: 0.5,
                            }}
                          >
                            {item.queueTask && item.status === "downloading" && (
                              <IconButton size="small" onClick={() => onPauseDownload(item.url)} title="暂停">
                                <PauseIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {item.queueTask && (item.status === "downloading" || item.status === "waiting" || item.status === "paused") && (
                              <IconButton size="small" onClick={() => onCancelDownload(item.url)} title="取消" sx={{ color: "error.main" }}>
                                <CancelIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                            {item.status === "failed" && (
                              <IconButton size="small" onClick={() => onRetryDownload(subscription.id)} title="重试" sx={{ color: "warning.main" }}>
                                <ReplayIcon sx={{ fontSize: 18 }} />
                              </IconButton>
                            )}
                          </Box>
                        </ListItem>
                      ))}
                    </List>
                  </Box>
                )}

                {/* 空列表提示 */}
                {!loadingVideos && unifiedItems.length === 0 && !isDead && (
                  <Box sx={{ p: 3, textAlign: "center" }}>
                    <Typography variant="body2" color="text.secondary">
                      暂无视频
                    </Typography>
                  </Box>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
