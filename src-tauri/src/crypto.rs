/// OAuth Token 结构（保留用于类型定义）
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Keyring Service 名称（常量）
pub const KEYRING_SERVICE: &str = "com.postium.mail";

/// Keyring 辅助函数：生成账号密码的用户名
pub fn password_username(account_id: i32) -> String {
    format!("account_{}", account_id)
}

/// Keyring 辅助函数：生成 OAuth Token 的用户名
pub fn oauth_username(account_id: i32) -> String {
    format!("oauth_{}", account_id)
}

/// 获取密钥存储路径描述（用于日志显示）
pub fn get_vault_path() -> anyhow::Result<String> {
    Ok("[操作系统原生密钥链]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_username() {
        assert_eq!(password_username(1), "account_1");
        assert_eq!(password_username(42), "account_42");
    }

    #[test]
    fn test_oauth_username() {
        assert_eq!(oauth_username(1), "oauth_1");
        assert_eq!(oauth_username(42), "oauth_42");
    }

    #[test]
    fn test_keyring_service() {
        assert_eq!(KEYRING_SERVICE, "com.postium.mail");
    }
}
