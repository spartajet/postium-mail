use crate::error::MailError;
use crate::infrastructure::protocols::types::{AttachmentInfo, MailEnvelope};
use crate::infrastructure::protocols::utils::format_address_list;
use async_imap::imap_proto::{self, Envelope};

/// 从 Cow<[u8]> 转为可读 String
pub fn cow_bytes_to_string(cow: &[u8]) -> String {
    String::from_utf8_lossy(cow).into_owned()
}

/// 从 Address 提取显示名
pub fn address_to_string(addr: &imap_proto::Address<'_>) -> Option<String> {
    addr.name
        .as_ref()
        .map(|n| cow_bytes_to_string(n))
        .or_else(|| addr.adl.as_ref().map(|m| cow_bytes_to_string(m)))
        .or_else(|| {
            let mailbox = addr.mailbox.as_ref().map(|m| cow_bytes_to_string(m));
            let host = addr.host.as_ref().map(|h| cow_bytes_to_string(h));
            match (mailbox, host) {
                (Some(m), Some(h)) => Some(format!("{m}@{h}")),
                (Some(m), None) => Some(m),
                _ => None,
            }
        })
}

/// 解析 IMAP ENVELOPE 响应
///
/// 从服务器返回的 ENVELOPE 数据中提取邮件头信息，包括：
/// - Subject: 邮件主题（使用自定义 RFC 2047 解码器）
/// - From: 发件人地址列表
/// - To: 收件人地址列表
/// - Cc: 抄送地址列表
/// - Bcc: 密送地址列表
pub fn parse_envelope(envelope: &Envelope) -> Result<MailEnvelope, MailError> {
    // 解析 Subject 字段
    let subject = envelope
        .subject
        .as_ref()
        .map(|subject| {
            let subject_str = String::from_utf8_lossy(subject.as_ref());
            match rfc2047_decoder::decode(subject_str.as_bytes()) {
                Ok(decoded) => decoded,
                Err(_) => subject_str.into_owned(),
            }
        })
        .unwrap_or_default();

    let from = envelope
        .from
        .as_ref()
        .map(|addrs| format_address_list(addrs.as_slice()))
        .unwrap_or_default();

    let to = envelope
        .to
        .as_ref()
        .map(|addrs| format_address_list(addrs.as_slice()))
        .unwrap_or_default();

    let cc = envelope
        .cc
        .as_ref()
        .map(|addrs| format_address_list(addrs.as_slice()))
        .unwrap_or_default();

    let bcc = envelope
        .bcc
        .as_ref()
        .map(|addrs| format_address_list(addrs.as_slice()))
        .unwrap_or_default();

    // 解析日期 (RFC 2822 格式)
    let date = envelope
        .date
        .as_ref()
        .and_then(|date_bytes| {
            let date_str = String::from_utf8_lossy(date_bytes.as_ref());
            chrono::DateTime::parse_from_rfc2822(&date_str)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        })
        .unwrap_or_else(|| {
            tracing::warn!("IMAP ENVELOPE 日期解析失败，使用当前时间");
            chrono::Utc::now()
        });

    Ok(MailEnvelope {
        subject,
        from,
        to,
        cc,
        bcc,
        date,
    })
}

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
                    .map(|(_, v)| v.clone().into_owned())
            })
        })
        .or_else(|| {
            common.ty.params.as_ref().and_then(|params| {
                params
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("name"))
                    .map(|(_, v)| v.clone().into_owned())
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
