/// 邮件数据模型 — 对应数据库 emails 表
///
/// 存储从 IMAP 服务器同步的邮件数据，包含邮件元信息、正文和状态标志。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 邮件唯一 ID（自增主键）
    pub id: i32,
    /// 所属账号 ID（外键关联 accounts 表）
    pub account_id: i32,
    /// 邮件所在文件夹（如 "INBOX"、"Sent"）
    pub folder: String,
    /// IMAP UID（文件夹内唯一）
    pub uid: u32,
    /// 邮件 Message-ID 头（全局唯一标识）
    pub message_id: Option<String>,
    /// 邮件主题
    pub subject: Option<String>,
    /// 发件人名称
    pub sender_name: Option<String>,
    /// 发件人邮箱地址
    pub sender_email: String,
    /// 收件人邮箱地址列表（逗号分隔）
    pub recipient_emails: String,
    /// 抄送邮箱地址列表（逗号分隔）
    pub cc_emails: Option<String>,
    /// 密送邮箱地址列表（逗号分隔）
    pub bcc_emails: Option<String>,
    /// 邮件预览文本（正文的精简摘要）
    pub preview: Option<String>,
    /// 纯文本正文
    pub body_text: Option<String>,
    /// HTML 正文
    pub body_html: Option<String>,
    /// 是否已读
    pub is_read: Option<bool>,
    /// 是否星标
    pub is_starred: Option<bool>,
    /// 是否草稿
    pub is_draft: Option<bool>,
    /// 是否已回复
    pub is_answered: Option<bool>,
    /// 是否已删除（标记为删除）
    pub is_deleted: Option<bool>,
    /// 发送时间戳（Unix 毫秒）
    pub sent_at: i64,
    /// 接收时间戳（Unix 毫秒）
    pub received_at: i64,
    /// 创建时间戳（Unix 毫秒，本地入库时间）
    pub created_at: i64,
    /// 最后更新时间戳（Unix 毫秒）
    pub updated_at: i64,
}
