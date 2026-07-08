//! IMAP 邮件解析模块
//!
//! 本模块使用 mail_parser 库解析 RFC822 格式的邮件内容，包括：
//! - 邮件头部的解析和提取
//! - 正文内容的解码（文本和 HTML）
//! - 字符编码的处理（RFC 2047、GB18030 等）
//! - BODYSTRUCTURE 的解析（用于附件信息提取）

use crate::infrastructure::protocols::types::AttachmentInfo;
use async_imap::imap_proto;
use base64::Engine;
use encoding_rs::GB18030;

use mail_parser::{Encoding, MessageParser, PartType};
use std::borrow::Cow;

// ──────────────────────────────────────────────
// mail_parser 辅助函数
// ──────────────────────────────────────────────

/// 使用 mail_parser 解析后的邮件头部信息
///
/// 包含从 RFC822 邮件中提取的所有关键头部字段。
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
/// # 参数
/// - `raw`: 原始 RFC822 邮件字节
/// - `fallback_ts`: 当邮件中无 Date 头时使用的时间戳
///
/// # 返回
/// 成功时返回解析后的邮件头部，失败时返回 None
pub fn parse_headers_from_raw(raw: &[u8], fallback_ts: i64) -> Option<ParsedHeaders> {
    let msg = MessageParser::default().parse(raw)?;
    Some(extract_headers_from_message_with_raw(
        &msg,
        raw,
        fallback_ts,
    ))
}

/// 从邮件中提取纯文本正文内容
///
/// # 参数
/// - `msg`: 已解析的邮件消息
///
/// # 返回
/// 成功时返回解码后的纯文本正文，失败时返回 None
///
/// # 功能
/// - 自动处理字符编码
/// - 检测并修复 Mojibake（乱码）
pub fn decoded_body_text(msg: &mail_parser::Message<'_>) -> Option<String> {
    decode_body_part_with_fallback(msg, *msg.text_body.first()?, BodyKind::Text)
}

/// 从邮件中提取 HTML 正文内容
///
/// # 参数
/// - `msg`: 已解析的邮件消息
///
/// # 返回
/// 成功时返回解码后的 HTML 正文，失败时返回 None
///
/// # 功能
/// - 自动处理字符编码
/// - 检测并修复 Mojibake（乱码）
pub fn decoded_body_html(msg: &mail_parser::Message<'_>) -> Option<String> {
    decode_body_part_with_fallback(msg, *msg.html_body.first()?, BodyKind::Html)
}

/// 从已解析的 mail_parser Message 中提取邮件头字段
///
/// # 参数
/// - `msg`: 已解析的邮件消息
/// - `fallback_ts`: 当邮件中无 Date 头时使用的时间戳
///
/// # 返回
/// 解析后的邮件头部信息
///
/// # 功能
/// 供调用方在已经拥有 `Message` 对象时使用，避免重复解析。
pub fn extract_headers_from_message(
    msg: &mail_parser::Message<'_>,
    fallback_ts: i64,
) -> ParsedHeaders {
    extract_headers_from_message_with_raw(msg, msg.raw_message(), fallback_ts)
}

