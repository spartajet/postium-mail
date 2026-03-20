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
