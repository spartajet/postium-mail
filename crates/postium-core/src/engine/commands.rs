//! 引擎命令与事件。

use crate::error::MailError;
use tokio::sync::oneshot;

/// 引擎命令（账号内串行执行）。
#[derive(Debug)]
pub enum EngineCommand {
    /// 立即同步指定文件夹（None = 全部订阅文件夹）。阶段 2 实装同步体。
    SyncNow {
        folder: Option<String>,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 读取邮件正文（懒加载）。
    FetchBody {
        folder: String,
        uid: u32,
        reply: oneshot::Sender<Result<String, MailError>>,
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
