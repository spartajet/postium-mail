use sea_orm::DbConn;
use anyhow::{anyhow, Result};
use tauri::{AppHandle, Emitter};
use std::sync::Arc;
use crate::models::account;
use crate::services::{
    account_service, folder_service, sync_state_service, sync_error_service,
    imap_service,
};
use crate::crypto::{KEYRING_SERVICE, password_username};
use tauri_plugin_keyring::KeyringExt;

/// 同步进度回调类型
pub type ProgressCallback = Box<dyn Fn(SyncProgress) + Send + Sync>;

/// 同步进度信息
#[derive(Clone, Debug)]
pub struct SyncProgress {
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

/// 同步阶段
#[derive(Clone, Debug, PartialEq)]
pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}

/// 同步结果
#[derive(Clone, Debug)]
pub struct SyncResult {
    pub total_synced: usize,
    pub folders_synced: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

/// 同步管理器
pub struct SyncManager {
    db: Arc<DbConn>,
    app_handle: AppHandle,
}

impl SyncManager {
    /// 创建新的同步管理器
    pub fn new(db: Arc<DbConn>, app_handle: AppHandle) -> Self {
        Self { db, app_handle }
    }

    /// 执行完整同步（自动判断首次同步或增量同步）
    pub async fn sync_account(
        &self,
        account_id: i32,
    ) -> Result<SyncResult> {
        let start_time = std::time::Instant::now();

        // 发送开始事件
        self.emit_progress(account_id, SyncProgress {
            stage: SyncStage::Connecting,
            folder: None,
            current: 0,
            total: 0,
            message: "正在连接服务器...".to_string(),
        })?;

        // 获取账号信息
        let account = match account_service::get_by_id(&self.db, account_id).await {
            Ok(Some(acc)) => acc,
            Ok(None) => {
                let _ = self.emit_progress(account_id, SyncProgress {
                    stage: SyncStage::Error,
                    folder: None,
                    current: 0,
                    total: 0,
                    message: "账号不存在".to_string(),
                });
                return Err(anyhow!("账号不存在"));
            }
            Err(e) => {
                let msg = format!("获取账号信息失败: {}", e);
                let _ = self.emit_progress(account_id, SyncProgress {
                    stage: SyncStage::Error,
                    folder: None,
                    current: 0,
                    total: 0,
                    message: msg.clone(),
                });
                return Err(anyhow!(msg));
            }
        };

        // 获取密码（从 Stronghold）
        let password = match self.get_account_password(&account).await {
            Ok(pwd) => pwd,
            Err(e) => {
                let msg = format!("获取密码失败: {}", e);
                let _ = self.emit_progress(account_id, SyncProgress {
                    stage: SyncStage::Error,
                    folder: None,
                    current: 0,
                    total: 0,
                    message: msg.clone(),
                });
                return Err(anyhow!(msg));
            }
        };

        tracing::info!("准备连接 IMAP 服务器: email={}, host={:?}, port={:?}",
            account.email, account.imap_host, account.imap_port);

        // 创建 IMAP 服务
        let mut imap_service = imap_service::ImapService::new();

        // 连接到 IMAP 服务器
        let host = account.imap_host.unwrap_or_default();
        let port = account.imap_port.unwrap_or(993) as u16;

        tracing::info!("开始 IMAP 连接: host={}, port={}", host, port);

        if let Err(e) = imap_service.connect(
            &host,
            port,
            &account.email,
            imap_service::ImapAuth::Password(password),
        ) {
            let msg = format!("连接 IMAP 服务器失败: {}", e);
            let _ = self.emit_progress(account_id, SyncProgress {
                stage: SyncStage::Error,
                folder: None,
                current: 0,
                total: 0,
                message: msg.clone(),
            });
            return Err(anyhow!(msg));
        }

        // 1. 同步文件夹列表
        self.emit_progress(account_id, SyncProgress {
            stage: SyncStage::SyncingFolders,
            folder: None,
            current: 0,
            total: 0,
            message: "正在同步文件夹列表...".to_string(),
        })?;

        let folders = self.sync_folders(&mut imap_service, account_id).await?;

        // 2. 判断是否首次同步
        let is_first_sync = self.is_first_sync(account_id).await?;

        // 3. 同步邮件
        let mut total_synced = 0;
        let mut total_errors = 0;

        for folder in &folders {
            self.emit_progress(account_id, SyncProgress {
                stage: SyncStage::SyncingEmails,
                folder: Some(folder.name.clone()),
                current: total_synced,
                total: 0,
                message: format!("正在同步 {}...", folder.name),
            })?;

            let result = if is_first_sync {
                self.first_sync_folder(
                    &mut imap_service,
                    account_id,
                    &folder.name,
                    &folder.imap_name,
                    1000, // 首次同步 1000 封
                ).await
            } else {
                self.incremental_sync_folder(
                    &mut imap_service,
                    account_id,
                    &folder.name,
                    &folder.imap_name,
                ).await
            };

            match result {
                Ok(count) => {
                    total_synced += count;
                    // 清除错误
                    let _ = sync_state_service::clear_errors(&self.db, account_id, &folder.name).await;
                }
                Err(e) => {
                    total_errors += 1;
                    // 记录错误
                    let _ = sync_state_service::record_error(
                        &self.db,
                        account_id,
                        &folder.name,
                        &e.to_string(),
                    ).await;
                    let _ = sync_error_service::create(
                        &self.db,
                        account_id,
                        Some(&folder.name),
                        "sync",
                        &e.to_string(),
                        None,
                        None,
                    ).await;
                }
            }
        }

        // 4. 更新账号同步时间
        let _ = account_service::update_last_sync(&self.db, account_id).await;

        let duration = start_time.elapsed().as_millis() as u64;

        // 5. 发送完成事件
        self.emit_progress(account_id, SyncProgress {
            stage: SyncStage::Completed,
            folder: None,
            current: total_synced,
            total: total_synced,
            message: format!("同步完成，共同步 {} 封邮件", total_synced),
        })?;

        Ok(SyncResult {
            total_synced,
            folders_synced: folders.len(),
            errors: total_errors,
            duration_ms: duration,
        })
    }

