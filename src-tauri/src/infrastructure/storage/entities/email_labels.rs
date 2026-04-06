use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[sea_orm(table_name = "email_labels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub email_id: i32,
    pub label_id: i32,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::emails::Entity",
        from = "Column::EmailId",
        to = "super::emails::Column::Id"
    )]
    Email,
    #[sea_orm(
        belongs_to = "super::labels::Entity",
        from = "Column::LabelId",
        to = "super::labels::Column::Id"
    )]
    Label,
}

impl Related<super::emails::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Email.def()
    }
}

impl Related<super::labels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Label.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
