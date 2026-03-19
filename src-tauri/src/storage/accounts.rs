//! 账号存储层
//!
//! 提供账号的数据库 CRUD 操作和 Keyring 密码管理

use sea_orm::{EntityTrait, ActiveModelTrait, ColumnTrait, QueryFilter, DbConn, Set};
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

use crate::crypto::{self, OAuthToken, KEYRING_SERVICE};
use crate::error::{Result, StorageError};
use crate::storage::models::account;
use crate::providers::ProviderPool;

/// 账号仓库
///
/// 处理账号的数据库操作和 Keyring 凭证管理
pub struct AccountRepository;

impl AccountRepository {
    /// 获取所有账号
    pub async fn get_all(db: &DbConn) -> Result<Vec<account::Model>> {
        let accounts = account::Entity::find()
            .all(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取账号列表失败: {}", e)))?;

        Ok(accounts)
    }

    /// 按 ID 获取账号
    pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<account::Model>> {
        let account = account::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取账号失败: {}", e)))?;

        Ok(account)
    }

    /// 按邮箱获取账号
    pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<account::Model>> {
        let account = account::Entity::find()
            .filter(account::Column::Email.eq(email))
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取账号失败: {}", e)))?;

        Ok(account)
    }

    /// 创建新账号
    ///
    /// # 参数
    ///
    /// * `db` - 数据库连接
    /// * `app_handle` - Tauri AppHandle（用于访问 Keyring）
    /// * `req` - 创建账号请求
    ///
    /// # 功能
    ///
    /// 1. 检查邮箱是否已存在
    /// 2. 使用 ProviderPool 检测服务商（如果 provider 为 "auto"）
    /// 3. 获取默认 IMAP/SMTP 配置
    /// 4. 保存账号到数据库
    /// 5. 保存密码/OAuthToken 到 Keyring
    pub async fn create(
        db: &DbConn,
        app_handle: &AppHandle,
        req: CreateAccountRequest,
    ) -> Result<account::Model> {
        // 检查邮箱是否已存在
        if let Some(_) = Self::get_by_email(db, &req.email).await? {
            return Err(StorageError::Database(format!("该邮箱地址已存在")).into());
        }

        // 如果 provider 为 "auto"，使用 ProviderPool 检测
        let provider = if req.provider == "auto" {
            let provider_pool = ProviderPool::with_defaults();
            let detected = provider_pool
                .detect_provider(&req.email)
                .await
                .map_err(|e| StorageError::Database(format!("检测服务商失败: {}", e)))?;
            detected.provider_id().to_string()
        } else {
            req.provider.clone()
        };

        // 获取服务商默认配置（如果未提供）
        let (imap_config, smtp_config) = if req.imap_host.is_none() || req.smtp_host.is_none() {
            account::get_provider_defaults(&provider)
                .ok_or_else(|| StorageError::Database(format!("未知的服务商: {}", provider)))?
        } else {
            (
                account::ImapConfig {
                    host: req.imap_host.clone().unwrap_or_default(),
                    port: req.imap_port.unwrap_or(993),
                    ssl: req.imap_ssl.unwrap_or(true),
                },
                account::SmtpConfig {
                    host: req.smtp_host.clone().unwrap_or_default(),
                    port: req.smtp_port.unwrap_or(587),
                    ssl: req.smtp_ssl.unwrap_or(true),
                },
            )
        };

        // 确定认证类型
        let auth_type = req.auth_type.clone().unwrap_or_else(|| {
            if req.oauth_token.is_some() {
                "oauth2".to_string()
            } else {
                "password".to_string()
            }
        });

        // 创建数据库记录
        let now = chrono::Utc::now().timestamp();
        let active_account = account::ActiveModel {
            name: Set(req.name.clone()),
            email: Set(req.email.clone()),
            provider: Set(provider.clone()),
            imap_host: Set(Some(imap_config.host)),
            imap_port: Set(Some(imap_config.port)),
            imap_ssl: Set(Some(imap_config.ssl)),
            smtp_host: Set(Some(smtp_config.host)),
            smtp_port: Set(Some(smtp_config.port)),
            smtp_ssl: Set(Some(smtp_config.ssl)),
            color: Set(req.color),
            sync_enabled: Set(true),
            last_sync_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            auth_type: Set(auth_type.clone()),
            oauth_provider: Set(req.oauth_provider.clone()),
            oauth_expires_at: Set(req.oauth_expires_at),
            ..Default::default()
        };

        let account = active_account
            .insert(db)
            .await
            .map_err(|e| StorageError::Database(format!("创建账号失败: {}", e)))?;

        // 保存凭证到 Keyring
        let keyring = app_handle.keyring();

        match auth_type.as_str() {
            "oauth2" => {
                // 保存 OAuth Token
                if let (Some(refresh_token), Some(expires_at)) = (
                    req.oauth_refresh_token,
                    req.oauth_expires_at,
                ) {
                    let token = OAuthToken {
                        refresh_token,
                        expires_at,
                    };
                    let token_json = serde_json::to_string(&token)
                        .map_err(|e| StorageError::Database(format!("序列化 Token 失败: {}", e)))?;

                    keyring
                        .set_password(
                            KEYRING_SERVICE,
                            &crypto::oauth_username(account.id),
                            &token_json,
                        )
                        .map_err(|e| StorageError::Keyring(format!("保存 Token 失败: {}", e)))?;
                }
            }
            _ => {
                // 保存密码
                keyring
                    .set_password(
                        KEYRING_SERVICE,
                        &crypto::password_username(account.id),
                        &req.password,
                    )
                    .map_err(|e| StorageError::Keyring(format!("保存密码失败: {}", e)))?;
            }
        }

        Ok(account)
    }

    /// 更新账号
    pub async fn update(
        db: &DbConn,
        id: i32,
        req: UpdateAccountRequest,
    ) -> Result<account::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| StorageError::NotFound("账号不存在".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let active_account = account::ActiveModel {
            id: Set(existing.id),
            name: Set(req.name.unwrap_or(existing.name)),
            email: Set(req.email.unwrap_or(existing.email)),
            provider: Set(req.provider.unwrap_or(existing.provider)),
            imap_host: Set(req.imap_host.or(existing.imap_host)),
            imap_port: Set(req.imap_port.or(existing.imap_port)),
            imap_ssl: Set(req.imap_ssl.or(existing.imap_ssl)),
            smtp_host: Set(req.smtp_host.or(existing.smtp_host)),
            smtp_port: Set(req.smtp_port.or(existing.smtp_port)),
            smtp_ssl: Set(req.smtp_ssl.or(existing.smtp_ssl)),
            color: Set(req.color),
            sync_enabled: Set(req.sync_enabled.unwrap_or(existing.sync_enabled)),
            updated_at: Set(now),
            ..Default::default()
        };

        let account = active_account
            .update(db)
            .await
            .map_err(|e| StorageError::Database(format!("更新账号失败: {}", e)))?;

        Ok(account)
    }

    /// 更新同步时间
    pub async fn update_sync_time(db: &DbConn, id: i32) -> Result<()> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| StorageError::NotFound("账号不存在".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let active_account = account::ActiveModel {
            id: Set(existing.id),
            last_sync_at: Set(Some(now)),
            updated_at: Set(now),
            ..Default::default()
        };

        active_account
            .update(db)
            .await
            .map_err(|e| StorageError::Database(format!("更新同步时间失败: {}", e)))?;

        Ok(())
    }

    /// 删除账号（包括 Keyring 凭证）
    pub async fn delete(db: &DbConn, app_handle: &AppHandle, id: i32) -> Result<()> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| StorageError::NotFound("账号不存在".to_string()))?;

