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
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
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
        let folders = self
            .resolve_history_folders(account_id, category.clone(), &provider.folder_mapping())
            .await?;
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
        let candidate_folders = category.resolve_folders(&provider.folder_mapping());
        if candidate_folders.is_empty() || category == EmailCategory::Starred {
            return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
        }

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

        // 连接 IMAP 后用服务器返回的真实文件夹名来解析，确保选出的文件夹确实存在，
        // 避免像旧逻辑那样回退到一个本地/远程都不存在的候选名（会导致 before_uid 退化为
        // 默认值 1，随即被判定为历史已耗尽，从而一封更早的邮件也拉不到）。
        let remote_folder_names = client
            .list_folders()
            .await?
            .into_iter()
            .map(|folder| folder.name)
            .collect::<Vec<_>>();
        let folders = self
            .resolve_history_candidate_folders(
                account_id,
                &candidate_folders,
                Some(&remote_folder_names),
            )
            .await?;
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

    async fn resolve_history_folders(
        &self,
        account_id: i32,
        category: EmailCategory,
        mapping: &crate::domain::providers::StandardFolder,
    ) -> Result<Vec<String>, MailError> {
        let candidates = category.resolve_folders(mapping);
        if candidates.is_empty() {
            return Ok(candidates);
        }

        // get_history_state 不连接 IMAP，只能基于本地已存文件夹名解析。
        self.resolve_history_candidate_folders(account_id, &candidates, None)
            .await
    }

    /// 基于本地与（可选的）远程文件夹名，为历史回填挑选所有真实存在的文件夹。
    ///
    /// 同一分类可能映射到多个别名（如网易发件箱同时存在 `Sent` 与 IMAP-UTF-7 的
    /// `&XfJT0ZAB-`），这些是不同的物理文件夹，各自的历史都要回填，因此返回候选中
    /// **所有**真实存在的文件夹，而非只挑一个。
    ///
    /// `remote_folders` 在 `sync_older_emails` 中由 IMAP `LIST` 返回；
    /// 在 `get_history_state` 这类不连接服务器的场景下传入 `None`，仅用本地已存名称解析。
    async fn resolve_history_candidate_folders(
        &self,
        account_id: i32,
        candidates: &[String],
        remote_folders: Option<&[String]>,
    ) -> Result<Vec<String>, MailError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let mut local_folders =
            sync_repo::distinct_folders_by_account(&self.db, account_id).await?;
        local_folders.extend(email_repo::distinct_folders_by_account(&self.db, account_id).await?);
        let local_folders = local_folders.into_iter().collect::<HashSet<_>>();
        let remote_folders = remote_folders
            .map(|folders| folders.iter().cloned().collect::<HashSet<_>>());

        Ok(choose_history_folders(
            candidates,
            &local_folders,
            remote_folders.as_ref(),
        ))
    }

    /// 获取账号的文件夹统计信息
    ///
    /// 使用单条 GROUP BY SQL 查询获取各文件夹的邮件数量统计，
    /// 相比 N+1 查询（多次查询）更高效。
    ///
    /// # 统计信息
    ///
    /// 返回每个文件夹的以下信息：
    /// - `folder`: 文件夹名称（如 "INBOX", "Sent"）
    /// - `total_count`: 总邮件数
    /// - `unread_count`: 未读邮件数
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    ///
    /// # 返回
    ///
    /// 返回文件夹统计信息列表。
    ///
    /// # SQL 查询示例
    ///
    /// ```sql
    /// SELECT
    ///     folder,
    ///     COUNT(*) as total_count,
    ///     SUM(CASE WHEN is_read = 0 THEN 1 ELSE 0 END) as unread_count
    /// FROM emails
    /// WHERE account_id = ? AND deleted_at IS NULL
    /// GROUP BY folder
    /// ```
    ///
    /// # 使用场景
    ///
    /// - 前端侧边栏显示未读邮件数
    /// - 文件夹列表显示邮件数量
    /// - 同步前后对比邮件变化
    ///
    /// # 性能优化
    ///
    /// 使用 GROUP BY 聚合，单次查询获取所有统计信息，
    /// 避免了对每个文件夹单独查询的 N+1 问题。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let stats = sync_service.get_folder_stats(1).await?;
    /// for stat in stats {
    ///     println!(
    ///         "{}: {} 封邮件，{} 封未读",
    ///         stat.folder, stat.total_count, stat.unread_count
    ///     );
    /// }
    /// ```
    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>, MailError> {
        email_repo::folder_stats_by_account(&self.db, account_id).await
    }
}

