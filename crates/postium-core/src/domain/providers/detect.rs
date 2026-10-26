//!
//! # 服务商探测 (Provider Detection)
//!
//! 本模块提供基于邮箱地址的「服务商自动探测」能力，以及在运行时列举全部已注册服务商信息。
//!
//! ## 主要功能
//!
//! - [`detect_provider`]: 给定邮箱地址，在指定 [`ProviderPool`] 中匹配并返回探测结果。
//!   匹配由各 [`super::MailProvider`] 的 `detect` 实现决定（通常按邮箱域名匹配）。
//! - [`list_providers`]: 从全局单例 [`PROVIDER_POOL`] 读取所有服务商信息，按 `sort_order`
//!   排序后返回，供前端展示服务商列表。
//!
//! ## 设计说明
//!
//! `detect_provider` 接收外部传入的 `&ProviderPool`，便于在单元测试中使用独立的测试池；
//! 而 `list_providers` 直接读取全局池，因此要求调用前已完成 [`super::pool::init_provider_pool`]。
//!

use crate::{domain::providers::pool::PROVIDER_POOL, error::MailError};

use super::ProviderInfo;
use super::pool::ProviderPool;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 服务商探测结果。
///
/// 由 [`detect_provider`] 返回，描述一个邮箱地址是否匹配到服务商，以及匹配到的服务商元信息。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderDetectionResult {
    /// 是否成功探测到匹配的服务商。
    pub detected: bool,
    /// 探测到的服务商 ID（如 `"gmail"`、`"qq"`）；未匹配时为 `None`。
    pub provider_id: Option<String>,
    /// 探测到的服务商展示名称（如 `"Gmail"`）；未匹配时为 `None`。
    pub provider_name: Option<String>,
    /// 该服务商支持的认证方式描述字符串（如 `"password"` / `"oauth2"` / `"both"`）。
    /// 未匹配时为空字符串。
    pub auth_types: String,
}

/// 检测邮箱对应的服务商
///
/// 在指定 `pool` 中遍历已注册服务商，返回首个匹配 `email` 域名的服务商信息。
/// 域名匹配的大小写敏感性由各服务商的 `detect` 实现决定（内置实现均为大小写不敏感）。
///
/// # 参数
///
/// - `email`: 待探测的邮箱地址（如 `"user@gmail.com"`）。
/// - `pool`:  用于探测的服务商池。测试场景可传入自行构造的池；生产场景通常传
///   `&PROVIDER_POOL.get()` 指向的全局池。
///
/// # 返回
///
/// 返回 [`ProviderDetectionResult`]：匹配成功时 `detected = true` 并填充服务商 ID / 名称 /
/// 认证方式；无任何匹配时 `detected = false` 且其余字段为空。
pub fn detect_provider(email: &str, pool: &ProviderPool) -> ProviderDetectionResult {
    if let Some(provider) = pool.detect_provider(email) {
        let info = provider.provider_info();
        return ProviderDetectionResult {
            detected: true,
            provider_id: Some(info.id.clone()),
            provider_name: Some(info.name.clone()),
            auth_types: info.auth_type.as_str().to_string(),
        };
    }

    ProviderDetectionResult {
        detected: false,
        provider_id: None,
        provider_name: None,
        auth_types: String::new(),
    }
}

/// 列出所有支持的服务商
///
/// 从全局单例 [`PROVIDER_POOL`] 读取全部服务商信息，按 `sort_order` 升序排列后返回。
///
/// # 返回
///
/// - `Ok(Vec<ProviderInfo>)`: 全部已注册服务商的展示信息。
/// - `Err(MailError::ProviderNotSupported)`: 全局池尚未初始化（未调用
///   [`super::pool::init_provider_pool`]）时返回。
pub fn list_providers() -> Result<Vec<ProviderInfo>, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported(
            "未找到provider pool".to_string(),
        ))?
        .clone();
    Ok(provider_pool.list_providers())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::providers::pool::ProviderPool;

    fn create_test_pool() -> ProviderPool {
        ProviderPool::default()
    }

    #[test]
    fn test_detect_gmail() {
        let pool = create_test_pool();
        let result = detect_provider("user@gmail.com", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("gmail"));
    }

    #[test]
    fn test_detect_qq_mail() {
        let pool = create_test_pool();
        let result = detect_provider("user@qq.com", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("qq"));
    }

    #[test]
    fn test_detect_163_mail() {
        let pool = create_test_pool();
        let result = detect_provider("user@163.com", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("yi"));
    }

    #[test]
    fn test_detect_outlook() {
        let pool = create_test_pool();
        let result = detect_provider("user@outlook.com", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("outlook"));
    }

    #[test]
    fn test_detect_unknown_domain() {
        let pool = create_test_pool();
        let result = detect_provider("user@unknown-custom-domain.xyz", &pool);
        assert!(!result.detected);
        assert!(result.provider_id.is_none());
        assert!(result.provider_name.is_none());
    }

    #[test]
    fn test_detect_case_insensitive() {
        let pool = create_test_pool();
        let result = detect_provider("User@GMAIL.COM", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("gmail"));
    }

    #[test]
    fn test_detect_empty_email() {
        let pool = create_test_pool();
        let result = detect_provider("", &pool);
        assert!(!result.detected);
    }

    #[test]
    fn test_detect_yahoo() {
        let pool = create_test_pool();
        let result = detect_provider("user@yahoo.com", &pool);
        assert!(result.detected);
        assert_eq!(result.provider_id.as_deref(), Some("yahoo"));
    }
}
