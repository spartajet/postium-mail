//!
//! # 邮件同步模块 (Mail Sync Module)
//!
//! 本模块实现了与邮件服务器的同步功能，是邮件客户端的核心功能之一。
//!
//! ## 模块组织
//!
//! ### folder_sync_dispatcher - 同步调度器
//!
//! 编排整个同步流程，协调各个同步步骤：
//! - 连接管理
//! - 文件夹发现
//! - 邮件同步策略选择
//! - 错误处理和重试
//!
//! 主要类型：
//! - `SyncOrchestrator`: 同步编排器，管理完整的同步生命周期
//!
//! ### folder_sync_full - 全量同步
//!
//! 实现文件夹的全量同步逻辑：
//! - 获取所有邮件 UID 列表
//! - 下载所有邮件的元数据和内容
//! - 建立本地索引
//!
//! 使用场景：
//! - 首次同步
//! - UIDVALIDITY 变化时（服务器重建）
//! - 用户手动触发全量同步
//!
//! ### folder_sync_increment - 增量同步
//!
//! 实现文件夹的增量同步逻辑：
//! - 基于上次同步的最高 UID
//! - 只获取新增或修改的邮件
//! - 显著提高同步效率
//!
//! 使用场景：
//! - 定期自动同步
//! - 用户手动刷新
//!
//! ### scheduler - 同步调度器
//!
//! 管理定时同步任务：
//! - 配置同步间隔
//! - 启动/停止定时任务
//! - 多账号并行同步
//!
//! 主要类型：
//! - `SyncScheduler`: 同步调度器，管理后台同步任务
//!
//! ## 同步流程
//!
//! ```text
//! 开始同步
//!    ↓
//! 连接 IMAP 服务器 (Connecting)
//!    ↓
//! 发现文件夹列表 (SyncingFolders)
//!    ↓
//! 对每个文件夹执行同步 (SyncingEmails)
//!    ├── 全量同步（首次）或增量同步
//!    ├── 下载新邮件
//!    ├── 更新已修改邮件
//!    └── 删除已移除邮件
//!    ↓
//! 同步完成 (Completed) 或出错 (Error)
//! ```
//!
//! ## 进度通知
//!
//! 同步过程中通过 Tauri 事件系统向前端发送进度通知：
//!
//! ```typescript
//! import { listen } from '@tauri-apps/api/event';
//!
//! const unlisten = await listen('sync-progress-event', (event) => {
//!     const progress = event.payload.progress;
//!     console.log(`阶段: ${progress.stage}`);
//!     console.log(`进度: ${progress.current}/${progress.total}`);
//! });
//! ```
//!
//! ## 同步策略
//!
//! ### 全量同步 (Full Sync)
//!
//! - 下载文件夹中的所有邮件
//! - 建立完整的本地索引
//! - 适用于首次同步或重建索引
//!
//! ### 增量同步 (Incremental Sync)
//!
//! - 基于 UID (Unique Identifier) 进行差异同步
//! - 只同步上次同步后的新邮件
//! - 效率远高于全量同步
//!
//! ## 错误处理
//!
//! - 网络错误：自动重试（最多 3 次）
//! - 认证错误：通知用户重新认证
//! - 服务器错误：记录日志并跳过当前文件夹
//! - 数据冲突：以服务器数据为准
//!
//! ## 性能优化
//!
//! - 使用 IMAP UID 进行增量同步，减少数据传输
//! - 并行同步不同文件夹
//! - 附件按需下载（首次只下载邮件头和正文）
//! - 使用管道 (pipelining) 提高 IMAP 命令效率
//!

/// 文件夹同步调度模块
///
/// 包含 `SyncOrchestrator` 同步编排器，负责协调完整的同步流程。
pub mod folder_sync_dispatcher;

/// 同步历史窗口模块
pub mod history;

/// 文件夹全量同步模块
///
/// 实现文件夹的全量同步逻辑，用于首次同步或重建索引。
pub mod folder_sync_full;

/// 文件夹增量同步模块
///
/// 实现基于 UID 的增量同步逻辑，提高同步效率。
pub mod folder_sync_increment;

/// 同步调度器模块
///
/// 管理定时同步任务，支持多账号并行同步。
pub mod scheduler;

/// 重新导出同步编排器
///
/// `SyncOrchestrator` 是同步流程的核心组件，协调各个同步步骤。
pub use folder_sync_dispatcher::SyncOrchestrator;

/// 重新导出同步调度器
///
/// `SyncScheduler` 负责管理后台定时同步任务。
pub use scheduler::SyncScheduler;

