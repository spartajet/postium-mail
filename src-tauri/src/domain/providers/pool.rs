//!
//! # 服务商注册中心 (Provider Pool)
//!
//! 本模块提供全局唯一的 [`ProviderPool`]，作为应用中所有邮件服务商实例的「单一事实来源」。
//!
//! ## 设计要点
//!
//! - **全局唯一来源**：[`PROVIDER_POOL`] 是进程级单例，所有服务商查询（探测、按 ID 查询、列出）
//!   都应经由它完成，避免多处各自构造导致配置不一致。
//! - **惰性初始化**：使用 [`std::sync::OnceLock`] 配合 [`init_provider_pool`]，在首次访问时
//!   才构造默认池，构造过程线程安全且只执行一次。
//! - **注册分组**：[`ProviderPool::default`] 按「国际 / 国内 / 企业」三类集中注册全部内置服务商。
//!
//! ## 典型用法
//!
//! ```rust,ignore
//! use crate::domain::providers::pool::{init_provider_pool, PROVIDER_POOL};
//!
//! // 应用启动时初始化（幂等，多次调用安全）
//! init_provider_pool();
//!
//! let pool = PROVIDER_POOL.get().expect("pool 未初始化");
//! let provider = pool.get("gmail");
//! ```

use super::{MailProvider, ProviderInfo};
use std::collections::HashMap;
use std::sync::Arc;

/// 全局服务商池单例。
///
/// 进程内唯一的服务商注册中心，通过 [`init_provider_pool`] 惰性初始化。
/// 所有需要访问服务商配置的逻辑都应通过 [`OnceLock::get`] 读取此实例，
/// 而非自行 `ProviderPool::default()`，以保证全局一致。
pub static PROVIDER_POOL: std::sync::OnceLock<Arc<ProviderPool>> = std::sync::OnceLock::new();

/// 初始化全局服务商池 [`PROVIDER_POOL`]。
///
/// 在应用启动阶段调用一次即可（幂等：重复调用不会重复构造，仍返回已存在的实例）。
/// 使用 [`OnceLock::get_or_init`] 保证线程安全的惰性初始化。
pub fn init_provider_pool() {
    PROVIDER_POOL.get_or_init(|| Arc::new(ProviderPool::default()));
}

/// 服务商注册中心 — 唯一来源
///
/// 持有所有已注册的 [`MailProvider`] 实例（以 `provider_id` 为键），并提供查询、
/// 列举、探测等能力。生产环境应使用 [`PROVIDER_POOL`] 全局单例；测试场景可
/// 自行 `ProviderPool::default()` 或 `ProviderPool::new()` 构造独立实例。
pub struct ProviderPool {
    providers: HashMap<String, Arc<dyn MailProvider>>,
}

impl Default for ProviderPool {
    fn default() -> Self {
        let mut pool = Self::new();
        // ── 国际服务商 ──
        pool.register(Arc::new(super::personal::gmail::GmailProvider::new()));
        pool.register(Arc::new(super::personal::outlook::OutlookProvider::new()));
        pool.register(Arc::new(super::personal::yahoo::YahooProvider::new()));
        pool.register(Arc::new(super::personal::aol::AolMailProvider::new()));
        pool.register(Arc::new(super::personal::gmx::GmxMailProvider::new()));
        pool.register(Arc::new(super::personal::mailcom::MailComProvider::new()));
        pool.register(Arc::new(super::personal::zoho::ZohoProvider::new()));
        pool.register(Arc::new(super::personal::yandex::YandexMailProvider::new()));
        // ── 国内服务商 ──
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
        // ── 企业服务商 ──
        pool.register(Arc::new(
            super::enterprise::google_workspace::GoogleWorkspaceProvider::new(),
        ));
        pool.register(Arc::new(
            super::enterprise::microsoft_365::Microsoft365Provider::new(),
        ));
        pool.register(Arc::new(super::enterprise::custom::CustomProvider::new(
            "imap.postium.test",
            993,
            "smtp.postium.test",
            465,
        )));
        pool
    }
}

impl ProviderPool {
    /// 创建一个空的服务商池。
    ///
    /// 不会预注册任何服务商；如需内置全部服务商，请使用 [`ProviderPool::default`]。
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
        let mut list: Vec<ProviderInfo> = self
            .providers
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
