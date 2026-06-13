import { useState, useEffect, useRef, useCallback } from "react";
import { Box, Typography, Avatar, Chip, Alert, Skeleton, Button, List, ListItem, ListItemText, Link, Divider } from "@mui/material";
import {
  CheckCircle as SuccessIcon,
  Error as ErrorIcon,
  HeartBroken as DeadIcon,
  Refresh as RefreshIcon,
  PlayCircleOutline as VideoIcon,
} from "@mui/icons-material";
import type { Subscription, DownloadRecord, DownloadProgress, DownloadTask, ChannelInfo, VideoInfo } from "@/types";
import { getChannelInfo, getChannelVideos } from "@/lib/tauri";
import DownloadRecordList from "./DownloadRecordList";

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("zh-CN");
  } catch {
    return iso;
  }
}

/** 将秒数格式化为人类可读的时长字符串（如 "12:34", "0:30"） */
function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  const totalSeconds = Math.round(seconds);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${m}:${String(s).padStart(2, "0")}`;
}

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
  onResumeDownload,
  onCancelDownload,
  onRetryDownload,
}: DetailPanelProps) {
  // 异步加载状态
  const [channelInfo, setChannelInfo] = useState<ChannelInfo | null>(null);
  const [videoList, setVideoList] = useState<VideoInfo[]>([]);
  const [videoPage, setVideoPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [totalVideos, setTotalVideos] = useState(0);
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
      try {
        const result = await getChannelVideos(subscription.id, 1, 10);
        if (currentRequestId !== requestIdRef.current) return;
        setVideoList(result.videos);
        setVideoPage(1);
        setHasMore(result.has_more);
        setTotalVideos(result.total);
      } catch (e) {
        if (currentRequestId !== requestIdRef.current) return;
        setLoadError(String(e));
      }
      setLoadingVideos(false);
    };

    loadData();
  }, [subscription?.id]);

  // 加载更多视频
  const handleLoadMore = useCallback(async () => {
    if (!subscription || loadingVideos || !hasMore) return;

    const nextPage = videoPage + 1;
    const currentRequestId = ++requestIdRef.current;
    setLoadingVideos(true);

    try {
      const result = await getChannelVideos(subscription.id, nextPage, 10);
      if (currentRequestId !== requestIdRef.current) return;
      setVideoList((prev) => [...prev, ...result.videos]);
      setVideoPage(nextPage);
      setHasMore(result.has_more);
      setTotalVideos(result.total);
    } catch (e) {
      if (currentRequestId !== requestIdRef.current) return;
      console.warn("Failed to load more videos:", e);
    }
    setLoadingVideos(false);
  }, [subscription, videoPage, hasMore, loadingVideos]);

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

  const isDead = subscription.health_status === "dead";

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
              已下载: {subscription.download_count} 个
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
                {/* 视频列表 */}
                {videoList.length > 0 && (
                  <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
                    <Typography
                      variant="subtitle2"
                      color="text.secondary"
                      sx={{ px: 2, pt: 2, pb: 1 }}
                    >
                      最新视频 ({totalVideos > 0 ? totalVideos : videoList.length})
                    </Typography>
                    <List dense disablePadding>
                      {videoList.map((video) => (
                        <ListItem key={video.id} sx={{ px: 2 }}>
                          <VideoIcon sx={{ fontSize: 18, color: "text.disabled", mr: 1.5, flexShrink: 0 }} />
                          <ListItemText
                            primary={
                              <Link
                                href={video.url}
                                target="_blank"
                                rel="noopener noreferrer"
                                underline="hover"
                                color="inherit"
                                sx={{ fontSize: "0.875rem" }}
                              >
                                {video.title}
                              </Link>
                            }
                            secondary={
                              <span className="text-xs">
                                {video.upload_date && `${video.upload_date}`}
                                {video.upload_date && video.duration && " · "}
                                {video.duration && `${formatDuration(video.duration)}`}
                              </span>
                            }
                          />
                        </ListItem>
                      ))}
                    </List>
                    <Divider />
                  </Box>
                )}

                {/* 下载记录 */}
                <DownloadRecordList
                  records={records}
                  queueTasks={queueTasks}
                  subscriptionId={subscription.id}
                  progressMap={progressMap}
                  onPause={onPauseDownload}
                  onResume={onResumeDownload}
                  onCancel={onCancelDownload}
                  onRetry={onRetryDownload}
                />

                {/* 分页 - 加载更多 */}
                {hasMore && (
                  <div className="flex justify-center p-3">
                    <Button
                      size="small"
                      variant="outlined"
                      disabled={loadingVideos}
                      onClick={handleLoadMore}
                      startIcon={<RefreshIcon />}
                    >
                      {loadingVideos ? "加载中..." : `加载更多 (${totalVideos - videoList.length} 剩余)`}
                    </Button>
                  </div>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