use serde::{Deserialize, Serialize};
use specta::Type;
use smol::channel::Sender;

use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::contact_repo;

/// 同步进度事件的 UI 订阅端（GPUI 侧用 cx.spawn 消费；替代原 tauri AppHandle）
pub type SyncEventTx = Sender<SyncProgressEvent>;

pub const HISTORY_UID_BATCH_SIZE: usize = 50;

pub fn history_exhausted_before_uid(before_uid: u32) -> bool {
    before_uid <= 1
}

pub fn next_history_before_uid(batch: &[u32], previous_before_uid: u32) -> u32 {
    batch.iter().copied().min().unwrap_or(previous_before_uid)
}

/// ─────────────────────────────────────────────────────────────────────
// Task 7: 收件路径联系人自动收集
// ─────────────────────────────────────────────────────────────────────

/// 把 IMAP 地址显示串拆成裸邮箱列表。
///
/// `EmailHeader.from/to/cc` 与 `WholeEmailDto.cc_emails` 存的都是显示格式
/// （`"Alice <a@x.com>, b@y.com"`，多个地址以 `, ` 连接）：直接按 `,`/`;`
/// 切分会把 `"Alice <a@x.com>"` 整串当邮箱落库，因此逐 token 提取尖括号
/// 内的地址；无尖括号的 token 视为裸邮箱本身。空白 token 被丢弃。
///
/// 亦被 [`crate::infrastructure::storage::repository::contact_repo::backfill_from_emails`]
/// 复用：存量回填把聚合 SQL 产出的地址 token（cc/recipient 列可能存显示
/// 格式）归一为裸地址后再并入联系人表。
pub(crate) fn split_display_addresses(raw: &str) -> Vec<String> {
    raw.split([',', ';'])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| match (token.find('<'), token.rfind('>')) {
            (Some(start), Some(end)) if start < end => token[start + 1..end].trim().to_string(),
            _ => token.to_string(),
        })
        .filter(|email| !email.is_empty())
        .collect()
}

/// 骨架/历史同步路径的联系人收集：把一批 [`EmailHeader`] 的发件人、
/// 收件人与抄送地址并入 `contacts` 表（增量规则见
/// [`contact_repo::upsert_from_emails`]，`name` 为 `None`，时间取邮件日期）。
///
/// 收集失败只记 `warn` 日志，不阻断同步主流程。
pub(crate) async fn headers_to_contacts(db: &DbConn, headers: &[EmailHeader]) {
    let items: Vec<contact_repo::IncomingContact> = headers
        .iter()
        .flat_map(|h| {
            let at = h.date.timestamp();
            let mut v: Vec<contact_repo::IncomingContact> = split_display_addresses(&h.from)
                .into_iter()
                .map(|email| contact_repo::IncomingContact {
                    email,
                    name: None,
                    contacted_at: at,
                })
                .collect();
            for addr in split_display_addresses(&h.to) {
                v.push(contact_repo::IncomingContact {
                    email: addr,
                    name: None,
                    contacted_at: at,
                });
            }
            v.extend(split_display_addresses(&h.cc).into_iter().map(|a| {
                contact_repo::IncomingContact {
                    email: a,
                    name: None,
                    contacted_at: at,
                }
            }));
            v
        })
        .collect();
    if !items.is_empty() {
        if let Err(error) = contact_repo::upsert_from_emails(db, &items).await {
            tracing::warn!(error = %error, count = items.len(), "同步路径联系人自动收集失败");
        }
    }
}

/// 增量同步路径（[`WholeEmailDto`]）的联系人收集：发件人带 `sender_name`，
/// 收件人 / 抄送从对应列拆分，时间取 `sent_at`（Unix 秒）。
///
/// 收集失败只记 `warn` 日志，不阻断同步主流程。
pub(crate) async fn whole_emails_to_contacts(db: &DbConn, emails: &[WholeEmailDto]) {
    let items: Vec<contact_repo::IncomingContact> = emails
        .iter()
        .flat_map(|email| {
            let at = email.sent_at;
            let mut v = vec![contact_repo::IncomingContact {
                email: email.sender_email.clone(),
                name: email.sender_name.clone(),
                contacted_at: at,
            }];
            for addr in split_display_addresses(&email.recipient_emails) {
                v.push(contact_repo::IncomingContact {
                    email: addr,
                    name: None,
                    contacted_at: at,
                });
            }
            v.extend(
                split_display_addresses(email.cc_emails.as_deref().unwrap_or(""))
                    .into_iter()
                    .map(|a| contact_repo::IncomingContact {
                        email: a,
                        name: None,
                        contacted_at: at,
                    }),
            );
            v
        })
        .collect();
    if !items.is_empty() {
        if let Err(error) = contact_repo::upsert_from_emails(db, &items).await {
            tracing::warn!(error = %error, count = items.len(), "同步路径联系人自动收集失败");
        }
    }
}

