//! 同步状态管理
//!
//! 统一管理同步过程中的状态信息

use chrono::{DateTime, Utc};
use sea_orm::DbConn;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{MailError, Result};

/// 文件夹同步状态
#[derive(Debug, Clone)]
pub struct FolderSyncState {
    /// 文件夹名称
    pub folder: String,
    /// UIDVALIDITY
    pub uidvalidity: Option<u64>,
    /// UIDNEXT
    pub uidnext: Option<u64>,
    /// 上次同步的最高 UID
    pub last_sync_uid: Option<i64>,
    /// 上次同步时间
    pub last_sync_time: Option<DateTime<Utc>>,
    /// 同步状态
    pub status: SyncStatus,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

impl FolderSyncState {
    /// 创建新的文件夹同步状态
    pub fn new(folder: String) -> Self {
        Self {
            folder,
            uidvalidity: None,
            uidnext: None,
            last_sync_uid: None,
            last_sync_time: None,
            status: SyncStatus::Idle,
            error: None,
        }
    }

    /// 是否需要全量同步
    pub fn needs_full_sync(&self, current_uidvalidity: u64) -> bool {
        match self.uidvalidity {
            Some(stored_uidvalidity) => stored_uidvalidity != current_uidvalidity,
            None => true,
        }
    }

    /// 更新 UIDVALIDITY
    pub fn update_uidvalidity(&mut self, uidvalidity: u64) {
        self.uidvalidity = Some(uidvalidity);
    }

    /// 更新 UIDNEXT
    pub fn update_uidnext(&mut self, uidnext: u64) {
        self.uidnext = Some(uidnext);
    }

    /// 更新同步 UID
    pub fn update_last_sync_uid(&mut self, uid: u32) {
        self.last_sync_uid = Some(uid as i64);
        self.last_sync_time = Some(Utc::now());
    }
}

/// 同步状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// 空闲
    Idle,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 失败
    Failed,
}

/// 统一的同步状态管理器
///
/// 负责管理所有账号和文件夹的同步状态
pub struct SyncState {
    /// 数据库连接
    db: Arc<DbConn>,
    /// 状态缓存 (account_id -> folder -> state)
    state_cache: Arc<RwLock<HashMap<i32, HashMap<String, FolderSyncState>>>>,
}

impl SyncState {
    /// 创建新的同步状态管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self {
            db,
            state_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取文件夹同步状态
    pub async fn get_folder_state(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<Option<FolderSyncState>> {
        // 先从缓存获取
        {
            let cache = self.state_cache.read().await;
            if let Some(account_state) = cache.get(&account_id)
                && let Some(folder_state) = account_state.get(folder)
            {
                return Ok(Some(folder_state.clone()));
            }
        }

        // 从数据库获取
        let state = self.load_folder_state_from_db(account_id, folder).await?;

        // 更新缓存
        if let Some(state) = &state {
            let mut cache = self.state_cache.write().await;
            let account_state = cache.entry(account_id).or_default();
            account_state.insert(folder.to_string(), state.clone());
        }

        Ok(state)
    }

    /// 更新文件夹同步状态
    pub async fn update_folder_state(
        &self,
        account_id: i32,
        folder: &str,
        state: FolderSyncState,
    ) -> Result<()> {
        // 更新缓存
        {
            let mut cache = self.state_cache.write().await;
            let account_state = cache.entry(account_id).or_default();
            account_state.insert(folder.to_string(), state.clone());
        }

        // 更新数据库
        self.save_folder_state_to_db(account_id, folder, &state).await
    }

    /// 从数据库加载文件夹同步状态
    async fn load_folder_state_from_db(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<Option<FolderSyncState>> {
        use crate::storage::models::folder_sync_state;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

        match folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::Folder.eq(folder))
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(model)) => Ok(Some(FolderSyncState {
                folder: model.folder,
                uidvalidity: model.uidvalidity.map(|v| v as u64),
                uidnext: model.uidnext.map(|v| v as u64),
                last_sync_uid: model.last_sync_uid.map(|v| v as i64),
                last_sync_time: model.synced_at.map(|ts| {
                    DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)
                }),
                status: SyncStatus::Idle, // 从数据库加载的状态默认为 Idle
                error: None,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(MailError::Internal(format!("加载文件夹同步状态失败: {}", e))),
        }
    }

    /// 保存文件夹同步状态到数据库
    async fn save_folder_state_to_db(
        &self,
        account_id: i32,
        folder: &str,
        state: &FolderSyncState,
    ) -> Result<()> {
        use crate::storage::models::folder_sync_state;
        use sea_orm::{ActiveModelTrait, Set, EntityTrait, QueryFilter, ColumnTrait};

        // 查找现有记录
        let existing = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::Folder.eq(folder))
            .one(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("查询文件夹同步状态失败: {}", e)))?;

