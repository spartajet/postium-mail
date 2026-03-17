//! 错误类型定义
//!
//! 提供统一的错误类型体系

use std::time::Duration;
use thiserror::Error;

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// 低严重度：用户可忽略，自动恢复
    Low,
    /// 中等严重度：影响部分功能，需要重试
    Medium,
    /// 高严重度：影响核心功能，需要用户干预
    High,
    /// 致命错误：应用无法继续
    Critical,
}

/// 邮件客户端统一错误类型
#[derive(Error, Debug)]
pub enum MailError {
    /// 连接错误
    #[error("连接错误: {0}")]
    Connection(#[from] ConnectionError),

    /// 认证错误
    #[error("认证错误: {0}")]
    Authentication(#[from] AuthError),

    /// 同步错误
    #[error("同步错误: {0}")]
    Sync(#[from] SyncError),

    /// OAuth 错误
    #[error("OAuth 错误: {0}")]
    OAuth(#[from] OAuthError),

    /// 存储错误
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),

    /// 限流错误
    #[error("请求过于频繁，请 {retry_after} 秒后重试")]
    RateLimit { retry_after: u64 },

    /// 通用错误
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 连接错误
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("连接超时")]
    Timeout,

    #[error("SSL/TLS 错误: {0}")]
    Ssl(String),

    #[error("主机无法访问: {0}")]
    HostUnreachable(String),

    #[error("连接被拒绝")]
    ConnectionRefused,

    #[error("DNS 解析失败: {0}")]
    DnsFailed(String),
}

impl ConnectionError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ConnectionError::Timeout => ErrorSeverity::Medium,
            ConnectionError::HostUnreachable(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ConnectionError::Timeout | ConnectionError::ConnectionRefused
        )
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            ConnectionError::Timeout => Duration::from_secs(5),
            ConnectionError::ConnectionRefused => Duration::from_secs(10),
            _ => Duration::from_secs(5),
        }
    }
}

/// 认证错误
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("用户名或密码错误")]
    InvalidCredentials,

    #[error("账号被锁定")]
    AccountLocked,

    #[error("需要应用专用密码")]
    AppPasswordRequired,

    #[error("需要双因素认证")]
    TwoFactorRequired,

    #[error("OAuth Token 已过期")]
    OAuthExpired,

    #[error("OAuth 授权已撤销")]
    OAuthRevoked,
}

impl AuthError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AuthError::OAuthExpired => ErrorSeverity::Medium,
            AuthError::TwoFactorRequired => ErrorSeverity::High,
            _ => ErrorSeverity::High,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, AuthError::OAuthExpired)
    }

    pub fn retry_delay(&self) -> Duration {
        Duration::from_secs(0)
    }
}

/// 同步错误
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("文件夹不存在: {0}")]
    FolderNotFound(String),

    #[error("邮件数据损坏: {0}")]
    MessageCorrupted(String),

    #[error("无效的 UID: {0}")]
    UidInvalid(u32),

    #[error("存储配额已满")]
    QuotaExceeded,

    #[error("部分同步失败: {success}/{total}")]
    PartialFailure { success: usize, total: usize },

    #[error("UIDVALIDITY 变化，需要重新同步")]
    UidValidityChanged,

    #[error("同步状态不一致")]
    StateInconsistent,

    #[error("同步已被取消")]
    Cancelled,
}

impl SyncError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            SyncError::Cancelled => ErrorSeverity::Low,
            SyncError::UidValidityChanged => ErrorSeverity::Medium,
            SyncError::PartialFailure { .. } => ErrorSeverity::Medium,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        !matches!(self, SyncError::Cancelled)
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            SyncError::UidValidityChanged => Duration::from_secs(0),
            _ => Duration::from_secs(5),
        }
    }
}

/// OAuth 错误
#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Token 已过期")]
    TokenExpired,

    #[error("Token 刷新失败: {0}")]
    RefreshFailed(String),

    #[error("无效的授权码")]
    InvalidGrant,

    #[error("访问被拒绝")]
    AccessDenied,

    #[error("网络错误: {0}")]
    NetworkError(String),
}

impl OAuthError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            OAuthError::TokenExpired => ErrorSeverity::Medium,
            OAuthError::AccessDenied => ErrorSeverity::Critical,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            OAuthError::TokenExpired | OAuthError::RefreshFailed(_)
        )
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            OAuthError::TokenExpired => Duration::from_secs(0),
            _ => Duration::from_secs(5),
        }
    }
}

/// 存储错误
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("Keyring 错误: {0}")]
    Keyring(String),

    #[error("数据不存在: {0}")]
    NotFound(String),

    #[error("文件 IO 错误: {0}")]
    Io(String),
}

impl StorageError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            StorageError::NotFound(_) => ErrorSeverity::Medium,
            StorageError::Io(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Critical,
        }
    }

    pub fn is_retryable(&self) -> bool {
        false
    }

    pub fn retry_delay(&self) -> Duration {
        Duration::from_secs(0)
    }
}

// 实现 From trait 用于自动转换
impl From<sea_orm::DbErr> for MailError {
    fn from(err: sea_orm::DbErr) -> Self {
        MailError::Storage(StorageError::Database(err.to_string()))
    }
}

impl From<tokio::task::JoinError> for MailError {
    fn from(err: tokio::task::JoinError) -> Self {
        MailError::Internal(format!("任务执行错误: {}", err))
    }
}

impl MailError {
    /// 获取错误严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            MailError::Connection(e) => e.severity(),
            MailError::Authentication(e) => e.severity(),
            MailError::Sync(e) => e.severity(),
            MailError::OAuth(e) => e.severity(),
            MailError::Storage(e) => e.severity(),
            MailError::RateLimit { .. } => ErrorSeverity::Low,
            MailError::Internal(_) => ErrorSeverity::Critical,
        }
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self.severity(), ErrorSeverity::Low | ErrorSeverity::Medium)
    }

    /// 获取建议的重试延迟（秒）
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            MailError::RateLimit { retry_after } => Some(Duration::from_secs(*retry_after)),
            MailError::Connection(e) => Some(e.retry_delay()),
            MailError::Authentication(AuthError::OAuthExpired) => Some(Duration::from_secs(0)),
            MailError::Sync(e) => Some(e.retry_delay()),
            MailError::OAuth(e) => Some(e.retry_delay()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_severity() {
        let err = ConnectionError::Timeout;
        assert_eq!(err.severity(), ErrorSeverity::Medium);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_auth_error_severity() {
        let err = AuthError::InvalidCredentials;
        assert_eq!(err.severity(), ErrorSeverity::High);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_oauth_error_retryable() {
        let err = AuthError::OAuthExpired;
        assert!(err.is_retryable());
    }

    #[test]
    fn test_sync_error_cancelled() {
        let err = SyncError::Cancelled;
        assert_eq!(err.severity(), ErrorSeverity::Low);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_mail_error_conversion() {
        let conn_err = MailError::from(ConnectionError::Timeout);
        assert_eq!(conn_err.severity(), ErrorSeverity::Medium);

        let auth_err = MailError::from(AuthError::InvalidCredentials);
        assert_eq!(auth_err.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_rate_limit_error() {
        let err = MailError::RateLimit { retry_after: 60 };
        assert_eq!(err.severity(), ErrorSeverity::Low);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(Duration::from_secs(60)));
    }
}
