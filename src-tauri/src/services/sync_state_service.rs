use sea_orm::{*, sea_query::Expr};
use anyhow::{anyhow, Result};
use crate::models::{sync_state, SyncStateEntity};

/// 获取账号的所有同步状态
pub async fn get_by_account(db: &DbConn, account_id: i32) -> Result<Vec<sync_state::Model>> {
    SyncStateEntity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .all(db)
        .await
        .map_err(|e| anyhow!("获取同步状态列表失败: {}", e))
}

/// 获取账号的文件夹同步状态
pub async fn get_by_account_and_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>> {
    SyncStateEntity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .one(db)
        .await
        .map_err(|e| anyhow!("获取同步状态失败: {}", e))
}

/// 创建或更新同步状态
pub async fn upsert(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    last_sync_uid: Option<i32>,
    highest_uid: Option<i32>,
    sync_count: i32,
    is_first_sync: bool,
) -> Result<sync_state::Model> {
    let now = chrono::Utc::now().timestamp();

    // 尝试查找现有记录
    if let Some(existing) = get_by_account_and_folder(db, account_id, folder).await? {
        // 保存需要的值，避免 move 后使用
        let existing_is_first = existing.is_first_sync;
        let existing_highest = existing.highest_uid.unwrap_or(0);
        let existing_sync_count = existing.sync_count;

        // 更新现有记录
        let mut active: sync_state::ActiveModel = existing.into();
        active.last_sync_uid = Set(last_sync_uid);
        active.highest_uid = Set(Some(highest_uid.unwrap_or(existing_highest)));
        active.sync_count = Set(existing_sync_count + sync_count);
        active.is_first_sync = Set(is_first_sync && existing_is_first);
        active.error_count = Set(0); // 成功同步，清除错误计数
        active.last_error = Set(None);
        active.updated_at = Set(now);

        Ok(active.update(db).await.map_err(|e| anyhow!("更新同步状态失败: {}", e))?)
    } else {
        // 创建新记录
        let new_state = sync_state::ActiveModel {
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            last_sync_uid: Set(last_sync_uid),
            last_sync_at: Set(Some(now)),
            highest_uid: Set(highest_uid),
            total_emails: Set(None), // 稍后更新
            sync_count: Set(sync_count),
            is_first_sync: Set(is_first_sync),
            error_count: Set(0),
            last_error: Set(None),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(new_state.insert(db).await.map_err(|e| anyhow!("创建同步状态失败: {}", e))?)
    }
}

/// 更新同步进度（在同步过程中调用）
pub async fn update_progress(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    synced_count: i32,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    SyncStateEntity::update_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .col_expr(
            sync_state::Column::SyncCount,
            Expr::value(synced_count),
        )
        .col_expr(
            sync_state::Column::UpdatedAt,
            Expr::value(now),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("更新同步进度失败: {}", e))?;

    Ok(())
}

/// 记录同步错误
pub async fn record_error(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    error_message: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    // 增加错误计数
    SyncStateEntity::update_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .col_expr(
            sync_state::Column::ErrorCount,
            Expr::col(sync_state::Column::ErrorCount).add(1),
        )
        .col_expr(
            sync_state::Column::LastError,
            Expr::value(error_message),
        )
        .col_expr(
            sync_state::Column::UpdatedAt,
            Expr::value(now),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("记录同步错误失败: {}", e))?;

    Ok(())
}

/// 清除错误计数
pub async fn clear_errors(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    SyncStateEntity::update_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .col_expr(
            sync_state::Column::ErrorCount,
            Expr::value(0),
        )
        .col_expr(
            sync_state::Column::LastError,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            sync_state::Column::UpdatedAt,
            Expr::value(now),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("清除错误计数失败: {}", e))?;

    Ok(())
}

/// 删除同步状态
pub async fn delete(db: &DbConn, id: i32) -> Result<()> {
    SyncStateEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除同步状态失败: {}", e))?;

    Ok(())
}

/// 检查是否需要首次同步
pub async fn needs_first_sync(db: &DbConn, account_id: i32, folder: &str) -> Result<bool> {
    if let Some(state) = get_by_account_and_folder(db, account_id, folder).await? {
        Ok(state.is_first_sync)
    } else {
        Ok(true)
    }
}
