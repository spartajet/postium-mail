use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::entities::{email_labels, labels};
use sea_orm::*;

pub async fn list_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<labels::Model>, MailError> {
    Ok(labels::Entity::find()
        .filter(labels::Column::AccountId.eq(account_id))
        .all(db)
        .await?)
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<labels::Model>, MailError> {
    Ok(labels::Entity::find_by_id(id).one(db).await?)
}

pub async fn create(db: &DbConn, model: labels::ActiveModel) -> Result<labels::Model, MailError> {
    Ok(model.insert(db).await?)
}

pub async fn update(
    db: &DbConn,
    id: i32,
    model: labels::ActiveModel,
) -> Result<labels::Model, MailError> {
    let mut model = model;
    model.id = Set(id);
    Ok(model.update(db).await?)
}

pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    email_labels::Entity::delete_many()
        .filter(email_labels::Column::LabelId.eq(id))
        .exec(db)
        .await?;

    labels::Entity::delete_by_id(id).exec(db).await?;
    Ok(())
}

pub async fn delete_by_account<C>(db: &C, account_id: i32) -> Result<(), MailError>
where
    C: ConnectionTrait,
{
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
        DELETE FROM email_labels
        WHERE email_id IN (
            SELECT id FROM emails WHERE account_id = ?
        )
        "#,
        [account_id.into()],
    ))
    .await?;

    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
        DELETE FROM email_labels
        WHERE label_id IN (
            SELECT id FROM labels WHERE account_id = ?
        )
        "#,
        [account_id.into()],
    ))
    .await?;

    labels::Entity::delete_many()
        .filter(labels::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(())
}

pub async fn get_labels_for_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<labels::Model>, MailError> {
    let email_labels = email_labels::Entity::find()
        .filter(email_labels::Column::EmailId.eq(email_id))
        .all(db)
        .await?;

    let label_ids: Vec<i32> = email_labels.iter().map(|el| el.label_id).collect();
    if label_ids.is_empty() {
        return Ok(vec![]);
    }

    Ok(labels::Entity::find()
        .filter(labels::Column::Id.is_in(label_ids))
        .all(db)
        .await?)
}

pub async fn add_label_to_email(
    db: &DbConn,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    let existing = email_labels::Entity::find()
        .filter(email_labels::Column::EmailId.eq(email_id))
        .filter(email_labels::Column::LabelId.eq(label_id))
        .one(db)
        .await?;
    if existing.is_some() {
        return Ok(());
    }

    let now = chrono::Utc::now().timestamp();
    let model = email_labels::ActiveModel {
        email_id: Set(email_id),
        label_id: Set(label_id),
        created_at: Set(now),
        ..Default::default()
    };
    model.insert(db).await?;
    Ok(())
}

pub async fn remove_label_from_email(
    db: &DbConn,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    email_labels::Entity::delete_many()
        .filter(email_labels::Column::EmailId.eq(email_id))
        .filter(email_labels::Column::LabelId.eq(label_id))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn list_emails_by_label(db: &DbConn, label_id: i32) -> Result<Vec<i32>, MailError> {
    let email_labels = email_labels::Entity::find()
        .filter(email_labels::Column::LabelId.eq(label_id))
        .all(db)
        .await?;
    Ok(email_labels.iter().map(|el| el.email_id).collect())
}
