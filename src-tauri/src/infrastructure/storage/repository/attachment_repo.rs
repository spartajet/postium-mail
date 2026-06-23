use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::attachments;
use sea_orm::*;

pub async fn list_by_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<attachments::Model>, MailError> {
    Ok(attachments::Entity::find()
        .filter(attachments::Column::EmailId.eq(email_id))
        .all(db)
        .await?)
}

pub async fn create(
    db: &DbConn,
    model: attachments::ActiveModel,
) -> Result<attachments::Model, MailError> {
    Ok(model.insert(db).await?)
}

pub async fn bulk_insert(
    db: &DbConn,
    models: Vec<attachments::ActiveModel>,
) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }
    attachments::Entity::insert_many(models).exec(db).await?;
    Ok(())
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<attachments::Model>, MailError> {
    Ok(attachments::Entity::find_by_id(id).one(db).await?)
}

pub async fn delete_by_email(db: &DbConn, email_id: i32) -> Result<(), MailError> {
    attachments::Entity::delete_many()
        .filter(attachments::Column::EmailId.eq(email_id))
        .exec(db)
        .await?;
    Ok(())
}
