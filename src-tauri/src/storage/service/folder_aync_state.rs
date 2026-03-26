//! 文件夹同步状态管理器
//!
//! 管理文件夹同步状态，不管理文件夹配置

use crate::error::{MailError, Result};
use crate::protocols::imap::FolderInfo as ImapFolderInfo;
use crate::providers::StandardFolder;
use crate::storage::models::folder_sync_state;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DbConn, EntityTrait, ExprTrait, QueryFilter, Set,
    sea_query::Expr,
};

/// 更新文件夹同步状态
///
/// 只更新同步时间戳，IMAP 元数据（uidvalidity, uidnext）
/// 需要在同步邮件时单独获取并更新。
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder_infos` - 从 IMAP 服务器获取的文件夹信息列表
///
/// # 返回
///
/// 返回更新结果
pub async fn update_sync_states(
    db: &DbConn,
    account_id: i32,
    folder_infos: &[ImapFolderInfo],
) -> Result<SyncStateUpdateResult> {
    tracing::info!(
        "开始更新文件夹同步状态: account_id={}, count={}",
        account_id,
        folder_infos.len()
    );

    let mut updated_count = 0;

    for info in folder_infos {
        // 使用 upsert（插入或更新）
        let existing_state = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::ImapName.eq(&info.name))
            .one(db)
            .await?;

        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = existing_state {
            // 更新现有记录
            let mut active: folder_sync_state::ActiveModel = existing.into();
            // IMAP 元数据需要在同步邮件时单独更新
            active.synced_at = Set(Some(now));
            active.updated_at = Set(Some(now));

            active
                .update(db)
                .await
                .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "更新文件夹同步状态: account_id={}, imap_name={}",
                account_id,
                info.name
            );

            updated_count += 1;
        } else {
            // 创建新记录（IMAP 元数据初始为 None）
            let active = folder_sync_state::ActiveModel {
                id: ActiveValue::NotSet, // 自增
                account_id: Set(account_id),
                imap_name: Set(info.name.clone()),
                uidvalidity: Set(None),
                uidnext: Set(None),
                synced_at: Set(Some(now)),
                ..Default::default()
            };

            active
                .insert(db)
                .await
                .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "创建文件夹同步状态: account_id={}, imap_name={}",
                account_id,
                info.name
            );

            updated_count += 1;
        }
    }

    tracing::info!(
        "文件夹同步状态更新完成: account_id={}, updated={}",
        account_id,
        updated_count
    );

    Ok(SyncStateUpdateResult {
        updated: updated_count,
    })
}

/// 更新文件夹同步状态（包含类型识别）
///
/// 与 `update_sync_states` 类似，但额外根据服务商的文件夹映射识别每个文件夹的标准类型，
/// 并将 `folder_type` 存储到数据库。
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder_infos` - 从 IMAP 服务器获取的文件夹信息列表
/// * `folder_mapping` - 服务商特定的文件夹名称映射
///
/// # 返回
///
/// 返回更新结果
pub async fn update_sync_states_with_types(
    db: &DbConn,
    account_id: i32,
    folder_infos: &[ImapFolderInfo],
    folder_mapping: &StandardFolder,
) -> Result<SyncStateUpdateResult> {
    tracing::info!(
        "开始更新文件夹同步状态（含类型识别）: account_id={}, count={}",
        account_id,
        folder_infos.len()
    );

    let mut updated_count = 0;

    for info in folder_infos {
        // 识别文件夹类型
        let folder_type = folder_mapping.find_standard_type(&info.name);
        tracing::debug!(
            "识别文件夹类型: imap_name={}, folder_type={}",
            info.name,
            folder_type
        );

        // 使用 upsert（插入或更新）
        let existing_state = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::ImapName.eq(&info.name))
            .one(db)
            .await?;

        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = existing_state {
            // 更新现有记录
            let mut active: folder_sync_state::ActiveModel = existing.into();
            active.folder_type = Set(Some(folder_type.to_string()));
            active.synced_at = Set(Some(now));
            active.updated_at = Set(Some(now));

            active
                .update(db)
                .await
                .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "更新文件夹同步状态: account_id={}, imap_name={}, folder_type={}",
                account_id,
                info.name,
                folder_type
            );

            updated_count += 1;
        } else {
            // 创建新记录
            let active = folder_sync_state::ActiveModel {
                id: ActiveValue::NotSet,
                account_id: Set(account_id),
                imap_name: Set(info.name.clone()),
                folder_type: Set(Some(folder_type.to_string())),
                uidvalidity: Set(None),
                uidnext: Set(None),
                synced_at: Set(Some(now)),
                ..Default::default()
            };

            active
                .insert(db)
                .await
                .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "创建文件夹同步状态: account_id={}, imap_name={}, folder_type={}",
                account_id,
                info.name,
                folder_type
            );

            updated_count += 1;
        }
    }

    tracing::info!(
        "文件夹同步状态更新完成（含类型识别）: account_id={}, updated={}",
        account_id,
        updated_count
    );

    Ok(SyncStateUpdateResult {
        updated: updated_count,
    })
}

