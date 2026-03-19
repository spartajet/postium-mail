//! SMTP 错误类型定义

use thiserror::Error;

/// SMTP 错误类型
#[derive(Error, Debug)]
pub enum SmtpError {
    /// 连接错误
    #[error("SMTP 连接错误: {0}")]
    Connection(String),

    /// 认证错误
    #[error("SMTP 认证失败: {0}")]
    Authentication(String),

    /// 发送错误
    #[error("SMTP 发送失败: {0}")]
    SendFailed(String),

    /// 邮件构建错误
    #[error("邮件构建失败: {0}")]
    BuildFailed(String),

    /// 配置错误
    #[error("SMTP 配置错误: {0}")]
    Config(String),

    /// 未连接
    #[error("SMTP 未连接")]
    NotConnected,

    /// 不支持的操作
    #[error("不支持的操作: {0}")]
    Unsupported(String),
}

impl SmtpError {
    /// 获取错误严重程度
    pub fn severity(&self) -> crate::error::ErrorSeverity {
        match self {
            SmtpError::Connection(_) => crate::error::ErrorSeverity::High,
            SmtpError::Authentication(_) => crate::error::ErrorSeverity::High,
            SmtpError::SendFailed(_) => crate::error::ErrorSeverity::Medium,
            SmtpError::BuildFailed(_) => crate::error::ErrorSeverity::Medium,
            SmtpError::Config(_) => crate::error::ErrorSeverity::Critical,
            SmtpError::NotConnected => crate::error::ErrorSeverity::High,
            SmtpError::Unsupported(_) => crate::error::ErrorSeverity::Low,
        }
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SmtpError::Connection(_) | SmtpError::SendFailed(_)
        )
    }
}

/// SMTP 结果类型
pub type SmtpResult<T> = Result<T, SmtpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_error_severity() {
        let err = SmtpError::Connection("timeout".to_string());
        assert_eq!(err.severity(), crate::error::ErrorSeverity::High);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_smtp_error_not_retryable() {
        let err = SmtpError::Config("invalid port".to_string());
        assert_eq!(err.severity(), crate::error::ErrorSeverity::Critical);
        assert!(!err.is_retryable());
    }
}
