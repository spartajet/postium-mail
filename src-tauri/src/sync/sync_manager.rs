//! 同步管理器
//!
//! 管理邮件同步流程，协调所有同步组件

use crate::error::{MailError, Result};
use crate::sync::{delta_sync::DeltaSync, folder_manager::FolderManager, mail_processor::MailProcessor, change_detector::ChangeDetector};
use crate::auth::{AuthManager, ImapAuthInfo};
use crate::providers::{ProviderPool, AuthType};
use crate::services::imap::{AsyncImapClient, ImapAuth};
use sea_orm::DbConn;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// 同步进度信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncProgress {
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

/// 同步阶段
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}

/// 同步结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub total_synced: usize,
    pub folders_synced: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

/// 同步管理器
///
/// 负责协调所有同步组件，管理同步流程
pub struct SyncManager {
    db: Arc<DbConn>,
    app_handle: AppHandle,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    delta_sync: Arc<DeltaSync>,
    folder_manager: Arc<FolderManager>,
    mail_processor: Arc<MailProcessor>,
    change_detector: Arc<ChangeDetector>,
}

impl SyncManager {
    /// 创建新的同步管理器
    pub fn new(
        db: Arc<DbConn>,
        app_handle: AppHandle,
        auth_manager: Arc<AuthManager>,
        provider_pool: Arc<ProviderPool>,
    ) -> Self {
        let delta_sync = Arc::new(DeltaSync::new(db.clone()));
        let folder_manager = Arc::new(FolderManager::new(db.clone()));
        let mail_processor = Arc::new(MailProcessor::new(db.clone()));
        let change_detector = Arc::new(ChangeDetector::new(db.clone()));

        Self {
            db,
            app_handle,
            auth_manager,
            provider_pool,
            delta_sync,
            folder_manager,
            mail_processor,
            change_detector,
        }
    }