        let now = Utc::now().timestamp();

        if let Some(model) = existing {
            // 更新现有记录
            let mut active_model: folder_sync_state::ActiveModel = model.into();
            active_model.uidvalidity = Set(state.uidvalidity.map(|v| v as i64));
            active_model.uidnext = Set(state.uidnext.map(|v| v as i64));
            active_model.last_sync_uid = Set(state.last_sync_uid.map(|v| v as i32));
            active_model.synced_at = Set(state.last_sync_time.map(|dt| dt.timestamp()));
            active_model.updated_at = Set(Some(now));

            active_model
                .update(self.db.as_ref())
                .await
                .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;
        } else {
            // 创建新记录
            let new_state = folder_sync_state::ActiveModel {
                account_id: Set(account_id),
                folder: Set(folder.to_string()),
                uidvalidity: Set(state.uidvalidity.map(|v| v as i64)),
                uidnext: Set(state.uidnext.map(|v| v as i64)),
                last_sync_uid: Set(state.last_sync_uid.map(|v| v as i32)),
                synced_at: Set(state.last_sync_time.map(|dt| dt.timestamp())),
                created_at: Set(Some(now)),
                updated_at: Set(Some(now)),
                ..Default::default()
            };

            new_state
                .insert(self.db.as_ref())
                .await
                .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;
        }

        Ok(())
    }

    /// 获取账号的所有文件夹状态
    pub async fn get_account_states(&self, account_id: i32) -> Result<Vec<FolderSyncState>> {
        use crate::storage::models::folder_sync_state;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QueryOrder};

        let models = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .order_by_asc(folder_sync_state::Column::Folder)
            .all(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("获取账号同步状态失败: {}", e)))?;

        let states = models
            .iter()
            .map(|model| FolderSyncState {
                folder: model.folder.clone(),
                uidvalidity: model.uidvalidity.map(|v| v as u64),
                uidnext: model.uidnext.map(|v| v as u64),
                last_sync_uid: model.last_sync_uid.map(|v| v as i64),
                last_sync_time: model.synced_at.map(|ts| {
                    DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)
                }),
                status: SyncStatus::Idle,
                error: None,
            })
            .collect();

        Ok(states)
    }

    /// 清除账号的状态缓存
    pub async fn clear_account_cache(&self, account_id: i32) {
        let mut cache = self.state_cache.write().await;
        cache.remove(&account_id);
    }

    /// 清除所有状态缓存
    pub async fn clear_all_cache(&self) {
        let mut cache = self.state_cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_sync_state() {
        let mut state = FolderSyncState::new("INBOX".to_string());
        assert!(!state.needs_full_sync(123)); // None 意味着需要全量同步

        state.update_uidvalidity(123);
        assert!(!state.needs_full_sync(123)); // 相同不需要
        assert!(state.needs_full_sync(456)); // 不同需要
    }

    #[test]
    fn test_folder_sync_state_updates() {
        let mut state = FolderSyncState::new("INBOX".to_string());
        state.update_uidvalidity(123);
        state.update_uidnext(456);
        state.update_last_sync_uid(789);

        assert_eq!(state.uidvalidity, Some(123));
        assert_eq!(state.uidnext, Some(456));
        assert_eq!(state.last_sync_uid, Some(789));
        assert!(state.last_sync_time.is_some());
    }
}
