use chrono::{DateTime, Utc};

// 注意：ImapAuth 已移至 auth.rs 模块

/// 邮件标志
#[derive(Debug, Clone)]
pub struct EmailFlags {
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub deleted: bool,
}

/// 附件信息
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
}

/// 邮件数据
#[derive(Debug, Clone)]
pub struct EmailData {
    pub uid: u32,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub date: DateTime<Utc>,
    pub body_text: String,
    pub body_html: String,
    pub raw: String,
    pub flags: EmailFlags,
    pub attachments: Vec<EmailAttachment>,
}

/// RFC 6154 Special-Use Mailboxes 属性
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUse {
    /// 所有邮件 (\All)
    All,
    /// 归档 (\Archive)
    Archive,
    /// 草稿 (\Drafts)
    Drafts,
    /// 已标记/星标 (\Flagged)
    Flagged,
    /// 垃圾邮件 (\Junk)
    Junk,
    /// 已发送 (\Sent)
    Sent,
    /// 已删除 (\Trash)
    Trash,
}

/// 文件夹信息（包含 RFC 6154 special-use 属性）
#[derive(Debug, Clone)]
pub struct FolderInfo {
    /// 文件夹名称（原始 IMAP 名称）
    pub name: String,
    /// RFC 6154 special-use 属性（如果有）
    pub special_use: Option<SpecialUse>,
    /// 标准名称（基于 special-use 或名称映射）
    pub standard_name: String,
}

/// 文件夹 IMAP 元数据
#[derive(Debug, Clone)]
pub struct FolderMetadata {
    /// IMAP UIDVALIDITY 值 - 文件夹 UID 的有效性标识符
    pub uidvalidity: u64,
    /// 预期的下一个 UID
    pub uidnext: u64,
    /// CONDSTORE 扩展的最高修改序列号（如果支持）
    pub highest_modseq: Option<u64>,
    /// 文件夹中存在的邮件数量
    pub exists: u32,
    /// 最近邮件数量
    pub recent: u32,
    /// 未读邮件数量（如果可用）
    pub unseen: Option<u32>,
}

/// 邮件头信息（仅包含邮件头，用于骨架同步）
#[derive(Debug, Clone)]
pub struct EmailHeader {
    pub uid: u32,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub date: DateTime<Utc>,
    pub flags: EmailFlags,
}

// ========== IMAP IDLE 支持 (RFC 2177) ==========

/// IDLE 事件
///
/// 表示服务器通过 IDLE 推送的变更事件
#[derive(Clone, Debug, PartialEq)]
pub enum IdleEvent {
    /// 新邮件到达
    NewEmail {
        folder: String,
        uid: u32,
    },
    /// 邮件标志变更
    FlagsChanged {
        folder: String,
        uid: u32,
        flags: Vec<String>,
    },
    /// 邮件被删除
    EmailDeleted {
        folder: String,
        uid: u32,
    },
    /// 连接断开
    Disconnected,
    /// 发生错误
    Error(String),
}

/// IDLE 状态
#[derive(Clone, Debug, PartialEq)]
pub enum IdleState {
    /// 未连接
    Disconnected,
    /// 已连接，未进入 IDLE
    Connected,
    /// IDLE 模式激活中
    IdleActive,
    /// IDLE 暂停（处理服务器响应）
    IdlePaused,
    /// 正在重连
    Reconnecting,
}

/// 重连配置
#[derive(Clone, Debug)]
pub struct ReconnectConfig {
    /// 是否启用自动重连
    pub enabled: bool,
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始重连延迟（秒）
    pub initial_delay_secs: u64,
    /// 最大重连延迟（秒）
    pub max_delay_secs: u64,
    /// 退避指数
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 10,
            initial_delay_secs: 5,
            max_delay_secs: 300,  // 5分钟
            backoff_multiplier: 2.0,
        }
    }
}

/// IDLE 句柄
///
/// 用于控制 IDLE 连接的生命周期
#[derive(Clone)]
pub struct IdleHandle {
    /// 文件夹名称
    pub folder: String,
    /// 是否处于活跃状态
    pub active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 账号ID
    pub account_id: i32,
}

impl IdleHandle {
    /// 创建新的 IDLE 句柄
    pub fn new(folder: String, account_id: i32) -> Self {
        Self {
            folder,
            active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            account_id,
        }
    }

    /// 停止 IDLE
    pub fn stop(&self) {
        self.active.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// 检查是否活跃
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }
}
