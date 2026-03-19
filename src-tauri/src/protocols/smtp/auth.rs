//! SMTP 认证类型定义

use serde::{Deserialize, Serialize};

/// SMTP 认证方法
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SmtpAuth {
    /// 密码认证
    Password(String),

    /// OAuth2 认证（XOAUTH2）
    OAuth2(String),
}

impl SmtpAuth {
    /// 获取认证的用户名部分
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
