//! 服务商池
//!
//! 管理所有邮件服务商实例

use crate::error::{Result, MailError};
use super::{MailProvider, AccountType};

/// 服务商池
pub struct ProviderPool {
    providers: Vec<Box<dyn MailProvider>>,
}

impl ProviderPool {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// 注册服务商
    pub fn register(&mut self, provider: Box<dyn MailProvider>) {
        self.providers.push(provider);
    }

    /// 根据邮箱地址检测服务商
    pub async fn detect_provider(&self, email: &str) -> Result<Box<dyn MailProvider>> {
        for provider in &self.providers {
            if provider.detect(email).await? {
                return Ok(provider.box_clone());
            }
        }

        Err(MailError::Internal(format!("未找到适合的服务商: {}", email)))
    }

    /// 获取所有服务商
    pub fn list_providers(&self) -> Vec<Box<dyn MailProvider>> {
        self.providers.iter().map(|p| p.box_clone()).collect()
    }

    /// 按账号类型筛选服务商
    pub fn list_by_type(&self, account_type: AccountType) -> Vec<Box<dyn MailProvider>> {
        self.providers
            .iter()
            .filter(|p| p.account_type() == account_type)
            .map(|p| p.box_clone())
            .collect()
    }
}

impl Default for ProviderPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities};
    use async_trait::async_trait;

    struct MockProvider {
        id: &'static str,
        name: &'static str,
        account_type: AccountType,
    }

    #[async_trait]
    impl MailProvider for MockProvider {
        fn provider_id(&self) -> &str { self.id }
        fn provider_name(&self) -> &str { self.name }
        fn account_type(&self) -> AccountType { self.account_type }
        fn auth_types(&self) -> Vec<AuthType> { vec![AuthType::Password] }
        fn default_imap_config(&self) -> ImapServerConfig { Default::default() }
        fn default_smtp_config(&self) -> SmtpServerConfig { Default::default() }
        fn capabilities(&self) -> ProviderCapabilities { Default::default() }
        async fn detect(&self, email: &str) -> Result<bool> {
            Ok(email.ends_with("@example.com"))
        }
        fn supported_domains(&self) -> Vec<&'static str> { vec!["example.com"] }
        fn box_clone(&self) -> Box<dyn MailProvider> {
            Box::new(MockProvider {
                id: self.id,
                name: self.name,
                account_type: self.account_type,
            })
        }
    }

    #[tokio::test]
    async fn test_provider_pool() {
        let mut pool = ProviderPool::new();

        let provider = Box::new(MockProvider {
            id: "test",
            name: "Test Provider",
            account_type: AccountType::Personal,
        });

        pool.register(provider);

        // 测试检测
        let detected = pool.detect_provider("user@example.com").await;
        assert!(detected.is_ok());

        // 测试未找到
        let not_found = pool.detect_provider("user@other.com").await;
        assert!(not_found.is_err());
    }
}
