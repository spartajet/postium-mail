use sea_orm::{*, sea_query::Expr};
use anyhow::{anyhow, Result};
use crate::models::{sync_error, SyncErrorEntity};

/// 获取账号的所有同步错误
pub async fn get_by_account(
    db: &DbConn,
    account_id: i32,
    limit: Option<u64>,
) -> Result<Vec<sync_error::Model>> {
    let mut query = SyncErrorEntity::find()
        .filter(sync_error::Column::AccountId.eq(account_id))
        .order_by_desc(sync_error::Column::CreatedAt);

    if let Some(limit) = limit {
        query = query.limit(limit);
    }

    query
        .all(db)
        .await
        .map_err(|e| anyhow!("获取同步错误列表失败: {}", e))
}

/// 获取未解决的错误
pub async fn get_unresolved(
    db: &DbConn,
    account_id: i32,
    limit: Option<u64>,
) -> Result<Vec<sync_error::Model>> {
    let mut query = SyncErrorEntity::find()
        .filter(sync_error::Column::AccountId.eq(account_id))
        .filter(sync_error::Column::Resolved.eq(false))
        .order_by_desc(sync_error::Column::CreatedAt);

    if let Some(limit) = limit {
        query = query.limit(limit);
    }

    query
        .all(db)
        .await
        .map_err(|e| anyhow!("获取未解决错误失败: {}", e))
}

/// 记录同步错误
pub async fn create(
    db: &DbConn,
    account_id: i32,
    folder: Option<&str>,
    error_type: &str,
    error_message: &str,
    uid: Option<i32>,
    stack_trace: Option<&str>,
) -> Result<sync_error::Model> {
    let now = chrono::Utc::now().timestamp();

    let new_error = sync_error::ActiveModel {
        account_id: Set(account_id),
        folder: Set(folder.map(|s| s.to_string())),
        error_type: Set(error_type.to_string()),
        error_message: Set(error_message.to_string()),
        uid: Set(uid),
        stack_trace: Set(stack_trace.map(|s| s.to_string())),
        resolved: Set(false),
        created_at: Set(now),
        ..Default::default()
    };

    new_error
        .insert(db)
        .await
        .map_err(|e| anyhow!("记录同步错误失败: {}", e))
}

/// 标记错误为已解决
pub async fn mark_resolved(db: &DbConn, id: i32) -> Result<()> {
    SyncErrorEntity::update_many()
        .filter(sync_error::Column::Id.eq(id))
        .col_expr(
            sync_error::Column::Resolved,
            Expr::value(true),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("标记错误已解决失败: {}", e))?;

    Ok(())
}

/// 批量标记账号的错误为已解决
pub async fn mark_all_resolved(db: &DbConn, account_id: i32) -> Result<()> {
    SyncErrorEntity::update_many()
        .filter(sync_error::Column::AccountId.eq(account_id))
        .filter(sync_error::Column::Resolved.eq(false))
        .col_expr(
            sync_error::Column::Resolved,
            Expr::value(true),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("批量标记错误已解决失败: {}", e))?;

    Ok(())
}

/// 删除旧的已解决错误
pub async fn cleanup_old_resolved(
    db: &DbConn,
    account_id: i32,
    days: i64,
) -> Result<u64> {
    let cutoff_time = chrono::Utc::now().timestamp() - (days * 24 * 60 * 60);

    let result = SyncErrorEntity::delete_many()
        .filter(sync_error::Column::AccountId.eq(account_id))
        .filter(sync_error::Column::Resolved.eq(true))
        .filter(sync_error::Column::CreatedAt.lt(cutoff_time))
        .exec(db)
        .await
        .map_err(|e| anyhow!("清理旧错误失败: {}", e))?;

    Ok(result.rows_affected)
}

/// 删除同步错误
pub async fn delete(db: &DbConn, id: i32) -> Result<()> {
    SyncErrorEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除同步错误失败: {}", e))?;

    Ok(())
}
