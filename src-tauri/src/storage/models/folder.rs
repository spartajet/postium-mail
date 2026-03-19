use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "folders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub name: String,               // 标准化名称 (inbox, sent, etc.)
    pub imap_name: String,          // IMAP 服务器原始名称 (INBOX, Sent Items)
    pub parent_id: Option<i32>,     // 父文件夹 ID (支持嵌套结构)
    pub attributes: Option<String>, // 文件夹属性 (JSON: \Noselect, \HasChildren 等)
    pub email_count: i32,           // 邮件数量
    pub unread_count: i32,          // 未读数量
    pub synced_at: Option<i64>,     // 最后同步时间
    // IMAP 元数据字段
    pub uidvalidity: Option<i64>,   // IMAP UIDVALIDITY 值
    pub uidnext: Option<i64>,       // 预期的下一个 UID
    pub highest_modseq: Option<i64>, // CONDSTORE 扩展的最高修改序列号
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
    #[sea_orm(has_many = "super::email::Entity")]
    Emails,
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl Related<super::email::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emails.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            email_count: ActiveValue::Set(0),
            unread_count: ActiveValue::Set(0),
            ..ActiveModelTrait::default()
        }
    }
}

// 前端传输用的 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderDto {
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub imap_name: String,
    pub parent_id: Option<i32>,
    pub email_count: i32,
    pub unread_count: i32,
    pub synced_at: Option<i64>,
    // IMAP 元数据
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub highest_modseq: Option<i64>,
}

impl From<Model> for FolderDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            name: model.name,
            imap_name: model.imap_name,
            parent_id: model.parent_id,
            email_count: model.email_count,
            unread_count: model.unread_count,
            synced_at: model.synced_at,
            uidvalidity: model.uidvalidity,
            uidnext: model.uidnext,
            highest_modseq: model.highest_modseq,
        }
    }
}

// 标准文件夹名称枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StandardFolder {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Spam,
    Trash,
    Archive,
    Custom(String),
}

impl StandardFolder {
    pub fn as_str(&self) -> &str {
        match self {
            StandardFolder::Inbox => "inbox",
            StandardFolder::Starred => "starred",
            StandardFolder::Sent => "sent",
            StandardFolder::Drafts => "drafts",
            StandardFolder::Spam => "spam",
            StandardFolder::Trash => "trash",
            StandardFolder::Archive => "archive",
            StandardFolder::Custom(name) => name,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "inbox" => StandardFolder::Inbox,
            "starred" => StandardFolder::Starred,
            "sent" => StandardFolder::Sent,
            "drafts" => StandardFolder::Drafts,
            "spam" => StandardFolder::Spam,
            "trash" => StandardFolder::Trash,
            "archive" => StandardFolder::Archive,
            other => StandardFolder::Custom(other.to_string()),
        }
    }
}
