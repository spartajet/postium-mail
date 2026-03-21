//! 服务商池
//!
//! 管理所有邮件服务商实例

use super::{AccountType, AuthType, MailProvider, ProviderCapabilities};
use crate::error::{MailError, Result};

/// 服务商信息（用于前端展示）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    /// 服务商唯一标识
    pub id: String,
    /// 服务商显示名称
    pub name: String,
    /// 账号类型
    pub account_type: AccountType,
    /// 支持的认证类型
    pub auth_types: Vec<AuthType>,
    /// 支持的域名列表
    pub domains: Vec<String>,
    /// 服务商能力
    pub capabilities: ProviderCapabilities,
    /// 是否支持 OAuth
    pub supports_oauth: bool,
}

impl From<&dyn MailProvider> for ProviderInfo {
    fn from(provider: &dyn MailProvider) -> Self {
        Self {
            id: provider.provider_id().to_string(),
            name: provider.provider_name().to_string(),
            account_type: provider.account_type(),
            auth_types: provider.auth_types(),
            domains: provider
                .supported_domains()
                .into_iter()
                .map(String::from)
                .collect(),
            capabilities: provider.capabilities(),
            supports_oauth: provider.oauth_config().is_some(),
        }
    }
}

/// 服务商池
pub struct ProviderPool {
    providers: Vec<Box<dyn MailProvider>>,
}

impl Default for ProviderPool {
    fn default() -> Self {
        use super::enterprise::{CustomProvider, GoogleWorkspaceProvider, Microsoft365Provider};
        use super::personal::{
            AolMailProvider, ChinaMailProvider, CmccMailProvider, Cn21MailProvider,
            GmxMailProvider, ICloudProvider, Mail163Provider, MailComProvider, Net263MailProvider,
            QqMailProvider, SohuMailProvider, TomMailProvider, YandexMailProvider,
            ZohoMailProvider,
        };
        use super::personal::{GmailProvider, OutlookProvider, SinaMailProvider, YahooProvider};

        let mut pool = Self {
            providers: Vec::new(),
        };

        // 个人邮箱服务商
        pool.register(Box::new(GmailProvider));
        pool.register(Box::new(OutlookProvider));
        pool.register(Box::new(YahooProvider));
        pool.register(Box::new(AolMailProvider));
        pool.register(Box::new(GmxMailProvider));
        pool.register(Box::new(MailComProvider));
        pool.register(Box::new(ZohoMailProvider));
        pool.register(Box::new(YandexMailProvider));

        // 国内邮箱服务商
        pool.register(Box::new(Mail163Provider));
        pool.register(Box::new(QqMailProvider));
        pool.register(Box::new(SinaMailProvider));
        pool.register(Box::new(SohuMailProvider));
        pool.register(Box::new(TomMailProvider));
        pool.register(Box::new(CmccMailProvider));
        pool.register(Box::new(ChinaMailProvider));
        pool.register(Box::new(Net263MailProvider));
        pool.register(Box::new(Cn21MailProvider));
        pool.register(Box::new(ICloudProvider));

        // 企业邮箱服务商（使用默认配置）
        pool.register(Box::new(Microsoft365Provider::with_defaults()));
        pool.register(Box::new(GoogleWorkspaceProvider::with_defaults()));
        pool.register(Box::new(CustomProvider::with_servers("Custom", "", "")));

        pool
    }
}

impl ProviderPool {
    /// 注册服务商
    pub fn register(&mut self, provider: Box<dyn MailProvider>) {
        self.providers.push(provider);
    }

