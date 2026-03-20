//! 重试策略实现
//!
//! 提供指数退避重试机制

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use super::types::AuthError;
use super::MailError;

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始退避时间（毫秒）
    pub initial_delay_ms: u64,
    /// 最大退避时间（毫秒）
    pub max_delay_ms: u64,
    /// 退避倍数
    pub multiplier: f64,
    /// 是否添加随机抖动
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// 用于网络操作的重试配置（更多重试）
    pub fn network() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_ms: 500,
            max_delay_ms: 10000,
            multiplier: 1.5,
            jitter: true,
        }
    }

    /// 用于数据库操作的重试配置（较少重试）
    pub fn database() -> Self {
        Self {
            max_attempts: 2,
            initial_delay_ms: 100,
            max_delay_ms: 1000,
            multiplier: 2.0,
            jitter: false,
        }
    }
}

/// 判断错误是否可重试的 trait
pub trait IsRetryable: std::fmt::Debug {
    /// 判断是否可重试
    fn is_retryable(&self) -> bool;

    /// 获取建议的重试延迟
    fn retry_delay(&self) -> Option<Duration>;
}

// 为 MailError 实现 trait
impl IsRetryable for MailError {
    fn is_retryable(&self) -> bool {
        match self {
            MailError::Connection(conn_err) => conn_err.is_retryable(),
            MailError::Authentication(AuthError::OAuthExpired) => true,
            MailError::Sync(sync_err) => sync_err.is_retryable(),
            MailError::OAuth(oauth_err) => oauth_err.is_retryable(),
            MailError::RateLimit { .. } => true,
            _ => false,
        }
    }

    fn retry_delay(&self) -> Option<Duration> {
        self.retry_delay()
    }
}

/// 重试执行器
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(RetryConfig::default())
    }

    pub fn with_network_config() -> Self {
        Self::new(RetryConfig::network())
    }

    pub fn with_database_config() -> Self {
        Self::new(RetryConfig::database())
    }

    /// 执行带重试的异步操作
    pub async fn execute<F, Fut, T>(
        &self,
        mut operation: F,
        operation_name: &str,
    ) -> Result<T, MailError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, MailError>>,
    {
        let mut attempt = 0;
        let mut delay = Duration::from_millis(self.config.initial_delay_ms);

        loop {
            attempt += 1;

            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!("{} 在第 {} 次尝试后成功", operation_name, attempt);
                    }
                    return Ok(result);
                }
                Err(err) => {
                    if attempt >= self.config.max_attempts || !err.is_retryable() {
                        warn!("{} 失败，已尝试 {} 次，放弃重试", operation_name, attempt);
                        return Err(err);
                    }

                    let delay_ms = delay.as_millis();
                    warn!(
                        "{} 失败（尝试 {}/{}），{}ms 后重试: {}",
                        operation_name, attempt, self.config.max_attempts, delay_ms, err
                    );

                    // 添加抖动
                    let actual_delay = if self.config.jitter {
                        let jitter_ms = (delay_ms as f64 * 0.1) as u64;
                        let jitter = rand::random::<u64>() % (2 * jitter_ms + 1);
                        let jitter_delay = delay_ms as i64 - jitter as i64 + jitter as i64;
                        Duration::from_millis(std::cmp::max(1, jitter_delay as u64))
                    } else {
                        delay
                    };

                    sleep(actual_delay).await;

                    // 计算下一次延迟
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.multiplier) as u64,
                        ),
                        Duration::from_millis(self.config.max_delay_ms),
                    );
                }
            }
        }
    }

    /// 执行带重试的操作（使用闭包）
    pub async fn execute_with_context<F, Fut, T>(
        &self,
        operation: F,
        context: impl Fn() -> String,
    ) -> Result<T, MailError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, MailError>>,
    {
        self.execute(operation, &context()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::types::{AuthError, ConnectionError};
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_network() {
        let config = RetryConfig::network();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 10000);
    }

    #[test]
    fn test_retry_config_database() {
        let config = RetryConfig::database();
        assert_eq!(config.max_attempts, 2);
        assert_eq!(config.initial_delay_ms, 100);
        assert!(!config.jitter);
    }

    #[tokio::test]
    async fn test_retry_executor_success() {
        let executor = RetryExecutor::with_default_config();
        let mut attempts = 0;

        let result = executor
            .execute(
                || {
                    attempts += 1;
                    async move {
                        if attempts < 3 {
                            Err(MailError::Connection(ConnectionError::Timeout))
                        } else {
                            Ok("success")
                        }
                    }
                },
                "test_operation",
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn test_retry_executor_non_retryable() {
        let executor = RetryExecutor::with_default_config();
        let mut attempts = 0;

        let result: Result<&str, MailError> = executor
            .execute(
                || {
                    attempts += 1;
                    async {
                        Err::<&str, _>(MailError::Authentication(AuthError::InvalidCredentials))
                    }
                },
                "test_operation",
            )
            .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1); // 不应重试
    }

    #[tokio::test]
    async fn test_retry_executor_max_attempts() {
        let executor = RetryExecutor::with_network_config();
        let mut attempts = 0;

        let result: Result<&str, MailError> = executor
            .execute(
                || {
                    attempts += 1;
                    async { Err::<&str, _>(MailError::Connection(ConnectionError::Timeout)) }
                },
                "test_operation",
            )
            .await;

        assert!(result.is_err());
        assert_eq!(attempts, 5); // 达到最大重试次数
    }
}