    /// 同步文件夹列表
    async fn sync_folders(
        &self,
        imap_service: &mut imap_service::ImapService,
        account_id: i32,
    ) -> Result<Vec<crate::models::folder::Model>> {
        // 定义要同步的标准文件夹列表（固定顺序）
        let standard_folders = &[
            ("inbox", "收件箱"),
            ("starred", "星标邮件"),
            ("drafts", "草稿箱"),
            ("sent", "已发送"),
            ("archive", "归档"),
            ("spam", "垃圾邮件"),
        ];

        // 获取服务器上的所有文件夹及其属性
        let server_folders = imap_service.list_folders_with_attributes().await?;

        tracing::info!("从服务器获取到 {} 个文件夹", server_folders.len());

        let mut synced_folders = Vec::new();

        // 为每个标准文件夹查找对应的服务器文件夹
        for (standard_name, display_name) in standard_folders {
            tracing::info!("🔍 查找标准文件夹: {} ({})", display_name, standard_name);

            // 查找第一个匹配的服务器文件夹
            let matched_folder = server_folders.iter()
                .find(|folder_info| {
                    // 方法 1: 优先使用 RFC 6154 special-use 属性
                    if let Some(special_use) = folder_info.special_use {
                        let matches = match special_use {
                            imap_service::SpecialUse::All => *standard_name == "archive",
                            imap_service::SpecialUse::Archive => *standard_name == "archive",
                            imap_service::SpecialUse::Drafts => *standard_name == "drafts",
                            imap_service::SpecialUse::Flagged => *standard_name == "starred",
                            imap_service::SpecialUse::Junk => *standard_name == "spam",
                            imap_service::SpecialUse::Sent => *standard_name == "sent",
                            imap_service::SpecialUse::Trash => *standard_name == "trash",
                        };
                        if matches {
                            tracing::debug!("  ✅ RFC 6154 属性匹配: {:?} == {}", special_use, standard_name);
                            return true;
                        }
                    }

                    // 方法 2: 回退到标准名称匹配
                    if folder_info.standard_name == *standard_name {
                        tracing::debug!("  ✅ 标准名称匹配: {} == {}", folder_info.standard_name, standard_name);
                        return true;
                    }

                    false
                });

            if let Some(folder_info) = matched_folder {
                // 找到匹配的文件夹
                let folder = folder_service::find_or_create(
                    &self.db,
                    account_id,
                    standard_name,
                    &folder_info.name,
                ).await?;

                tracing::info!("  ✅ 找到匹配: {} -> {} (special_use: {:?})",
                    display_name, folder_info.name, folder_info.special_use);
                synced_folders.push(folder);
            } else {
                tracing::warn!("  ⚠️  未找到 '{}' 对应的服务器文件夹", display_name);
            }
        }

        tracing::info!("共同步了 {} 个标准文件夹", synced_folders.len());

        Ok(synced_folders)
    }

    /// 首次同步文件夹
    async fn first_sync_folder(
        &self,
        imap_service: &mut imap_service::ImapService,
        account_id: i32,
        folder_name: &str,
        imap_folder: &str,
        limit: usize,
    ) -> Result<usize> {
        // 获取最新邮件的 UID 列表
        let uids = imap_service.list_uids(imap_folder, limit).await?;

        if uids.is_empty() {
            return Ok(0);
        }

        let highest_uid = *uids.first().unwrap_or(&0) as i32;
        let mut synced_count = 0;

        // 同步邮件
        for uid in uids {
            match self.sync_email(imap_service, account_id, folder_name, imap_folder, uid).await {
                Ok(_) => synced_count += 1,
                Err(e) => {
                    tracing::error!("同步邮件 UID {} 失败: {}", uid, e);
                    // 继续同步下一封邮件
                }
            }
        }

        // 创建或更新同步状态
        let _ = sync_state_service::upsert(
            &self.db,
            account_id,
            folder_name,
            Some(highest_uid),
            Some(highest_uid),
            synced_count as i32,
            true, // is_first_sync
        ).await;

        Ok(synced_count)
    }

