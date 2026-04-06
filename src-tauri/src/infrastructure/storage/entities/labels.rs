use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[sea_orm(table_name = "labels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::email_labels::Entity")]
    EmailLabels,
}

impl Related<super::email_labels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EmailLabels.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
