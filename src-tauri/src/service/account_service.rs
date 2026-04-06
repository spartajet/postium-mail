use crate::domain::auth::AuthManager;
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::entities::accounts;
use crate::infrastructure::storage::repository::account_repo;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

// ─── DTO ───

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AccountDto {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub auth_type: String,
    pub account_type: String,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
}

impl From<accounts::Model> for AccountDto {
    fn from(m: accounts::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            email: m.email,
            display_name: m.display_name,
            provider: m.provider,
            color: m.color,
            sync_enabled: m.sync_enabled.unwrap_or(true),
            auth_type: m.auth_type.unwrap_or_else(|| "password".into()),
            account_type: m.account_type,
            last_sync_at: m.last_sync_at,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateAccountRequest {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub auth_type: String,
    pub password: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl_mode: Option<String>,
    pub color: Option<String>,
    pub account_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAccountRequest {
    pub id: i32,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
}

// ─── Service ───

pub struct AccountService {
    db: DbConn,
    auth: Arc<AuthManager>,
}

impl AccountService {
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self { db, auth }
    }

    pub async fn list(&self) -> Result<Vec<AccountDto>, MailError> {
        let accounts = account_repo::list(&self.db).await?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    pub async fn get(&self, id: i32) -> Result<AccountDto, MailError> {
        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;
        Ok(account.into())
    }

    pub async fn create(&self, req: CreateAccountRequest) -> Result<AccountDto, MailError> {
        tracing::info!(email = %req.email, provider = %req.provider, "创建账号");
        let now = chrono::Utc::now().timestamp();
        // let is_oauth2 = req.auth_type == "OAuth2";

        let model = accounts::ActiveModel {
            name: Set(req.name),
            email: Set(req.email),
            display_name: Set(req.display_name),
            provider: Set(req.provider.clone()),
            imap_host: Set(req.imap_host),
            imap_port: Set(req.imap_port),
            imap_ssl_mode: Set(req.imap_ssl_mode),
            smtp_host: Set(req.smtp_host),
            smtp_port: Set(req.smtp_port),
            smtp_ssl_mode: Set(req.smtp_ssl_mode),
            color: Set(req.color),
            auth_type: Set(Some(req.auth_type.clone())),
            account_type: Set(req.account_type.unwrap_or_else(|| "personal".into())),
            sync_enabled: Set(Some(true)),
            last_sync_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let account = account_repo::create(&self.db, model).await?;

        // Keyring: 密码账号存 password, OAuth2 账号由后端回调自动存储 refresh_token
        self.auth.save_password(&account.email, &req.password)?;
        Ok(account.into())
    }

    pub async fn update(&self, req: UpdateAccountRequest) -> Result<AccountDto, MailError> {
        tracing::info!(id = req.id, "更新账号");
        let existing = account_repo::get_by_id(&self.db, req.id)
            .await?
            .ok_or(MailError::AccountNotFound(req.id))?;

        let mut model: accounts::ActiveModel = existing.into();
        if let Some(name) = req.name {
            model.name = Set(name);
        }
        if let Some(display_name) = req.display_name {
            model.display_name = Set(Some(display_name));
        }
        if let Some(color) = req.color {
            model.color = Set(Some(color));
        }
        if let Some(sync_enabled) = req.sync_enabled {
            model.sync_enabled = Set(Some(sync_enabled));
        }
        model.updated_at = Set(chrono::Utc::now().timestamp());

        let account = account_repo::update(&self.db, req.id, model).await?;
        Ok(account.into())
    }

    pub async fn delete(&self, id: i32) -> Result<(), MailError> {
        tracing::info!(id, "删除账号");
        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;

        account_repo::delete(&self.db, id).await?;

        // 从 Keyring 删除密码
        let _ = self.auth.delete_password(&account.email);

        Ok(())
    }

    pub async fn update_password(&self, id: i32, password: String) -> Result<(), MailError> {
        tracing::info!(id, "更新账号密码");
        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;
        self.auth.save_password(&account.email, &password)?;
        Ok(())
    }

    /// 从 OAuth2 回调创建账号
    pub async fn create_oauth2_account(
        &self,
        params: CreateOAuth2AccountParams,
    ) -> Result<AccountDto, MailError> {
        tracing::info!(email = %params.email, "创建 OAuth2 账号");
        let now = chrono::Utc::now().timestamp();
        let account_name = params
            .email
            .split('@')
            .next()
            .unwrap_or(&params.email)
            .to_string();

        let model = accounts::ActiveModel {
            name: Set(account_name),
            email: Set(params.email),
            display_name: Set(params.display_name),
            provider: Set(params.provider_id),
            imap_host: Set(Some(params.imap_host)),
            imap_port: Set(Some(params.imap_port as i32)),
            imap_ssl_mode: Set(Some(params.imap_ssl_mode)),
            smtp_host: Set(Some(params.smtp_host)),
            smtp_port: Set(Some(params.smtp_port as i32)),
            smtp_ssl_mode: Set(Some(params.smtp_ssl_mode)),
            color: Set(params.color),
            auth_type: Set(Some("OAuth2".to_string())),
            account_type: Set("personal".to_string()),
            sync_enabled: Set(Some(true)),
            last_sync_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let account = account_repo::create(&self.db, model).await?;
        Ok(account.into())
    }
}

/// OAuth2 账号创建参数（由 OAuth2 模块填充，交给 AccountService 写库）
#[derive(Debug, Clone)]
pub struct CreateOAuth2AccountParams {
    pub email: String,
    pub display_name: Option<String>,
    pub provider_id: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_ssl_mode: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_ssl_mode: String,
    pub color: Option<String>,
}