    /// 根据邮箱地址智能检测服务商
    ///
    /// 检测策略：
    /// 1. 首先检查个人邮箱服务商的域名
    /// 2. 如果没有匹配，尝试查询 MX 记录检测企业邮箱
    /// 3. 最后回退到自定义服务商
    #[tracing::instrument(
        skip(self),
        fields(
            email,
            provider_count = self.providers.len()
        ),
        level = "info"
    )]
    pub async fn detect_provider(&self, email: &str) -> Result<Box<dyn MailProvider>> {
        tracing::info!("开始检测邮箱服务商: email={}", email);

        // 1. 先尝试域名匹配
        tracing::debug!("步骤 1: 尝试匹配个人邮箱服务商");
        for provider in &self.providers {
            if provider.account_type() == AccountType::Personal {
                tracing::trace!("尝试检测: provider_id={}", provider.provider_id());
                if provider.detect(email).await? {
                    tracing::info!(
                        "匹配到个人邮箱服务商: email={}, provider={}",
                        email,
                        provider.provider_id()
                    );
                    return Ok(provider.box_clone());
                }
            }
        }

        // 2. 尝试 MX 记录检测企业邮箱
        tracing::debug!("步骤 2: 未匹配到个人邮箱，尝试检测企业邮箱");
        let domain = email.split('@').nth(1).ok_or_else(|| {
            tracing::error!("无效的邮箱地址: email={}", email);
            MailError::Internal("无效的邮箱地址".to_string())
        })?;

        tracing::debug!("提取域名: domain={}", domain);

        if let Some(provider) = self.detect_enterprise_provider(domain).await {
            tracing::info!(
                "匹配到企业邮箱服务商: email={}, domain={}, provider={}",
                email,
                domain,
                provider.provider_id()
            );
            return Ok(provider);
        }

        tracing::warn!(
            "未检测到企业邮箱服务商，回退到自定义配置: domain={}",
            domain
        );

        // 3. 回退到自定义服务商
        for provider in &self.providers {
            if provider.provider_id() == "custom" {
                tracing::info!("使用自定义服务商: email={}", email);
                return Ok(provider.box_clone());
            }
        }

        tracing::error!(
            "未找到适合的服务商: email={}, 已尝试 {} 个服务商",
            email,
            self.providers.len()
        );
        Err(MailError::Internal(format!(
            "未找到适合的服务商: {}",
            email
        )))
    }

    /// 通过 MX 记录检测企业邮箱服务商
    ///
    /// 返回识别到的服务商，如果没有识别到则返回 None
    #[tracing::instrument(skip(self), fields(domain), level = "debug")]
    pub async fn detect_enterprise_provider(&self, domain: &str) -> Option<Box<dyn MailProvider>> {
        tracing::debug!("开始检测企业邮箱服务商: domain={}", domain);

        // TODO: 实现 MX 记录查询
        // 当前使用简单的启发式规则

        // Microsoft 365 特征域名
        tracing::trace!("检查是否为 Microsoft 365 域名: domain={}", domain);
        if self.is_microsoft_365_domain(domain).await {
            tracing::info!("识别为 Microsoft 365 企业邮箱: domain={}", domain);
            return self.find_provider_by_id("microsoft365");
        }

        // Google Workspace 特征域名
        tracing::trace!("检查是否为 Google Workspace 域名: domain={}", domain);
        if self.is_google_workspace_domain(domain).await {
            tracing::info!("识别为 Google Workspace 企业邮箱: domain={}", domain);
            return self.find_provider_by_id("googleworkspace");
        }

        tracing::debug!("未识别到已知企业邮箱服务商: domain={}", domain);
        None
    }

    /// 检查是否为 Microsoft 365 域名
    async fn is_microsoft_365_domain(&self, domain: &str) -> bool {
        // 常见的 Microsoft 365 MX 记录模式
        let _microsoft_mx_patterns = ["*.mail.protection.outlook.com", "*.outlook.com"];

        // TODO: 实际查询 MX 记录
        // 当前使用已知的企业域名列表作为替代
        let _known_enterprise_domains: [&str; 0] = [
            // 企业通常使用自己的域名
        ];

        // 简单的启发式：如果域名不是常见的个人邮箱域名，
        // 且以 .com、.org、.net 等顶级域名结尾，可能是企业邮箱
        let common_personal_domains = [
            "gmail.com",
            "googlemail.com",
            "yahoo.com",
            "yahoo.co.uk",
            "hotmail.com",
            "outlook.com",
            "live.com",
            "aol.com",
            "aim.com",
            "gmx.com",
            "gmx.net",
            "gmx.de",
            "mail.com",
            "zoho.com",
            "yandex.com",
            "yandex.ru",
            "163.com",
            "126.com",
            "qq.com",
            "foxmail.com",
            "sina.com",
            "sina.cn",
            "sohu.com",
            "sohu.net",
            "tom.com",
            "139.com",
            "139.com.cn",
            "10086.cn",
            "10086.com",
            "china.com",
            "263.net",
            "263.com",
            "21cn.com",
            "21cn.net",
            "icloud.com",
            "me.com",
        ];

        if common_personal_domains.contains(&domain) {
            return false;
        }

        // TODO: 这里应该查询实际的 MX 记录
        // 当前返回 false，等待实际实现
        false
    }

    /// 检查是否为 Google Workspace 域名
    async fn is_google_workspace_domain(&self, domain: &str) -> bool {
        // Google Workspace MX 记录模式
        let _google_mx_patterns = ["*.gmail.com", "googlemail.com"];

        // TODO: 实际查询 MX 记录
        // 当前返回 false，等待实际实现
        false
    }

    /// 根据 ID 查找服务商
    fn find_provider_by_id(&self, id: &str) -> Option<Box<dyn MailProvider>> {
        for provider in &self.providers {
            if provider.provider_id() == id {
                return Some(provider.box_clone());
            }
        }
        None
    }

    /// 根据邮箱地址快速获取服务商信息（不检测）
    pub fn get_provider_info_by_email(&self, email: &str) -> Vec<ProviderInfo> {
        let domain = email.split('@').nth(1).unwrap_or("");

        self.providers
            .iter()
            .filter(|p| p.supported_domains().iter().any(|d| d == &domain))
            .map(|p| ProviderInfo::from(p.as_ref()))
            .collect()
    }

    /// 获取所有服务商信息
    pub fn get_providers_info(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .map(|p| ProviderInfo::from(p.as_ref()))
            .collect()
    }

    /// 按账号类型筛选服务商
    pub fn get_providers_by_type(&self, account_type: AccountType) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .filter(|p| p.account_type() == account_type)
            .map(|p| ProviderInfo::from(p.as_ref()))
            .collect()
    }

    /// 获取所有服务商实例
    pub fn list_providers(&self) -> Vec<Box<dyn MailProvider>> {
        self.providers.iter().map(|p| p.box_clone()).collect()
    }

    /// 按账号类型筛选服务商实例
    pub fn list_by_type(&self, account_type: AccountType) -> Vec<Box<dyn MailProvider>> {
        self.providers
            .iter()
            .filter(|p| p.account_type() == account_type)
            .map(|p| p.box_clone())
            .collect()
    }

    /// 根据邮箱地址检测服务商（旧方法，保留向后兼容）
    pub async fn detect(&self, email: &str) -> Result<Box<dyn MailProvider>> {
        self.detect_provider(email).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ImapServerConfig, SmtpServerConfig};
    use super::*;
    use async_trait::async_trait;
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    struct MockProvider {
        id: &'static str,
        name: &'static str,
        account_type: AccountType,
    }

    #[async_trait]
    impl MailProvider for MockProvider {
        fn provider_id(&self) -> &str {
            self.id
        }
        fn provider_name(&self) -> &str {
            self.name
        }
        fn account_type(&self) -> AccountType {
            self.account_type
        }
        fn auth_types(&self) -> Vec<AuthType> {
            vec![AuthType::Password]
        }
        fn imap_config(&self, _email: &str) -> ImapServerConfig {
            Default::default()
        }
        fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
            Default::default()
        }
        fn capabilities(&self) -> ProviderCapabilities {
            Default::default()
        }
        async fn detect(&self, email: &str) -> Result<bool> {
            Ok(email.ends_with("@example.com"))
        }
        fn supported_domains(&self) -> Vec<&'static str> {
            vec!["example.com"]
        }
        fn box_clone(&self) -> Box<dyn MailProvider> {
            Box::new(MockProvider {
                id: self.id,
                name: self.name,
                account_type: self.account_type,
            })
        }
    }

    #[tokio::test]
    async fn test_provider_pool_detection() {
        init_tracing();
        let mut pool = ProviderPool::default();

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

    #[tokio::test]
    async fn test_default() {
        init_tracing();
        let pool = ProviderPool::default();

        // 应该包含所有默认服务商
        let providers = pool.get_providers_info();
        assert!(providers.len() >= 21); // Gmail, Outlook, Yahoo, AOL, GMX, Mail.com, Zoho, Yandex, 163(yi), QQ, Sina, Sohu, Tom, CMCC, China, 263, 21CN, iCloud, Microsoft365, GoogleWorkspace, Custom

        // 验证包含关键服务商
        let ids: Vec<_> = providers.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&"gmail".to_string()));
        assert!(ids.contains(&"outlook".to_string()));
        assert!(ids.contains(&"yahoo".to_string()));
        assert!(ids.contains(&"aol".to_string()));
        assert!(ids.contains(&"gmx".to_string()));
        assert!(ids.contains(&"mailcom".to_string()));
        assert!(ids.contains(&"zoho".to_string()));
        assert!(ids.contains(&"yandex".to_string()));
        assert!(ids.contains(&"yi".to_string())); // 163邮箱的ID是"yi"
        assert!(ids.contains(&"qq".to_string()));
        assert!(ids.contains(&"sina".to_string()));
        assert!(ids.contains(&"sohu".to_string()));
        assert!(ids.contains(&"tom".to_string()));
        assert!(ids.contains(&"cmcc".to_string()));
        assert!(ids.contains(&"china".to_string()));
        assert!(ids.contains(&"net263".to_string()));
        assert!(ids.contains(&"cn21".to_string()));
        assert!(ids.contains(&"icloud".to_string()));
        assert!(ids.contains(&"microsoft365".to_string()));
        assert!(ids.contains(&"google-workspace".to_string()));
        assert!(ids.contains(&"custom".to_string()));
    }

    #[tokio::test]
    async fn test_get_providers_by_type() {
        init_tracing();
        let pool = ProviderPool::default();

        let personal = pool.get_providers_by_type(AccountType::Personal);
        assert!(!personal.is_empty());

        let enterprise = pool.get_providers_by_type(AccountType::Enterprise);
        assert!(!enterprise.is_empty());
    }

    #[tokio::test]
    async fn test_detect_gmail() {
        init_tracing();
        let pool = ProviderPool::default();

        let provider = pool.detect_provider("user@gmail.com").await.unwrap();
        assert_eq!(provider.provider_id(), "gmail");
    }

    #[tokio::test]
    async fn test_detect_outlook() {
        init_tracing();
        let pool = ProviderPool::default();

        let provider = pool.detect_provider("user@outlook.com").await.unwrap();
        assert_eq!(provider.provider_id(), "outlook");
    }

    #[tokio::test]
    async fn test_detect_mail163() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 163.com
        let provider = pool.detect_provider("user@163.com").await.unwrap();
        assert_eq!(provider.provider_id(), "yi");

        // 测试 126.com
        let provider = pool.detect_provider("user@126.com").await.unwrap();
        assert_eq!(provider.provider_id(), "yi");

        // 测试 yeah.net
        let provider = pool.detect_provider("user@yeah.net").await.unwrap();
        assert_eq!(provider.provider_id(), "yi");
    }

    #[tokio::test]
    async fn test_detect_qqmail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 qq.com
        let provider = pool.detect_provider("user@qq.com").await.unwrap();
        assert_eq!(provider.provider_id(), "qq");

        // 测试 foxmail.com
        let provider = pool.detect_provider("user@foxmail.com").await.unwrap();
        assert_eq!(provider.provider_id(), "qq");
    }

    #[tokio::test]
    async fn test_detect_icloud() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 icloud.com
        let provider = pool.detect_provider("user@icloud.com").await.unwrap();
        assert_eq!(provider.provider_id(), "icloud");

        // 测试 me.com
        let provider = pool.detect_provider("user@me.com").await.unwrap();
        assert_eq!(provider.provider_id(), "icloud");

        // 测试 mac.com
        let provider = pool.detect_provider("user@mac.com").await.unwrap();
        assert_eq!(provider.provider_id(), "icloud");
    }

    #[tokio::test]
    async fn test_detect_sinamail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 sina.com
        let provider = pool.detect_provider("user@sina.com").await.unwrap();
        assert_eq!(provider.provider_id(), "sina");

        // 测试 sina.cn
        let provider = pool.detect_provider("user@sina.cn").await.unwrap();
        assert_eq!(provider.provider_id(), "sina");

        // 测试 vip.sina.com
        let provider = pool.detect_provider("user@vip.sina.com").await.unwrap();
        assert_eq!(provider.provider_id(), "sina");
    }

    #[tokio::test]
    async fn test_detect_cmccmail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 139.com
        let provider = pool.detect_provider("user@139.com").await.unwrap();
        assert_eq!(provider.provider_id(), "cmcc");

        // 测试 139.com.cn
        let provider = pool.detect_provider("user@139.com.cn").await.unwrap();
        assert_eq!(provider.provider_id(), "cmcc");

        // 测试 10086.cn
        let provider = pool.detect_provider("user@10086.cn").await.unwrap();
        assert_eq!(provider.provider_id(), "cmcc");

        // 测试 10086.com
        let provider = pool.detect_provider("user@10086.com").await.unwrap();
        assert_eq!(provider.provider_id(), "cmcc");
    }

    #[tokio::test]
    async fn test_detect_sohumail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 sohu.com
        let provider = pool.detect_provider("user@sohu.com").await.unwrap();
        assert_eq!(provider.provider_id(), "sohu");

        // 测试 vip.sohu.com
        let provider = pool.detect_provider("user@vip.sohu.com").await.unwrap();
        assert_eq!(provider.provider_id(), "sohu");

        // 测试 sohu.net
        let provider = pool.detect_provider("user@sohu.net").await.unwrap();
        assert_eq!(provider.provider_id(), "sohu");
    }

    #[tokio::test]
    async fn test_detect_chinamail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 china.com
        let provider = pool.detect_provider("user@china.com").await.unwrap();
        assert_eq!(provider.provider_id(), "china");

        // 测试 mail.china.com
        let provider = pool.detect_provider("user@mail.china.com").await.unwrap();
        assert_eq!(provider.provider_id(), "china");
    }

    #[tokio::test]
    async fn test_detect_tommail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 tom.com
        let provider = pool.detect_provider("user@tom.com").await.unwrap();
        assert_eq!(provider.provider_id(), "tom");

        // 测试 mail.tom.com
        let provider = pool.detect_provider("user@mail.tom.com").await.unwrap();
        assert_eq!(provider.provider_id(), "tom");

        // 测试 163.tom.com
        let provider = pool.detect_provider("user@163.tom.com").await.unwrap();
        assert_eq!(provider.provider_id(), "tom");
    }

    #[tokio::test]
    async fn test_detect_net263mail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 263.net
        let provider = pool.detect_provider("user@263.net").await.unwrap();
        assert_eq!(provider.provider_id(), "net263");

        // 测试 263.com
        let provider = pool.detect_provider("user@263.com").await.unwrap();
        assert_eq!(provider.provider_id(), "net263");

        // 测试 x263.net
        let provider = pool.detect_provider("user@x263.net").await.unwrap();
        assert_eq!(provider.provider_id(), "net263");
    }

    #[tokio::test]
    async fn test_detect_cn21mail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 21cn.com
        let provider = pool.detect_provider("user@21cn.com").await.unwrap();
        assert_eq!(provider.provider_id(), "cn21");

        // 测试 21cn.net
        let provider = pool.detect_provider("user@21cn.net").await.unwrap();
        assert_eq!(provider.provider_id(), "cn21");

        // 测试 mail.21cn.com
        let provider = pool.detect_provider("user@mail.21cn.com").await.unwrap();
        assert_eq!(provider.provider_id(), "cn21");
    }

    #[tokio::test]
    async fn test_detect_aolmail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 aol.com
        let provider = pool.detect_provider("user@aol.com").await.unwrap();
        assert_eq!(provider.provider_id(), "aol");

        // 测试 aim.com
        let provider = pool.detect_provider("user@aim.com").await.unwrap();
        assert_eq!(provider.provider_id(), "aol");

        // 测试 verizon.net
        let provider = pool.detect_provider("user@verizon.net").await.unwrap();
        assert_eq!(provider.provider_id(), "aol");
    }

    #[tokio::test]
    async fn test_detect_gmxmail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 gmx.com
        let provider = pool.detect_provider("user@gmx.com").await.unwrap();
        assert_eq!(provider.provider_id(), "gmx");

        // 测试 gmx.de
        let provider = pool.detect_provider("user@gmx.de").await.unwrap();
        assert_eq!(provider.provider_id(), "gmx");

        // 测试 gmx.net
        let provider = pool.detect_provider("user@gmx.net").await.unwrap();
        assert_eq!(provider.provider_id(), "gmx");
    }

    #[tokio::test]
    async fn test_detect_zohomail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 zoho.com
        let provider = pool.detect_provider("user@zoho.com").await.unwrap();
        assert_eq!(provider.provider_id(), "zoho");

        // 测试 zohomail.com
        let provider = pool.detect_provider("user@zohomail.com").await.unwrap();
        assert_eq!(provider.provider_id(), "zoho");

        // 测试 zoho.eu
        let provider = pool.detect_provider("user@zoho.eu").await.unwrap();
        assert_eq!(provider.provider_id(), "zoho");
    }

    #[tokio::test]
    async fn test_detect_yandexmail() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 yandex.com
        let provider = pool.detect_provider("user@yandex.com").await.unwrap();
        assert_eq!(provider.provider_id(), "yandex");

        // 测试 yandex.ru
        let provider = pool.detect_provider("user@yandex.ru").await.unwrap();
        assert_eq!(provider.provider_id(), "yandex");

        // 测试 ya.ru
        let provider = pool.detect_provider("user@ya.ru").await.unwrap();
        assert_eq!(provider.provider_id(), "yandex");
    }

    #[tokio::test]
    async fn test_detect_mailcom() {
        init_tracing();
        let pool = ProviderPool::default();

        // 测试 mail.com
        let provider = pool.detect_provider("user@mail.com").await.unwrap();
        assert_eq!(provider.provider_id(), "mailcom");

        // 测试 email.com
        let provider = pool.detect_provider("user@email.com").await.unwrap();
        assert_eq!(provider.provider_id(), "mailcom");
    }
}
