//! SMTP 认证类型定义
//!
//! 定义 SMTP 协议支持的认证方式。
//!
//! # 支持的认证方式
//!
//! ## 密码认证 (LOGIN/PLAIN)
//!
//! 使用用户名和密码进行 SMTP AUTH 认证。
//! 适用于大多数邮件服务商，但需要应用专用密码（App Password）。
//!
//! **注意**: 对于 Gmail 等服务商，需要使用应用专用密码而非账号密码。
//!
//! ## OAuth2 认证 (XOAUTH2)
//!
//! 使用 OAuth2 访问令牌进行 SMTP AUTH 认证。
//!
//! **优点**:
//! - 无需存储用户密码
//! - 可以精细控制权限
//! - 令牌可以撤销和刷新
//!
//! **支持的服务商**:
//! - Google (Gmail)
//! - Microsoft (Outlook, Office 365)
//!
//! # SASL 机制
//!
//! SMTP 认证使用 SASL（Simple Authentication and Security Layer）机制：
//!
//! | 机制 | 描述 | 使用场景 |
//! |------|------|---------|
//! | PLAIN | 用户名和密码明文传输（需要 TLS） | 最常用 |
//! | LOGIN | 与 PLAIN 类似，但分步交互 | 某些旧服务器 |
//! | XOAUTH2 | OAuth2 令牌认证 | 现代服务 |
//!
//! # 使用示例
//!
//! ```rust
//! use crate::protocols::smtp::SmtpAuth;
//!
//! // 密码认证
//! let password_auth = SmtpAuth::Password("app_password".to_string());
//!
//! // OAuth2 认证
//! let oauth_auth = SmtpAuth::OAuth2("ya29.a0AfH6...".to_string());
//! ```

use serde::{Deserialize, Serialize};

/// SMTP 认证方法
///
/// 表示 SMTP 连接时使用的认证信息。
///
/// # 变体说明
///
/// ## Password
///
/// 传统密码认证，使用 SMTP AUTH PLAIN 或 LOGIN 命令。
///
/// **安全提示**:
/// - 必须在 TLS 连接上使用
/// - 建议使用应用专用密码而非账号主密码
/// - 不要在代码中硬编码密码
///
/// ## OAuth2
///
/// OAuth2 认证，使用 SASL XOAUTH2 机制。
///
/// **令牌格式**:
/// ```text
/// user={email}\x01auth=Bearer {access_token}\x01\x01
/// ```
///
/// **获取令牌**: 通过 OAuth2 授权流程获取访问令牌。
///
/// **刷新令牌**: 访问令牌通常 1 小时后过期，需要使用刷新令牌获取新的访问令牌。
///
/// # 使用建议
///
/// - 优先使用 OAuth2 认证，更安全且支持更精细的权限控制
/// - 对于不支持 OAuth2 的服务商，回退到密码认证
/// - 密码认证时应使用应用专用密码，而非账号主密码
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SmtpAuth {
    /// 密码认证
    ///
    /// 使用 SMTP AUTH PLAIN 或 LOGIN 命令进行认证。
    /// 需要提供应用专用密码（App Password）而非账号密码。
    Password(String),

    /// OAuth2 认证（XOAUTH2）
    ///
    /// 使用 SASL XOAUTH2 机制进行认证。
    /// 访问令牌应该从 OAuth2 流程中获取并定期刷新。
    OAuth2(String),
}

impl SmtpAuth {
    /// 获取认证的用户名部分
    ///
    /// **注意**: 当前实现总是返回 None，用户名在连接时单独传入。
    ///
    /// # 返回
    ///
    /// 总是返回 None（保留用于未来扩展）
    pub fn username(&self) -> Option<&str> {
        match self {
            SmtpAuth::Password(_) => None,
            SmtpAuth::OAuth2(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_auth_password() {
        let auth = SmtpAuth::Password("secret".to_string());
        assert_eq!(auth.username(), None);
    }

    #[test]
    fn test_smtp_auth_oauth2() {
        let auth = SmtpAuth::OAuth2("token".to_string());
        assert_eq!(auth.username(), None);
    }
}
