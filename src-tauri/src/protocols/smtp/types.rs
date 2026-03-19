//! SMTP 数据类型定义

use serde::{Deserialize, Serialize};

/// 邮件地址
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    pub email: String,
    pub name: Option<String>,
}

impl EmailAddress {
    /// 创建新的邮件地址
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: None,
        }
    }

    /// 创建带名称的邮件地址
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub data: Vec<u8>,
}

/// 发送邮件请求
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
