use chrono::{DateTime, Utc};

/// 文件夹 IMAP 元数据
///
/// 包含文件夹的 IMAP 协议元数据，用于增量同步和状态管理。
///
/// # 字段说明
///
/// ## UID 相关
/// - `uidvalidity`: UIDVALIDITY 值，文件夹 UID 的有效性标识符
/// - `uidnext`: 预期的下一个 UID
///
/// ## 统计信息
/// - `exists`: 文件夹中存在的邮件数量
/// - `recent`: 最近邮件数量（\Recent 标志）
/// - `unseen`: 未读邮件数量（如果可用）
///
/// # UIDVALIDITY 说明
///
/// UIDVALIDITY 是一个递增的数字，每当文件夹的内容
/// 发生不可逆变化时会改变（如文件夹被删除后重新创建）。
/// 如果 UIDVALIDITY 改变，之前存储的 UID 不再有效。
#[derive(Debug, Clone)]
pub struct FolderMetadata {
    /// IMAP UIDVALIDITY 值 - 文件夹 UID 的有效性标识符
    pub uidvalidity: u64,
    /// 预期的下一个 UID
    pub uidnext: u64,
    /// 文件夹中存在的邮件数量
    pub exists: u32,
    /// 最近邮件数量
    pub recent: u32,
    /// 未读邮件数量（如果可用）
    pub unseen: Option<u32>,
    /// 昵称（解码后的可读名称，用于非英文字符文件夹）
    pub nick_name: String,
}

/// 邮件头信息（仅包含邮件头，用于骨架同步）
///
/// 轻量级的邮件表示，仅包含邮件头和标志信息。
/// 适用于列表展示，不需要下载完整的邮件正文。
///
/// # 字段说明
///
/// 与 `EmailData` 相比，缺少以下字段：
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 正文
/// - `raw`: 原始邮件源码
/// - `attachments`: 附件列表
///
/// # 性能优势
///
/// - 减少数据传输量
/// - 加快列表加载速度
/// - 降低内存使用
///
/// # 使用场景
///
/// - 邮件列表展示
/// - 搜索结果预览
/// - 骨架同步（Skeleton Sync）
#[derive(Debug, Clone)]
pub struct EmailHeader {
    /// IMAP UID
    pub uid: u32,
    /// 邮件主题
    pub subject: String,
    /// 发件人
    pub from: String,
    /// 收件人
    pub to: String,
    /// 抄送
    pub cc: String,
    /// 邮件日期
    pub date: DateTime<Utc>,
    /// 邮件标志
    pub flags: EmailFlags,
    /// 附件元信息列表（从 BODYSTRUCTURE 解析）
    pub attachments: Vec<AttachmentInfo>,
}

/// 邮件标志
///
/// 表示 IMAP 邮件系统标志（System Flags）和用户关键字。
///
/// # IMAP 标志说明
///
/// 根据 RFC 3501，系统标志包括：
/// - `\Seen`: 邮件已被阅读
/// - `\Answered`: 邮件已被回复
/// - `\Flagged`: 邮件已被标记为重要
/// - `\Deleted`: 邮件已标记为删除
/// - `\Draft`: 邮件是草稿
/// - `\Recent`: 邮件是最近的（此会话中新到达的）
///
/// # 字段说明
///
/// - `seen`: 对应 `\Seen` 标志，邮件是否已读
/// - `flagged`: 对应 `\Flagged` 标志，邮件是否被星标/标记
/// - `answered`: 对应 `\Answered` 标志，邮件是否已回复
/// - `deleted`: 对应 `\Deleted` 标志，邮件是否被标记删除
#[derive(Debug, Clone)]
pub struct EmailFlags {
    /// 邮件是否已读（\Seen）
    pub seen: bool,
    /// 邮件是否被标记/星标（\Flagged）
    pub flagged: bool,
    /// 邮件是否已回复（\Answered）
    pub answered: bool,
    /// 邮件是否被标记删除（\Deleted）
    pub deleted: bool,
    /// 邮件是否被标记草稿 （\Draft）
    pub draft: bool,
    ///邮件是否被标记最近的 （\Recent）
    pub recent: bool,
}

