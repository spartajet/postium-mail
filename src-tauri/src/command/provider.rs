//! 服务商检测 Commands
//!
//! 提供服务商检测和配置获取功能

use super::AuthManagerState;
use crate::providers::AuthType;
use serde::{Deserialize, Serialize};

/// 服务商检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDetectionResult {
    /// 服务商唯一标识
    pub provider_id: String,
    /// 服务商显示名称
    pub provider_name: String,
    /// 支持的认证类型
    pub auth_types: Vec<String>,
    /// 推荐的认证类型
    pub recommended_auth_type: String,
    /// IMAP 配置
    pub imap_config: ImapServerConfigDto,
    /// SMTP 配置
    pub smtp_config: SmtpServerConfigDto,
    /// 服务商能力
    pub capabilities: ProviderCapabilitiesDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapServerConfigDto {
    pub host: String,
    pub port: u16,
    pub ssl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpServerConfigDto {
    pub host: String,
    pub port: u16,
    pub ssl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilitiesDto {
    pub supports_oauth: bool,
    pub supports_idle: bool,
    pub max_message_size: Option<u64>,
}

/// 检测邮箱所属服务商
///
/// 根据邮箱地址自动检测服务商，返回服务商信息和支持的认证类型
#[tauri::command]
pub async fn detect_provider(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
) -> Result<ProviderDetectionResult, String> {
    let auth_manager = auth_manager_state.clone_manager();

    // 1. 检测服务商
    let provider = auth_manager
        .provider_pool()
        .detect_provider(&email)
        .await
        .map_err(|e| e.to_string())?;

    // 2. 获取配置（缓存 provider_info 避免多次调用）
    let provider_info = provider.provider_info();
    let imap_config = provider.imap_config(&email);
    let smtp_config = provider.smtp_config(&email);
    let auth_types = provider_info.auth_types.clone();
    let capabilities = provider_info.capabilities.clone();

    // 3. 确定推荐的认证类型（优先 OAuth）
    let recommended_auth_type = if auth_types.contains(&AuthType::OAuth2) {
        "oauth2".to_string()
    } else {
        "password".to_string()
    };

    // 4. 转换为 DTO
    Ok(ProviderDetectionResult {
        provider_id: provider_info.id.clone(),
        provider_name: provider_info.name.clone(),
        auth_types: auth_types
            .into_iter()
            .map(|t| match t {
                AuthType::OAuth2 => "oauth2".to_string(),
                AuthType::Password => "password".to_string(),
                AuthType::AppPassword => "app_password".to_string(),
                AuthType::Auto => "auto".to_string(),
            })
            .collect(),
        recommended_auth_type,
        imap_config: ImapServerConfigDto {
            host: imap_config.host,
            port: imap_config.port,
            ssl: format!("{:?}", imap_config.ssl),
        },
        smtp_config: SmtpServerConfigDto {
            host: smtp_config.host,
            port: smtp_config.port,
            ssl: format!("{:?}", smtp_config.ssl),
        },
        capabilities: ProviderCapabilitiesDto {
            supports_oauth: capabilities.supports_oauth,
            supports_idle: capabilities.supports_idle,
            max_message_size: capabilities.max_message_size,
        },
    })
}

/// 获取所有支持的服务商列表
///
/// 返回应用支持的所有邮件服务商信息
#[tauri::command]
pub async fn list_providers(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
) -> Result<Vec<ProviderInfoDto>, String> {
    let auth_manager = auth_manager_state.clone_manager();

    let providers = auth_manager
        .provider_pool()
        .list_providers();

    Ok(providers
        .into_iter()
        .map(|p| {
            let info = p.provider_info();
            ProviderInfoDto {
                id: info.id.clone(),
                name: info.name.clone(),
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfoDto {
    pub id: String,
    pub name: String,
}