        // 清理 Keyring 凭证
        let keyring = app_handle.keyring();

        // 删除密码
        let _ = keyring.delete_password(
            KEYRING_SERVICE,
            &crypto::password_username(id),
        );

        // 删除 OAuth Token
        let _ = keyring.delete_password(
            KEYRING_SERVICE,
            &crypto::oauth_username(id),
        );

        // 删除数据库记录
        account::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除账号失败: {}", e)))?;

        Ok(())
    }

    /// 从 Keyring 获取账号密码
    pub fn get_password(app_handle: &AppHandle, id: i32) -> Result<String> {
        let keyring = app_handle.keyring();
        let password = keyring
            .get_password(KEYRING_SERVICE, &crypto::password_username(id))
            .map_err(|e| StorageError::Keyring(format!("获取密码失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("密码未找到".to_string()))?;

        Ok(password)
    }

    /// 从 Keyring 获取 OAuth Token
    pub fn get_oauth_token(app_handle: &AppHandle, id: i32) -> Result<OAuthToken> {
        let keyring = app_handle.keyring();
        let token_json = keyring
            .get_password(KEYRING_SERVICE, &crypto::oauth_username(id))
            .map_err(|e| StorageError::Keyring(format!("获取 Token 失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("Token 未找到".to_string()))?;

        let token: OAuthToken = serde_json::from_str(&token_json)
            .map_err(|e| StorageError::Database(format!("解析 Token 失败: {}", e)))?;

        Ok(token)
    }

    /// 保存密码到 Keyring
    pub fn save_password(app_handle: &AppHandle, id: i32, password: &str) -> Result<()> {
        let keyring = app_handle.keyring();
        keyring
            .set_password(KEYRING_SERVICE, &crypto::password_username(id), password)
            .map_err(|e| StorageError::Keyring(format!("保存密码失败: {}", e)))?;

        Ok(())
    }

    /// 保存 OAuth Token 到 Keyring
    pub fn save_oauth_token(app_handle: &AppHandle, id: i32, token: &OAuthToken) -> Result<()> {
        let keyring = app_handle.keyring();
        let token_json = serde_json::to_string(token)
            .map_err(|e| StorageError::Database(format!("序列化 Token 失败: {}", e)))?;

        keyring
            .set_password(KEYRING_SERVICE, &crypto::oauth_username(id), &token_json)
            .map_err(|e| StorageError::Keyring(format!("保存 Token 失败: {}", e)))?;

        Ok(())
    }
}

/// 创建账号请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub email: String,
    pub provider: String,
    pub password: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
    pub auth_type: Option<String>,
    pub oauth_provider: Option<String>,
    pub oauth_token: Option<String>,
    pub oauth_refresh_token: Option<String>,
    pub oauth_expires_at: Option<i64>,
}

/// 更新账号请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub provider: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
}

/// 从 models::account::CreateAccountRequest 转换
impl From<account::CreateAccountRequest> for CreateAccountRequest {
    fn from(req: account::CreateAccountRequest) -> Self {
        Self {
            name: req.name,
            email: req.email,
            provider: req.provider,
            password: req.password,
            imap_host: req.imap_host,
            imap_port: req.imap_port,
            imap_ssl: req.imap_ssl,
            smtp_host: req.smtp_host,
            smtp_port: req.smtp_port,
            smtp_ssl: req.smtp_ssl,
            color: req.color,
            auth_type: req.auth_type,
            oauth_provider: req.oauth_provider,
            oauth_token: req.oauth_token,
            oauth_refresh_token: req.oauth_refresh_token,
            oauth_expires_at: req.oauth_expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_create_request() {
        let req = account::CreateAccountRequest {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            provider: "gmail".to_string(),
            password: "password123".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl: None,
            color: Some("#FF0000".to_string()),
            auth_type: Some("password".to_string()),
            oauth_provider: None,
            oauth_token: None,
            oauth_refresh_token: None,
            oauth_expires_at: None,
        };

        let converted = CreateAccountRequest::from(req);
        assert_eq!(converted.name, "Test");
        assert_eq!(converted.email, "test@example.com");
        assert_eq!(converted.provider, "gmail");
        assert_eq!(converted.color, Some("#FF0000".to_string()));
    }
}
