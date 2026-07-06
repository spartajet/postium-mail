//！ ═════════════════════════════════════════════════════════════════════════
//！ 同步服务模块 (Sync Service)
//！ ═════════════════════════════════════════════════════════════════════════
//！
//！ 本模块负责与邮件服务器（IMAP/SMTP）的同步操作，包括：
//！ 1. 账号同步 - 从邮件服务器拉取新邮件、同步邮件状态
//！ 2. 进度通知 - 通过 Tauri 事件系统向前端推送同步进度
//！ 3. 文件夹统计 - 提供各文件夹的邮件数量统计
//！
//！ 设计特点：
//！ - 使用 SyncOrchestrator 编排复杂的同步流程
//！ - 通过 Tauri 事件系统实时通知前端同步进度
//！ - 支持分阶段同步（连接、列出文件夹、同步邮件等）
//！ - 提供详细的同步结果统计
//！ ═════════════════════════════════════════════════════════════════════════

use crate::domain::auth::AuthManager;
use crate::domain::auth::manager::Credentials;
use crate::domain::folders::{FolderCategory, FolderRegistry, RemoteFolder};
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::sync::SyncProgressEmitter;
use crate::domain::sync::folder_sync_dispatcher::SyncOrchestrator;
use crate::domain::sync::folder_sync_full::fetch_emails_body;
use crate::domain::sync::{
    FolderStat, HISTORY_UID_BATCH_SIZE, InitialSyncRange, SyncProgress, SyncStage,
    history_exhausted_before_uid, next_history_before_uid,
};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::{account_repo, email_repo, sync_repo};
use crate::service::account_connection::imap_config_from_account;
use crate::service::email_service::EmailCategory;
use crate::service::email_service::local_folder_registry_inputs;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistorySyncState {
    pub account_id: i32,
    pub category: EmailCategory,
    pub history_synced_since: Option<i64>,
    pub history_before_uid: Option<u32>,
    pub history_exhausted: bool,
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OlderSyncResult {
    pub new_emails: u64,
    pub updated_emails: u64,
    pub window_start: i64,
    pub window_end: i64,
    pub history_exhausted: bool,
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncAccountFailure {
    pub account_id: i32,
    pub email: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncAllAccountsResult {
    pub total: usize,
    pub succeeded: Vec<i32>,
    pub failed: Vec<SyncAccountFailure>,
}

/// 同步服务
///
/// 负责处理所有与邮件服务器同步相关的业务逻辑。
///
/// # 字段
///
/// - `db`: 数据库连接，用于持久化同步的邮件数据
/// - `auth`: 认证管理器，用于获取连接邮件服务器所需的凭证
///
/// # 主要功能
///
/// - 同步账号（带进度通知）
/// - 获取文件夹统计信息
pub struct SyncService {
    /// 数据库连接
    db: DbConn,
    /// 认证管理器（线程安全）
    auth: Arc<AuthManager>,
}

impl SyncService {
    /// 创建新的同步服务实例
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `auth`: 认证管理器（使用 Arc 包装以便共享）
    ///
    /// # 返回
    ///
    /// 返回初始化好的 SyncService 实例
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self { db, auth }
    }

    /// 同步账号（带进度通知）
    ///
    /// 这是主要的同步方法，会启动一个完整的同步流程，并实时向前端发送进度事件。
    /// 前端可以监听这些事件来显示同步进度条、状态消息等。
    ///
    /// # 工作流程
    ///
    /// 1. **连接阶段**: 连接到 IMAP 服务器
    /// 2. **列出文件夹**: 获取服务器上的文件夹列表
    /// 3. **同步文件夹**: 逐个同步文件夹中的邮件
    /// 4. **完成**: 返回同步结果统计
    ///
    /// # 进度通知
    ///
    /// 同步过程中会发送多个 Tauri 事件到前端，每个事件包含：
    /// - `stage`: 当前同步阶段（Connecting, Syncing, Completed, Error 等）
    /// - `folder`: 当前正在同步的文件夹（如果有）
    /// - `current`: 当前进度计数
    /// - `total`: 总数量
    /// - `message`: 进度消息文本
    ///
    /// # 参数
    ///
    /// - `app_handle`: Tauri 应用句柄，用于发送事件到前端
    /// - `account_id`: 要同步的账号 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 同步结果
    ///
    /// 同步完成后会发送 `Completed` 事件，包含：
    /// - `new_emails`: 新增的邮件数量
    /// - `updated_emails`: 更新的邮件数量
    /// - `duration_ms`: 同步耗时（毫秒）
    ///
    /// # 错误处理
    ///
    /// 如果同步失败，会发送 `Error` 事件，包含错误消息。
    ///
    /// # 前端监听示例
    ///
    /// ```typescript,ignore
    /// // 监听同步进度事件
    /// const unlisten = await listen<SyncProgress>('sync:progress', (event) => {
    ///     const progress = event.payload;
    ///     console.log(`阶段: ${progress.stage}`);
    ///     console.log(`消息: ${progress.message}`);
    ///
    ///     // 更新进度条
    ///     if (progress.total > 0) {
    ///         const percent = (progress.current / progress.total) * 100;
    ///         updateProgressBar(percent);
    ///     }
    ///
    ///     // 显示消息
    ///     showToast(progress.message);
    /// });
    ///
    /// // 开始同步
    /// await invoke('sync_account_with_progress', { accountId: 1 });
    /// ```
    ///
    /// # 同步阶段说明
    ///
    /// - `Connecting`: 正在连接到邮件服务器
    /// - `ListingFolders`: 正在获取文件夹列表
    /// - `SyncingFolder`: 正在同步某个文件夹
    /// - `Completed`: 同步完成
    /// - `Error`: 同步失败
    pub async fn sync_account_with_progress(
        &self,
        app_handle: tauri::AppHandle,
        account_id: i32,
    ) -> Result<(), MailError> {
        // 记录日志，便于追踪
        tracing::info!(account_id, "开始同步（带进度）");

        // ─── 创建进度事件发射器 ───
        // 这个发射器会通过 Tauri 事件系统向前端发送进度更新
        let emitter = SyncProgressEmitter::new(app_handle);

        // ─── 发送"正在连接"事件 ───
        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::Connecting,
            folder: None,
            folder_display_name: None,
            current: 0,
            total: 0,
            message: "正在连接...".into(),
        });

        // ─── 创建同步编排器 ───
        // SyncOrchestrator 负责协调整个同步流程
        // .with_emitter() 将进度发射器注册到编排器，使其能发送进度事件
        let orchestrator =
            SyncOrchestrator::new(self.db.clone(), self.auth.clone()).with_emitter(emitter.clone());

        // ─── 执行同步 ───
        match orchestrator.sync_account(account_id).await {
            // ─── 同步成功 ───
            Ok(result) => {
                // 记录详细的同步结果
                tracing::info!(
                    account_id,
                    new_emails = result.new_emails,
                    updated_emails = result.updated_emails,
                    duration_ms = result.duration_ms,
                    "同步完成"
                );

                // 发送"同步完成"事件
                emitter.emit(SyncProgress {
                    account_id,
                    stage: SyncStage::Completed,
                    folder: None,
                    folder_display_name: None,
                    current: result.new_emails,
                    total: result.new_emails + result.updated_emails,
                    message: format!(
                        "同步完成: {} 封新邮件, {} 封更新",
                        result.new_emails, result.updated_emails
                    ),
                });

                Ok(())
            }

            // ─── 同步失败 ───
            Err(e) => {
                // 发送"同步失败"事件，包含错误消息
                emitter.emit(SyncProgress {
                    account_id,
                    stage: SyncStage::Error,
                    folder: None,
                    folder_display_name: None,
                    current: 0,
                    total: 0,
                    message: format!("同步失败: {e}"),
                });

                // 返回错误，让调用方处理
                Err(e)
            }
        }
    }

    pub async fn sync_account_with_range(
        &self,
        app_handle: tauri::AppHandle,
        account_id: i32,
        range: InitialSyncRange,
    ) -> Result<(), MailError> {
        let emitter = SyncProgressEmitter::new(app_handle);
        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::Connecting,
            folder: None,
            folder_display_name: None,
            current: 0,
            total: 0,
            message: "正在连接...".into(),
        });

        let orchestrator =
            SyncOrchestrator::new(self.db.clone(), self.auth.clone()).with_emitter(emitter);
        orchestrator
            .sync_account_with_range(account_id, range)
            .await
            .map(|_| ())
    }

    pub async fn sync_all_accounts_with_progress(
        &self,
        app_handle: tauri::AppHandle,
    ) -> Result<SyncAllAccountsResult, MailError> {
        self.sync_all_accounts_with_runner(|account_id| {
            self.sync_account_with_progress(app_handle.clone(), account_id)
        })
        .await
    }

    async fn sync_all_accounts_with_runner<F, Fut>(
        &self,
        mut runner: F,
    ) -> Result<SyncAllAccountsResult, MailError>
    where
        F: FnMut(i32) -> Fut,
        Fut: Future<Output = Result<(), MailError>>,
    {
        let accounts = account_repo::list(&self.db).await?;
        let mut outcomes = Vec::with_capacity(accounts.len());

        for account in &accounts {
            outcomes.push(runner(account.id).await);
        }

        collect_sync_all_accounts_result(accounts, outcomes)
    }

    pub async fn get_history_state(
        &self,
        account_id: i32,
        category: EmailCategory,
    ) -> Result<HistorySyncState, MailError> {
        if category == EmailCategory::Starred {
            return Err(MailError::InvalidParam("星标邮件不支持历史回填".into()));
        }

        let account = account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))?;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".into(),
            ))?
            .clone();
        let provider = provider_pool
            .get(&account.provider)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
        // get_history_state 不连接 IMAP，只能用本地已存文件夹名做降级解析：
        // SPECIAL-USE 置空、no_select=false，仅靠跨语言关键词 + provider 候选名兜底。
        let (local_names, known_categories) =
            local_folder_registry_inputs(&self.db, account_id).await?;
        let registry = FolderRegistry::builder()
            .remote_folders(local_names)
            .known_categories(known_categories)
            .provider_mapping(provider.folder_mapping())
            .allow_unverified_provider_fallback(true)
            .build();
        let cat = FolderCategory::from_email_category(&category)
            .ok_or(MailError::InvalidParam("星标邮件不支持历史回填".into()))?;
        let folders = registry.resolve(cat);
        if folders.is_empty() {
            return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
        }

        let mut history_synced_since: Option<i64> = None;
        let mut history_before_uid: Option<u32> = None;
        let mut history_exhausted = true;
        for folder in &folders {
            let state = sync_repo::get_sync_state(&self.db, account_id, folder).await?;
            history_synced_since = match (
                history_synced_since,
                state.as_ref().and_then(|s| s.history_synced_since),
            ) {
                (Some(existing), Some(next)) => Some(existing.min(next)),
                (None, next) => next,
                (existing, None) => existing,
            };
            history_before_uid = match (
                history_before_uid,
                state.as_ref().and_then(|s| s.history_before_uid),
            ) {
                (Some(existing), Some(next)) => Some(existing.min(next)),
                (None, next) => next,
                (existing, None) => existing,
            };
            history_exhausted &= state.and_then(|s| s.history_exhausted).unwrap_or(false);
        }

        Ok(HistorySyncState {
            account_id,
            category,
            history_synced_since,
            history_before_uid,
            history_exhausted,
            folders,
        })
    }

    pub async fn sync_older_emails(
        &self,
        account_id: i32,
        category: EmailCategory,
    ) -> Result<OlderSyncResult, MailError> {
        let account = account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))?;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".into(),
            ))?
            .clone();
        let provider = provider_pool
            .get(&account.provider)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
        if category == EmailCategory::Starred {
            return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
        }
        let cat = FolderCategory::from_email_category(&category)
            .ok_or(MailError::InvalidParam("当前分类不支持历史回填".into()))?;

        let credentials = self
            .auth
            .get_credentials(
                &account.email,
                &provider.provider_info().auth_type,
                Some(&account.provider),
            )
            .await?;
        let imap_config = imap_config_from_account(&account)?;
        let mut client = match credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &account.email, &password).await?
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await?
            }
        };

        // 连接 IMAP 后用服务器返回的真实文件夹（带 SPECIAL-USE）构建 Registry，
        // 确保选出的文件夹确实存在，避免回退到本地/远程都不存在的候选名（会导致
        // before_uid 退化为默认值 1，随即被判定为历史已耗尽，从而一封更早的邮件也拉不到）。
        let remote_folders = client.list_folders().await?;
        let remote: Vec<RemoteFolder> = remote_folders
            .iter()
            .map(|f| RemoteFolder {
                name: f.name.clone(),
                special_use: f.special_use.clone(),
                no_select: f.no_select,
            })
            .collect();
        let registry = FolderRegistry::builder()
            .remote_folders(remote)
            .provider_mapping(provider.folder_mapping())
            .build();
        let folders = registry.resolve(cat);
        if folders.is_empty() {
            client.logout().await.ok();
            return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
        }
        for folder in &folders {
            if let Some(category) = registry.classify(folder) {
                sync_repo::upsert_folder_category(&self.db, account_id, folder, category.as_str())
                    .await?;
            }
        }
        tracing::debug!(
            account_id,
            category = ?category,
            folders = ?folders,
            "历史回填解析文件夹完成"
        );

        let mut new_emails = 0u64;
        let mut updated_emails = 0u64;
        let mut min_window_start = i64::MAX;
        let mut max_window_end = i64::MIN;
        let mut folder_exhausted_states = Vec::with_capacity(folders.len());

        for folder in &folders {
            let state = sync_repo::get_sync_state(&self.db, account_id, folder).await?;
            let before_uid = history_before_uid_for_folder(
                &self.db,
                account_id,
                folder,
                state.as_ref(),
                Some(&mut client),
            )
            .await?;
            if history_exhausted_before_uid(before_uid) {
                sync_repo::update_history_state(
                    &self.db,
                    account_id,
                    folder,
                    state.as_ref().and_then(|s| s.history_synced_since),
                    Some(before_uid),
                    true,
                )
                .await?;
                folder_exhausted_states.push(true);
                continue;
            }

            let uids = client
                .list_uids_before_uid(folder, before_uid, HISTORY_UID_BATCH_SIZE)
                .await?;

            if uids.is_empty() {
                sync_repo::update_history_state(
                    &self.db,
                    account_id,
                    folder,
                    state.as_ref().and_then(|s| s.history_synced_since),
                    Some(before_uid),
                    true,
                )
                .await?;
                folder_exhausted_states.push(true);
                continue;
            }

            let next_before_uid = next_history_before_uid(&uids, before_uid);
            let mut folder_min_sent_at = state.as_ref().and_then(|s| s.history_synced_since);
            let mut folder_window_start = i64::MAX;
            let mut folder_window_end = i64::MIN;

            for group in uids.chunks(10) {
                let email_headers = client.fetch_email_headers_by_uids(folder, group).await?;
                for header in &email_headers {
                    let sent_at = header.date.timestamp();
                    folder_min_sent_at =
                        Some(folder_min_sent_at.map_or(sent_at, |existing| existing.min(sent_at)));
                    folder_window_start = folder_window_start.min(sent_at);
                    folder_window_end = folder_window_end.max(sent_at);
                }
                new_emails += email_repo::save_batch_email_headers(
                    &self.db,
                    account_id,
                    folder,
                    &email_headers,
                )
                .await? as u64;
            }
            fetch_emails_body(self.db.clone(), account_id, folder, &uids, &mut client).await?;
            updated_emails += uids.len() as u64;

            let folder_history_exhausted = history_exhausted_before_uid(next_before_uid);

            sync_repo::update_history_state(
                &self.db,
                account_id,
                folder,
                folder_min_sent_at,
                Some(next_before_uid),
                folder_history_exhausted,
            )
            .await?;
            folder_exhausted_states.push(folder_history_exhausted);

            if folder_window_start != i64::MAX {
                min_window_start = min_window_start.min(folder_window_start);
            }
            if folder_window_end != i64::MIN {
                max_window_end = max_window_end.max(folder_window_end);
            }
        }

        client.logout().await.ok();

        Ok(OlderSyncResult {
            new_emails,
            updated_emails,
            window_start: normalized_history_window_bound(min_window_start),
            window_end: normalized_history_window_bound(max_window_end),
            history_exhausted: all_history_exhausted(folder_exhausted_states),
            folders,
        })
    }

    /// 获取账号侧边栏分类统计信息。
    ///
    /// 先按本地真实文件夹做 GROUP BY 统计，再通过 FolderRegistry 折叠到
    /// `inbox`、`sent`、`drafts` 等侧边栏分类，避免前端拿到原始 IMAP 名称
    /// 或 UTF-7 编码名后无法匹配分类。
    ///
    /// # 统计信息
    ///
    /// 返回每个分类的以下信息：
    /// - `folder`: 侧边栏分类 key（如 "inbox", "sent", "starred"）
    /// - `total`: 总邮件数
    /// - `unread`: 未读邮件数
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    ///
    /// # 返回
    ///
    /// 返回侧边栏分类统计信息列表。
    ///
    /// # 使用场景
    ///
    /// - 前端侧边栏显示未读邮件数
    /// - 文件夹列表显示邮件数量
    /// - 同步前后对比邮件变化
    ///
    /// # 性能优化
    ///
    /// 使用 GROUP BY 聚合真实文件夹统计，再在内存中按分类合并，
    /// 避免了对每个文件夹单独查询的 N+1 问题。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let stats = sync_service.get_folder_stats(1).await?;
    /// for stat in stats {
    ///     println!(
    ///         "{}: {} 封邮件，{} 封未读",
    ///         stat.folder, stat.total, stat.unread
    ///     );
    /// }
    /// ```
    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>, MailError> {
        let account = account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))?;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".into(),
            ))?
            .clone();
        let provider = provider_pool
            .get(&account.provider)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
        let (local_names, known_categories) =
            local_folder_registry_inputs(&self.db, account_id).await?;
        let registry = FolderRegistry::builder()
            .remote_folders(local_names)
            .known_categories(known_categories)
            .provider_mapping(provider.folder_mapping())
            .build();
        let mut stats = aggregate_folder_stats_by_category(
            email_repo::folder_stats_by_account(&self.db, account_id).await?,
            &registry,
        );
        let starred = email_repo::starred_stats_by_account(&self.db, account_id).await?;
        if starred.total > 0 || starred.unread > 0 {
            stats.push(starred);
            stats.sort_by(|a, b| a.folder.cmp(&b.folder));
        }
        Ok(stats)
    }

    pub async fn get_folder_stats_for_all_accounts(&self) -> Result<Vec<FolderStat>, MailError> {
        let accounts = account_repo::list(&self.db).await?;
        let mut outcomes = Vec::with_capacity(accounts.len());

        for account in &accounts {
            outcomes.push(self.get_folder_stats(account.id).await);
        }

        collect_all_account_folder_stats(accounts, outcomes)
    }
}