    /// 增量同步文件夹
    async fn incremental_sync_folder(
        &self,
        imap_service: &mut imap_service::ImapService,
        account_id: i32,
        folder_name: &str,
        imap_folder: &str,
    ) -> Result<usize> {
        // 获取上次同步的 UID
        let sync_state = sync_state_service::get_by_account_and_folder(
            &self.db,
            account_id,
            folder_name,
        ).await?;

        let last_uid = match sync_state {
            Some(state) => state.last_sync_uid,
            None => return Ok(0), // 无同步状态，跳过
        };

        if last_uid.is_none() || last_uid == Some(0) {
            // 降级为首次同步
            return self.first_sync_folder(imap_service, account_id, folder_name, imap_folder, 100).await;
        }

        // 获取大于 last_uid 的所有邮件
        let uids = imap_service.list_uids_after(imap_folder, last_uid.unwrap()).await?;

        if uids.is_empty() {
            return Ok(0);
        }

        let default_uid = last_uid.unwrap() as u32;
        let highest_uid = uids.first().copied().unwrap_or(default_uid) as i32;
        let mut synced_count = 0;

        // 同步新邮件
        for uid in uids {
            match self.sync_email(imap_service, account_id, folder_name, imap_folder, uid).await {
                Ok(_) => synced_count += 1,
                Err(e) => {
                    tracing::error!("同步邮件 UID {} 失败: {}", uid, e);
                }
            }
        }

        // 更新同步状态
        let _ = sync_state_service::upsert(
            &self.db,
            account_id,
            folder_name,
            Some(highest_uid),
            Some(highest_uid),
            synced_count as i32,
            false, // not first sync
        ).await;

        Ok(synced_count)
    }

    /// 同步单封邮件
    async fn sync_email(
        &self,
        imap_service: &mut imap_service::ImapService,
        account_id: i32,
        folder_name: &str,
        imap_folder: &str,
        uid: u32,
    ) -> Result<()> {
        // 检查邮件是否已存在
        let exists = imap_service.email_exists_by_uid(
            &self.db,
            account_id,
            uid as i32,
            folder_name,
        ).await;

        if exists {
            return Ok(()); // 已存在，跳过
        }

        // 获取邮件
        let email_data = imap_service.fetch_email(imap_folder, uid).await?;

        // 解析并保存
        imap_service.save_email(&self.db, account_id, folder_name, uid, &email_data).await?;

        Ok(())
    }

    /// 检查是否首次同步
    async fn is_first_sync(&self, account_id: i32) -> Result<bool> {
        let states = sync_state_service::get_by_account(&self.db, account_id).await?;
        Ok(states.is_empty())
    }

    /// 获取账号密码（从 Keyring）
    async fn get_account_password(&self, account: &account::Model) -> Result<String> {
        tracing::info!(
            "开始获取账号密码: account_id={}, email={}, auth_type={}",
            account.id,
            account.email,
            account.auth_type
        );

        if account.auth_type == "oauth" {
            // OAuth 认证
            Err(anyhow!("OAuth 认证暂不支持"))
        } else {
            // 密码认证 - 从 Keyring 获取
            let username = password_username(account.id);
            let password = self
                .app_handle
                .keyring()
                .get_password(KEYRING_SERVICE, &username)
                .map_err(|e| anyhow!("获取密码失败: {}", e))?
                .ok_or_else(|| anyhow!("账号密码不存在（username={}）", username))?;

            tracing::info!(
                "成功获取账号密码: account_id={}, password_len={}",
                account.id,
                password.len()
            );

            Ok(password)
        }
    }

    /// 发送进度事件
    fn emit_progress(&self, account_id: i32, progress: SyncProgress) -> Result<()> {
        let event_name = format!("sync-progress-{}", account_id);

        let stage_str = match progress.stage {
            SyncStage::Connecting => "connecting",
            SyncStage::SyncingFolders => "syncing_folders",
            SyncStage::SyncingEmails => "syncing_emails",
            SyncStage::Completed => "completed",
            SyncStage::Error => "error",
        };

        let payload = serde_json::json!({
            "account_id": account_id,
            "stage": stage_str,
            "folder": progress.folder,
            "current": progress.current,
            "total": progress.total,
            "message": progress.message,
        });

        self.app_handle
            .emit(&event_name, payload)
            .map_err(|e| anyhow!("发送进度事件失败: {}", e))?;

        Ok(())
    }
}

// 用于测试的辅助函数
#[cfg(test)]
impl SyncManager {
    pub async fn for_test(db: Arc<DbConn>, app_handle: AppHandle) -> Self {
        // 注意：测试环境需要配置 Keyring
        // 这里需要根据实际测试框架进行调整
        todo!("配置测试环境的 Keyring 实例")
    }
}
