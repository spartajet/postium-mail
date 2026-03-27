//! 同步管理器
//!
//! 管理邮件同步流程，协调所有同步组件

use crate::auth::{AuthManager, ImapAuthInfo};
use crate::error::{MailError, Result, SyncError};
use crate::protocols::imap::{AsyncImapClient, ImapAuth};
use crate::providers::{AuthType, ProviderPool};
use crate::storage;
use crate::storage::service::aync_state::{save_or_update_sync_state, update_last_sync_uid};
use crate::sync::folder::dispatcher::determine_sync_mode;
use crate::sync::strategy::full_sync::sync_folder_full;
use crate::sync::strategy::incremental_sync::sync_folder_increamental;
use crate::sync::structs::SyncResult;
use crate::sync::{SyncProgress, SyncStage, structs::SyncStrategy};
use crate::sync::{change::ChangeDetector, mail_processor::MailProcessor};
use sea_orm::DbConn;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::instrument;

/// 同步管理器
///
/// 负责协调所有同步组件，管理同步流程
pub struct SyncManager {
    db: Arc<DbConn>,
    app_handle: AppHandle,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
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
        let mail_processor = Arc::new(MailProcessor::new(db.clone()));
        let change_detector = Arc::new(ChangeDetector::new(db.clone()));

        // 初始化全量同步和增量同步引擎

        Self {
            db,
            app_handle,
            auth_manager,
            provider_pool,
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
                account_id,
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
                account_id,
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
        folders_to_sync.dedup();

        tracing::info!(
            "从 provider 获取到 {} 个标准文件夹: {:?}",
            folders_to_sync.len(),
            folders_to_sync
        );

        // 7. 对每个标准文件夹执行同步
        // let mut total_synced = 0;
        // let mut sync_errors = 0;

        tracing::info!("开始同步 {} 个标准文件夹", folders_to_sync.len());
        let mut sync_results = Vec::new();

        for (idx, folder_name) in folders_to_sync.iter().enumerate() {
            // 更新进度
            let _ = self.emit_progress(
                account_id,
                SyncProgress {
                    account_id,
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

            let sync_mode =
                determine_sync_mode(self.db.as_ref(), account_id, folder_name, &mut imap_client)
                    .await
                    .map_err(|e| MailError::Sync(SyncError::QuotaExceeded))?;

            let sync_result = match sync_mode {
                crate::sync::structs::SyncMode::Full {
                    uidvalidity,
                    folder_nick_name,
                } => {
                    let sync_result = sync_folder_full(
                        self.db.as_ref(),
                        account_id,
                        folder_name,
                        &mut imap_client,
                    )
                    .await?;
                    tracing::info!(
                        "全量同步文件夹完成: folder={}, new_emails={}, last_sync_uid={}",
                        folder_name,
                        sync_result.new_emails,
                        sync_result.last_sync_uid
                    );
                    let state = save_or_update_sync_state(
                        self.db.as_ref(),
                        account_id,
                        folder_name,
                        uidvalidity,
                        sync_result.last_sync_uid,
                        folder_nick_name,
                    )
                    .await?;
                    tracing::info!(
                        "全量同步文件夹完成: folder={}, new_emails={}, last_sync_uid={}",
                        folder_name,
                        sync_result.new_emails,
                        sync_result.last_sync_uid
                    );
                    sync_result
                }
                crate::sync::structs::SyncMode::Incremental {
                    last_sync_uid,
                    folder_nick_name,
                } => {
                    let sync_result = sync_folder_increamental(
                        self.db.as_ref(),
                        account_id,
                        folder_name,
                        last_sync_uid,
                        &mut imap_client,
                    )
                    .await?;
                    update_last_sync_uid(
                        self.db.as_ref(),
                        account_id,
                        folder_name,
                        sync_result.last_sync_uid,
                    )
                    .await?;
                    tracing::info!(
                        "增量同步文件夹完成: folder={}, new_emails={}, modified_emails={}, deleted_emails={}",
                        folder_name,
                        sync_result.new_emails,
                        sync_result.modified_emails,
                        sync_result.deleted_emails
                    );
                    sync_result
                }
            };
            sync_results.push(sync_result);
        }

        let total_synced = sync_results
            .iter()
            .map(|r| r.new_emails + r.modified_emails + r.deleted_emails)
            .sum::<usize>();
        let folders_synced = sync_results.len();
        let errors = sync_results
            .iter()
            .filter(|r| r.new_emails == 0 && r.modified_emails == 0 && r.deleted_emails == 0)
            .count();
        let duration_ms = sync_results.iter().map(|r| r.duration_ms).sum::<u64>();

        // 发送完成事件
        let _ = self.emit_progress(
            account_id,
            SyncProgress {
                account_id,
                stage: SyncStage::Completed,
                folder: None,
                current: total_synced,
                total: folders_to_sync.len(),
                message: format!("同步完成，共处理 {} 封邮件", total_synced),
            },
        );

        Ok(SyncResult {
            strategy_used: SyncStrategy::UidSearch,
            new_emails: 0,
            modified_emails: 0,
            deleted_emails: 0,
            last_sync_uid: 0,
            flags_changed: 0,
            duration_ms: 0,
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

    /// 停止同步
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    pub async fn stop_sync(&self, account_id: i32) -> Result<()> {
        tracing::info!("停止同步账号: {}", account_id);
        // TODO: 实现停止同步逻辑
        //
        // 当前状态：占位实现，仅记录日志
        // 需要实现：
        // 1. 设置停止标志，中断正在进行的同步操作
        // 2. 关闭 IMAP 连接
        // 3. 取消进行中的邮件获取操作
        // 4. 清理同步状态
        //
        // 技术要点：
        // - 需要使用 CancellationToken 或类似的协作式取消机制
        // - 需要确保 IMAP 客户端正确断开连接
        // - 需要处理停止操作的幂等性（多次调用安全）
        Ok(())
    }

    /// 发送同步进度事件
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `progress` - 进度信息
    fn emit_progress(&self, account_id: i32, progress: SyncProgress) -> Result<()> {
        // 使用前端期望的事件名称格式：sync-progress-{account_id}
        let event_name = format!("sync-progress-{}", account_id);
        tracing::info!("📤 发送进度事件: event_name={}, stage={:?}", event_name, progress.stage);

        self.app_handle
            .emit(
                &event_name,
                progress,
            )
            .map_err(|e| MailError::Internal(format!("发送进度事件失败: {}", e)))?;
        Ok(())
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
            account_id: 1,
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
}
