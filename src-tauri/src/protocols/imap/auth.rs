//! IMAP 认证类型定义
//!
//! 定义了 IMAP 协议支持的认证方式。
//!
//! # 支持的认证方式
//!
//! ## 传统密码认证
//!
//! 使用用户名和密码进行认证，通过 IMAP `LOGIN` 命令实现。
//! 适用于大多数邮件服务商，但需要应用专用密码（App Password）。
//!
//! ## OAuth2 认证
//!
//! 使用 OAuth2 访问令牌进行认证，通过 IMAP `SASL` 和 `XOAUTH2` 机制实现。
//! 优点：
//! - 无需存储用户密码
//! - 可以精细控制权限
//! - 令牌可以撤销和刷新
//!
//! 支持的服务商：
//! - Google (Gmail)
//! - Microsoft (Outlook, Office 365)
//!
//! # 示例
//!
//! ```rust
//! use imap::auth::ImapAuth;
//!
//! // 传统密码认证
//! let password_auth = ImapAuth::Password("app_password".to_string());
//!
//! // OAuth2 认证
//! let oauth_auth = ImapAuth::OAuth2 {
//!     email: "user@example.com".to_string(),
//!     access_token: "ya29.a0AfH6...".to_string(),
//! };
//! ```

/// IMAP 认证方式
///
/// 表示 IMAP 连接时使用的认证信息。
///
/// # 变体说明
///
/// ## Password
///
/// 传统密码认证，使用 IMAP `LOGIN` 命令。
///
/// **注意**: 对于 Gmail 等服务商，需要使用应用专用密码而非账号密码。
///
/// ## OAuth2
///
/// OAuth2 认证，使用 SASL XOAUTH2 机制。
///
/// **字段**:
/// - `email`: OAuth 邮箱地址（通常与账号邮箱相同）
/// - `access_token`: OAuth2 访问令牌
///
/// **生成格式**:
/// ```text
/// user={email}\x01auth=Bearer {access_token}\x01\x01
/// ```
///
/// # 使用建议
///
/// - 优先使用 OAuth2 认证，更安全且支持更精细的权限控制
/// - 对于不支持 OAuth2 的服务商，回退到密码认证
/// - 密码认证时应使用应用专用密码，而非账号主密码
#[derive(Debug, Clone)]
pub enum ImapAuth {
    /// 传统密码认证
    ///
    /// 使用 IMAP LOGIN 命令进行认证。
    /// 需要提供应用专用密码（App Password）而非账号密码。
    Password(String),

    /// OAuth2 认证
    ///
    /// 使用 SASL XOAUTH2 机制进行认证。
    /// 访问令牌应该从 OAuth2 流程中获取并定期刷新。
    OAuth2 {
        /// OAuth 邮箱地址（可能与认证邮箱不同）
        email: String,
        /// OAuth 访问令牌
        access_token: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_display() {
        let auth = ImapAuth::Password("test".to_string());
        assert!(matches!(auth, ImapAuth::Password(_)));
    }

    #[test]
    fn test_oauth_auth_creation() {
        let auth = ImapAuth::OAuth2 {
            email: "user@example.com".to_string(),
            access_token: "token".to_string(),
        };
        assert!(matches!(auth, ImapAuth::OAuth2 { .. }));
    }
}
