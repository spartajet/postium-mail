//! 服务商配置

use serde::{Deserialize, Serialize};

/// 服务商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// 配置ID
    pub id: String,
    /// 配置名称
    pub name: String,
    /// 是否启用
    pub enabled: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
        }
    }
}
