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
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub highest_modseq: Option<i64>,
    pub synced_at: Option<i64>,
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

// 前端传输用的 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSyncStateDto {
    pub id: i32,
    pub account_id: i32,
    pub imap_name: String,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub highest_modseq: Option<i64>,
    pub synced_at: Option<i64>,
}

impl From<Model> for FolderSyncStateDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            imap_name: model.imap_name,
            uidvalidity: model.uidvalidity,
            uidnext: model.uidnext,
            highest_modseq: model.highest_modseq,
            synced_at: model.synced_at,
        }
    }
}