/// 从已解析的邮件和原始字节中提取邮件头字段
///
/// # 参数
/// - `msg`: 已解析的邮件消息
/// - `raw`: 原始 RFC822 字节（用于回退解码）
/// - `fallback_ts`: 当邮件中无 Date 头时使用的时间戳
///
/// # 返回
/// 解析后的邮件头部信息
///
/// # 功能
/// - 提取所有标准邮件头字段
/// - 格式化地址为显示字符串
/// - 处理多个收件人情况
pub fn extract_headers_from_message_with_raw(
    msg: &mail_parser::Message<'_>,
    raw: &[u8],
    fallback_ts: i64,
) -> ParsedHeaders {
    let subject = decode_subject_with_fallback(msg, raw);
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

/// 解析主题，支持多种编码回退机制
///
/// # 参数
/// - `msg`: 已解析的邮件消息
/// - `raw`: 原始 RFC822 字节（用于回退解码）
///
/// # 返回
/// 成功时返回解码后的主题，失败时返回 None
///
/// # 功能
/// - 优先使用 mail_parser 的解码结果
/// - 检测 Mojibake（乱码）并尝试回退方案
/// - 支持 GB18030 原始字节解码
fn decode_subject_with_fallback(msg: &mail_parser::Message<'_>, raw: &[u8]) -> Option<String> {
    let parsed = msg.subject().map(decode_rfc2047);
    if parsed.as_ref().is_some_and(|subject| !is_mojibake(subject)) {
        return parsed;
    }

    if let Some(raw_subject) = raw_header_value(raw, b"subject") {
        if let Ok(ascii_subject) = std::str::from_utf8(raw_subject) {
            let decoded = decode_rfc2047(ascii_subject);
            if !is_mojibake(&decoded) && decoded != ascii_subject {
                return Some(decoded);
            }
        }

        let decoded = decode_gb18030_lossy(raw_subject);
        if !is_mojibake(&decoded) {
            return Some(decoded);
        }
    }

    parsed
}

/// 从原始 RFC822 字节中提取指定头部的值
///
/// # 参数
/// - `raw`: 原始 RFC822 字节
/// - `header_name`: 头部名称（如 b"subject"）
///
/// # 返回
/// 成功时返回头部值的字节切片，失败时返回 None
///
/// # 功能
/// - 正确处理头部折叠（continuation lines）
/// - 大小写不敏感匹配
fn raw_header_value<'a>(raw: &'a [u8], header_name: &[u8]) -> Option<&'a [u8]> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .or_else(|| raw.windows(2).position(|window| window == b"\n\n"))
        .unwrap_or(raw.len());
    let headers = &raw[..header_end];
    let mut offset = 0;

    while offset < headers.len() {
        let line_end = headers[offset..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map(|pos| offset + pos)
            .unwrap_or(headers.len());
        let mut line = &headers[offset..line_end];
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }

        if let Some(colon_pos) = line.iter().position(|&byte| byte == b':')
            && line[..colon_pos].eq_ignore_ascii_case(header_name)
        {
            let value_start = offset + colon_pos + 1;
            let mut end = line_end;
            let mut next = if line_end < headers.len() {
                line_end + 1
            } else {
                line_end
            };
            while next < headers.len() && matches!(headers.get(next), Some(b' ' | b'\t')) {
                end = headers[next..]
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map(|pos| next + pos)
                    .unwrap_or(headers.len());
                next = if end < headers.len() { end + 1 } else { end };
            }
            return Some(trim_ascii_whitespace(&headers[value_start..end]));
        }

        offset = if line_end < headers.len() {
            line_end + 1
        } else {
            line_end
        };
    }

    None
}

/// 去除字节切片两端的 ASCII 空白字符
///
/// # 参数
/// - `bytes`: 输入字节切片
///
/// # 返回
/// 去除两端空白后的字节切片
fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|pos| pos + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

/// 邮件正文类型枚举
enum BodyKind {
    Text,
    Html,
}

/// 解码邮件正文部分，支持多种编码回退
///
/// # 参数
/// - `msg`: 已解析的邮件消息
/// - `part_id`: 部分ID
/// - `kind`: 正文类型
///
/// # 返回
/// 成功时返回解码后的正文，失败时返回 None
///
/// # 功能
/// - 优先使用 mail_parser 的解码结果
/// - 检测 Mojibake 并尝试原始字节解码
/// - 支持 Base64 和 Quoted-Printable 传输编码
fn decode_body_part_with_fallback(
    msg: &mail_parser::Message<'_>,
    part_id: u32,
    kind: BodyKind,
) -> Option<String> {
    let part = msg.parts.get(part_id as usize)?;
    let parsed = match (&part.body, &kind) {
        (PartType::Text(text), BodyKind::Text) | (PartType::Html(text), BodyKind::Html) => {
            Some(text.to_string())
        }
        _ => None,
    };
    if parsed.as_ref().is_some_and(|text| !is_mojibake(text)) {
        return parsed;
    }

    let raw = msg.raw_message();
    let body = raw.get(part.offset_body as usize..part.offset_end as usize)?;
    let decoded_transfer = decode_transfer_encoded_body(body, part.encoding)?;
    Some(decode_gb18030_lossy(&decoded_transfer))
}

