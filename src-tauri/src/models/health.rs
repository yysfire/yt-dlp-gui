use serde::{Deserialize, Serialize};

/// 订阅健康状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// 频道正常可达
    Ok,
    /// 频道存在异常（限流、服务端错误等）
    Warning,
    /// 频道已失效（404、DNS 错误等）
    Dead,
}

/// 单个订阅的健康检查结果（运行时传输用，不持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// 关联的订阅 ID
    pub subscription_id: String,
    /// 检查的 URL
    pub url: String,
    /// 检查结果
    pub status: HealthStatus,
    /// 详细原因（如 "404 Not Found"）
    pub detail: String,
    /// 响应延迟（毫秒）
    pub latency_ms: u64,
    /// 检查时间（ISO 8601）
    pub checked_at: String,
}

/// 健康检查摘要（运行时传输用，推送给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    /// 检查总数
    pub total: usize,
    /// 正常数
    pub ok: usize,
    /// 警告数
    pub warning: usize,
    /// 失效数
    pub dead: usize,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 详细结果列表
    pub results: Vec<HealthCheckResult>,
}

impl HealthCheckSummary {
    /// 创建一个空的即将开始检查的摘要，total 表示待检查数量
    pub fn pending(total: usize) -> Self {
        Self {
            total,
            ok: 0,
            warning: 0,
            dead: 0,
            duration_ms: 0,
            results: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serde() {
        let status = HealthStatus::Ok;
        let json = serde_json::to_string(&status).expect("serialize Ok");
        assert_eq!(json, "\"ok\"");

        let dead: HealthStatus = serde_json::from_str("\"dead\"").expect("deserialize dead");
        assert_eq!(dead, HealthStatus::Dead);

        let warning: HealthStatus = serde_json::from_str("\"warning\"").expect("deserialize warning");
        assert_eq!(warning, HealthStatus::Warning);
    }

    #[test]
    fn test_health_check_summary_pending() {
        let summary = HealthCheckSummary::pending(10);
        assert_eq!(summary.total, 10);
        assert_eq!(summary.ok, 0);
        assert_eq!(summary.duration_ms, 0);
        assert!(summary.results.is_empty());
    }

    #[test]
    fn test_health_check_result_serialization() {
        use chrono::Utc;
        let result = HealthCheckResult {
            subscription_id: "test-id".to_string(),
            url: "https://youtube.com/@test".to_string(),
            status: HealthStatus::Ok,
            detail: "OK".to_string(),
            latency_ms: 150,
            checked_at: Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("test-id"));
        assert!(json.contains("\"ok\""));
        assert!(json.contains("150"));
        let deserialized: HealthCheckResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.subscription_id, "test-id");
        assert_eq!(deserialized.latency_ms, 150);
    }
}
