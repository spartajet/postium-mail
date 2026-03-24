use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub folder: String,            // 文件夹名称
    pub last_sync_uid: Option<i32>, // 最后同步的 UID
    pub last_sync_at: Option<i64>,  // 最后同步时间
    pub highest_uid: Option<i32>,   // 文件夹最高 UID
    pub total_emails: Option<i32>,  // 总邮件数
    pub sync_count: i32,            // 已同步邮件数
    pub is_first_sync: bool,        // 是否首次同步
    pub error_count: i32,           // 连续错误次数
    pub last_error: Option<String>, // 最后错误信息
    pub updated_at: i64,            // 更新时间
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
            sync_count: ActiveValue::Set(0),
            is_first_sync: ActiveValue::Set(true),
            error_count: ActiveValue::Set(0),
            updated_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            ..ActiveModelTrait::default()
        }
    }
}

// 前端传输用的 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStateDto {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub last_sync_uid: Option<i32>,
    pub last_sync_at: Option<i64>,
    pub highest_uid: Option<i32>,
    pub total_emails: Option<i32>,
    pub sync_count: i32,
    pub is_first_sync: bool,
    pub error_count: i32,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

impl From<Model> for SyncStateDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            folder: model.folder,
            last_sync_uid: model.last_sync_uid,
            last_sync_at: model.last_sync_at,
            highest_uid: model.highest_uid,
            total_emails: model.total_emails,
            sync_count: model.sync_count,
            is_first_sync: model.is_first_sync,
            error_count: model.error_count,
            last_error: model.last_error,
            updated_at: model.updated_at,
        }
    }
}

// 同步状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Completed,
    Error,
}