fn all_history_exhausted(states: impl IntoIterator<Item = bool>) -> bool {
    states.into_iter().all(|exhausted| exhausted)
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

/// 为历史回填挑选所有真实存在的文件夹名。
///
/// 同一分类可能映射到多个别名（如网易发件箱同时存在 `Sent` 与 IMAP-UTF-7 的
/// `&XfJT0ZAB-`），这些是不同的物理文件夹，各自都要独立回填历史，因此返回候选中
/// **所有**真实存在的文件夹。
///
/// 判定优先级（与初始同步 [`resolve_sync_folders`](crate::domain::sync) 保持一致）：
/// 1. **远程真实存在**：候选名出现在 IMAP `LIST` 返回的文件夹中（最可靠）。
/// 2. **本地已存**：候选名已存在于 `sync_state` / `emails` 表中（覆盖不连接服务器的场景）。
/// 3. **候选第一个**：当传入了 `remote_folders` 却没有任何候选命中远程时，
///    回退到候选数组的第一个，交由后续 IMAP 查询兜底；若未传入远程集合
///    （`get_history_state` 场景）且本地也无命中，则返回空，避免凭空选一个不存在的名字。
///
/// 注意：旧实现只在本地集合中挑**一个**，命中失败就回退到 `candidates[0]`，
/// 导致发件箱/草稿箱/垃圾箱等文件夹常常选到本地/远程都不存在的名字，
/// 进而 `history_before_uid` 退化为 `1`、被误判为历史已耗尽。
fn choose_history_folders(
    candidates: &[String],
    local_folders: &HashSet<String>,
    remote_folders: Option<&HashSet<String>>,
) -> Vec<String> {
    // 优先匹配远程真实存在的所有候选（如 QQ 的 "Sent Messages"、Gmail 的 "[Gmail]/Sent Mail"）。
    if let Some(remote) = remote_folders {
        let matched: Vec<String> = candidates
            .iter()
            .filter(|c| remote.contains(*c))
            .cloned()
            .collect();
        if !matched.is_empty() {
            return matched;
        }
        // 远程一个都没命中：回退到候选第一个，交给后续 IMAP 查询兜底（与初始同步一致）。
        return candidates.first().cloned().into_iter().collect();
    }

    // 未连接服务器（get_history_state）：收集所有本地已存的候选。
    candidates
        .iter()
        .filter(|c| local_folders.contains(*c))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{all_history_exhausted, choose_history_folders};
    use std::collections::HashSet;

    #[test]
    fn all_history_exhausted_should_return_false_when_any_folder_has_older_mail() {
        assert!(!all_history_exhausted([true, false, true]));
    }

    #[test]
    fn all_history_exhausted_should_return_true_when_every_folder_is_exhausted() {
        assert!(all_history_exhausted([true, true]));
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn choose_history_folders_should_return_all_remote_matches() {
        // 网易发件箱：Sent 与 IMAP-UTF-7 的 "&XfJT0ZAB-" 在服务器上都真实存在，
        // 两者是不同的物理文件夹，各自的历史都要回填。
        let candidates = vec!["Sent".to_string(), "&XfJT0ZAB-".to_string()];
        let local = set(&["INBOX"]);
        let remote = set(&["Sent", "&XfJT0ZAB-"]);

        let chosen = choose_history_folders(&candidates, &local, Some(&remote));
        assert_eq!(chosen, vec!["Sent".to_string(), "&XfJT0ZAB-".to_string()]);
    }

    #[test]
    fn choose_history_folders_should_return_only_remote_present_candidate() {
        // 候选有两个，但服务器上只有第二个真实存在。
        let candidates = vec!["Sent".to_string(), "&XfJT0ZAB-".to_string()];
        let local = set(&["Sent"]);
        let remote = set(&["INBOX", "&XfJT0ZAB-"]);

        let chosen = choose_history_folders(&candidates, &local, Some(&remote));
        assert_eq!(chosen, vec!["&XfJT0ZAB-".to_string()]);
    }

    #[test]
    fn choose_history_folders_should_fall_back_to_first_candidate_when_no_remote_match() {
        // 远程一个都没命中时，回退到候选第一个（与初始同步 resolve_sync_folders 一致）。
        let candidates = vec!["Sent".to_string(), "Sent Items".to_string()];
        let local = set(&["INBOX"]);
        let remote = set(&["INBOX"]);

        let chosen = choose_history_folders(&candidates, &local, Some(&remote));
        assert_eq!(chosen, vec!["Sent".to_string()]);
    }

    #[test]
    fn choose_history_folders_should_return_all_local_matches_without_remote() {
        // get_history_state 不连接服务器，只能靠本地已存名称解析，返回所有本地命中的候选。
        let candidates = vec![
            "Sent Messages".to_string(),
            "Sent".to_string(),
            "Sent Items".to_string(),
        ];
        let local = set(&["Sent", "Sent Items"]);

        let chosen = choose_history_folders(&candidates, &local, None);
        assert_eq!(
            chosen,
            vec!["Sent".to_string(), "Sent Items".to_string()]
        );
    }

    #[test]
    fn choose_history_folders_should_return_empty_when_no_local_match_without_remote() {
        // 未连接服务器且本地无命中：返回空，避免凭空选一个不存在的名字。
        let candidates = vec!["Sent".to_string(), "Sent Items".to_string()];
        let local = set(&["INBOX"]);

        let chosen = choose_history_folders(&candidates, &local, None);
        assert!(chosen.is_empty());
    }

    #[test]
    fn choose_history_folders_should_return_empty_for_empty_candidates() {
        let local = set(&["INBOX"]);
        let chosen = choose_history_folders(&[], &local, None);
        assert!(chosen.is_empty());
    }
}
