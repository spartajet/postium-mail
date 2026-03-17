//! 同步状态管理
//!
//! 管理同步状态，包括最高 MODSEQ、UID 等元数据

use crate::error::{MailError, Result};
use crate::models::sync_state;
use sea_orm::{DbConn, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set};
use std::sync::Arc;
use chrono::Utc;

/// 同步状态管理器
///
/// 负责管理文件夹的同步状态，包括：
/// - highest_modseq: CONDSTORE 最高修改序列号
/// - last_sync_uid: 上次同步的最高 UID
/// - last_sync_at: 上次同步时间
pub struct SyncStateManager {
    db: Arc<DbConn>,
}

impl SyncStateManager {
    /// 创建新的同步状态管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 获取文件夹的同步状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    ///
    /// # 返回
    ///
    /// 返回同步状态（如果存在）
    pub async fn get_state(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<Option<sync_state::Model>> {
        use sea_orm::EntityTrait;

        let states = sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .filter(sync_state::Column::Folder.eq(folder))
            .all(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("获取同步状态失败: {}", e)))?;

        Ok(states.first().cloned())
    }

    /// 获取或创建同步状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    ///
    /// # 返回
    ///
    /// 返回同步状态（如果不存在则创建新的）
    pub async fn get_or_create_state(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<sync_state::Model> {
        // 尝试获取现有状态
        if let Some(state) = self.get_state(account_id, folder).await? {
            return Ok(state);
        }

        // 创建新状态
        let new_state = sync_state::ActiveModel {
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            ..Default::default()
        };

        let inserted = new_state
            .insert(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("创建同步状态失败: {}", e)))?;

        // 重新查询以获取完整数据
        self.get_state(account_id, folder)
            .await?
            .ok_or_else(|| MailError::Internal("创建同步状态后无法查询".to_string()))
    }

    /// 更新 highest_modseq
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `highest_modseq` - 最高的 MODSEQ 值
    pub async fn update_highest_modseq(
        &self,
        account_id: i32,
        folder: &str,
        highest_modseq: i64,
    ) -> Result<()> {
        use sea_orm::{EntityTrait, ActiveModelTrait, IntoActiveModel};

        let state = self.get_or_create_state(account_id, folder).await?;

        let mut active_model: sync_state::ActiveModel = state.into();
        active_model.highest_modseq = Set(Some(highest_modseq));
        active_model.updated_at = Set(Utc::now().timestamp());

        active_model
            .update(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新 highest_modseq 失败: {}", e)))?;

        tracing::debug!(
            "更新 highest_modseq: account_id={}, folder={}, modseq={}",
            account_id,
            folder,
            highest_modseq
        );

        Ok(())
    }

    /// 更新 last_sync_uid
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `last_sync_uid` - 最后同步的 UID
    pub async fn update_last_sync_uid(
        &self,
        account_id: i32,
        folder: &str,
        last_sync_uid: i32,
    ) -> Result<()> {
        use sea_orm::{EntityTrait, ActiveModelTrait, IntoActiveModel};

        let state = self.get_or_create_state(account_id, folder).await?;

        let mut active_model: sync_state::ActiveModel = state.into();
        active_model.last_sync_uid = Set(Some(last_sync_uid));
        active_model.updated_at = Set(Utc::now().timestamp());

        active_model
            .update(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新 last_sync_uid 失败: {}", e)))?;

        tracing::debug!(
            "更新 last_sync_uid: account_id={}, folder={}, uid={}",
            account_id,
            folder,
            last_sync_uid
        );

        Ok(())
    }

    /// 更新同步完成时间
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `sync_count` - 本次同步的邮件数量
    pub async fn update_sync_completed(
        &self,
        account_id: i32,
        folder: &str,
        sync_count: i32,
    ) -> Result<()> {
        use sea_orm::{EntityTrait, ActiveModelTrait, IntoActiveModel};

        let state = self.get_or_create_state(account_id, folder).await?;

        // 在 move 之前保存需要的值
        let old_sync_count = state.sync_count;
        let total_sync_count = old_sync_count + sync_count;

        let mut active_model: sync_state::ActiveModel = state.into();
        active_model.last_sync_at = Set(Some(Utc::now().timestamp()));
        active_model.sync_count = Set(total_sync_count);
        active_model.error_count = Set(0); // 重置错误计数
        active_model.last_error = Set(None);
        active_model.updated_at = Set(Utc::now().timestamp());

        active_model
            .update(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新同步完成状态失败: {}", e)))?;

        tracing::info!(
            "同步完成: account_id={}, folder={}, count={}, total={}",
            account_id,
            folder,
            sync_count,
            total_sync_count
        );

        Ok(())
    }

    /// 更新同步错误
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `error` - 错误信息
    pub async fn update_sync_error(
        &self,
        account_id: i32,
        folder: &str,
        error: String,
    ) -> Result<()> {
        use sea_orm::{EntityTrait, ActiveModelTrait, IntoActiveModel};

        let state = self.get_or_create_state(account_id, folder).await?;

        // 在 move 之前保存需要的值
        let old_error_count = state.error_count;
        let new_error_count = old_error_count + 1;

        let mut active_model: sync_state::ActiveModel = state.into();
        active_model.error_count = Set(new_error_count);
        active_model.last_error = Set(Some(error));
        active_model.updated_at = Set(Utc::now().timestamp());

        active_model
            .update(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新同步错误状态失败: {}", e)))?;

        tracing::error!(
            "同步错误: account_id={}, folder={}, error_count={}",
            account_id,
            folder,
            new_error_count
        );

        Ok(())
    }
}

impl Default for SyncStateManager {
    fn default() -> Self {
        // 需要数据库连接，这里提供默认实现但不建议使用
        panic!("SyncStateManager 需要数据库连接，请使用 new() 构造")
    }
}
