//! 文件夹同步状态模型
//!
//! 存储文件夹的 IMAP 同步状态信息（UIDVALIDITY、UIDNEXT、同步时间等）

use sea_orm::Set;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_state")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    /// 文件夹名称（IMAP 原始名称，通常是英文）
    pub folder: String,
    /// 文件夹昵称（IMAP UTF-7 编码的原始名称，如 &V4NXPpCuTvY-）
    ///
    /// 用于显示给用户，因为 IMAP UTF-7 编码的名称对用户不友好。
    /// 可以后续通过 IMAP UTF-7 解码还原为可读的中文或其他语言名称。
    pub folder_nick_name: Option<String>,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub synced_at: Option<i64>,
    pub last_sync_uid: Option<i32>,
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
