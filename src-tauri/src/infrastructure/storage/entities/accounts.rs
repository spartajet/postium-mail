use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub imap_ssl_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub smtp_ssl_mode: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
    pub last_sync_at: Option<i64>,
    pub auth_type: Option<String>,
    pub account_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::emails::Entity")]
    Emails,
    #[sea_orm(has_many = "super::sync_state::Entity")]
    SyncState,
    #[sea_orm(has_many = "super::sync_errors::Entity")]
    SyncErrors,
}

impl Related<super::emails::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emails.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
