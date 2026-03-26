//! OAuth 回调处理模块
//!
//! 处理 OAuth 回调的业务逻辑，包括 Token 交换和账号创建

use std::sync::Arc;

use crate::auth::AuthManager;
use crate::command::{DatabaseState, KeyringState};
use crate::providers;
use crate::storage;
use crate::storage::AccountDto;

/// OAuth 回调处理器
///
/// 负责处理 OAuth 回调的业务逻辑，包括 Token 交换和账号创建
pub struct OAuthCallbackHandler {
    /// 认证管理器
    auth_manager: Arc<AuthManager>,
}

impl OAuthCallbackHandler {
    /// 创建新的回调处理器
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self { auth_manager }
    }

    /// 获取认证管理器的引用
    pub fn auth_manager(&self) -> Arc<AuthManager> {
        Arc::clone(&self.auth_manager)
    }

    /// 处理 OAuth 回调，交换授权码并创建账号
    ///
    /// # 参数
    ///
    /// * `db_state` - 数据库状态
    /// * `keyring_state` - Keyring 状态
    /// * `email` - 邮箱地址
    /// * `code` - OAuth 授权码
    /// * `state` - OAuth 状态参数
    ///
    /// # 返回
    ///
    /// 返回创建的账号信息
    pub async fn handle_callback(
        &self,
        db_state: DatabaseState,
        keyring_state: KeyringState,
        email: &str,
        code: &str,
        state: &str,
    ) -> Result<AccountDto, String> {
        // 打印接收到的参数（用于调试）
        tracing::info!("========== 交换 OAuth Token ==========");
        tracing::info!("Email: {}", email);
        tracing::info!(
            "Code (前20字符): {}",
            &code.chars().take(20).collect::<String>()
        );
        tracing::info!("Code 长度: {}", code.len());
        tracing::info!("State: {}", state);
        tracing::info!("====================================");

        // 使用 AuthManager 进行 OAuth 认证
        let auth_result = self
            .auth_manager
            .authenticate_oauth(email, code, state)
            .await
            .map_err(|e| e.to_string())?;

        // 获取服务商配置
        let provider_pool = self.auth_manager.provider_pool();
        let provider = provider_pool
            .detect_provider(email)
            .await
            .map_err(|e| e.to_string())?;

        let imap_config = provider.imap_config(email);
        let smtp_config = provider.smtp_config(email);

        // 缓存 provider_info 避免多次调用
        let provider_info = provider.provider_info();

        // 构建账号创建请求
        let account_req = storage::CreateAccountRequest {
            name: auth_result
                .display_name
                .unwrap_or_else(|| email.split('@').next().unwrap_or("用户").to_string()),
            email: auth_result.email.clone(),
            provider: provider_info.id.clone(),
            password: String::new(),
            imap_host: Some(imap_config.host),
            imap_port: Some(imap_config.port as i32),
            imap_ssl: Some(matches!(
                imap_config.ssl,
                providers::SslMode::Implicit | providers::SslMode::StartTls
            )),
            smtp_host: Some(smtp_config.host),
            smtp_port: Some(smtp_config.port as i32),
            smtp_ssl: Some(matches!(smtp_config.ssl, providers::SslMode::StartTls)),
            color: Some("#0078D4".to_string()),
            auth_type: Some("oauth2".to_string()),
            oauth_provider: Some(provider_info.id.clone()),
            oauth_token: auth_result.id_token,
            oauth_refresh_token: Some(String::new()),
            oauth_expires_at: auth_result.expires_at,
        };

        // 创建账号
        let db = db_state.clone_conn();
        let account = storage::AccountRepository::create(&db, &keyring_state.app_handle, account_req)
            .await
            .map_err(|e| e.to_string())?;

        // 迁移 Token（从临时 account_id 0 迁移到真实 account_id）
        let token_manager = self.auth_manager.token_manager();
        token_manager
            .migrate_token_account(0, account.id)
            .await
            .map_err(|e| e.to_string())?;

        tracing::info!("OAuth 回调处理成功，创建账号: {}", account.email);

        Ok(account.into())
    }
}

/// URL 解码辅助函数
///
/// # 参数
///
/// * `value` - URL 编码的字符串
///
/// # 返回
///
/// 返回解码后的字符串
pub fn url_decode(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            // 读取接下来的两个十六进制字符
            let hex1 = chars.next();
            let hex2 = chars.next();

            if let (Some(h1), Some(h2)) = (hex1, hex2) {
                // 解析十六进制
                if let (Some(d1), Some(d2)) = (h1.to_digit(16), h2.to_digit(16)) {
                    let byte = (d1 * 16 + d2) as u8;
                    result.push(byte as char);
                } else {
                    // 无效的十六进制，保持原样
                    result.push(c);
                    if let Some(h) = hex1 {
                        result.push(h);
                    }
                }
            } else {
                // 不完整的转义序列，保持原样
                result.push(c);
            }
        } else if c == '+' {
            // URL 编码中 '+' 表示空格
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("test%40example.com"), "test@example.com");
        assert_eq!(url_decode("abc+def"), "abc def");
    }
}