/// 检测字符串是否包含大量乱码字符
///
/// # 参数
/// - `value`: 待检测的字符串
///
/// # 返回
/// 包含2个或更多替换字符时返回 true
///
/// # 功能
/// - 通过计算 Unicode 替换字符（U+FFFD）的数量判断
/// - 用于检测编码错误导致的乱码
fn is_mojibake(value: &str) -> bool {
    value.chars().filter(|&ch| ch == '\u{fffd}').count() >= 2
}

/// 使用 GB18030 编码解码字节（lossy 模式）
///
/// # 参数
/// - `bytes`: 待解码的字节
///
/// # 返回
/// 解码后的字符串
///
/// # 功能
/// - 用于处理中文邮件中常见的 GB18030/GBK 编码
/// - Lossy 模式会替换无效字符而非失败
fn decode_gb18030_lossy(bytes: &[u8]) -> String {
    let (decoded, _, _) = GB18030.decode(bytes);
    decoded.into_owned()
}

/// 解码传输编码的邮件正文
///
/// # 参数
/// - `body`: 编码后的正文字节
/// - `encoding`: 传输编码类型
///
/// # 返回
/// 成功时返回解码后的字节，失败时返回 None
///
/// # 功能
/// - 支持 Base64 解码（忽略空白字符）
/// - 支持 Quoted-Printable 解码（鲁棒模式）
/// - 无编码时直接返回原字节
fn decode_transfer_encoded_body(body: &[u8], encoding: Encoding) -> Option<Vec<u8>> {
    match encoding {
        Encoding::None => Some(body.to_vec()),
        Encoding::Base64 => {
            let normalized = body
                .iter()
                .copied()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>();
            base64::engine::general_purpose::STANDARD
                .decode(normalized)
                .ok()
        }
        Encoding::QuotedPrintable => {
            quoted_printable::decode(body, quoted_printable::ParseMode::Robust).ok()
        }
    }
}

/// 将 mail_parser 地址格式化为 "Name <email>" 显示字符串
///
/// # 参数
/// - `addr`: mail_parser 地址对象
///
/// # 返回
/// 格式化后的地址字符串，空地址时返回空字符串
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

/// 格式化地址，空字符串时返回 None
///
/// # 参数
/// - `addr`: mail_parser 地址对象
///
/// # 返回
/// 格式化后的地址字符串，空地址时返回 None
fn format_mp_address_opt(addr: Option<&mail_parser::Address>) -> Option<String> {
    let s = format_mp_address_display(addr);
    if s.is_empty() { None } else { Some(s) }
}

