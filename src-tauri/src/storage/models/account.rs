//! 账号实体模型
//!
//! 定义邮箱账号的数据库实体和相关 DTO。
//!
//! # 实体字段
//!
//! | 字段 | 类型 | 说明 | 默认值 |
//! |------|------|------|--------|
//! | id | i32 | 主键 | 自动生成 |
//! | name | String | 账号显示名称 | - |
//! | email | String | 邮箱地址（唯一） | - |
//! | provider | String | 服务商 ID | - |
//! | imap_host | Option\<String\> | IMAP 服务器地址 | 自动配置 |
//! | imap_port | Option\<i32\> | IMAP 端口 | 自动配置 |
//! | imap_ssl | Option\<bool\> | IMAP SSL | true |
//! | smtp_host | Option\<String\> | SMTP 服务器地址 | 自动配置 |
//! | smtp_port | Option\<i32\> | SMTP 端口 | 自动配置 |
//! | smtp_ssl | Option\<bool\> | SMTP SSL | true |
//! | color | Option\<String\> | UI 显示颜色 | - |
//! | sync_enabled | bool | 是否启用同步 | true |
//! | last_sync_at | Option\<i64\> | 最后同步时间 | - |
//! | auth_type | String | 认证类型 | "password" |
//! | oauth_provider | Option\<String\> | OAuth 服务商 | - |
//! | oauth_expires_at | Option\<i64\> | Token 过期时间 | - |
//!
//! # 安全注意事项
//!
//! 敏感信息不存储在数据库中：
//!
//! - `password`: 已删除，使用 Keyring 存储
//! - `oauth_token`: 已删除，使用 Keyring 存储
//! - `oauth_refresh_token`: 已删除，使用 Keyring 存储
//!
//! # 关联关系
//!
//! ```text
//! Account
//!   ├─ 1:N ─ Email (账号的所有邮件)
//!   └─ 1:N ─ Folder (账号的所有文件夹)
//! ```

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
    // 敏感字段已删除，存储在 Stronghold 中
    // password: String,                    // 已删除
    // oauth_token: Option<String>,         // 已删除
    // oauth_refresh_token: Option<String>, // 已删除
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    // OAuth 2.0 相关字段（仅保留非敏感字段）
    pub auth_type: String,              // 'password' | 'oauth2'
    pub oauth_provider: Option<String>, // 'microsoft' | 'google'
    pub oauth_expires_at: Option<i64>,  // Unix 时间戳
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
            auth_type: ActiveValue::Set("password".to_string()),
            ..ActiveModelTrait::default()
        }
    }
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
