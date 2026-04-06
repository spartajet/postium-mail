use super::{MailProvider, ProviderInfo};
use std::collections::HashMap;
use std::sync::Arc;

pub static PROVIDER_POOL: std::sync::OnceLock<Arc<ProviderPool>> = std::sync::OnceLock::new();

pub fn init_provider_pool() {
    PROVIDER_POOL.get_or_init(|| Arc::new(ProviderPool::default()));
}

/// 服务商注册中心 — 唯一来源
pub struct ProviderPool {
    providers: HashMap<String, Arc<dyn MailProvider>>,
}

impl Default for ProviderPool {
    fn default() -> Self {
        let mut pool = Self::new();
        // 国际
        pool.register(Arc::new(super::personal::gmail::GmailProvider::new()));
        pool.register(Arc::new(super::personal::outlook::OutlookProvider::new()));
        pool.register(Arc::new(super::personal::yahoo::YahooProvider::new()));
        pool.register(Arc::new(super::personal::aol::AolMailProvider::new()));
        pool.register(Arc::new(super::personal::gmx::GmxMailProvider::new()));
        pool.register(Arc::new(super::personal::mailcom::MailComProvider::new()));
        pool.register(Arc::new(super::personal::zoho::ZohoProvider::new()));
        pool.register(Arc::new(super::personal::yandex::YandexMailProvider::new()));
        // 国内
        pool.register(Arc::new(super::personal::yi::NetEaseProvider::new()));
        pool.register(Arc::new(super::personal::qq::QqMailProvider::new()));
        pool.register(Arc::new(super::personal::sina::SinaMailProvider::new()));
        pool.register(Arc::new(super::personal::sohu::SohuMailProvider::new()));
        pool.register(Arc::new(super::personal::tom::TomMailProvider::new()));
        pool.register(Arc::new(super::personal::cmcc::CmccMailProvider::new()));
        pool.register(Arc::new(super::personal::china::ChinaMailProvider::new()));
        pool.register(Arc::new(super::personal::net263::Net263MailProvider::new()));
        pool.register(Arc::new(super::personal::cn21::Cn21MailProvider::new()));
        pool.register(Arc::new(super::personal::icloud::ICloudProvider::new()));
        // 企业
        pool.register(Arc::new(
            super::enterprise::google_workspace::GoogleWorkspaceProvider::new(),
        ));
        pool.register(Arc::new(
            super::enterprise::microsoft_365::Microsoft365Provider::new(),
        ));
        pool
    }
}

impl ProviderPool {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// 注册一个服务商
    pub fn register(&mut self, provider: Arc<dyn MailProvider>) {
        let id = provider.provider_info().id.clone();
        self.providers.insert(id, provider);
    }

    /// 按 ID 查询
    pub fn get(&self, id: &str) -> Option<Arc<dyn MailProvider>> {
        self.providers.get(id).cloned()
    }

    /// 列出所有服务商信息（给前端用，按 sort_order 排序）
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        let mut list: Vec<ProviderInfo> = self.providers
            .values()
            .map(|p| p.provider_info().clone())
            .collect();
        list.sort_by_key(|p| p.sort_order);
        list
    }

    /// 返回所有支持 OAuth2 的 provider
    pub fn oauth_providers(&self) -> HashMap<String, Arc<dyn MailProvider>> {
        self.providers
            .iter()
            .filter(|(_, p)| p.oauth_config().is_some())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// 检测邮箱对应的服务商
    pub fn detect_provider(&self, email: &str) -> Option<Arc<dyn MailProvider>> {
        self.providers.values().find(|p| p.detect(email)).cloned()
    }
}
