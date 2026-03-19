//! IMAP 认证类型定义
//!
//! 支持多种认证方式：
//! - 传统密码认证
//! - OAuth2/XOAUTH2 认证

/// IMAP 认证方式
#[derive(Debug, Clone)]
pub enum ImapAuth {
    /// 传统密码认证
    Password(String),

    /// OAuth2 认证
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
