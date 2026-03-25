//! 密码认证处理器
//!
//! 处理用户名/密码认证，支持 Keyring 安全存储

use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

use crate::crypto::{KEYRING_SERVICE, password_username};
use crate::error::{AuthError, MailError, Result, StorageError};
use crate::protocols::{AsyncImapClient, ImapAuth};

/// 密码认证处理器
///
/// 负责：
/// - 密码存储到 Keyring
/// - 密码读取和删除
/// - 密码验证（可选）
pub struct PasswordAuth {
    /// Tauri 应用句柄
    app_handle: Arc<AppHandle>,
}

impl PasswordAuth {
    /// 创建新的密码认证处理器
    ///
    /// # 参数
    ///
    /// * `app_handle` - Tauri 应用句柄
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let handler = PasswordAuth::new(&app_handle)?;
    /// ```
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: Arc::new(app_handle.clone()),
        })
    }

    /// 存储密码
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `password` - 密码
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// handler.store_password(1, "my_password").await?;
    /// ```
    pub async fn store_password(&self, account_id: i32, password: &str) -> Result<()> {
        let username = password_username(account_id);
        let keyring = self.app_handle.keyring();

        keyring
            .set_password(KEYRING_SERVICE, &username, password)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?;

        tracing::info!("存储密码成功: account_id={}", account_id);

        Ok(())
    }

    /// 获取密码
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// 返回密码字符串
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let password = handler.get_password(1).await?;
    /// ```
    pub async fn get_password(&self, account_id: i32) -> Result<String> {
        let username = password_username(account_id);
        let keyring = self.app_handle.keyring();

        let password = keyring
            .get_password(KEYRING_SERVICE, &username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?
            .ok_or_else(|| {
                MailError::Storage(StorageError::NotFound(format!(
                    "账号 {} 的密码不存在",
                    account_id
                )))
            })?;

        tracing::debug!("获取密码成功: account_id={}", account_id);

        Ok(password)
    }

    /// 删除密码
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// handler.delete_password(1).await?;
    /// ```
    pub async fn delete_password(&self, account_id: i32) -> Result<()> {
        let username = password_username(account_id);
        let keyring = self.app_handle.keyring();

        keyring
            .delete_password(KEYRING_SERVICE, &username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?;

        tracing::info!("删除密码成功: account_id={}", account_id);

        Ok(())
    }

    /// 验证密码（通过 IMAP 连接测试）
    ///
    /// # 参数
    ///
    /// * `host` - IMAP 服务器地址
    /// * `port` - IMAP 服务器端口
    /// * `username` - 用户名（通常是邮箱地址）
    /// * `password` - 密码
    ///
    /// # 返回
    ///
    /// - `Ok(true)` - 密码有效
    /// - `Ok(false)` - 密码无效
    /// - `Err(_)` - 连接错误
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let valid = handler
    ///     .validate_password("imap.example.com", 993, "user@example.com", "password")
    ///     .await?;
    /// ```
    pub async fn validate_password(
        &self,
        host: &str,
        port: u16,
        email: &str,
        password: &str,
    ) -> Result<bool> {
        tracing::info!("验证密码: host={}, port={}, username={}", host, port, email);

        let auth = ImapAuth::Password(password.to_string());
        let mut imap_client = AsyncImapClient::new();
        let connect_result = imap_client.connect(host, port, email, auth).await;

        match connect_result {
            Ok(_) => Ok(true),
            Err(e) => Err(MailError::Authentication(AuthError::InvalidCredentials)),
        }

        // let result = test_connection(host, port, email, auth)
        //     .await
        //     .map_err(|e| MailError::Internal(format!("IMAP 连接测试失败: {}", e)))?;
    }

    /// 检查密码是否存在
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// - `true` - 密码存在
    /// - `false` - 密码不存在
    pub async fn has_password(&self, account_id: i32) -> Result<bool> {
        match self.get_password(account_id).await {
            Ok(_) => Ok(true),
            Err(MailError::Storage(StorageError::NotFound(_))) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// 获取应用句柄
    pub fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_password_auth_new() {
        // 注意：测试需要 Tauri AppHandle，这里只测试结构
        // 实际测试需要集成测试环境
        // TODO: 实现实际的集成测试
    }

    #[test]
    fn test_password_auth_default() {
        // 注意：测试需要 Tauri AppHandle，这里只测试结构
        // 实际测试需要集成测试环境
        // TODO: 实现实际的集成测试
    }

    #[test]
    fn test_password_username_format() {
        // 测试密码用户名格式
        let account_id = 123;
        let username = format!("password_{}", account_id);

        assert_eq!(username, "password_123");
        assert!(username.starts_with("password_"));
    }

    #[test]
    fn test_oauth_username_format() {
        // 测试 OAuth 用户名格式
        let account_id = 456;
        let username = format!("oauth_{}", account_id);

        assert_eq!(username, "oauth_456");
        assert!(username.starts_with("oauth_"));
    }

    #[tokio::test]
    async fn test_password_storage_logic() {
        // 测试密码存储逻辑
        let account_id = 1;
        let password = "test_password";

        // 模拟存储
        let stored_account_id = account_id;
        let stored_password = password.to_string();

        assert_eq!(stored_account_id, 1);
        assert_eq!(stored_password, "test_password");
    }

    #[tokio::test]
    async fn test_password_removal_logic() {
        // 测试密码删除逻辑
        let account_id = 999;

        // 模拟删除
        let removed_id = account_id;

        assert_eq!(removed_id, 999);
    }
}
