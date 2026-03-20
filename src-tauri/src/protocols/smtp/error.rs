//! SMTP 错误类型定义
//!
//! 定义 SMTP 协议操作中可能出现的所有错误类型。
//! 使用 `thiserror` 提供结构化的错误信息和良好的错误链追踪。
//!
//! # 错误分类
//!
//! ## 连接错误 (Connection)
//!
//! 表示无法建立到 SMTP 服务器的网络连接。
//!
//! **可能的原因**:
//! - 网络不可达
//! - 服务器地址错误
//! - 连接超时
//! - 证书验证失败
//! - 服务器拒绝连接
//!
//! ## 认证错误 (Authentication)
//!
//! 表示用户身份验证失败。
//!
//! **可能的原因**:
//! - 用户名或密码错误
//! - OAuth 令牌无效或过期
//! - 认证方式不支持
//! - 账号被锁定
//!
//! ## 发送错误 (SendFailed)
//!
//! 表示邮件发送过程中出现错误。
//!
//! **可能的原因**:
//! - 收件人地址无效
//! - 邮件大小超过限制
//! - 服务器存储空间不足
//! - 被反垃圾邮件规则拦截
//! - 网络中断
//!
//! ## 构建错误 (BuildFailed)
//!
//! 表示邮件构建过程中出现错误。
//!
//! **可能的原因**:
//! - 收件人地址格式错误
//! - 附件编码失败
//! - MIME 构建失败
//!
//! ## 配置错误 (Config)
//!
//! 表示 SMTP 配置无效。
//!
//! **可能的原因**:
//! - 端口号无效
//! - 主机名无效
//! - 缺少必要的配置项
//!
//! # 错误处理策略
//!
//! ## 严重程度 (severity)
//!
//! | 级别 | 说明 | 错误类型 |
//! |------|------|---------|
//! | Critical | 配置错误，必须修复 | Config |
//! | High | 严重错误，影响功能 | Connection, Authentication, NotConnected |
//! | Medium | 中等错误，可能重试 | SendFailed, BuildFailed |
//! | Low | 轻微错误，可忽略 | Unsupported |
//!
//! ## 重试策略 (is_retryable)
//!
//! - **可重试**: Connection, SendFailed
//! - **不可重试**: Authentication, Config, Unsupported
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use crate::protocols::smtp::{SmtpError, SmtpResult};
//!
//! async fn send_email() -> SmtpResult<()> {
//!     // ... 发送逻辑 ...
//!     Err(SmtpError::Connection("超时".to_string()))
//! }
//!
//! // 处理错误
//! match send_email().await {
//!     Ok(_) => println!("邮件已发送"),
//!     Err(e) => {
//!         if e.is_retryable() {
//!             println!("可重试的错误: {}", e);
//!         } else {
//!             println!("不可重试的错误: {}", e);
//!         }
//!     }
//! }
//! ```

use thiserror::Error;

/// SMTP 错误类型
///
/// 表示 SMTP 协议操作中可能出现的各种错误情况。
#[derive(Error, Debug)]
pub enum SmtpError {
    /// 连接错误
    ///
    /// 表示无法建立到 SMTP 服务器的网络连接。
    #[error("SMTP 连接错误: {0}")]
    Connection(String),

    /// 认证错误
    ///
    /// 表示用户身份验证失败。
    #[error("SMTP 认证失败: {0}")]
    Authentication(String),

    /// 发送错误
    ///
    /// 表示邮件发送过程中出现错误。
    #[error("SMTP 发送失败: {0}")]
    SendFailed(String),

    /// 邮件构建错误
    ///
    /// 表示邮件构建过程中出现错误。
    #[error("邮件构建失败: {0}")]
    BuildFailed(String),

    /// 配置错误
    ///
    /// 表示 SMTP 配置无效。
    #[error("SMTP 配置错误: {0}")]
    Config(String),

    /// 未连接
    ///
    /// 表示尝试在未连接状态下执行操作。
    #[error("SMTP 未连接")]
    NotConnected,

    /// 不支持的操作
    ///
    /// 表示请求的操作不支持。
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
        matches!(self, SmtpError::Connection(_) | SmtpError::SendFailed(_))
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
