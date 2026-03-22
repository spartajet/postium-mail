//! 同步管理器
//!
//! 管理邮件同步流程，协调所有同步组件

use crate::error::{MailError, Result};
use crate::storage;
use chrono::Datelike; // 添加 Datelike trait来访问日期方法
use crate::sync::{delta_sync::DeltaSync, folder_manager::FolderManager, mail_processor::MailProcessor, change_detector::ChangeDetector, sync_state::SyncStateManager};
use crate::auth::{AuthManager, ImapAuthInfo};
use crate::providers::{ProviderPool, AuthType};
use crate::protocols::imap::{AsyncImapClient, ImapAuth};
use sea_orm::DbConn;
use serde::{Serialize, Deserialize};
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
    sync_state_manager: Arc<SyncStateManager>,
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
        let sync_state_manager = Arc::new(SyncStateManager::new(db.clone()));

        Self {
            db,
            app_handle,
            auth_manager,
            provider_pool,
            delta_sync,
            folder_manager,
            mail_processor,
            change_detector,
            sync_state_manager,
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

        // 2. 检测服务商
        let provider = self
            .provider_pool
            .detect_provider(&account.email)
            .await
            .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

        // 3. 获取 IMAP 配置
        let imap_config = provider.imap_config(&account.email);

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

        // 7. 更新文件夹同步状态（不再同步文件夹，只更新状态）
        let sync_state_result = self
            .folder_manager
            .update_sync_states(account_id, &folder_infos)
            .await?;

        tracing::info!(
            "文件夹同步状态更新完成: updated={}",
            sync_state_result.updated
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

        // 1. 获取文件夹 IMAP 元数据（uidvalidity, uidnext, highest_modseq）
        let metadata = imap_client.fetch_folder_metadata(folder).await.map_err(|e| {
            tracing::warn!("获取文件夹元数据失败: {}, 跳过元数据更新", e);
            // 元数据获取失败不应阻断同步流程
            crate::error::MailError::Internal(format!("获取文件夹元数据失败: {}", e))
        });

        // 2. 检测 UIDVALIDITY 变化
        let mut needs_full_resync = false;
        if let Ok(ref meta) = &metadata {
            let uidvalidity_changed = self.folder_manager
                .check_uidvalidity_changed(account_id, folder, meta.uidvalidity)
                .await
                .unwrap_or(false);

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

        // 3. 更新文件夹元数据
        if let Ok(meta) = metadata {
            self.folder_manager
                .update_folder_metadata(
                    account_id,
                    folder,
                    Some(meta.uidvalidity),
                    Some(meta.uidnext),
                    meta.highest_modseq,
                )
                .await
                .map_err(|e| {
                    tracing::warn!("更新文件夹元数据失败: {}", e);
                    // 元数据更新失败不应阻断同步流程
                    e
                })?;

            tracing::debug!(
                "文件夹元数据已更新: uidvalidity={}, uidnext={}, highest_modseq={:?}",
                meta.uidvalidity,
                meta.uidnext,
                meta.highest_modseq
            );
        }

        // 2. 检查 CONDSTORE 支持
        let supports_condstore = imap_client
            .check_condstore_support()
            .await
            .unwrap_or(false);

        tracing::info!(
            "文件夹 {} CONDSTORE 支持: {}",
            folder,
            supports_condstore
        );

        // 3. 获取服务器 UID 列表（默认只同步最近3个月的邮件）
        // 计算3个月前的日期
        // 注意：IMAP SINCE 命令需要英文月份缩写（RFC 3501）
        // 格式：dd-MMM-yyyy（如 20-Dec-2025）
        let three_months_ago = chrono::Utc::now() - chrono::Duration::days(90);
        let date_since = format_imap_date(three_months_ago);

        tracing::info!(
            "使用 SINCE 命令获取最近3个月的邮件: folder={}, since={}",
            folder,
            date_since
        );

        let server_uids = imap_client.list_uids_since(folder, &date_since).await.map_err(|e| {
            MailError::Internal(format!("获取服务器 UID 列表失败: {}", e))
        })?;

        tracing::info!(
            "文件夹 {} 最近3个月的邮件数: {}",
            folder,
            server_uids.len()
        );

        // 4. CONDSTORE 增量同步（如果支持）
        let (condstore_modified_uids, highest_modseq) = if supports_condstore && !server_uids.is_empty() {
            // TODO: 从 SyncState 获取上一次同步的 highest_modseq
            // 临时实现：使用 0 表示获取所有变更
            let last_modseq = 0u64;

            // 尝试使用 SEARCH MODSEQ 获取变更的邮件
            tracing::info!("尝试 SEARCH MODSEQ {}:*", last_modseq);

            match imap_client.search_modified_since(last_modseq).await {
                Ok(modified_uids) => {
                    tracing::info!("SEARCH MODSEQ 返回 {} 个变更邮件", modified_uids.len());

                    // 获取当前的 HIGHESTMODSEQ（用于下次同步）
                    // 注意：这需要重新 SELECT 文件夹才能获取
                    let highest_modseq = None; // TODO: 从 SELECT 响应中获取

                    (Some(modified_uids), highest_modseq)
                }
                Err(e) => {
                    tracing::warn!("SEARCH MODSEQ 失败: {}, 降级到 UID 搜索", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // 5. 获取服务器标志（用于 UID 搜索降级）
        let server_uids_with_flags: Vec<(u32, Vec<String>)> = if condstore_modified_uids.is_none() && !server_uids.is_empty() {
            // 不支持 CONDSTORE 或 CONDSTORE 失败，使用 UID 搜索模式
            if let Ok(uids_modseq) = imap_client.fetch_modseqs(&server_uids).await {
                uids_modseq
                    .into_iter()
                    .map(|(uid, modseq)| {
                        let flags = if modseq.is_some() {
                            vec!["$MODSEQ".to_string()]
                        } else {
                            vec![]
                        };
                        (uid, flags)
                    })
                    .collect()
            } else {
                server_uids.iter().map(|&uid| (uid, vec![])).collect()
            }
        } else {
            // CONDSTORE 模式，不需要获取 flags
            vec![]
        };

        // 6. 调用 sync_folder 进行增量同步（传递 IMAP 客户端和 CONDSTORE 数据）
        let result = self
            .sync_folder(
                account_id,
                folder,
                Some(&server_uids),
                Some(&server_uids_with_flags),
                Some(imap_client),
                condstore_modified_uids.as_deref(),
                highest_modseq,
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
    /// * `imap_client` - IMAP 客户端引用（可选，用于获取邮件内容）
    /// * `condstore_modified_uids` - CONDSTORE SEARCH MODSEQ 返回的修改 UID（可选）
    /// * `highest_modseq` - 服务器当前的 HIGHESTMODSEQ（可选）
    #[allow(clippy::too_many_arguments)]
    pub async fn sync_folder(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: Option<&[u32]>,
        server_uids_with_flags: Option<&[(u32, Vec<String>)]>,
        imap_client: Option<&mut AsyncImapClient>,
        condstore_modified_uids: Option<&[u32]>,
        highest_modseq: Option<u64>,
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

        // 2. 处理新邮件和修改的邮件
        let new_emails_count = change_detection_result.new_emails.len();
        let modified_emails_count = change_detection_result.modified_emails.len();

        // 如果提供了 IMAP 客户端，获取邮件内容
        if let Some(client) = imap_client {
            // 合并新邮件和修改的邮件 UID
            let all_uids: Vec<u32> = change_detection_result.new_emails
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
                        let mail_data = crate::sync::mail_processor::from_imap_email(&email_data, folder);
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
                let process_result = self.mail_processor
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

            tracing::info!("已删除 {} 个邮件", change_detection_result.deleted_emails.len());
        }

        // 4. 更新同步状态
        let total_synced = new_emails_count + modified_emails_count;
        if total_synced > 0 {
            // 更新同步完成状态
            if let Err(e) = self
                .sync_state_manager
                .update_sync_completed(account_id, folder, total_synced as i32)
                .await
            {
                tracing::warn!("更新同步状态失败: {}", e);
            }
        }

        // 5. 返回同步结果
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
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"
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