/// 更新文件夹的 IMAP 元数据
///
/// 在同步邮件时调用，更新 uidvalidity, uidnext
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `imap_name` - IMAP 文件夹名称
/// * `uidvalidity` - IMAP UIDVALIDITY 值
/// * `uidnext` - 预期的下一个 UID
pub async fn update_folder_metadata(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    uidvalidity: Option<u64>,
    uidnext: Option<u64>,
) -> Result<()> {
    let existing_state = folder_sync_state::Entity::find()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .one(db)
        .await?;

    let now = chrono::Utc::now().timestamp();

    if let Some(existing) = existing_state {
        // 更新现有记录
        let mut active: folder_sync_state::ActiveModel = existing.into();
        active.uidvalidity = Set(uidvalidity.map(|v| v as i64));
        active.uidnext = Set(uidnext.map(|v| v as i64));
        active.synced_at = Set(Some(now));
        active.updated_at = Set(Some(now));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新文件夹元数据失败: {}", e)))?;
    } else {
        // 创建新记录
        let active = folder_sync_state::ActiveModel {
            id: ActiveValue::NotSet,
            account_id: Set(account_id),
            imap_name: Set(imap_name.to_string()),
            uidvalidity: Set(uidvalidity.map(|v| v as i64)),
            uidnext: Set(uidnext.map(|v| v as i64)),
            synced_at: Set(Some(now)),
            ..Default::default()
        };

        active
            .insert(db)
            .await
            .map_err(|e| MailError::Internal(format!("创建文件夹元数据失败: {}", e)))?;
    }

    Ok(())
}

/// 获取文件夹同步状态
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `imap_name` - IMAP 文件夹名称
///
/// # 返回
///
/// 返回同步状态
pub async fn get_sync_state(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
) -> Result<Option<folder_sync_state::Model>> {
    let state = folder_sync_state::Entity::find()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .one(db)
        .await?;

    Ok(state)
}

