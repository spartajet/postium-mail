use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[sea_orm(table_name = "sync_errors")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub folder: Option<String>,
    pub error_type: String,
    pub error_message: String,
    pub uid: Option<u32>,
    pub stack_trace: Option<String>,
    pub resolved: Option<bool>,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id"
    )]
    Account,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
