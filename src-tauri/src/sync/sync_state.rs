//! 同步状态管理
//!
//! 管理同步状态，包括最高 MODSEQ、UID 等元数据

use crate::error::{MailError, Result};
use crate::models::sync_state;
use sea_orm::{DbConn, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, sea_query::Expr, ExprTrait};
use std::sync::Arc;
use chrono::Utc;

/// 同步状态管理器
///
/// 负责管理文件夹的同步状态，包括：
/// - highest_modseq: CONDSTORE 最高修改序列号
/// - last_sync_uid: 上次同步的最高 UID
/// - last_sync_at: 上次同步时间
/// - sync_count: 同步邮件数量
/// - error_count: 错误计数
pub struct SyncStateManager {
    db: Arc<DbConn>,
}

impl SyncStateManager {
    /// 创建新的同步状态管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 获取账号的所有同步状态
    pub async fn get_all_by_account(
        &self,
        account_id: i32,
    ) -> Result<Vec<sync_state::Model>> {
        let states = sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .all(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("获取同步状态列表失败: {}", e)))?;

        Ok(states)
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

    /// 创建或更新同步状态（完整版本）
    ///
    /// 整合自 services::sync_state_service::upsert
    pub async fn upsert(
        &self,
        account_id: i32,
        folder: &str,
        last_sync_uid: Option<i32>,
        highest_uid: Option<i32>,
        sync_count: i32,
        is_first_sync: bool,
    ) -> Result<sync_state::Model> {
        let now = Utc::now().timestamp();

        // 尝试查找现有记录
        if let Some(existing) = self.get_state(account_id, folder).await? {
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

            Ok(active.update(self.db.as_ref()).await.map_err(|e| {
                MailError::Internal(format!("更新同步状态失败: {}", e))
            })?)
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

            Ok(new_state.insert(self.db.as_ref()).await.map_err(|e| {
                MailError::Internal(format!("创建同步状态失败: {}", e))
            })?)
        }
    }

    /// 更新同步进度（在同步过程中调用）
    ///
    /// 整合自 services::sync_state_service::update_progress
    pub async fn update_progress(
        &self,
        account_id: i32,
        folder: &str,
        synced_count: i32,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        sync_state::Entity::update_many()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .filter(sync_state::Column::Folder.eq(folder))
            .col_expr(
                sync_state::Column::SyncCount,
                Expr::val(synced_count),
            )
            .col_expr(
                sync_state::Column::UpdatedAt,
                Expr::val(now),
            )
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新同步进度失败: {}", e)))?;

        Ok(())
    }

    /// 记录同步错误
    ///
    /// 整合自 services::sync_state_service::record_error
    pub async fn record_error(
        &self,
        account_id: i32,
        folder: &str,
        error_message: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        // 增加错误计数
        sync_state::Entity::update_many()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .filter(sync_state::Column::Folder.eq(folder))
            .col_expr(
                sync_state::Column::ErrorCount,
                Expr::col(sync_state::Column::ErrorCount).add(1),
            )
            .col_expr(
                sync_state::Column::LastError,
                Expr::val(error_message),
            )
            .col_expr(
                sync_state::Column::UpdatedAt,
                Expr::val(now),
            )
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("记录同步错误失败: {}", e)))?;

        Ok(())
    }

    /// 清除错误计数
    ///
    /// 整合自 services::sync_state_service::clear_errors
    pub async fn clear_errors(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        sync_state::Entity::update_many()
            .filter(sync_state::Column::AccountId.eq(account_id))
            .filter(sync_state::Column::Folder.eq(folder))
            .col_expr(
                sync_state::Column::ErrorCount,
                Expr::val(0),
            )
            .col_expr(
                sync_state::Column::LastError,
                Expr::val(Option::<String>::None),
            )
            .col_expr(
                sync_state::Column::UpdatedAt,
                Expr::val(now),
            )
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("清除错误计数失败: {}", e)))?;

        Ok(())
    }

    /// 删除同步状态
    pub async fn delete(&self, id: i32) -> Result<()> {
        sync_state::Entity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("删除同步状态失败: {}", e)))?;

        Ok(())
    }

    /// 检查是否需要首次同步
    ///
    /// 整合自 services::sync_state_service::needs_first_sync
    pub async fn needs_first_sync(&self, account_id: i32, folder: &str) -> Result<bool> {
        if let Some(state) = self.get_state(account_id, folder).await? {
            Ok(state.is_first_sync)
        } else {
            Ok(true)
        }
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
