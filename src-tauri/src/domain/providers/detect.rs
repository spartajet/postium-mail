use crate::{domain::providers::pool::PROVIDER_POOL, error::MailError};

use super::ProviderInfo;
use super::pool::ProviderPool;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderDetectionResult {
    pub detected: bool,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub auth_types: String,
}

/// 检测邮箱对应的服务商
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