/// 保存或更新文件夹同步状态
///
/// 这是一个 upsert 操作，用于保存或更新文件夹的同步状态。
/// 主要用于同步邮件时更新 UIDVALIDITY 和最后同步的 UID。
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - IMAP 文件夹名称
/// * `uidvalidity` - IMAP UIDVALIDITY 值
/// * `last_sync_uid` - 最后同步的 UID
///
/// # 返回
///
/// 返回保存或更新后的同步状态模型
pub async fn save_or_update_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidvalidity: u64,
    last_sync_uid: i32,
) -> Result<folder_sync_state::Model> {
    let now = Utc::now().timestamp();

    // 查找是否已存在该账号+文件夹的记录
    let existing_state = folder_sync_state::Entity::find()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(folder))
        .one(db)
        .await?;

    if let Some(existing) = existing_state {
        // 更新现有记录
        let mut active: folder_sync_state::ActiveModel = existing.into();
        active.uidvalidity = Set(Some(uidvalidity as i64));
        active.last_sync_uid = Set(Some(last_sync_uid));
        active.synced_at = Set(Some(now));
        active.updated_at = Set(Some(now));

        let updated = active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;

        tracing::debug!(
            "更新文件夹同步状态: account_id={}, folder={}, uidvalidity={}, last_sync_uid={}",
            account_id,
            folder,
            uidvalidity,
            last_sync_uid
        );

        Ok(updated)
    } else {
        // 创建新记录
        let active = folder_sync_state::ActiveModel {
            id: ActiveValue::NotSet,
            account_id: Set(account_id),
            imap_name: Set(folder.to_string()),
            uidvalidity: Set(Some(uidvalidity as i64)),
            last_sync_uid: Set(Some(last_sync_uid)),
            synced_at: Set(Some(now)),
            sync_count: Set(0),
            is_first_sync: Set(true),
            error_count: Set(0),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };

        let inserted = active
            .insert(db)
            .await
            .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;

        tracing::info!(
            "创建文件夹同步状态: account_id={}, folder={}, uidvalidity={}, last_sync_uid={}",
            account_id,
            folder,
            uidvalidity,
            last_sync_uid
        );

        Ok(inserted)
    }
}

