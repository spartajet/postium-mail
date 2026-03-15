use chrono::{DateTime, Utc};

/// IMAP 认证方法
#[derive(Debug, Clone)]
pub enum ImapAuth {
    Password(String),
}

/// 邮件标志
#[derive(Debug, Clone)]
pub struct EmailFlags {
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub deleted: bool,
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