    /// 执行账号同步
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// 返回同步结果
    pub async fn sync_account(&self, account_id: i32) -> Result<SyncResult> {
        let start_time = std::time::Instant::now();

        tracing::info!("开始同步账号: {}", account_id);

        // 发送开始事件
        let _ = self.emit_progress(
            account_id,
            SyncProgress {
                stage: SyncStage::Connecting,
                folder: None,
                current: 0,
                total: 0,
                message: "正在连接服务器...".to_string(),
            },
        );

        // 1. 获取账号信息
        let account = crate::services::account_service::get_by_id(&self.db, account_id)
            .await
            .map_err(|e| MailError::Internal(format!("获取账号信息失败: {}", e)))?
            .ok_or_else(|| MailError::Internal(format!("账号 {} 不存在", account_id)))?;

        // 2. 检测服务商
        let provider = self
            .provider_pool
            .detect_provider(&account.email)
            .await
            .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

        // 3. 获取 IMAP 配置
        let imap_config = provider.default_imap_config();

        // 4. 连接到 IMAP 服务器
        let mut imap_client = self
            .connect_imap(account_id, &imap_config, &account.email, &account.auth_type)
            .await?;

        // 5. 发送文件夹同步开始事件
        let _ = self.emit_progress(
            account_id,
            SyncProgress {
                stage: SyncStage::SyncingFolders,
                folder: None,
                current: 0,
                total: 0,
                message: "正在同步文件夹...".to_string(),
            },
        );

        // 6. 同步文件夹列表
        let folder_infos = imap_client.list_folders_with_attributes().await.map_err(|e| {
            MailError::Internal(format!("获取文件夹列表失败: {}", e))
        })?;

        tracing::info!("获取到 {} 个文件夹", folder_infos.len());

        // 7. 使用 FolderManager 同步文件夹到数据库
        let folder_sync_result = self
            .folder_manager
            .sync_folders_from_info(account_id, &folder_infos)
            .await?;

        tracing::info!(
            "文件夹同步完成: created={}, updated={}",
            folder_sync_result.new_folders,
            folder_sync_result.updated_folders
        );

        // 8. 对每个文件夹执行增量同步
        let mut total_synced = 0;
        let mut sync_errors = 0;

        for (idx, folder_info) in folder_infos.iter().enumerate() {
            // 更新进度
            let _ = self.emit_progress(
                account_id,
                SyncProgress {
                    stage: SyncStage::SyncingEmails,
                    folder: Some(folder_info.name.clone()),
                    current: idx + 1,
                    total: folder_infos.len(),
                    message: format!("正在同步 {}...", folder_info.name),
                },
            );

            // 同步单个文件夹
            match self
                .sync_folder_internal(account_id, &folder_info.name, &mut imap_client)
                .await
            {
                Ok(result) => {
                    total_synced += result.total_changes();
                    tracing::info!(
                        "文件夹 {} 同步完成: new={}, modified={}, deleted={}",
                        folder_info.name,
                        result.new_emails,
                        result.modified_emails,
                        result.deleted_emails
                    );
                }
                Err(e) => {
                    sync_errors += 1;
                    tracing::error!("文件夹 {} 同步失败: {}", folder_info.name, e);
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            "账号同步完成: account_id={}, synced={}, errors={}, duration={}ms",
            account_id,
            total_synced,
            sync_errors,
            duration_ms
        );

        // 发送完成事件
        let _ = self.emit_progress(
            account_id,
            SyncProgress {
                stage: SyncStage::Completed,
                folder: None,
                current: total_synced,
                total: folder_infos.len(),
                message: format!("同步完成，共处理 {} 封邮件", total_synced),
            },
        );

        Ok(SyncResult {
            total_synced,
            folders_synced: folder_infos.len(),
            errors: sync_errors,
            duration_ms,
        })
    }

    /// 连接到 IMAP 服务器
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `imap_config` - IMAP 服务器配置
    /// * `email` - 邮箱地址
    /// * `auth_type_str` - 认证类型字符串（"password" 或 "oauth2"）
    ///
    /// # 返回
    ///
    /// 返回已连接的 IMAP 客户端
    async fn connect_imap(
        &self,
        account_id: i32,
        imap_config: &crate::providers::ImapServerConfig,
        email: &str,
        auth_type_str: &str,
    ) -> Result<AsyncImapClient> {
        tracing::info!(
            "连接到 IMAP 服务器: {}:{}, auth_type={}",
            imap_config.host,
            imap_config.port,
            auth_type_str
        );

        // 将字符串转换为 AuthType
        let auth_type = match auth_type_str {
            "oauth2" => AuthType::OAuth2,
            "password" => AuthType::Password,
            _ => return Err(MailError::Internal(format!("不支持的认证类型: {}", auth_type_str))),
        };

        // 使用 AuthManager 获取认证信息
        let auth_info = self
            .auth_manager
            .get_imap_auth(account_id, email, &auth_type)
            .await?;

        // 将 ImapAuthInfo 转换为 ImapAuth
        let imap_auth = match auth_info {
            ImapAuthInfo::Password { username, password } => {
                ImapAuth::Password(password)
            }
            ImapAuthInfo::OAuth { email: oauth_email, xoauth2 } => {
                ImapAuth::OAuth2 {
                    email: oauth_email,
                    access_token: xoauth2,
                }
            }
        };

        // 创建 IMAP 客户端并连接
        let mut imap_client = AsyncImapClient::new();
        imap_client
            .connect(
                &imap_config.host,
                imap_config.port,
                email,
                imap_auth,
            )
            .await
            .map_err(|e| MailError::Internal(format!("IMAP 连接失败: {}", e)))?;

        tracing::info!("IMAP 连接成功");

        Ok(imap_client)
    }

    /// 内部方法：同步单个文件夹
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `imap_client` - IMAP 客户端引用
    async fn sync_folder_internal(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut AsyncImapClient,
    ) -> Result<crate::sync::delta_sync::DeltaSyncResult> {
        tracing::debug!(
            "开始同步文件夹: account_id={}, folder={}",
            account_id,
            folder
        );

        // 1. 检查 CONDSTORE 支持
        let supports_condstore = imap_client
            .check_condstore_support()
            .await
            .unwrap_or(false);

        tracing::info!(
            "文件夹 {} CONDSTORE 支持: {}",
            folder,
            supports_condstore
        );

        // 2. 获取服务器所有 UID 列表
        let server_uids = imap_client.list_uids(folder, 10000).await.map_err(|e| {
            MailError::Internal(format!("获取服务器 UID 列表失败: {}", e))
        })?;

        tracing::info!(
            "文件夹 {} 服务器邮件数: {}",
            folder,
            server_uids.len()
        );

        // 3. 获取服务器标志（如果支持 CONDSTORE，尝试使用 MODSEQ）
        let server_uids_with_flags: Vec<(u32, Vec<String>)> = if supports_condstore && !server_uids.is_empty() {
            // 尝试使用 CONDSTORE 获取 MODSEQ
            let modseq_result = imap_client.fetch_modseqs(&server_uids).await;

            match modseq_result {
                Ok(uids_modseq) => {
                    tracing::info!("使用 MODSEQ 获取标志: {} 个邮件", uids_modseq.len());
                    uids_modseq
                        .into_iter()
                        .map(|(uid, modseq)| {
                            // 转换为标志列表（简化版本）
                            let flags = if modseq.is_some() {
                                vec!["$MODSEQ".to_string()] // 占位符，表示有 MODSEQ
                            } else {
                                vec![]
                            };
                            (uid, flags)
                        })
                        .collect()
                }
                Err(_) => {
                    tracing::warn!("CONDSTORE FETCH 失败，降级到无标志模式");
                    server_uids.iter().map(|&uid| (uid, vec![])).collect()
                }
            }
        } else {
            // 不支持 CONDSTORE，使用普通模式
            server_uids.iter().map(|&uid| (uid, vec![])).collect()
        };

        // 4. 调用 sync_folder 进行增量同步
        let result = self
            .sync_folder(
                account_id,
                folder,
                Some(&server_uids),
                Some(&server_uids_with_flags),
            )
            .await?;

        Ok(result)
    }

    /// 同步单个文件夹
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids` - 服务器 UID 列表（可选）
    /// * `server_uids_with_flags` - 服务器 UID 和标志列表（可选）
    pub async fn sync_folder(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: Option<&[u32]>,
        server_uids_with_flags: Option<&[(u32, Vec<String>)]>,
    ) -> Result<crate::sync::delta_sync::DeltaSyncResult> {
        tracing::info!(
            "开始同步文件夹: account_id={}, folder={}, server_uids={}",
            account_id,
            folder,
            server_uids.map(|u| u.len()).unwrap_or(0)
        );

        // 如果没有提供服务器数据，返回空结果
        let (server_uids, server_uids_with_flags) = match (server_uids, server_uids_with_flags) {
            (Some(uids), Some(flags)) => (uids, flags),
            _ => {
                tracing::warn!("缺少服务器数据，跳过同步");
                return Ok(crate::sync::delta_sync::DeltaSyncResult::empty(
                    crate::sync::delta_sync::SyncStrategy::UidSearch,
                ));
            }
        };

        // 1. 使用 ChangeDetector 检测变更
        // 注意：这里暂时使用 None 作为 last_sync_uid，ChangeDetector 会自动推断
        let change_detection_result = self
            .change_detector
            .detect_changes(
                account_id,
                folder,
                server_uids,
                None, // last_sync_uid - 从本地状态推断
                false, // supports_condstore - 暂时使用 false
            )
            .await
            .map_err(|e| {
                MailError::Internal(format!("变更检测失败: {}", e))
            })?;

        tracing::info!(
            "变更检测结果: new={}, modified={}, deleted={}",
            change_detection_result.new_emails.len(),
            change_detection_result.modified_emails.len(),
            change_detection_result.deleted_emails.len()
        );

        // 如果没有变更，直接返回
        if !change_detection_result.has_changes() {
            tracing::info!("无变更，跳过同步");
            return Ok(crate::sync::delta_sync::DeltaSyncResult::empty(
                crate::sync::delta_sync::SyncStrategy::UidSearch,
            ));
        }

        // 2. 使用 MailProcessor 处理新邮件
        let new_emails_count = change_detection_result.new_emails.len();
        let modified_emails_count = change_detection_result.modified_emails.len();

        // TODO: 实际获取和处理邮件内容
        // let new_uids: Vec<u32> = change_detection_result.new_emails.iter().map(|&uid| uid).collect();
        // for uid in new_uids {
        //     // 使用 AsyncImapClient 获取邮件
        //     let email_data = imap_client.fetch_email(folder, uid).await?;
        //     // 使用 MailProcessor 保存到数据库
        //     self.mail_processor.save_mail_to_db(account_id, folder, &email_data).await?;
        // }

        // 3. 更新已删除的邮件
        if !change_detection_result.deleted_emails.is_empty() {
            self.mail_processor
                .delete_mails(account_id, folder, &change_detection_result.deleted_emails)
                .await
                .map_err(|e| MailError::Internal(format!("删除邮件失败: {}", e)))?;

            tracing::info!("已删除 {} 个邮件", change_detection_result.deleted_emails.len());
        }

        // 4. 返回同步结果
        Ok(crate::sync::delta_sync::DeltaSyncResult {
            strategy_used: crate::sync::delta_sync::SyncStrategy::UidSearch,
            new_emails: new_emails_count,
            modified_emails: modified_emails_count,
            deleted_emails: change_detection_result.deleted_emails.len(),
            flags_changed: 0,
            duration_ms: 0, // TODO: 添加实际耗时统计
        })
    }

    /// 停止同步
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    pub async fn stop_sync(&self, account_id: i32) -> Result<()> {
        tracing::info!("停止同步账号: {}", account_id);
        // TODO: 实现停止同步逻辑
        Ok(())
    }

    /// 发送同步进度事件
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `progress` - 进度信息
    fn emit_progress(&self, account_id: i32, progress: SyncProgress) -> Result<()> {
        // 使用时间戳代替 UUID 生成唯一事件 ID
        let event_id = chrono::Utc::now().timestamp_millis();

        self.app_handle
            .emit(
                &format!("sync://progress/{}/{}", account_id, event_id),
                progress,
            )
            .map_err(|e| MailError::Internal(format!("发送进度事件失败: {}", e)))?;
        Ok(())
    }

    /// 获取 DeltaSync 引用
    pub fn delta_sync(&self) -> &Arc<DeltaSync> {
        &self.delta_sync
    }

    /// 获取 FolderManager 引用
    pub fn folder_manager(&self) -> &Arc<FolderManager> {
        &self.folder_manager
    }

    /// 获取 MailProcessor 引用
    pub fn mail_processor(&self) -> &Arc<MailProcessor> {
        &self.mail_processor
    }

    /// 获取 ChangeDetector 引用
    pub fn change_detector(&self) -> &Arc<ChangeDetector> {
        &self.change_detector
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_stage_equality() {
        assert_eq!(SyncStage::Connecting, SyncStage::Connecting);
        assert_ne!(SyncStage::Connecting, SyncStage::Completed);
    }

    #[test]
    fn test_sync_progress_creation() {
        let progress = SyncProgress {
            stage: SyncStage::SyncingEmails,
            folder: Some("INBOX".to_string()),
            current: 10,
            total: 100,
            message: "同步中...".to_string(),
        };

        assert_eq!(progress.stage, SyncStage::SyncingEmails);
        assert_eq!(progress.current, 10);
        assert_eq!(progress.total, 100);
    }

    #[test]
    fn test_sync_result_default() {
        let result = SyncResult {
            total_synced: 0,
            folders_synced: 0,
            errors: 0,
            duration_ms: 0,
        };

        assert_eq!(result.total_synced, 0);
        assert_eq!(result.folders_synced, 0);
        assert_eq!(result.errors, 0);
    }
}
