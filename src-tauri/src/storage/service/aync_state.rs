//! 文件夹同步状态管理器
//!
//! 管理文件夹同步状态，不管理文件夹配置

use crate::error::{MailError, Result};
use crate::protocols::imap::FolderInfo as ImapFolderInfo;
use crate::storage::models::sync_state;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DbConn, EntityTrait, QueryFilter, Set};

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
        let existing_state = sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .filter(sync_state::Column::Folder.eq(&info.name))
            .one(db)
            .await?;

        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = existing_state {
            // 更新现有记录
            let mut active: sync_state::ActiveModel = existing.into();
            active.synced_at = Set(Some(now));
            active.updated_at = Set(Some(now));

            active
                .update(db)
                .await
                .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "更新文件夹同步状态: account_id={}, folder={}",
                account_id,
                info.name
            );

            updated_count += 1;
        } else {
            // 创建新记录（IMAP 元数据初始为 None）
            let active = sync_state::ActiveModel {
                id: ActiveValue::NotSet, // 自增
                account_id: Set(account_id),
                folder: Set(info.name.clone()),
                folder_nick_name: Set(None),
                uidvalidity: Set(None),
                uidnext: Set(None),
                synced_at: Set(Some(now)),
                last_sync_uid: Set(None),
                ..Default::default()
            };

            active
                .insert(db)
                .await
                .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;

            tracing::debug!(
                "创建文件夹同步状态: account_id={}, folder={}",
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

/// 更新文件夹的 IMAP 元数据
///
/// 在同步邮件时调用，更新 uidvalidity, uidnext
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `uidvalidity` - IMAP UIDVALIDITY 值
/// * `uidnext` - 预期的下一个 UID
pub async fn update_folder_metadata(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidvalidity: Option<u64>,
    uidnext: Option<u64>,
) -> Result<()> {
    let existing_state = sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .one(db)
        .await?;

    let now = chrono::Utc::now().timestamp();

    if let Some(existing) = existing_state {
        // 更新现有记录
        let mut active: sync_state::ActiveModel = existing.into();
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
        let active = sync_state::ActiveModel {
            id: ActiveValue::NotSet,
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            folder_nick_name: Set(None),
            uidvalidity: Set(uidvalidity.map(|v| v as i64)),
            uidnext: Set(uidnext.map(|v| v as i64)),
            synced_at: Set(Some(now)),
            last_sync_uid: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
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
/// * `folder` - 文件夹名称
///
/// # 返回
///
/// 返回同步状态
pub async fn get_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>> {
    let state = sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
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
/// * `folder` - 文件夹名称
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
) -> Result<sync_state::Model> {
    let now = Utc::now().timestamp();

    // 查找是否已存在该账号+文件夹的记录
    let existing_state = sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
        .one(db)
        .await?;

    if let Some(existing) = existing_state {
        // 更新现有记录
        let mut active: sync_state::ActiveModel = existing.into();
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
        let active = sync_state::ActiveModel {
            id: ActiveValue::NotSet,
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            folder_nick_name: Set(None),
            uidvalidity: Set(Some(uidvalidity as i64)),
            last_sync_uid: Set(Some(last_sync_uid)),
            synced_at: Set(Some(now)),
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

pub async fn update_last_sync_uid(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    last_sync_uid: i32,
) -> Result<()> {
    if let Some(existing) = get_sync_state(db, account_id, folder).await? {
        let mut active: sync_state::ActiveModel = existing.into();
        active.last_sync_uid = Set(Some(last_sync_uid));
        active.updated_at = Set(Some(Utc::now().timestamp()));

        active
            .update(db)
            .await
            .map_err(|e| MailError::Internal(format!("更新 last_sync_uid 失败: {}", e)))?;

        tracing::debug!(
            "更新 last_sync_uid: account_id={}, folder={}, uid={}",
            account_id,
            folder,
            last_sync_uid
        );
    }

    Ok(())
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
pub async fn get_all_sync_states(db: &DbConn, account_id: i32) -> Result<Vec<sync_state::Model>> {
    let states = sync_state::Entity::find()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .all(db)
        .await?;

    Ok(states)
}

/// 重置文件夹同步状态
///
/// 当 UIDVALIDITY 变化时调用，清除本地同步状态以便进行完整同步。
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
pub async fn reset_sync_state(db: &DbConn, account_id: i32, folder: &str) -> Result<()> {
    tracing::info!(
        "重置文件夹同步状态: account_id={}, folder={}",
        account_id,
        folder
    );

    if let Some(existing) = get_sync_state(db, account_id, folder).await? {
        let mut active: sync_state::ActiveModel = existing.into();
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

/// 删除文件夹同步状态
pub async fn delete(db: &DbConn, account_id: i32, folder: &str) -> Result<()> {
    sync_state::Entity::delete_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .filter(sync_state::Column::Folder.eq(folder))
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