fn aggregate_folder_stats_by_category(
    stats: Vec<FolderStat>,
    registry: &FolderRegistry,
) -> Vec<FolderStat> {
    let mut grouped = HashMap::<String, FolderStat>::new();

    for stat in stats {
        let folder = registry
            .classify(&stat.folder)
            .map(|category| category.as_str().to_string())
            .unwrap_or_else(|| stat.folder.clone());
        let entry = grouped.entry(folder.clone()).or_insert(FolderStat {
            folder,
            total: 0,
            unread: 0,
        });
        entry.total += stat.total;
        entry.unread += stat.unread;
    }

    let mut stats = grouped.into_values().collect::<Vec<_>>();
    stats.sort_by(|a, b| a.folder.cmp(&b.folder));
    stats
}

fn collect_sync_all_accounts_result(
    accounts: Vec<crate::infrastructure::storage::models::accounts::Model>,
    outcomes: Vec<Result<(), MailError>>,
) -> Result<SyncAllAccountsResult, MailError> {
    let total = accounts.len();
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for (account, outcome) in accounts.into_iter().zip(outcomes) {
        match outcome {
            Ok(()) => succeeded.push(account.id),
            Err(err) => {
                failed.push(SyncAccountFailure {
                    account_id: account.id,
                    email: Some(account.email),
                    message: err.to_string(),
                });
            }
        }
    }

    if total > 0 && succeeded.is_empty() && !failed.is_empty() {
        return Err(MailError::SyncFailed(format!(
            "所有账号同步失败: {}",
            failed
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    Ok(SyncAllAccountsResult {
        total,
        succeeded,
        failed,
    })
}

fn collect_all_account_folder_stats(
    accounts: Vec<crate::infrastructure::storage::models::accounts::Model>,
    outcomes: Vec<Result<Vec<FolderStat>, MailError>>,
) -> Result<Vec<FolderStat>, MailError> {
    let mut grouped = HashMap::<String, FolderStat>::new();
    let mut failures = Vec::new();

    for (account, outcome) in accounts.into_iter().zip(outcomes) {
        match outcome {
            Ok(stats) => {
                for stat in stats {
                    let entry = grouped.entry(stat.folder.clone()).or_insert(FolderStat {
                        folder: stat.folder,
                        total: 0,
                        unread: 0,
                    });
                    entry.total += stat.total;
                    entry.unread += stat.unread;
                }
            }
            Err(err) => {
                tracing::warn!(
                    account_id = account.id,
                    error = %err,
                    "所有账号统计跳过异常账号"
                );
                failures.push(err);
            }
        }
    }

    if grouped.is_empty() {
        if let Some(err) = failures.into_iter().next() {
            return Err(err);
        }
    }

    let mut stats = grouped.into_values().collect::<Vec<_>>();
    stats.sort_by(|a, b| a.folder.cmp(&b.folder));
    Ok(stats)
}

fn all_history_exhausted(states: impl IntoIterator<Item = bool>) -> bool {
    let mut saw_folder = false;
    let all_exhausted = states.into_iter().all(|exhausted| {
        saw_folder = true;
        exhausted
    });
    saw_folder && all_exhausted
}

async fn history_before_uid_for_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    state: Option<&crate::infrastructure::storage::models::sync_state::Model>,
    imap_client: Option<&mut ImapClient>,
) -> Result<u32, MailError> {
    if let Some(before_uid) = state.and_then(|s| s.history_before_uid) {
        return Ok(before_uid);
    }
    if let Some(local_min_uid) = email_repo::min_uid_by_folder(db, account_id, folder).await? {
        return Ok(local_min_uid);
    }
    if let Some(uidnext) = state.and_then(|s| s.uidnext) {
        return Ok(uidnext);
    }

    // 该文件夹本地从未同步过（如同一分类的第二个别名文件夹）：
    // 用 IMAP EXAMINE 拿到服务器真实的 uidnext 作为回填起点，
    // 避免直接退化成 1 被误判为历史已耗尽、一封邮件也拉不到。
    if let Some(client) = imap_client {
        let metadata = client.fetch_folder_metadata(folder).await?;
        return Ok(metadata.uidnext as u32);
    }

    Ok(1)
}

fn normalized_history_window_bound(value: i64) -> i64 {
    if value == i64::MAX || value == i64::MIN {
        0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MailError;
    use crate::infrastructure::storage::DbConn;
    use crate::infrastructure::storage::models::accounts;
    use crate::infrastructure::storage::repository::account_repo::{self, AccountWrite};
    use std::sync::{Arc, Mutex};

    fn remote_folder(name: &str) -> RemoteFolder {
        RemoteFolder {
            name: name.to_string(),
            special_use: vec![],
            no_select: false,
        }
    }

    #[test]
    fn all_history_exhausted_should_return_false_when_any_folder_has_older_mail() {
        assert!(!all_history_exhausted([true, false, true]));
    }

    #[test]
    fn all_history_exhausted_should_return_true_when_every_folder_is_exhausted() {
        assert!(all_history_exhausted([true, true]));
    }

    #[test]
    fn all_history_exhausted_should_return_false_when_no_folder_was_checked() {
        assert!(!all_history_exhausted([]));
    }

    #[test]
    fn aggregate_folder_stats_uses_sidebar_categories() {
        let registry = FolderRegistry::builder()
            .remote_folders(vec![remote_folder("INBOX"), remote_folder("&XfJT0ZAB-")])
            .build();
        let stats = vec![
            FolderStat {
                folder: "INBOX".to_string(),
                total: 3,
                unread: 2,
            },
            FolderStat {
                folder: "&XfJT0ZAB-".to_string(),
                total: 4,
                unread: 1,
            },
            FolderStat {
                folder: "Unknown".to_string(),
                total: 9,
                unread: 5,
            },
        ];

        let aggregated = aggregate_folder_stats_by_category(stats, &registry);

        assert_eq!(
            aggregated
                .iter()
                .find(|stat| stat.folder == "inbox")
                .map(|stat| stat.unread),
            Some(2)
        );
        assert_eq!(
            aggregated
                .iter()
                .find(|stat| stat.folder == "sent")
                .map(|stat| stat.unread),
            Some(1)
        );
        assert_eq!(
            aggregated
                .iter()
                .find(|stat| stat.folder == "Unknown")
                .map(|stat| stat.unread),
            Some(5)
        );
    }

    fn test_account(id: i32, email: &str) -> accounts::Model {
        accounts::Model {
            id,
            name: format!("Account {id}"),
            email: email.to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl: None,
            smtp_ssl_mode: None,
            color: None,
            sync_enabled: Some(true),
            last_sync_at: None,
            auth_type: Some("Password".to_string()),
            account_type: "personal".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn insert_test_account(db: &DbConn, email: &str) -> accounts::Model {
        account_repo::create(
            db,
            AccountWrite {
                name: email.to_string(),
                email: email.to_string(),
                display_name: None,
                provider: "gmail".to_string(),
                imap_host: None,
                imap_port: None,
                imap_ssl: None,
                imap_ssl_mode: None,
                smtp_host: None,
                smtp_port: None,
                smtp_ssl: None,
                smtp_ssl_mode: None,
                color: None,
                sync_enabled: Some(true),
                last_sync_at: None,
                auth_type: Some("Password".to_string()),
                account_type: "personal".to_string(),
                created_at: 0,
                updated_at: 0,
            },
        )
        .await
        .unwrap()
    }

    #[test]
    fn sync_all_accounts_with_progress_should_return_partial_failures_when_some_accounts_fail() {
        let accounts = vec![
            test_account(1, "ok@example.com"),
            test_account(2, "fail@example.com"),
        ];
        let outcomes = vec![Ok(()), Err(MailError::SyncFailed("imap offline".into()))];

        let result = collect_sync_all_accounts_result(accounts, outcomes).unwrap();

        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, vec![1]);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].account_id, 2);
        assert_eq!(result.failed[0].email.as_deref(), Some("fail@example.com"));
        assert!(result.failed[0].message.contains("imap offline"));
    }

    #[test]
    fn sync_all_accounts_with_progress_should_return_error_when_all_accounts_fail() {
        let accounts = vec![
            test_account(1, "a@example.com"),
            test_account(2, "b@example.com"),
        ];
        let outcomes = vec![
            Err(MailError::SyncFailed("first failed".into())),
            Err(MailError::AuthFailed("bad password".into())),
        ];

        let err = collect_sync_all_accounts_result(accounts, outcomes).unwrap_err();

        match err {
            MailError::SyncFailed(message) => {
                assert!(message.contains("所有账号同步失败"));
                assert!(message.contains("first failed"));
                assert!(message.contains("bad password"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn sync_all_accounts_with_runner_should_call_every_account_and_return_partial_failures() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let account_a = insert_test_account(&db, "ok@example.com").await;
        let account_b = insert_test_account(&db, "fail@example.com").await;
        let service = SyncService::new(db, Arc::new(AuthManager::in_memory()));
        let called_ids = Arc::new(Mutex::new(Vec::new()));
        let called_ids_for_runner = called_ids.clone();

        let result = service
            .sync_all_accounts_with_runner(move |account_id| {
                let called_ids = called_ids_for_runner.clone();
                async move {
                    called_ids.lock().unwrap().push(account_id);
                    if account_id == account_b.id {
                        Err(MailError::SyncFailed("imap offline".into()))
                    } else {
                        Ok(())
                    }
                }
            })
            .await
            .unwrap();

        assert_eq!(
            *called_ids.lock().unwrap(),
            vec![account_a.id, account_b.id]
        );
        assert_eq!(result.total, 2);
        assert_eq!(result.succeeded, vec![account_a.id]);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].account_id, account_b.id);
        assert_eq!(result.failed[0].email.as_deref(), Some("fail@example.com"));
        assert!(result.failed[0].message.contains("imap offline"));
    }

    #[tokio::test]
    async fn sync_all_accounts_with_runner_should_return_error_when_all_accounts_fail() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let account_a = insert_test_account(&db, "a@example.com").await;
        let account_b = insert_test_account(&db, "b@example.com").await;
        let service = SyncService::new(db, Arc::new(AuthManager::in_memory()));
        let called_ids = Arc::new(Mutex::new(Vec::new()));
        let called_ids_for_runner = called_ids.clone();

        let err = service
            .sync_all_accounts_with_runner(move |account_id| {
                let called_ids = called_ids_for_runner.clone();
                async move {
                    called_ids.lock().unwrap().push(account_id);
                    if account_id == account_a.id {
                        Err(MailError::SyncFailed("first failed".into()))
                    } else {
                        Err(MailError::AuthFailed("bad password".into()))
                    }
                }
            })
            .await
            .unwrap_err();

        assert_eq!(
            *called_ids.lock().unwrap(),
            vec![account_a.id, account_b.id]
        );
        match err {
            MailError::SyncFailed(message) => {
                assert!(message.contains("所有账号同步失败"));
                assert!(message.contains("first failed"));
                assert!(message.contains("bad password"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