/// 初始同步范围
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InitialSyncRange {
    Week,
    Month,
    ThreeMonths,
    Year,
    All,
}

/// 同步时间窗口
///
/// `start` 和 `end` 都使用 Unix 秒级时间戳。
/// 当窗口映射到 IMAP 搜索条件时，`end` 对应 `BEFORE <date>` 的排他上界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncWindow {
    pub start: Option<i64>,
    pub end: Option<i64>,
}

/// 同步进度事件
///
/// 通过 Tauri 事件系统向前端发送同步进度通知。
/// 前端可以通过监听 `sync-progress-event` 事件来获取实时同步状态。
///
/// # 使用示例
///
/// ```typescript
/// import { listen } from '@tauri-apps/api/event';
///
/// const unlisten = await listen('sync-progress-event', (event) => {
///     const { progress } = event.payload;
///     console.log(`同步进度: ${progress.stage}`);
///     console.log(`消息: ${progress.message}`);
/// });
/// ```
#[derive(Debug, Clone, Serialize, Type)]
pub struct SyncProgressEvent {
    /// 同步进度信息
    pub progress: SyncProgress,
}

/// 同步进度发射器
///
/// 负责向前端发送同步进度事件。
/// 独立于 `SyncOrchestrator`，便于在不同层级使用。
///
/// # 设计说明
///
/// 将进度发射功能封装为独立结构体的原因：
/// 1. 解耦同步逻辑和事件发送
/// 2. 便于单元测试（可以 mock 发射器）
/// 3. 可以在不同组件中复用
///
/// # 使用示例
///
/// ```rust,ignore
/// let emitter = SyncProgressEmitter::new(app_handle);
///
/// // 发送进度
/// emitter.emit(SyncProgress {
///     account_id: 1,
///     stage: SyncStage::SyncingEmails,
///     folder: Some("INBOX".to_string()),
///     current: 10,
///     total: 100,
///     message: "正在同步邮件...".to_string(),
/// });
/// ```
#[derive(Clone)]
pub struct SyncProgressEmitter {
    /// 事件通道发送端：UI 持有对应 Receiver
    event_tx: SyncEventTx,
}

impl SyncProgressEmitter {
    /// 创建新的进度发射器
    ///
    /// - `event_tx`: 同步进度事件通道发送端
    pub fn new(event_tx: SyncEventTx) -> Self {
        Self { event_tx }
    }

    /// 发送同步进度事件
    ///
    /// 通道满或接收端已关闭时静默丢弃，不影响同步流程。
    pub fn emit(&self, progress: SyncProgress) {
        let event = SyncProgressEvent { progress };
        let _ = self.event_tx.try_send(event);
    }
}

/// 同步阶段枚举
///
/// 定义同步过程中的各个阶段，用于进度通知和状态展示。
///
/// # 阶段说明
///
/// - `Connecting`: 正在连接到 IMAP 服务器
/// - `SyncingFolders`: 正在获取文件夹列表
/// - `SyncingEmails`: 正在同步邮件内容
/// - `Completed`: 同步完成
/// - `Error`: 同步出错
///
/// # 状态转换
///
/// ```text
/// Connecting → SyncingFolders → SyncingEmails → Completed
///                 ↓                  ↓
///               Error              Error
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SyncStage {
    /// 正在连接到 IMAP 服务器
    Connecting,
    /// 正在同步文件夹列表
    SyncingFolders,
    /// 正在同步邮件内容
    SyncingEmails,
    /// 同步完成
    Completed,
    /// 同步出错
    Error,
}

/// 同步进度信息
///
/// 包含同步过程中的详细进度信息，用于前端展示同步状态。
///
/// # 字段说明
///
/// - `account_id`: 正在同步的账号 ID
/// - `stage`: 当前同步阶段
/// - `folder`: 当前正在同步的文件夹（可选）
/// - `folder_display_name`: 当前文件夹的可读展示名（可选）
/// - `current`: 当前进度值（已处理的邮件数）
/// - `total`: 总数量（当前文件夹的总邮件数）
/// - `message`: 进度描述消息
///
/// # 使用示例
///
/// ```rust,ignore
/// let progress = SyncProgress {
///     account_id: 1,
///     stage: SyncStage::SyncingEmails,
///     folder: Some("INBOX".to_string()),
///     folder_display_name: Some("INBOX".to_string()),
///     current: 25,
///     total: 100,
///     message: "正在同步 INBOX...".to_string(),
/// };
/// emitter.emit(progress);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncProgress {
    /// 正在同步的账号 ID
    pub account_id: i32,
    /// 当前同步阶段
    pub stage: SyncStage,
    /// 当前正在同步的文件夹名称（可选）
    pub folder: Option<String>,
    /// 当前文件夹的可读展示名（可选）
    pub folder_display_name: Option<String>,
    /// 当前进度值（已处理的数量）
    pub current: usize,
    /// 总数量（当前任务的总数）
    pub total: usize,
    /// 进度描述消息
    pub message: String,
}

