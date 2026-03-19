//! 同步错误管理
//!
//! 管理同步过程中的错误记录和状态

use crate::error::{MailError, Result};
use crate::storage::models::sync_error;
use sea_orm::{DbConn, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, QueryOrder, QuerySelect, sea_query::Expr};
use std::sync::Arc;
use chrono::Utc;

/// 同步错误管理器
///
/// 负责管理同步过程中的错误记录，包括：
/// - 错误记录的创建和查询
/// - 错误状态的标记（已解决/未解决）
/// - 旧错误的清理
pub struct SyncErrorManager {
    db: Arc<DbConn>,
}

impl SyncErrorManager {
    /// 创建新的同步错误管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 获取账号的所有同步错误
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `limit` - 可选的记录数量限制
    ///
    /// # 返回
    ///
    /// 返回同步错误列表（按创建时间倒序）
    pub async fn get_by_account(
        &self,
        account_id: i32,
        limit: Option<u64>,
    ) -> Result<Vec<sync_error::Model>> {
        let mut query = sync_error::Entity::find()
            .filter(sync_error::Column::AccountId.eq(account_id))
            .order_by_desc(sync_error::Column::CreatedAt);

        if let Some(limit) = limit {
            query = query.limit(limit);
        }

        query
            .all(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("获取同步错误列表失败: {}", e)))
    }

    /// 获取未解决的错误
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `limit` - 可选的记录数量限制
    ///
    /// # 返回
    ///
    /// 返回未解决的错误列表（按创建时间倒序）
    pub async fn get_unresolved(
        &self,
        account_id: i32,
        limit: Option<u64>,
    ) -> Result<Vec<sync_error::Model>> {
        let mut query = sync_error::Entity::find()
            .filter(sync_error::Column::AccountId.eq(account_id))
            .filter(sync_error::Column::Resolved.eq(false))
            .order_by_desc(sync_error::Column::CreatedAt);

        if let Some(limit) = limit {
            query = query.limit(limit);
        }

        query
            .all(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("获取未解决错误失败: {}", e)))
    }

    /// 记录同步错误
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 可选的文件夹名称
    /// * `error_type` - 错误类型
    /// * `error_message` - 错误信息
    /// * `uid` - 可选的邮件 UID
    /// * `stack_trace` - 可选的堆栈跟踪
    ///
    /// # 返回
    ///
    /// 返回创建的错误记录
    pub async fn create(
        &self,
        account_id: i32,
        folder: Option<&str>,
        error_type: &str,
        error_message: &str,
        uid: Option<i32>,
        stack_trace: Option<&str>,
    ) -> Result<sync_error::Model> {
        let now = Utc::now().timestamp();

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
            .insert(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("记录同步错误失败: {}", e)))
    }

    /// 标记错误为已解决
    ///
    /// # 参数
    ///
    /// * `id` - 错误记录 ID
    pub async fn mark_resolved(&self, id: i32) -> Result<()> {
        sync_error::Entity::update_many()
            .filter(sync_error::Column::Id.eq(id))
            .col_expr(
                sync_error::Column::Resolved,
                Expr::val(true),
            )
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("标记错误已解决失败: {}", e)))?;

        tracing::debug!("标记同步错误为已解决: id={}", id);

        Ok(())
    }

    /// 批量标记账号的错误为已解决
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    pub async fn mark_all_resolved(&self, account_id: i32) -> Result<()> {
        let result = sync_error::Entity::update_many()
            .filter(sync_error::Column::AccountId.eq(account_id))
            .filter(sync_error::Column::Resolved.eq(false))
            .col_expr(
                sync_error::Column::Resolved,
                Expr::val(true),
            )
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("批量标记错误已解决失败: {}", e)))?;

        tracing::info!(
            "批量标记同步错误为已解决: account_id={}, affected={}",
            account_id,
            result.rows_affected
        );

        Ok(())
    }

    /// 删除旧的已解决错误
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `days` - 保留天数（删除超过此天数的已解决错误）
    ///
    /// # 返回
    ///
    /// 返回删除的记录数
    pub async fn cleanup_old_resolved(
        &self,
        account_id: i32,
        days: i64,
    ) -> Result<u64> {
        let cutoff_time = Utc::now().timestamp() - (days * 24 * 60 * 60);

        let result = sync_error::Entity::delete_many()
            .filter(sync_error::Column::AccountId.eq(account_id))
            .filter(sync_error::Column::Resolved.eq(true))
            .filter(sync_error::Column::CreatedAt.lt(cutoff_time))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("清理旧错误失败: {}", e)))?;

        tracing::info!(
            "清理旧同步错误: account_id={}, days={}, deleted={}",
            account_id,
            days,
            result.rows_affected
        );

        Ok(result.rows_affected)
    }

    /// 删除同步错误
    ///
    /// # 参数
    ///
    /// * `id` - 错误记录 ID
    pub async fn delete(&self, id: i32) -> Result<()> {
        sync_error::Entity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("删除同步错误失败: {}", e)))?;

        tracing::debug!("删除同步错误: id={}", id);

        Ok(())
    }
}

impl Default for SyncErrorManager {
    fn default() -> Self {
        // 需要数据库连接，这里提供默认实现但不建议使用
        panic!("SyncErrorManager 需要数据库连接，请使用 new() 构造")
    }
}
