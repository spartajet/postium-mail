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
use crate::domain::sync::SyncProgressEmitter;
use crate::domain::sync::folder_sync_dispatcher::SyncOrchestrator;
use crate::domain::sync::{FolderStat, SyncProgress, SyncStage};
use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::email_repo;
use std::sync::Arc;

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
