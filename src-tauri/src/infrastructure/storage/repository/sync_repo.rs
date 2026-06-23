use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::{sync_errors, sync_state};
use sea_orm::*;

pub async fn upsert_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidnext: Option<u32>,
    uidvalidity: Option<u32>,
    last_sync_uid: Option<u32>,
) -> Result<(), MailError> {
    let existing = sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .one(db)
        .await?;

    let now = chrono::Utc::now().timestamp();

    if let Some(model) = existing {
        let mut active: sync_state::ActiveModel = model.into();
        if let Some(uidnext) = uidnext {
            active.uidnext = Set(Some(uidnext));
        }
        if let Some(uidvalidity) = uidvalidity {
            active.uidvalidity = Set(Some(uidvalidity));
        }
        if let Some(last_sync_uid) = last_sync_uid {
            active.last_sync_uid = Set(Some(last_sync_uid));
        }
        active.synced_at = Set(Some(now));
        active.updated_at = Set(Some(now));
        active.update(db).await?;
    } else {
        let model = sync_state::ActiveModel {
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            uidnext: Set(uidnext),
            uidvalidity: Set(uidvalidity),
            synced_at: Set(Some(now)),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            last_sync_uid: Set(last_sync_uid),
            ..Default::default()
        };
        model.insert(db).await?;
    }

    Ok(())
}

pub async fn get_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>, MailError> {
    Ok(sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .one(db)
        .await?)
}

pub async fn log_error(
    db: &DbConn,
    account_id: i32,
    folder: Option<String>,
    error_type: &str,
    error_message: &str,
    uid: Option<u32>,
) -> Result<(), MailError> {
    let now = chrono::Utc::now().timestamp();
    let model = sync_errors::ActiveModel {
        account_id: Set(account_id),
        folder: Set(folder),
        error_type: Set(error_type.to_string()),
        error_message: Set(error_message.to_string()),
        uid: Set(uid),
        resolved: Set(Some(false)),
        created_at: Set(now),
        ..Default::default()
    };
    model.insert(db).await?;
    Ok(())
}

pub async fn list_unresolved_errors(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<sync_errors::Model>, MailError> {
    Ok(sync_errors::Entity::find()
        .filter(sync_errors::Column::AccountId.eq(account_id))
        .filter(sync_errors::Column::Resolved.eq(false))
        .all(db)
        .await?)
}

pub async fn delete_by_account<C>(db: &C, account_id: i32) -> Result<(), MailError>
where
    C: ConnectionTrait,
{
    sync_errors::Entity::delete_many()
        .filter(sync_errors::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    sync_state::Entity::delete_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(())
}