/// 同步结果统计
///
/// 记录一次同步操作的统计信息，包括新增、更新、删除的邮件数量和耗时。
///
/// # 字段说明
///
/// - `new_emails`: 新增的邮件数量
/// - `updated_emails`: 更新的邮件数量（如标志变化）
/// - `deleted_emails`: 删除的邮件数量
/// - `duration_ms`: 同步耗时（毫秒）
///
/// # 使用示例
///
/// ```rust,ignore
/// let result = SyncResult {
///     new_emails: 10,
///     updated_emails: 5,
///     deleted_emails: 2,
///     duration_ms: 3500,
/// };
/// println!("同步完成: 新增 {} 封, 更新 {} 封, 删除 {} 封, 耗时 {} ms",
///     result.new_emails, result.updated_emails, result.deleted_emails, result.duration_ms);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncResult {
    /// 新增的邮件数量
    pub new_emails: usize,
    /// 更新的邮件数量（如标志变化）
    pub updated_emails: usize,
    /// 删除的邮件数量
    pub deleted_emails: usize,
    /// 同步耗时（毫秒）
    pub duration_ms: u64,
}

/// 文件夹统计信息
///
/// 记录单个文件夹的邮件统计信息，用于在侧边栏显示文件夹状态。
///
/// # 字段说明
///
/// - `folder`: 文件夹名称（如 "INBOX", "Sent"）
/// - `total`: 文件夹中的邮件总数
/// - `unread`: 未读邮件数量
///
/// # 使用示例
///
/// ```rust,ignore
/// let stat = FolderStat {
///     folder: "INBOX".to_string(),
///     total: 150,
///     unread: 5,
/// };
/// println!("{}: {} 封邮件, {} 封未读", stat.folder, stat.total, stat.unread);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderStat {
    /// 文件夹名称
    pub folder: String,
    /// 邮件总数
    pub total: usize,
    /// 未读邮件数量
    pub unread: usize,
}

