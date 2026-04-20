use crate::infrastructure::protocols::types::AttachmentInfo;
use async_imap::imap_proto;

use mail_parser::MessageParser;

// ──────────────────────────────────────────────
// mail_parser 辅助函数
// ──────────────────────────────────────────────

/// 使用 mail_parser 解析后的邮件头部信息
#[derive(Debug, Clone)]
pub struct ParsedHeaders {
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    /// "Name <email>" 格式的发件人显示字符串
    pub from_display: String,
    /// "Name <email>" 格式的收件人显示字符串
    pub to_display: String,
    /// 逗号分隔的收件人邮箱
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub bcc_emails: Option<String>,
    pub message_id: Option<String>,
    /// Unix 时间戳（秒）
    pub sent_at: i64,
}

/// 使用 mail_parser 解析原始 RFC822 数据，提取邮件头字段
///
/// `fallback_ts`：当邮件中无 Date 头时使用的时间戳
pub fn parse_headers_from_raw(raw: &[u8], fallback_ts: i64) -> Option<ParsedHeaders> {
    let msg = MessageParser::default().parse(raw)?;
    Some(extract_headers_from_message(&msg, fallback_ts))
}

/// 从已解析的 mail_parser Message 中提取邮件头字段
///
/// 供调用方在已经拥有 `Message` 对象时使用，避免重复解析。
pub fn extract_headers_from_message(
    msg: &mail_parser::Message<'_>,
    fallback_ts: i64,
) -> ParsedHeaders {
    let subject = msg.subject().map(|s| s.to_string());
    let message_id = msg.message_id().map(|s| s.to_string());
    let sent_at = msg.date().map(|d| d.to_timestamp()).unwrap_or(fallback_ts);

    // From
    let sender_name = msg
        .from()
        .and_then(|a| a.first().and_then(|p| p.name().map(|n| n.to_string())));
    let sender_email = msg
        .from()
        .and_then(|a| a.first().and_then(|p| p.address().map(|a| a.to_string())))
        .unwrap_or_default();
    let from_display = format_mp_address_display(msg.from());

    // To
    let to_display = format_mp_address_display(msg.to());
    let recipient_emails = format_mp_address_emails(msg.to());

    // CC / BCC
    let cc_emails = format_mp_address_opt(msg.cc());
    let bcc_emails = format_mp_address_opt(msg.bcc());

    ParsedHeaders {
        subject,
        sender_name,
        sender_email,
        from_display,
        to_display,
        recipient_emails,
        cc_emails,
        bcc_emails,
        message_id,
        sent_at,
    }
}

