use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub password: String,                    // AES-256 加密
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::email::Entity")]
    Emails,
}

impl Related<super::email::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emails.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            updated_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            sync_enabled: ActiveValue::Set(true),
            imap_ssl: ActiveValue::Set(Some(true)),
            smtp_ssl: ActiveValue::Set(Some(true)),
            ..ActiveModelTrait::default()
        }
    }
}

// 前端传输用的 DTO（不包含密码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDto {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<Model> for AccountDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            email: model.email,
            provider: model.provider,
            imap_host: model.imap_host,
            imap_port: model.imap_port,
            imap_ssl: model.imap_ssl,
            smtp_host: model.smtp_host,
            smtp_port: model.smtp_port,
            smtp_ssl: model.smtp_ssl,
            color: model.color,
            sync_enabled: model.sync_enabled,
            last_sync_at: model.last_sync_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

// 创建账号请求
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

// 预设的服务商配置
pub fn get_provider_defaults(provider: &str) -> Option<(ImapConfig, SmtpConfig)> {
    match provider {
        "gmail" => Some((
            ImapConfig {
                host: "imap.gmail.com".to_string(),
                port: 993,
                ssl: true,
            },
            SmtpConfig {
                host: "smtp.gmail.com".to_string(),
                port: 587,
                ssl: true,
            },
        )),
        "outlook" | "hotmail" => Some((
            ImapConfig {
                host: "outlook.office365.com".to_string(),
                port: 993,
                ssl: true,
            },
            SmtpConfig {
                host: "smtp-mail.outlook.com".to_string(),
                port: 587,
                ssl: true,
            },
        )),
        "icloud" => Some((
            ImapConfig {
                host: "imap.mail.me.com".to_string(),
                port: 993,
                ssl: true,
            },
            SmtpConfig {
                host: "smtp.mail.me.com".to_string(),
                port: 587,
                ssl: true,
            },
        )),
        "yahoo" => Some((
            ImapConfig {
                host: "imap.mail.yahoo.com".to_string(),
                port: 993,
                ssl: true,
            },
            SmtpConfig {
                host: "smtp.mail.yahoo.com".to_string(),
                port: 587,
                ssl: true,
            },
        )),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: i32,
    pub ssl: bool,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: i32,
    pub ssl: bool,
}
