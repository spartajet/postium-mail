//! 同步管理器
//!
//! 管理邮件同步流程，协调所有同步组件

use crate::auth::{AuthManager, ImapAuthInfo};
use crate::error::{MailError, Result};
use crate::protocols::imap::{AsyncImapClient, ImapAuth, three_months_ago_imap_format};
use crate::providers::{AuthType, ProviderPool};
use crate::storage;
use crate::sync::SyncStrategy;
use crate::sync::{
    change_detector::ChangeDetector, delta_sync::DeltaSync, folder_manager::FolderManager,
    mail_processor::MailProcessor,
};
use chrono::Datelike; // 添加 Datelike trait来访问日期方法
use sea_orm::DbConn;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::instrument;

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
    #[instrument(skip(self), fields(account_id))]
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
        let account = storage::AccountRepository::get_by_id(&self.db, account_id)
            .await
            .map_err(|e| MailError::Internal(format!("获取账号信息失败: {}", e)))?
            .ok_or_else(|| MailError::Internal(format!("账号 {} 不存在", account_id)))?;

        tracing::debug!(
            "账号信息: email={}, auth_type={}, provider={}",
            account.email,
            account.auth_type,
            account.provider
        );

        // 2. 获取服务商（从账号的 provider 字段）
        let provider = self
            .provider_pool
            .find_provider_by_id(&account.provider)
            .ok_or_else(|| MailError::Internal(format!("未找到服务商: {}", account.provider)))?;

        // 缓存 provider_info 以避免多次调用
        let provider_info = provider.provider_info();

        tracing::info!(
            "使用账号服务商: {} (provider_id={})",
            provider_info.name,
            account.provider
        );

        // 2.1 获取服务商文件夹映射（用于识别文件夹类型）
        let folder_mapping = provider.folder_mapping();

        tracing::debug!(
            "文件夹映射: inbox={:?}, sent={:?}, drafts={:?}, spam={:?}, trash={:?}, archive={:?}",
            folder_mapping.inbox,
            folder_mapping.sent,
            folder_mapping.drafts,
            folder_mapping.spam,
            folder_mapping.trash,
            folder_mapping.archive
        );

        // 3. 获取 IMAP 配置
        let imap_config = provider.imap_config(&account.email);

        tracing::debug!(
            "IMAP 配置: host={}, port={}, ssl={:?}",
            imap_config.host,
            imap_config.port,
            imap_config.ssl
        );

        // 4. 连接到 IMAP 服务器
        let mut imap_client = self
            .connect_imap(account_id, &imap_config, &account.email, &account.auth_type)
            .await?;

        tracing::info!("IMAP 连接成功");

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

        // 6. 从 provider 获取需要同步的文件夹列表
        let mut folders_to_sync: Vec<String> = Vec::new();
        folders_to_sync.extend(folder_mapping.inbox.clone());
        folders_to_sync.extend(folder_mapping.sent.clone());
        folders_to_sync.extend(folder_mapping.drafts.clone());
        folders_to_sync.extend(folder_mapping.spam.clone());
        folders_to_sync.extend(folder_mapping.trash.clone());
        folders_to_sync.extend(folder_mapping.archive.clone());

        // 去重（有些服务商可能用相同的名字表示不同类型）
        folders_to_sync.sort();
        folders_to_sync.dedup();

        tracing::info!(
            "从 provider 获取到 {} 个标准文件夹: {:?}",
            folders_to_sync.len(),
            folders_to_sync
        );

        // 7. 对每个标准文件夹执行同步
        let mut total_synced = 0;
        let mut sync_errors = 0;

        tracing::info!("开始同步 {} 个标准文件夹", folders_to_sync.len());

        for (idx, folder_name) in folders_to_sync.iter().enumerate() {
            // 更新进度
            let _ = self.emit_progress(
                account_id,
                SyncProgress {
                    stage: SyncStage::SyncingEmails,
                    folder: Some(folder_name.clone()),
                    current: idx + 1,
                    total: folders_to_sync.len(),
                    message: format!("正在同步 {}...", folder_name),
                },
            );

            tracing::debug!(
                "开始同步标准文件夹 [{}/{}]: {}",
                idx + 1,
                folders_to_sync.len(),
                folder_name
            );

            // 同步单个文件夹
            match self
                .sync_folder_internal(account_id, folder_name, &mut imap_client)
                .await
            {
                Ok(result) => {
                    total_synced += result.total_changes();
                    tracing::info!(
                        "文件夹 {} 同步完成: new={}, modified={}, deleted={}",
                        folder_name,
                        result.new_emails,
                        result.modified_emails,
                        result.deleted_emails
                    );
                }
                Err(e) => {
                    // 如果文件夹不存在，记录警告但不计入错误
                    let error_msg = e.to_string().to_lowercase();
                    if error_msg.contains("mailbox")
                        || error_msg.contains("not found")
                        || error_msg.contains("nonexistent")
                        || error_msg.contains("不存在")
                    {
                        tracing::warn!("文件夹不存在，跳过: {}", folder_name);
                    } else {
                        sync_errors += 1;
                        tracing::error!("文件夹 {} 同步失败: {}", folder_name, e);
                    }
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
                total: folders_to_sync.len(),
                message: format!("同步完成，共处理 {} 封邮件", total_synced),
            },
        );

        Ok(SyncResult {
            total_synced,
            folders_synced: folders_to_sync.len(),
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
            _ => {
                return Err(MailError::Internal(format!(
                    "不支持的认证类型: {}",
                    auth_type_str
                )));
            }
        };

        // 使用 AuthManager 获取认证信息
        let auth_info = self
            .auth_manager
            .get_imap_auth(account_id, email, &auth_type)
            .await?;

        // 将 ImapAuthInfo 转换为 ImapAuth
        let imap_auth = match auth_info {
            ImapAuthInfo::Password { username, password } => ImapAuth::Password(password),
            ImapAuthInfo::OAuth {
                email: oauth_email,
                access_token: xoauth2,
            } => ImapAuth::OAuth2 {
                email: oauth_email,
                access_token: xoauth2,
            },
        };

        // 创建 IMAP 客户端并连接
        let mut imap_client = AsyncImapClient::new();
        imap_client
            .connect(&imap_config.host, imap_config.port, email, imap_auth)
            .await
            .map_err(|e| MailError::Internal(format!("IMAP 连接失败: {}", e)))?;

        tracing::info!("IMAP 连接成功");

        Ok(imap_client)
    }

    /// 内部方法：同步单个文件夹
    ///
    /// 全量同步文件夹
    ///
    /// 当 UIDVALIDITY 变化或首次同步时调用。获取所有邮件进行完整同步。
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `imap_client` - IMAP 客户端引用
    /// * `metadata` - 文件夹元数据（已在调用方获取）
    ///
    /// # 返回
    ///
    /// 返回同步结果
    async fn sync_folder_full(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut AsyncImapClient,
        metadata: &crate::protocols::imap::FolderMetadata,
    ) -> Result<crate::sync::delta_sync::DeltaSyncResult> {
        tracing::info!(
            "全量同步文件夹: account_id={}, folder={}, uidvalidity={}",
            account_id,
            folder,
            metadata.uidvalidity
        );

        // 1. 更新文件夹元数据（全量同步特有）
        self.folder_manager
            .update_folder_metadata(
                account_id,
                folder,
                Some(metadata.uidvalidity),
                Some(metadata.uidnext),
            )
            .await
            .map_err(|e| {
                tracing::warn!("更新文件夹元数据失败: {}", e);
                e
            })?;

        tracing::debug!(
            "文件夹元数据已更新: uidvalidity={}, uidnext={}",
            metadata.uidvalidity,
            metadata.uidnext
        );

        // 2. 获取近三个月的邮件 UID
        let date_since = three_months_ago_imap_format();
        tracing::info!(
            "全量同步，获取三个月内的邮件: folder={}, date_since={}",
            folder,
            date_since
        );
        let server_uids = imap_client
            .list_uids_since(folder, &date_since)
            .await
            .map_err(|e| MailError::Internal(format!("获取服务器 UID 列表失败: {}", e)))?;

        tracing::info!("文件夹 {} 需要同步的邮件数: {}", folder, server_uids.len());

        // 3. 调用 sync_folder 进行实际同步
        let result = self
            .sync_folder(
                account_id,
                folder,
                Some(&server_uids),
                None,
                Some(imap_client),
            )
            .await?;

        Ok(result)
    }

    /// 增量同步文件夹
    ///
    /// 在 UIDVALIDITY 未变化时调用，仅同步新增和修改的邮件。
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `imap_client` - IMAP 客户端引用
    /// * `capabilities` - 服务器能力信息（保留用于扩展）
    /// * `metadata` - 文件夹元数据（已在调用方获取）
    ///
    /// # 返回
    ///
    /// 返回同步结果
    async fn sync_folder_incremental(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut AsyncImapClient,
        metadata: &crate::protocols::imap::FolderMetadata,
    ) -> Result<crate::sync::delta_sync::DeltaSyncResult> {
        tracing::info!(
            "增量同步文件夹: account_id={}, folder={}",
            account_id,
            folder
        );

        // 1. 获取本地 last_sync_uid
        let last_sync_uid = self
            .folder_manager
            .get_sync_state(account_id, folder)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.last_sync_uid)
            .map(|v| v as u32)
            .unwrap_or(0);

        tracing::info!(
            "增量同步，获取 last UID > {} 的邮件: folder={}",
            last_sync_uid,
            folder
        );

        // 2. 获取新增邮件 UID 列表
        let server_uids = imap_client
            .list_uids_after(folder, last_sync_uid)
            .await
            .map_err(|e| MailError::Internal(format!("获取服务器 UID 列表失败: {}", e)))?;

        tracing::info!("文件夹 {} 需要同步的邮件数: {}", folder, server_uids.len());

        if server_uids.is_empty() {
            tracing::info!("文件夹 {} 没有新邮件，跳过同步", folder);
            return Ok(crate::sync::delta_sync::DeltaSyncResult {
                strategy_used: SyncStrategy::UidSearch,
                new_emails: 0,
                modified_emails: 0,
                deleted_emails: 0,
                flags_changed: 0,
                duration_ms: 0,
            });
        }

        // 3. 调用 sync_folder 进行实际同步
        let result = self
            .sync_folder(
                account_id,
                folder,
                Some(&server_uids),
                None,
                Some(imap_client),
            )
            .await?;

        Ok(result)
    }

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

        // 1. 获取文件夹 IMAP 元数据（uidvalidity, uidnext）
        let metadata = imap_client
            .fetch_folder_metadata(folder)
            .await
            .map_err(|e| {
                tracing::warn!("获取文件夹元数据失败: {}, 跳过元数据更新", e);
                // 元数据获取失败不应阻断同步流程
                crate::error::MailError::Internal(format!("获取文件夹元数据失败: {}", e))
            });
        tracing::info!("获取文件夹元数据成功: {:?}", metadata);

        // 2. 检测 UIDVALIDITY 变化，决定同步策略
        let mut needs_full_resync = false;
        if let Ok(meta) = &metadata {
            let uidvalidity_changed = self
                .folder_manager
                .check_uidvalidity_changed(account_id, folder, meta.uidvalidity)
                .await
                .unwrap_or(false);
            tracing::info!(
                "UIDVALIDITY 变化: folder={}, uidvalidity_changed={}",
                folder,
                uidvalidity_changed
            );

            if uidvalidity_changed {
                tracing::warn!(
                    "检测到 UIDVALIDITY 变化: folder={}, server_uidvalidity={}",
                    folder,
                    meta.uidvalidity
                );

                // 重置本地同步状态
                self.folder_manager
                    .reset_sync_state(account_id, folder)
                    .await
                    .map_err(|e| {
                        tracing::error!("重置同步状态失败: {}", e);
                        e
                    })?;

                needs_full_resync = true;
                tracing::info!("已重置文件夹同步状态，将进行完整同步: folder={}", folder);
            }
        }

        // 3. 根据同步策略调用相应方法
        if let Ok(meta) = metadata {
            if needs_full_resync {
                self.sync_folder_full(account_id, folder, imap_client, &meta)
                    .await
            } else {
                self.sync_folder_incremental(account_id, folder, imap_client, &meta)
                    .await
            }
        } else {
            // 元数据获取失败，返回空结果
            Ok(crate::sync::delta_sync::DeltaSyncResult::empty(
                crate::sync::delta_sync::SyncStrategy::UidSearch,
            ))
        }
    }

    /// 同步单个文件夹
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids` - 服务器 UID 列表（可选）
    /// * `server_uids_with_flags` - 服务器 UID 和标志列表（可选）
    /// * `imap_client` - IMAP 客户端引用（可选，用于获取邮件内容）
    #[allow(clippy::too_many_arguments)]
    pub async fn sync_folder(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: Option<&[u32]>,
        server_uids_with_flags: Option<&[(u32, Vec<String>)]>,
        imap_client: Option<&mut AsyncImapClient>,
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
            )
            .await
            .map_err(|e| MailError::Internal(format!("变更检测失败: {}", e)))?;

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

        // 2. 处理新邮件和修改的邮件
        let new_emails_count = change_detection_result.new_emails.len();
        let modified_emails_count = change_detection_result.modified_emails.len();

        // 如果提供了 IMAP 客户端，获取邮件内容
        if let Some(client) = imap_client {
            // 合并新邮件和修改的邮件 UID
            let all_uids: Vec<u32> = change_detection_result
                .new_emails
                .iter()
                .chain(change_detection_result.modified_emails.iter())
                .copied()
                .collect();

            tracing::info!("获取 {} 个邮件的内容", all_uids.len());

            // 批量获取邮件
            let mut mail_data_list = Vec::new();
            for uid in all_uids {
                match client.fetch_email(folder, uid).await {
                    Ok(email_data) => {
                        // 转换 EmailData → MailData
                        let mail_data =
                            crate::sync::mail_processor::from_imap_email(&email_data, folder);
                        mail_data_list.push(mail_data);
                    }
                    Err(e) => {
                        tracing::error!("获取邮件失败: uid={}, error={}", uid, e);
                        // 继续处理下一个邮件
                    }
                }
            }

            // 使用 MailProcessor 批量处理邮件
            if !mail_data_list.is_empty() {
                let process_result = self
                    .mail_processor
                    .process_mails(account_id, folder, mail_data_list)
                    .await
                    .map_err(|e| MailError::Internal(format!("处理邮件失败: {}", e)))?;

                tracing::info!(
                    "邮件处理完成: success={}, failed={}, skipped={}",
                    process_result.success_count,
                    process_result.failed_count,
                    process_result.skipped_count
                );
            }
        } else {
            tracing::warn!("未提供 IMAP 客户端，跳过邮件内容获取");
        }

        // 3. 更新已删除的邮件
        if !change_detection_result.deleted_emails.is_empty() {
            self.mail_processor
                .delete_mails(account_id, folder, &change_detection_result.deleted_emails)
                .await
                .map_err(|e| MailError::Internal(format!("删除邮件失败: {}", e)))?;

            tracing::info!(
                "已删除 {} 个邮件",
                change_detection_result.deleted_emails.len()
            );
        }

        // 4. 更新同步状态
        let total_synced = new_emails_count + modified_emails_count;
        if total_synced > 0 {
            // 更新同步完成状态
            if let Err(e) = self
                .folder_manager
                .update_sync_completed(account_id, folder, total_synced as i32)
                .await
            {
                tracing::warn!("更新同步状态失败: {}", e);
            }
        }

        // 5. 更新 last_sync_uid（使用服务器 UID 中的最大值）
        if let Some(&max_uid) = server_uids.iter().max()
            && let Err(e) = self
                .folder_manager
                .update_last_sync_uid(account_id, folder, max_uid as i32)
                .await
        {
            tracing::warn!("更新 last_sync_uid 失败: {}", e);
        }

        // 6. 返回同步结果
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

/// 格式化日期为 IMAP SINCE 命令所需的格式
///
/// IMAP SINCE 命令需要英文月份缩写（RFC 3501）：
/// 格式：dd-MMM-yyyy（如 20-Dec-2025）
///
/// 注意：不能使用 chrono 的 %b 格式化，因为它会根据系统语言环境
/// 生成不同的月份名称（如中文系统会生成 "12月"）
fn format_imap_date(datetime: chrono::DateTime<chrono::Utc>) -> String {
    const MONTH_NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let day = datetime.day();
    let month = MONTH_NAMES[datetime.month() as usize - 1];
    let year = datetime.year();

    format!("{:02}-{}-{}", day, month, year)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    #[test]
    fn test_sync_stage_equality() {
        assert_eq!(SyncStage::Connecting, SyncStage::Connecting);
        assert_ne!(SyncStage::Connecting, SyncStage::Completed);
    }

    #[test]
    fn test_sync_progress_creation() {
        init_tracing();
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