/// 将 mail_parser 地址格式化为 "Name <email>" 显示字符串（空时返回空字符串）
fn format_mp_address_display(addr: Option<&mail_parser::Address>) -> String {
    addr.map(|a| {
        a.iter()
            .filter_map(|p| match (p.name(), p.address()) {
                (Some(name), Some(email)) => Some(format!("{name} <{email}>")),
                (None, Some(email)) => Some(email.to_string()),
                (Some(name), None) => Some(name.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    })
    .unwrap_or_default()
}

/// 同 `format_mp_address_display`，但空字符串时返回 None
fn format_mp_address_opt(addr: Option<&mail_parser::Address>) -> Option<String> {
    let s = format_mp_address_display(addr);
    if s.is_empty() { None } else { Some(s) }
}

/// 从 mail_parser 地址中提取逗号分隔的邮箱地址
fn format_mp_address_emails(addr: Option<&mail_parser::Address>) -> String {
    addr.map(|a| {
        a.iter()
            .filter_map(|p| p.address().map(|a| a.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    })
    .unwrap_or_default()
}

// ──────────────────────────────────────────────
// BODYSTRUCTURE 附件提取（仍需保留，用于 IMAP section_path）
// ──────────────────────────────────────────────

/// 从 BODYSTRUCTURE 中递归提取附件信息
///
/// 遍历 MIME 树，找到所有被判定为附件的叶子节点。
/// `section` 参数用于跟踪当前节点在 MIME 树中的路径（如 "2"、"3.1"），
/// 该路径后续用于 `UID FETCH BODY.PEEK[<section>]` 按需下载附件内容。
pub fn extract_attachments(
    body: &imap_proto::BodyStructure<'_>,
    section: &str,
) -> Vec<AttachmentInfo> {
    let mut attachments = Vec::new();

    match body {
        imap_proto::BodyStructure::Basic { common, other, .. } => {
            if is_attachment(common) {
                attachments.push(build_attachment_info(common, other, section));
            }
        }
        imap_proto::BodyStructure::Text { common, other, .. } => {
            if is_attachment(common) {
                attachments.push(build_attachment_info(common, other, section));
            }
        }
        imap_proto::BodyStructure::Message {
            common,
            other,
            body: inner_body,
            ..
        } => {
            if is_attachment(common) {
                attachments.push(build_attachment_info(common, other, section));
            }
            let inner_section = if section.is_empty() {
                "1".to_string()
            } else {
                format!("{}.1", section)
            };
            attachments.extend(extract_attachments(inner_body, &inner_section));
        }
        imap_proto::BodyStructure::Multipart { bodies, .. } => {
            for (i, sub_body) in bodies.iter().enumerate() {
                let sub_section = if section.is_empty() {
                    format!("{}", i + 1)
                } else {
                    format!("{}.{}", section, i + 1)
                };
                attachments.extend(extract_attachments(sub_body, &sub_section));
            }
        }
    }

    attachments
}

/// 判断一个 MIME 部分是否是附件
///
/// 判定规则（满足任一即视为附件）：
/// 1. Content-Disposition 为 "attachment"
/// 2. Content-Disposition 包含 filename 参数
/// 3. Content-Type 包含 name 参数
pub fn is_attachment(common: &imap_proto::BodyContentCommon<'_>) -> bool {
    if let Some(ref disposition) = common.disposition {
        if disposition.ty.eq_ignore_ascii_case("attachment") {
            return true;
        }
        if disposition.params.as_ref().is_some_and(|params| {
            params
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("filename"))
        }) {
            return true;
        }
    }

    if common
        .ty
        .params
        .as_ref()
        .is_some_and(|params| params.iter().any(|(k, _)| k.eq_ignore_ascii_case("name")))
    {
        return true;
    }

    false
}

/// 对可能包含 RFC 2047 编码的字符串进行解码
fn decode_rfc2047(raw: &str) -> String {
    match rfc2047_decoder::decode(raw.as_bytes()) {
        Ok(decoded) => decoded,
        Err(_) => raw.to_string(),
    }
}

/// 从 BODYSTRUCTURE 的单部分节点构建 AttachmentInfo
pub fn build_attachment_info(
    common: &imap_proto::BodyContentCommon<'_>,
    other: &imap_proto::BodyContentSinglePart<'_>,
    section: &str,
) -> AttachmentInfo {
    let content_type = format!("{}/{}", common.ty.ty, common.ty.subtype);

    let filename = common
        .disposition
        .as_ref()
        .and_then(|d| {
            d.params.as_ref().and_then(|params| {
                params
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("filename"))
                    .map(|(_, v)| decode_rfc2047(&v.clone()))
            })
        })
        .or_else(|| {
            common.ty.params.as_ref().and_then(|params| {
                params
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("name"))
                    .map(|(_, v)| decode_rfc2047(&v.clone()))
            })
        });

    AttachmentInfo {
        filename,
        content_type,
        size: other.octets,
        section_path: section.to_string(),
        disposition: common
            .disposition
            .as_ref()
            .map(|d| d.ty.clone().into_owned()),
        content_id: other.id.as_ref().map(|id| id.clone().into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headers_from_basic_email() {
        let raw = b"From: Alice <alice@example.com>\r\n\
                    To: Bob <bob@example.com>\r\n\
                    Subject: Hello World\r\n\
                    Date: Mon, 20 Apr 2026 10:00:00 +0800\r\n\
                    Message-ID: <msg123@example.com>\r\n\
                    \r\n\
                    Body text here";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert_eq!(result.subject.as_deref(), Some("Hello World"));
        assert_eq!(result.sender_email, "alice@example.com");
        assert_eq!(result.sender_name.as_deref(), Some("Alice"));
        assert_eq!(result.recipient_emails, "bob@example.com");
        assert_eq!(result.message_id.as_deref(), Some("msg123@example.com"));
        assert!(result.sent_at > 0);
    }

    #[test]
    fn test_parse_headers_missing_date_uses_fallback() {
        let raw = b"From: alice@example.com\r\n\
                    Subject: No Date\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 1745126400).expect("应成功解析");

        assert_eq!(result.sent_at, 1745126400);
        assert_eq!(result.subject.as_deref(), Some("No Date"));
    }

    #[test]
    fn test_parse_headers_with_cc_and_bcc() {
        let raw = b"From: alice@example.com\r\n\
                    To: bob@example.com\r\n\
                    Cc: charlie@example.com\r\n\
                    Bcc: secret@example.com\r\n\
                    Subject: With CC\r\n\
                    Date: Mon, 20 Apr 2026 12:00:00 +0000\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert!(result.cc_emails.is_some());
        assert!(result.cc_emails.as_ref().unwrap().contains("charlie@example.com"));
        assert!(result.bcc_emails.is_some());
        assert!(result.bcc_emails.as_ref().unwrap().contains("secret@example.com"));
    }

    #[test]
    fn test_parse_headers_empty_sender_email() {
        let raw = b"Subject: No From\r\n\
                    Date: Mon, 20 Apr 2026 12:00:00 +0000\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert_eq!(result.sender_email, "");
    }

    #[test]
    fn test_parse_headers_multiple_recipients() {
        let raw = b"From: alice@example.com\r\n\
                    To: bob@example.com, charlie@example.com\r\n\
                    Subject: Multi\r\n\
                    Date: Mon, 20 Apr 2026 12:00:00 +0000\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert!(result.recipient_emails.contains("bob@example.com"));
        assert!(result.recipient_emails.contains("charlie@example.com"));
    }
}
