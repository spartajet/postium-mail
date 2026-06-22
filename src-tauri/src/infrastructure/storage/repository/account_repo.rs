use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::entities::accounts;
use sea_orm::sea_query::Expr;
use sea_orm::*;

pub async fn list(db: &DbConn) -> Result<Vec<accounts::Model>, MailError> {
    Ok(accounts::Entity::find()
        .order_by_asc(accounts::Column::Id)
        .all(db)
        .await?)
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<accounts::Model>, MailError> {
    Ok(accounts::Entity::find_by_id(id).one(db).await?)
}

pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<accounts::Model>, MailError> {
    Ok(accounts::Entity::find()
        .filter(accounts::Column::Email.eq(email))
        .one(db)
        .await?)
}

pub async fn create(
    db: &DbConn,
    model: accounts::ActiveModel,
) -> Result<accounts::Model, MailError> {
    Ok(model.insert(db).await?)
}

pub async fn update(
    db: &DbConn,
    id: i32,
    model: accounts::ActiveModel,
) -> Result<accounts::Model, MailError> {
    let mut model = model;
    model.id = Set(id);
    Ok(model.update(db).await?)
}

pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    accounts::Entity::delete_by_id(id).exec(db).await?;
    Ok(())
}

pub async fn update_last_sync(db: &DbConn, id: i32) -> Result<(), MailError> {
    let now = chrono::Utc::now().timestamp();
    accounts::Entity::update_many()
        .col_expr(accounts::Column::LastSyncAt, Expr::value(now))
        .col_expr(accounts::Column::UpdatedAt, Expr::value(now))
        .filter(accounts::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}