/// 同步模式枚举
///
/// 定义不同的同步策略，根据同步场景选择合适的模式。
///
/// # 变体说明
///
/// - `Full`: 全量同步，下载文件夹中的所有邮件
/// - `Incremental`: 增量同步，只同步新增的邮件
///
/// # 选择策略
///
/// - 首次同步：使用 `Full` 模式
/// - UIDVALIDITY 变化：使用 `Full` 模式（服务器重建）
/// - 定期同步：使用 `Incremental` 模式
/// - 手动刷新：使用 `Incremental` 模式
///
/// # UIDVALIDITY 说明
///
/// UIDVALIDITY 是 IMAP 服务器为每个文件夹分配的唯一标识符。
/// 当服务器重建或文件夹重新创建时，UIDVALIDITY 会发生变化，
/// 此时必须进行全量同步，因为之前的 UID 已经失效。
#[derive(Debug, Clone)]
pub enum SyncMode {
    /// 全量同步模式
    ///
    /// 下载文件夹中的所有邮件，建立完整的本地索引。
    ///
    /// # 字段说明
    ///
    /// - `uidvalidity`: IMAP 文件夹的 UIDVALIDITY 值，
    ///   用于下次同步时判断是否需要重新全量同步
    Full {
        /// IMAP 文件夹的 UIDVALIDITY 值
        uidvalidity: u64,
        /// IMAP UIDNEXT 值，用于空窗口首次同步时建立安全高水位
        uidnext: u64,
    },
    /// 增量同步模式
    ///
    /// 基于上次同步的最高 UID，只获取新增的邮件。
    ///
    /// # 字段说明
    ///
    /// - `last_sync_uid`: 上次同步的最高 UID，
    ///   同步时会获取 UID 大于此值的所有邮件
    Incremental {
        /// 上次同步的最高 UID
        last_sync_uid: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_batch_size_should_default_to_fifty() {
        assert_eq!(HISTORY_UID_BATCH_SIZE, 50);
    }

    #[test]
    fn history_exhausted_before_uid_should_be_true_for_first_uid() {
        assert!(history_exhausted_before_uid(1));
        assert!(history_exhausted_before_uid(0));
        assert!(!history_exhausted_before_uid(2));
    }

    #[test]
    fn next_history_before_uid_should_move_to_minimum_batch_uid() {
        assert_eq!(next_history_before_uid(&[41, 39, 40], 50), 39);
    }

    #[test]
    fn next_history_before_uid_should_keep_previous_when_batch_empty() {
        assert_eq!(next_history_before_uid(&[], 50), 50);
    }

    // ─────────────────────────────────────────────────────────────────
    // Task 7: 收件路径联系人自动收集
    // ─────────────────────────────────────────────────────────────────

    use crate::infrastructure::protocols::types::EmailFlags;

    fn test_flags() -> EmailFlags {
        EmailFlags {
            seen: false,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
            recent: false,
        }
    }

    fn test_header(
        from: &str,
        to: &str,
        cc: &str,
        date: chrono::DateTime<chrono::Utc>,
    ) -> EmailHeader {
        EmailHeader {
            uid: 1,
            subject: "contact collect".into(),
            from: from.into(),
            to: to.into(),
            cc: cc.into(),
            date,
            flags: test_flags(),
            attachments: vec![],
        }
    }

    #[test]
    fn split_display_addresses_should_extract_bare_and_angle_forms() {
        let raw = "Alice <alice@example.com>, b@example.com; Carol <  carol@example.com > ,, ;";
        assert_eq!(
            split_display_addresses(raw),
            vec![
                "alice@example.com".to_string(),
                "b@example.com".to_string(),
                "carol@example.com".to_string(),
            ]
        );
        assert!(split_display_addresses("  ").is_empty());
    }

    #[tokio::test]
    async fn headers_to_contacts_should_collect_sender_recipients_and_cc() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let date =
            chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("测试时间戳应可转换");
        let headers = vec![test_header(
            "Alice <alice@example.com>",
            "bob@example.com; Dave <dave@example.com>",
            "  ,carol@example.com,",
            date,
        )];

        headers_to_contacts(&db, &headers).await;

        for email in [
            "alice@example.com",
            "bob@example.com",
            "carol@example.com",
            "dave@example.com",
        ] {
            let contact = contact_repo::get_by_email(&db, email)
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("contacts 应包含 {email}"));
            assert_eq!(contact.source, "collected");
            assert_eq!(contact.last_contacted_at, Some(1_700_000_000));
        }
    }

    #[tokio::test]
    async fn headers_to_contacts_should_increment_frequency_on_resync() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let date =
            chrono::DateTime::from_timestamp(1_700_000_100, 0).expect("测试时间戳应可转换");
        let headers = vec![test_header("peer@example.com", "", "", date)];

        headers_to_contacts(&db, &headers).await;
        headers_to_contacts(&db, &headers).await;

        let contact = contact_repo::get_by_email(&db, "peer@example.com")
            .await
            .unwrap()
            .expect("再次同步后联系人应存在");
        assert_eq!(contact.frequency, 2, "重复收集应走 frequency+=1 增量规则");
    }

    #[tokio::test]
    async fn whole_emails_to_contacts_should_carry_sender_name() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let emails = vec![WholeEmailDto {
            id: 0,
            account_id: 1,
            folder: "INBOX".into(),
            uid: 7,
            message_id: None,
            sender_name: Some("Alice Sender".into()),
            sender_email: "alice@example.com".into(),
            recipient_emails: "bob@example.com, Erin <erin@example.com>".into(),
            cc_emails: Some("carol@example.com".into()),
            bcc_emails: None,
            subject: None,
            preview: None,
            body_text: None,
            body_html: None,
            attachments: vec![],
            is_read: false,
            is_starred: false,
            is_draft: false,
            is_answered: false,
            is_deleted: false,
            sent_at: 1_700_000_200,
            received_at: 1_700_000_200,
            created_at: 1_700_000_200,
        }];

        whole_emails_to_contacts(&db, &emails).await;

        let alice = contact_repo::get_by_email(&db, "alice@example.com")
            .await
            .unwrap()
            .expect("发件人应被收集");
        assert_eq!(alice.name.as_deref(), Some("Alice Sender"));
        assert_eq!(alice.last_contacted_at, Some(1_700_000_200));
        for email in ["bob@example.com", "erin@example.com", "carol@example.com"] {
            assert!(
                contact_repo::get_by_email(&db, email).await.unwrap().is_some(),
                "contacts 应包含 {email}"
            );
        }
    }
}
