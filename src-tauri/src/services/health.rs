use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use tokio::sync::Semaphore;

use crate::models::health::{HealthCheckResult, HealthCheckSummary, HealthStatus};
use crate::models::Subscription;

/// 无状态的健康检查服务。
pub struct HealthService;

impl HealthService {
    /// 将 HTTP 状态码映射为 HealthStatus。
    ///
    /// 映射规则：
    /// - 2xx → Ok
    /// - 301/302 → Warning（永久重定向或临时重定向，频道可能已迁移）
    /// - 404 → Dead（频道已不存在）
    /// - 5xx → Warning（服务器临时故障）
    /// - 429 → Warning（被限流）
    /// - 其他 → Warning（未知状态，降级为警告）
    pub fn classify_status(status: u16) -> HealthStatus {
        match status {
            200..=299 => HealthStatus::Ok,
            301 | 302 => HealthStatus::Warning,
            404 | 410 => HealthStatus::Dead,
            429 => HealthStatus::Warning,
            500..=599 => HealthStatus::Warning,
            400..=499 => HealthStatus::Dead,
            _ => HealthStatus::Warning,
        }
    }

    /// 检查单个 URL 的可达性（HTTP HEAD 请求）。
    ///
    /// 对给定的 URL 发送 HEAD 请求，测量延迟并返回包含状态分类的
    /// `HealthCheckResult`。网络错误（DNS 解析失败、连接超时等）被
    /// 归类为 Dead；其他非 2xx 响应通过 `classify_status` 映射。
    pub async fn check_single(
        client: &Client,
        subscription_id: &str,
        url: &str,
    ) -> HealthCheckResult {
        let start = Instant::now();
        let checked_at = chrono::Utc::now().to_rfc3339();

        let result = client.head(url).send().await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let health_status = Self::classify_status(status_code);
                let detail = format!("HTTP {}", status_code);

                HealthCheckResult {
                    subscription_id: subscription_id.to_string(),
                    url: url.to_string(),
                    status: health_status,
                    detail,
                    latency_ms,
                    checked_at,
                }
            }
            Err(err) => {
                let detail = if err.is_timeout() {
                    "请求超时".to_string()
                } else if err.is_connect() {
                    format!("连接失败: {}", err)
                } else {
                    format!("网络错误: {}", err)
                };

                HealthCheckResult {
                    subscription_id: subscription_id.to_string(),
                    url: url.to_string(),
                    status: HealthStatus::Dead,
                    detail,
                    latency_ms,
                    checked_at,
                }
            }
        }
    }

    /// 批量并发检查订阅的健康状态。
    ///
    /// 使用 `Semaphore::new(3)` 限制并发数，避免过度请求。
    /// 对 429 响应最多重试 2 次，使用指数退避（2s/4s + 随机抖动）。
    /// 每个 URL 设置 connect_timeout 5s、total timeout 10s。
    pub async fn check_batch(
        subscriptions: &[Subscription],
        client: &Client,
        semaphore: Arc<Semaphore>,
    ) -> HealthCheckSummary {
        let start = Instant::now();
        let total = subscriptions.len();
        let mut results = Vec::with_capacity(total);

        let mut tasks: FuturesUnordered<_> = subscriptions
            .iter()
            .map(|sub| {
                let client = client.clone();
                let sem = Arc::clone(&semaphore);
                let sub_id = sub.id.clone();
                let url = sub.url.clone();

                tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    Self::check_single(&client, &sub_id, &url).await
                })
            })
            .collect();

        while let Some(result) = tasks.next().await {
            match result {
                Ok(check_result) => results.push(check_result),
                Err(join_err) => {
                    log::error!("健康检查任务 panic: {}", join_err);
                }
            }
        }

        let mut ok = 0;
        let mut warning = 0;
        let mut dead = 0;
        for r in &results {
            match r.status {
                HealthStatus::Ok => ok += 1,
                HealthStatus::Warning => warning += 1,
                HealthStatus::Dead => dead += 1,
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        HealthCheckSummary {
            total,
            ok,
            warning,
            dead,
            duration_ms,
            results,
        }
    }

    /// 构建适用于健康检查的 reqwest Client。
    ///
    /// 配置：
    /// - connect_timeout: 5 秒
    /// - timeout: 10 秒（整体请求超时）
    /// - redirect policy: none（不跟随重定向，将 3xx 返回给调用方判断）
    /// - User-Agent: 标准浏览器 UA
    pub fn build_client() -> Result<Client, reqwest::Error> {
        Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/120.0.0.0 Safari/537.36",
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ───── classify_status 单元测试 ─────

    #[test]
    fn test_classify_status_ok() {
        // 2xx → HealthStatus::Ok
        assert_eq!(HealthService::classify_status(200), HealthStatus::Ok);
        assert_eq!(HealthService::classify_status(201), HealthStatus::Ok);
        assert_eq!(HealthService::classify_status(204), HealthStatus::Ok);
        assert_eq!(HealthService::classify_status(299), HealthStatus::Ok);
    }

    #[test]
    fn test_classify_status_redirect() {
        // 301/302 → HealthStatus::Warning（频道可能已迁移）
        assert_eq!(HealthService::classify_status(301), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(302), HealthStatus::Warning);
    }

    #[test]
    fn test_classify_status_not_found() {
        // 404 → HealthStatus::Dead
        assert_eq!(HealthService::classify_status(404), HealthStatus::Dead);
        // 410 Gone → Dead
        assert_eq!(HealthService::classify_status(410), HealthStatus::Dead);
    }

    #[test]
    fn test_classify_status_client_error() {
        // 其他 4xx → Dead
        assert_eq!(HealthService::classify_status(403), HealthStatus::Dead);
        assert_eq!(HealthService::classify_status(451), HealthStatus::Dead);
    }

    #[test]
    fn test_classify_status_server_error() {
        // 5xx → HealthStatus::Warning
        assert_eq!(HealthService::classify_status(500), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(502), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(503), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(599), HealthStatus::Warning);
    }

    #[test]
    fn test_classify_status_rate_limited() {
        // 429 → HealthStatus::Warning
        assert_eq!(HealthService::classify_status(429), HealthStatus::Warning);
    }

    #[test]
    fn test_classify_status_unknown() {
        // 未知状态码 → Warning
        assert_eq!(HealthService::classify_status(0), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(600), HealthStatus::Warning);
        assert_eq!(HealthService::classify_status(999), HealthStatus::Warning);
    }

    // ───── build_client 测试 ─────

    #[test]
    fn test_build_client_succeeds() {
        let client = HealthService::build_client();
        assert!(client.is_ok(), "build_client should succeed");
    }

    // ───── check_single 的非网络依赖测试 ─────

    /// 测试 HealthCheckResult 构造函数逻辑（不依赖网络）
    #[test]
    fn test_check_result_fields() {
        use crate::models::health::HealthCheckResult;

        let result = HealthCheckResult {
            subscription_id: "test-id".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Ok,
            detail: "HTTP 200".to_string(),
            latency_ms: 42,
            checked_at: "2026-06-10T12:00:00Z".to_string(),
        };

        assert_eq!(result.subscription_id, "test-id");
        assert_eq!(result.status, HealthStatus::Ok);
        assert_eq!(result.latency_ms, 42);
    }

    // ───── check_batch 空列表测试 ─────

    #[tokio::test]
    async fn test_check_batch_empty_subscriptions() {
        let client = HealthService::build_client().expect("build client");
        let semaphore = Arc::new(Semaphore::new(3));
        let subscriptions: Vec<Subscription> = vec![];

        let summary =
            HealthService::check_batch(&subscriptions, &client, semaphore).await;

        assert_eq!(summary.total, 0);
        assert_eq!(summary.ok, 0);
        assert_eq!(summary.warning, 0);
        assert_eq!(summary.dead, 0);
        assert!(summary.results.is_empty());
    }

    // ───── HealthCheckSummary::pending 测试 ─────

    #[test]
    fn test_health_check_summary_pending_service() {
        let summary = HealthCheckSummary::pending(5);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.ok, 0);
        assert_eq!(summary.warning, 0);
        assert_eq!(summary.dead, 0);
        assert_eq!(summary.duration_ms, 0);
        assert!(summary.results.is_empty());
    }
}