/// 获取账号的所有文件夹同步状态
///
/// # 参数
///
/// * `account_id` - 账号 ID
///
/// # 返回
///
/// 返回同步状态列表
pub async fn get_all_sync_states(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<folder_sync_state::Model>> {
    let states = folder_sync_state::Entity::find()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .all(db)
        .await?;

    Ok(states)
}

/// 获取指定类型的文件夹列表
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder_type` - 文件夹类型（inbox, sent, drafts, spam, trash, archive, other）
///
/// # 返回
///
/// 返回匹配类型的文件夹 IMAP 名称列表
pub async fn get_folders_by_type(
    db: &DbConn,
    account_id: i32,
    folder_type: &str,
) -> Result<Vec<String>> {
    let states = folder_sync_state::Entity::find()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::FolderType.eq(folder_type))
        .all(db)
        .await?;

    Ok(states.into_iter().map(|s| s.imap_name).collect())
}

/// 重置文件夹同步状态
///
/// 当 UIDVALIDITY 变化时调用，清除本地同步状态以便进行完整同步。
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `imap_name` - IMAP 文件夹名称
pub async fn reset_sync_state(db: &DbConn, account_id: i32, imap_name: &str) -> Result<()> {
    tracing::info!(
        "重置文件夹同步状态: account_id={}, folder={}",
        account_id,
        imap_name
    );

    if let Some(existing) = get_sync_state(db, account_id, imap_name).await? {
        let mut active: folder_sync_state::ActiveModel = existing.into();
        // 重置同步状态
        active.uidvalidity = Set(None);
        active.uidnext = Set(None);
        active.synced_at = Set(None);
        active.updated_at = Set(Some(chrono::Utc::now().timestamp()));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("重置同步状态失败: {}", e)))?;
    }

    Ok(())
}

// === 同步进度管理方法（合并自 SyncStateManager）===

/// 更新同步进度
///
/// 在同步过程中调用，更新已同步邮件数量
pub async fn update_progress(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    synced_count: i32,
) -> Result<()> {
    let now = Utc::now().timestamp();

    folder_sync_state::Entity::update_many()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .col_expr(
            folder_sync_state::Column::SyncCount,
            Expr::val(synced_count),
        )
        .col_expr(folder_sync_state::Column::UpdatedAt, Expr::val(now))
        .exec(db)
        .await
        .map_err(|e| MailError::Internal(format!("更新同步进度失败: {}", e)))?;

    Ok(())
}

/// 记录同步错误
///
/// 增加错误计数并记录错误信息
pub async fn record_error(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    error_message: &str,
) -> Result<()> {
    let now = Utc::now().timestamp();

    folder_sync_state::Entity::update_many()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .col_expr(
            folder_sync_state::Column::ErrorCount,
            Expr::col(folder_sync_state::Column::ErrorCount).add(1),
        )
        .col_expr(
            folder_sync_state::Column::LastError,
            Expr::val(error_message),
        )
        .col_expr(folder_sync_state::Column::UpdatedAt, Expr::val(now))
        .exec(db)
        .await
        .map_err(|e| MailError::Internal(format!("记录同步错误失败: {}", e)))?;

    Ok(())
}

/// 清除错误计数
///
/// 成功同步后调用，清除错误计数和错误信息
pub async fn clear_errors(db: &DbConn, account_id: i32, imap_name: &str) -> Result<()> {
    let now = Utc::now().timestamp();

    folder_sync_state::Entity::update_many()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .col_expr(folder_sync_state::Column::ErrorCount, Expr::val(0))
        .col_expr(
            folder_sync_state::Column::LastError,
            Expr::val(Option::<String>::None),
        )
        .col_expr(folder_sync_state::Column::UpdatedAt, Expr::val(now))
        .exec(db)
        .await
        .map_err(|e| MailError::Internal(format!("清除错误计数失败: {}", e)))?;

    Ok(())
}

/// 更新 last_sync_uid
pub async fn update_last_sync_uid(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    last_sync_uid: i32,
) -> Result<()> {
    if let Some(existing) = get_sync_state(db, account_id, imap_name).await? {
        let mut active: folder_sync_state::ActiveModel = existing.into();
        active.last_sync_uid = Set(Some(last_sync_uid));
        active.updated_at = Set(Some(Utc::now().timestamp()));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新 last_sync_uid 失败: {}", e)))?;

        tracing::debug!(
            "更新 last_sync_uid: account_id={}, imap_name={}, uid={}",
            account_id,
            imap_name,
            last_sync_uid
        );
    }

    Ok(())
}

/// 更新 highest_uid
pub async fn update_highest_uid(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    highest_uid: i32,
) -> Result<()> {
    if let Some(existing) = get_sync_state(db, account_id, imap_name).await? {
        let mut active: folder_sync_state::ActiveModel = existing.into();
        active.highest_uid = Set(Some(highest_uid));
        active.updated_at = Set(Some(Utc::now().timestamp()));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新 highest_uid 失败: {}", e)))?;

        tracing::debug!(
            "更新 highest_uid: account_id={}, imap_name={}, uid={}",
            account_id,
            imap_name,
            highest_uid
        );
    }

    Ok(())
}

/// 更新同步完成
///
/// 同步完成后调用，更新同步数量、清除错误计数
pub async fn update_sync_completed(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
    sync_count: i32,
) -> Result<()> {
    if let Some(existing) = get_sync_state(db, account_id, imap_name).await? {
        let old_sync_count = existing.sync_count;
        let total_sync_count = old_sync_count + sync_count;

        let mut active: folder_sync_state::ActiveModel = existing.into();
        active.sync_count = Set(total_sync_count);
        active.error_count = Set(0); // 重置错误计数
        active.last_error = Set(None);
        active.updated_at = Set(Some(Utc::now().timestamp()));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新同步完成状态失败: {}", e)))?;

        tracing::info!(
            "同步完成: account_id={}, imap_name={}, count={}, total={}",
            account_id,
            imap_name,
            sync_count,
            total_sync_count
        );
    }

    Ok(())
}

/// 删除文件夹同步状态
pub async fn delete(db: &DbConn, account_id: i32, imap_name: &str) -> Result<()> {
    folder_sync_state::Entity::delete_many()
        .filter(folder_sync_state::Column::AccountId.eq(account_id))
        .filter(folder_sync_state::Column::ImapName.eq(imap_name))
        .exec(db)
        .await
        .map_err(|e| MailError::Internal(format!("删除文件夹同步状态失败: {}", e)))?;

    Ok(())
}

/// 同步状态更新结果
#[derive(Debug, Clone)]
pub struct SyncStateUpdateResult {
    /// 更新的文件夹数量
    pub updated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_update_result() {
        let result = SyncStateUpdateResult { updated: 5 };
        assert_eq!(result.updated, 5);
    }
}
