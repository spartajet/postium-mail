//! 引擎命令与事件。

use crate::error::MailError;
use crate::infrastructure::protocols::types::{FetchedBodySection, WholeEmailDto};
use tokio::sync::oneshot;

/// 引擎命令（账号内串行执行）。
#[derive(Debug)]
pub enum EngineCommand {
    /// 立即同步指定文件夹（None = 全部订阅文件夹）。阶段 2 实装同步体。
    SyncNow {
        folder: Option<String>,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 获取邮件正文原文（BODY.PEEK[]）。
    FetchBody {
        folder: String,
        uid: u32,
        reply: oneshot::Sender<Result<String, MailError>>,
    },
    /// 移动邮件到其他文件夹（服务器支持 MOVE 扩展时用 MOVE，否则 COPY+DELETE）。
    Move {
        folder: String,
        uid: u32,
        to_folder: String,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 设置 flags（替换语义：最终 flags = flags）。
    SetFlags {
        folder: String,
        uid: u32,
        flags: Vec<String>,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 添加单条 flag（STORE +FLAGS，不动其他标志；已读/星标切换用）。
    AddFlags {
        folder: String,
        uid: u32,
        flag: String,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 移除单条 flag（STORE -FLAGS，不动其他标志）。
    RemoveFlags {
        folder: String,
        uid: u32,
        flag: String,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 追加邮件到文件夹（发信归档/草稿保存到服务器用）。
    Append {
        folder: String,
        email: String, // RFC822 原文
        flags: Vec<String>,
        /// 传入时追加后按 Message-ID SEARCH 确认 UID（APPEND 响应不回带 UID）；
        /// 找不到时报 ImapSearchFailed。
        message_id: Option<String>,
        /// 返回新 UID（未传 message_id 时为 0）。
        reply: oneshot::Sender<Result<u32, MailError>>,
    },
    /// 删除邮件（\Deleted + EXPUNGE）。
    Delete {
        folder: String,
        uid: u32,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 按 UID 拉取一封完整邮件（含正文与附件元信息），用于"刷新"。
    FetchEmailByUid {
        folder: String,
        uid: u32,
        reply: oneshot::Sender<Result<Option<WholeEmailDto>, MailError>>,
    },
    /// 下载指定 MIME section 的附件原始字节（带 Content-Transfer-Encoding）。
    FetchBodySection {
        folder: String,
        uid: u32,
        section_path: String,
        reply: oneshot::Sender<Result<Option<FetchedBodySection>, MailError>>,
    },
    /// 关闭引擎（账号删除/退出）。
    Shutdown,
}

/// 引擎事件（转发给 UI 层）。
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Progress { account_id: i32, message: String },
    Disconnected { account_id: i32 },
    CredentialsNeeded { account_id: i32 },
}