/// 从 mail_parser 地址中提取逗号分隔的邮箱地址
///
/// # 参数
/// - `addr`: mail_parser 地址对象
///
/// # 返回
/// 逗号分隔的邮箱地址字符串，空地址时返回空字符串
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
/// # 参数
/// - `body`: IMAP BODYSTRUCTURE 对象
/// - `section`: 当前 MIME 路径（如 "2"、"3.1"）
///
/// # 返回
/// 附件信息列表
///
/// # 功能
/// - 遍历 MIME 树，找到所有被判定为附件的叶子节点
/// - `section` 参数跟踪当前节点在 MIME 树中的路径
/// - 该路径用于 `UID FETCH BODY.PEEK[<section>]` 按需下载附件内容
pub fn extract_attachments(
    body: &imap_proto::BodyStructure<'_>,
    section: &str,
) -> Vec<AttachmentInfo> {
    let mut attachments = Vec::new();

    match body {
        imap_proto::BodyStructure::Basic { common, other, .. } => {
            if is_attachment(common, other) {
                attachments.push(build_attachment_info(common, other, section));
            }
        }
        imap_proto::BodyStructure::Text { common, other, .. } => {
            if is_attachment(common, other) {
                attachments.push(build_attachment_info(common, other, section));
            }
        }
        imap_proto::BodyStructure::Message {
            common,
            other,
            body: inner_body,
            ..
        } => {
            if is_attachment(common, other) {
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
/// # 参数
/// - `common`: BODYSTRUCTURE 公共字段
/// - `other`: BODYSTRUCTURE 单部分字段
///
/// # 返回
/// 是附件时返回 true
///
/// # 判定规则（满足任一即视为附件）
/// 1. Content-Disposition 为 "attachment"
/// 2. Content-Disposition 包含 filename 参数
/// 3. Content-Type 包含 name 参数
/// 4. image/* 且包含 Content-ID（用于 CID 内联图片）
pub fn is_attachment(
    common: &imap_proto::BodyContentCommon<'_>,
    other: &imap_proto::BodyContentSinglePart<'_>,
) -> bool {
    if let Some(ref disposition) = common.disposition {
        if disposition.ty.eq_ignore_ascii_case("attachment") {
            return true;
        }
        if disposition
            .params
            .as_ref()
            .is_some_and(|params| find_mime_param(params, "filename").is_some())
        {
            return true;
        }
    }

    if common
        .ty
        .params
        .as_ref()
        .is_some_and(|params| find_mime_param(params, "name").is_some())
    {
        return true;
    }

    common.ty.ty.eq_ignore_ascii_case("image") && other.id.is_some()
}

/// 对可能包含 RFC 2047 编码的字符串进行解码
///
/// # 参数
/// - `raw`: 可能包含编码的字符串
///
/// # 返回
/// 解码后的字符串
///
/// # 功能
/// - 支持 =?charset?encoding?encoded-text?= 格式
/// - 解码失败时返回原字符串
fn decode_rfc2047(raw: &str) -> String {
    match rfc2047_decoder::Decoder::new()
        .too_long_encoded_word_strategy(rfc2047_decoder::RecoverStrategy::Decode)
        .decode(raw.as_bytes())
    {
        Ok(decoded) => decoded,
        Err(_) => raw.to_string(),
    }
}

fn decode_rfc2231_value(raw: &str) -> String {
    let mut parts = raw.splitn(3, '\'');
    let value = match (parts.next(), parts.next(), parts.next()) {
        (Some(charset), Some(_language), Some(encoded)) if !charset.is_empty() => {
            let bytes = percent_decode_to_bytes(encoded);
            if charset.eq_ignore_ascii_case("gb18030")
                || charset.eq_ignore_ascii_case("gbk")
                || charset.eq_ignore_ascii_case("gb2312")
            {
                let (decoded, _, _) = GB18030.decode(&bytes);
                decoded.into_owned()
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            }
        }
        _ => String::from_utf8_lossy(&percent_decode_to_bytes(raw)).into_owned(),
    };
    decode_rfc2047(&value)
}

fn percent_decode_to_bytes(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn find_mime_param(params: &[(Cow<'_, str>, Cow<'_, str>)], name: &str) -> Option<String> {
    if let Some((_, value)) = params
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
    {
        return Some(decode_rfc2047(value));
    }

    let extended_name = format!("{name}*");
    if let Some((_, value)) = params
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(&extended_name))
    {
        return Some(decode_rfc2231_value(value));
    }

    let prefix = format!("{name}*");
    let mut segments = params
        .iter()
        .filter_map(|(key, value)| parse_continuation_param(key, value, &prefix))
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }

    segments.sort_by_key(|(index, _, _)| *index);
    if segments.first().is_none_or(|(index, _, _)| *index != 0) {
        return None;
    }

    let is_encoded = segments.iter().any(|(_, encoded, _)| *encoded);
    let joined = segments
        .into_iter()
        .map(|(_, _, value)| value)
        .collect::<String>();
    Some(if is_encoded {
        decode_rfc2231_value(&joined)
    } else {
        decode_rfc2047(&joined)
    })
}

fn parse_continuation_param(key: &str, value: &str, prefix: &str) -> Option<(usize, bool, String)> {
    if key.len() <= prefix.len() || !key[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let suffix = &key[prefix.len()..];
    let (digits, encoded) = suffix
        .strip_suffix('*')
        .map_or((suffix, false), |digits| (digits, true));
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, encoded, value.to_string()))
}

/// 从 BODYSTRUCTURE 的单部分节点构建 AttachmentInfo
///
/// # 参数
/// - `common`: BODYSTRUCTURE 公共字段
/// - `other`: BODYSTRUCTURE 单部分字段
/// - `section`: MIME section 路径
///
/// # 返回
/// 附件信息结构体
///
/// # 功能
/// - 提取文件名（从 Content-Disposition 或 Content-Type）
/// - 解码 RFC 2047 编码的文件名
/// - 组装完整的附件信息
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
            d.params
                .as_ref()
                .and_then(|params| find_mime_param(params, "filename"))
        })
        .or_else(|| {
            common
                .ty
                .params
                .as_ref()
                .and_then(|params| find_mime_param(params, "name"))
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

// ─── 测试模块 ───

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试检测无文件名的内联 CID 图片为附件
    #[test]
    fn detects_inline_cid_image_without_filename_as_attachment() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("image"),
                    subtype: Cow::Borrowed("png"),
                    params: None,
                },
                disposition: None,
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: Some(Cow::Borrowed("<logo@example.com>")),
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 128,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "2");

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].content_type, "image/png");
        assert_eq!(
            attachments[0].content_id.as_deref(),
            Some("<logo@example.com>")
        );
        assert_eq!(attachments[0].section_path, "2");
    }

    /// 测试顶层单部分附件保持空的 section_path
    #[test]
    fn top_level_single_part_attachment_keeps_empty_section_path() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentDisposition,
            ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("application"),
                    subtype: Cow::Borrowed("pdf"),
                    params: None,
                },
                disposition: Some(ContentDisposition {
                    ty: Cow::Borrowed("attachment"),
                    params: None,
                }),
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: None,
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 128,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "");

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].section_path, "");
    }

    /// 测试解码 RFC 2047 编码的附件文件名
    #[test]
    fn decodes_rfc2047_encoded_attachment_filename() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentDisposition,
            ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let encoded_filename = "=?utf-8?B?6YKA6K+35Ye9LTIwMjYg5pqR5YGH77yI56ysIDE2IOWxiu+8ieWFqOWbvemrmOagoeWkp+aooeWei+OAgeaZuuiDveS9k+S4jueUn+aIkOW8j+e8lueoi+WunuaImOeglOS/ruePrS5wZGY=?=";
        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("application"),
                    subtype: Cow::Borrowed("pdf"),
                    params: None,
                },
                disposition: Some(ContentDisposition {
                    ty: Cow::Borrowed("attachment"),
                    params: Some(vec![(
                        Cow::Borrowed("filename"),
                        Cow::Borrowed(encoded_filename),
                    )]),
                }),
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: None,
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 128,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "2");

        assert_eq!(
            attachments[0].filename.as_deref(),
            Some("邀请函-2026 暑假（第 16 届）全国高校大模型、智能体与生成式编程实战研修班.pdf")
        );
    }

    /// 测试解码 RFC 2231 编码的 Content-Disposition 文件名
    #[test]
    fn decodes_rfc2231_encoded_attachment_filename() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentDisposition,
            ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("application"),
                    subtype: Cow::Borrowed("pdf"),
                    params: None,
                },
                disposition: Some(ContentDisposition {
                    ty: Cow::Borrowed("attachment"),
                    params: Some(vec![(
                        Cow::Borrowed("filename*"),
                        Cow::Borrowed("utf-8'en'ni-supported-operating-systems-roadmap.pdf"),
                    )]),
                }),
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: None,
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 431,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "2");

        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0].filename.as_deref(),
            Some("ni-supported-operating-systems-roadmap.pdf")
        );
    }

    /// 测试从 RFC 2231 分段参数中还原附件文件名
    #[test]
    fn decodes_rfc2231_continued_attachment_filename() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentDisposition,
            ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("application"),
                    subtype: Cow::Borrowed("pdf"),
                    params: None,
                },
                disposition: Some(ContentDisposition {
                    ty: Cow::Borrowed("attachment"),
                    params: Some(vec![
                        (
                            Cow::Borrowed("filename*0*"),
                            Cow::Borrowed("utf-8''ni-supported-"),
                        ),
                        (
                            Cow::Borrowed("filename*1*"),
                            Cow::Borrowed("operating-systems-roadmap.pdf"),
                        ),
                    ]),
                }),
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: None,
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 431,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "2");

        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0].filename.as_deref(),
            Some("ni-supported-operating-systems-roadmap.pdf")
        );
    }

    /// 测试仅 Content-Type name* 参数也能识别并提取附件文件名
    #[test]
    fn detects_attachment_from_rfc2231_content_type_name_param() {
        use async_imap::imap_proto::{
            BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentEncoding, ContentType,
        };
        use std::borrow::Cow;

        let body = BodyStructure::Basic {
            common: BodyContentCommon {
                ty: ContentType {
                    ty: Cow::Borrowed("application"),
                    subtype: Cow::Borrowed("pdf"),
                    params: Some(vec![(
                        Cow::Borrowed("name*"),
                        Cow::Borrowed("utf-8''ni-supported-operating-systems-roadmap.pdf"),
                    )]),
                },
                disposition: None,
                language: None,
                location: None,
            },
            other: BodyContentSinglePart {
                id: None,
                md5: None,
                description: None,
                transfer_encoding: ContentEncoding::Base64,
                octets: 431,
            },
            extension: None,
        };

        let attachments = extract_attachments(&body, "2");

        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0].filename.as_deref(),
            Some("ni-supported-operating-systems-roadmap.pdf")
        );
    }

    /// 测试解析基本邮件的头部
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

    /// 测试缺失日期时使用回退时间戳
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

    /// 测试解码无编码字的原始 GB18030 主题
    #[test]
    fn decodes_raw_gb18030_subject_without_encoded_word() {
        let mut raw = b"From: alice@example.com\r\nSubject: ".to_vec();
        raw.extend_from_slice(&[
            0xd0, 0xc2, 0xc0, 0xcb, 0xd4, 0xc6, 0xd6, 0xd5, 0xd6, 0xb9, 0xb7, 0xfe, 0xce, 0xf1,
            0xb9, 0xab, 0xb8, 0xe6,
        ]);
        raw.extend_from_slice(b"\r\n\r\nBody");

        let result = parse_headers_from_raw(&raw, 0).expect("应成功解析");

        assert_eq!(result.subject.as_deref(), Some("新浪云终止服务公告"));
    }

    /// 测试解码 GBK RFC2047 编码的主题
    #[test]
    fn decodes_gbk_rfc2047_subject() {
        let raw = b"From: alice@example.com\r\n\
                    Subject: =?GBK?B?0MLAy9TG1tXWubf+zvG5q7jm?=\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert_eq!(result.subject.as_deref(), Some("新浪云终止服务公告"));
    }

    /// 测试解码无字符集声明的原始 GB18030 文本正文
    #[test]
    fn decodes_raw_gb18030_text_body_without_charset() {
        let mut raw =
            b"From: alice@example.com\r\nSubject: test\r\nContent-Type: text/plain\r\n\r\n"
                .to_vec();
        raw.extend_from_slice(&[
            0xd7, 0xf0, 0xbe, 0xb4, 0xb5, 0xc4, 0xd3, 0xc3, 0xbb, 0xa7, 0xa3, 0xba, 0x0a, 0xc4,
            0xfa, 0xba, 0xc3, 0xa3, 0xac, 0xb8, 0xd0, 0xd0, 0xbb, 0xc4, 0xfa, 0xb3, 0xa4, 0xc6,
            0xda, 0xd2, 0xd4, 0xc0, 0xb4, 0xb6, 0xd4, 0xd0, 0xc2, 0xc0, 0xcb, 0xd4, 0xc6, 0xb2,
            0xfa, 0xc6, 0xb7, 0xd3, 0xeb, 0xb7, 0xfe, 0xce, 0xf1, 0xb5, 0xc4, 0xd6, 0xa7, 0xb3,
            0xd6, 0xa1, 0xa3,
        ]);

        let message = MessageParser::default().parse(&raw).expect("应成功解析");

        assert_eq!(
            decoded_body_text(&message).as_deref(),
            Some("尊敬的用户：\n您好，感谢您长期以来对新浪云产品与服务的支持。")
        );
    }

    /// 测试解析带 CC 和 BCC 的邮件头
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
        assert!(
            result
                .cc_emails
                .as_ref()
                .unwrap()
                .contains("charlie@example.com")
        );
        assert!(result.bcc_emails.is_some());
        assert!(
            result
                .bcc_emails
                .as_ref()
                .unwrap()
                .contains("secret@example.com")
        );
    }

    /// 测试解析无发件人的邮件头
    #[test]
    fn test_parse_headers_empty_sender_email() {
        let raw = b"Subject: No From\r\n\
                    Date: Mon, 20 Apr 2026 12:00:00 +0000\r\n\
                    \r\n\
                    Body";
        let result = parse_headers_from_raw(raw, 0).expect("应成功解析");

        assert_eq!(result.sender_email, "");
    }

    /// 测试解析多个收件人的邮件头
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
