use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[sea_orm(table_name = "emails")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub bcc_emails: Option<String>,
    pub preview: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub is_draft: Option<bool>,
    pub is_answered: Option<bool>,
    pub is_deleted: Option<bool>,
    pub sent_at: i64,
    pub received_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id"
    )]
    Account,
    #[sea_orm(has_many = "super::attachments::Entity")]
    Attachments,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