/// 附件元信息（从 BODYSTRUCTURE 解析）
///
/// 仅包含附件的元数据，不包含附件的实际内容。
/// 通过 `section_path` 可以后续按需下载附件内容。
#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    /// 附件文件名（某些附件可能没有文件名）
    pub filename: Option<String>,
    /// MIME Content-Type，如 "application/pdf"、"image/png"
    pub content_type: String,
    /// 附件大小（字节）
    pub size: u32,
    /// 附件在 MIME 树中的 section 路径，如 "2"、"3.1"
    /// 用于后续通过 UID FETCH BODY.PEEK[<section>] 按需下载
    pub section_path: String,
    /// Content-Disposition，如 "attachment"、"inline"
    pub disposition: Option<String>,
    /// Content-ID，用于 HTML 邮件内嵌图片的 cid: 引用
    pub content_id: Option<String>,
}

/// 邮件信封信息
///
/// 从 IMAP ENVELOPE 响应中提取的邮件头信息。
#[derive(Debug, Clone)]
pub struct MailEnvelope {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub date: chrono::DateTime<chrono::Utc>,
}

/// 完整邮件 DTO
///
/// 包含邮件的所有字段，用于邮件详情展示和完整数据传输。
/// 对应 `emails` 表的全部列以及关联的附件列表。
///
/// # 与其他结构体的关系
///
/// - `EmailHeader`: 轻量级邮件头，仅用于骨架同步
/// - `MailEnvelope`: IMAP 信封信息，仅包含邮件头地址信息
/// - `EmailDto`（本结构体）: 完整邮件数据，包含正文、附件等所有字段
///
/// # 字段分组
///
/// ## 标识
/// - `id`: 数据库主键
/// - `account_id`: 所属账户
/// - `folder`: 所属文件夹
/// - `uid`: IMAP UID
/// - `message_id`: RFC 2822 Message-ID
///
/// ## 发件人/收件人
/// - `sender_name`: 发件人显示名
/// - `sender_email`: 发件人邮箱地址
/// - `recipient_emails`: 收件人列表
/// - `cc_emails`: 抄送列表
/// - `bcc_emails`: 密送列表
///
/// ## 内容
/// - `subject`: 邮件主题
/// - `preview`: 预览文本
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 正文
/// - `attachments`: 附件元信息列表
///
/// ## 标志
/// - `is_read`: 是否已读
/// - `is_starred`: 是否星标
/// - `is_draft`: 是否草稿
/// - `is_answered`: 是否已回复
/// - `is_deleted`: 是否已删除
///
/// ## 时间戳
/// - `sent_at`: 发送时间（Unix 毫秒）
/// - `received_at`: 接收时间（Unix 毫秒）
/// - `created_at`: 创建时间（Unix 毫秒）
/// - `updated_at`: 更新时间（Unix 毫秒）
#[derive(Debug, Clone)]
pub struct WholeEmailDto {
    // ─── 标识 ───
    /// 数据库主键
    pub id: i32,
    /// 所属账户 ID
    pub account_id: i32,
    /// 所属文件夹名称
    pub folder: String,
    /// IMAP UID
    pub uid: u32,
    /// RFC 2822 Message-ID 邮件头
    pub message_id: Option<String>,

    // ─── 发件人/收件人 ───
    /// 发件人显示名
    pub sender_name: Option<String>,
    /// 发件人邮箱地址
    pub sender_email: String,
    /// 收件人邮箱列表（逗号分隔）
    pub recipient_emails: String,
    /// 抄送邮箱列表（逗号分隔）
    pub cc_emails: Option<String>,
    /// 密送邮箱列表（逗号分隔）
    pub bcc_emails: Option<String>,

    // ─── 内容 ───
    /// 邮件主题
    pub subject: Option<String>,
    /// 预览文本（正文前若干字符）
    pub preview: Option<String>,
    /// 纯文本正文
    pub body_text: Option<String>,
    /// HTML 正文
    pub body_html: Option<String>,
    /// 附件元信息列表
    pub attachments: Vec<AttachmentInfo>,

    // ─── 标志 ───
    /// 是否已读
    pub is_read: bool,
    /// 是否星标
    pub is_starred: bool,
    /// 是否草稿
    pub is_draft: bool,
    /// 是否已回复
    pub is_answered: bool,
    /// 是否已删除
    pub is_deleted: bool,

    // ─── 时间戳（Unix 毫秒） ───
    /// 发送时间
    pub sent_at: i64,
    /// 接收时间
    pub received_at: i64,
    /// 数据库创建时间
    pub created_at: i64,
}
