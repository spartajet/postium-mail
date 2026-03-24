use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "folder_sync_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub imap_name: String,
    /// 文件夹标准类型: inbox, sent, drafts, spam, trash, archive, other
    pub folder_type: Option<String>,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub synced_at: Option<i64>,
    /// 同步进度字段（合并自 sync_states）
    pub last_sync_uid: Option<i32>,   // 最后同步的 UID
    pub highest_uid: Option<i32>,     // 文件夹最高 UID
    pub total_emails: Option<i32>,    // 总邮件数
    pub sync_count: i32,              // 已同步邮件数
    pub is_first_sync: bool,          // 是否首次同步
    pub error_count: i32,             // 连续错误次数
    pub last_error: Option<String>,   // 最后错误信息
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: Set(Some(chrono::Utc::now().timestamp())),
            updated_at: Set(Some(chrono::Utc::now().timestamp())),
            ..ActiveModelTrait::default()
        }
    }
}
