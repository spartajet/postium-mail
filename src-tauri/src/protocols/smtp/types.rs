//! SMTP 数据类型定义
//!
//! 定义 SMTP 邮件发送中使用的数据结构。
//!
//! # 核心类型
//!
//! - [`EmailAddress`][]: 邮件地址（包含邮箱和可选的显示名称）
//! - [`EmailAttachment`][]: 邮件附件
//! - [`SendEmailRequest`][]: 发送邮件请求
//! - [`SendEmailResult`][]: 发送邮件结果
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use crate::protocols::smtp::types::{EmailAddress, SendEmailRequest, EmailAttachment};
//!
//! // 创建邮件地址
//! let addr = EmailAddress::with_name("user@example.com", "张三");
//! // 使用 addr 进行后续操作
//!
//! // 构建发送请求
//! let request = SendEmailRequest {
//!     from: "sender@example.com".to_string(),
//!     to: vec!["recipient@example.com".to_string()],
//!     cc: Some(vec!["cc@example.com".to_string()]),
//!     bcc: None,
//!     subject: "测试邮件".to_string(),
//!     html_body: "<h1>HTML 内容</h1>".to_string(),
//!     text_body: Some("纯文本内容".to_string()),
//!     attachments: vec![],
//! };
//! ```

use serde::{Deserialize, Serialize};

/// 邮件地址
///
/// 表示一个邮件地址，包含邮箱地址和可选的显示名称。
///
/// # 字段说明
///
/// - `email`: 邮箱地址（如 "user@example.com"）
/// - `name`: 显示名称（如 "张三"），可选
///
/// # 格式
///
/// 显示格式遵循 RFC 5322 标准：
///
/// - 有名称: `"张三" <user@example.com>` 或 `张三 <user@example.com>`
/// - 无名称: `user@example.com`
///
/// # 示例
///
/// ```rust
/// use crate::protocols::smtp::types::EmailAddress;
///
/// // 无名称
/// let addr = EmailAddress::new("user@example.com");
/// assert_eq!(format!("{}", addr), "user@example.com");
///
/// // 有名称
/// let addr = EmailAddress::with_name("user@example.com", "张三");
/// assert_eq!(format!("{}", addr), "张三 <user@example.com>");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    pub email: String,
    pub name: Option<String>,
}

impl EmailAddress {
    /// 创建新的邮件地址
    ///
    /// # 参数
    ///
    /// - `email`: 邮箱地址
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: None,
        }
    }

    /// 创建带名称的邮件地址
    ///
    /// # 参数
    ///
    /// - `email`: 邮箱地址
    /// - `name`: 显示名称
    pub fn with_name(email: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: Some(name.into()),
        }
    }
}

impl std::fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{} <{}>", name, self.email),
            None => write!(f, "{}", self.email),
        }
    }
}

/// 邮件附件
///
/// 表示要附加到邮件的文件。
///
/// # 字段说明
///
/// - `filename`: 附件文件名
/// - `content_type`: MIME 内容类型（如 "application/pdf"）
/// - `size`: 附件大小（字节）
/// - `data`: 附件二进制数据
///
/// # 常见内容类型
///
/// | 文件类型 | Content-Type |
/// |---------|-------------|
/// | PDF | application/pdf |
/// | 图片 (JPEG) | image/jpeg |
/// | 图片 (PNG) | image/png |
/// | 文本 | text/plain |
/// | Word | application/msword |
/// | Excel | application/vnd.ms-excel |
///
/// # 注意事项
///
/// - 大多数 SMTP 服务器限制附件大小（通常 10-25 MB）
/// - 某些文件类型可能被安全策略拦截
/// - 文件名应使用 ASCII 编码或 RFC 2047 编码
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub data: Vec<u8>,
}

/// 发送邮件请求
///
/// 包含发送邮件所需的所有信息。
///
/// # 字段说明
///
/// ## 基本信息
///
/// - `from`: 发件人邮箱地址
/// - `to`: 收件人邮箱地址列表
/// - `cc`: 抄送人邮箱地址列表（可选）
/// - `bcc`: 密送人邮箱地址列表（可选）
/// - `subject`: 邮件主题
///
/// ## 邮件内容
///
/// - `html_body`: HTML 正文（必需）
/// - `text_body`: 纯文本正文（可选，推荐提供）
/// - `attachments`: 附件列表
///
/// # 内容建议
///
/// - **HTML 正文**: 现代邮件客户端的主要展示格式
/// - **纯文本正文**: 为不支持 HTML 的客户端提供降级方案
/// - **两者都提供**: 确保最佳兼容性
///
/// # 注意事项
///
/// - 主题行应简洁明了（建议不超过 78 字符）
/// - 收件人列表至少需要一个有效地址
/// - BCC 收件人不会出现在邮件头中
/// - 附件大小受 SMTP 服务器限制
///
/// # 示例
///
/// ```rust
/// use crate::protocols::smtp::types::SendEmailRequest;
///
/// let request = SendEmailRequest {
///     from: "sender@example.com".to_string(),
///     to: vec!["recipient@example.com".to_string()],
///     cc: Some(vec!["cc@example.com".to_string()]),
///     bcc: None,
///     subject: "会议邀请".to_string(),
///     html_body: "<h1>会议邀请</h1><p>请参加下周一的会议。</p>".to_string(),
///     text_body: Some("会议邀请\n\n请参加下周一的会议。".to_string()),
///     attachments: vec![],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    /// 发件人
    pub from: String,
    /// 收件人列表
    pub to: Vec<String>,
    /// 抄送人列表
    pub cc: Option<Vec<String>>,
    /// 密送列表
    pub bcc: Option<Vec<String>>,
    /// 主题
    pub subject: String,
    /// HTML 正文
    pub html_body: String,
    /// 纯文本正文
    pub text_body: Option<String>,
    /// 附件
    pub attachments: Vec<EmailAttachment>,
}

/// 发送邮件结果
///
/// 包含邮件发送成功后的返回信息。
///
/// # 字段说明
///
/// - `message_id`: 服务器分配的 Message-ID
/// - `sent_at`: 发送时间（UTC）
///
/// # Message-ID 说明
///
/// - Message-ID 是邮件的唯一标识符
/// - 格式通常为: `<local-id@domain>`
/// - 可用于追踪邮件传递状态
/// - 可用于引用邮件（如回复、转发）
///
/// # 示例
///
/// ```text
/// Message-ID: <abc123@smtp.gmail.com>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailResult {
    /// Message-ID
    pub message_id: String,
    /// 发送时间
    pub sent_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_address() {
        let addr = EmailAddress::new("test@example.com");
        assert_eq!(addr.email, "test@example.com");
        assert!(addr.name.is_none());

        let formatted = format!("{}", addr);
        assert_eq!(formatted, "test@example.com");
    }

    #[test]
    fn test_email_address_with_name() {
        let addr = EmailAddress::with_name("test@example.com", "Test User");
        assert_eq!(addr.email, "test@example.com");
        assert_eq!(addr.name.as_ref().unwrap(), "Test User");

        let formatted = format!("{}", addr);
        assert_eq!(formatted, "Test User <test@example.com>");
    }

    #[test]
    fn test_send_email_request() {
        let request = SendEmailRequest {
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            cc: None,
            bcc: None,
            subject: "Test".to_string(),
            html_body: "<p>Test</p>".to_string(),
            text_body: Some("Test".to_string()),
            attachments: vec![],
        };

        assert_eq!(request.subject, "Test");
    }
}
