use crate::{domain::providers::pool::PROVIDER_POOL, error::MailError};

use super::pool::ProviderPool;
use super::ProviderInfo;
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
