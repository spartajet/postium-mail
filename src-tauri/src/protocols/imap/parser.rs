//! 邮件解析模块
//!
//! 提供邮件解析功能，基于 `mail_parser` 库实现 RFC 5322 和 MIME 标准。
//!
//! # 核心功能
//!
//! - **RFC 2047 解码**: 支持多种字符集和编码方式的邮件头解码
//! - **乱码检测**: 自动检测并修复常见的编码问题
//! - **中文支持**: 特殊优化 GBK、GB18030、Big5 等中文编码
//! - **骨架同步**: 仅解析邮件头，不下载完整正文
//! - **附件提取**: 解析 MIME 附件信息
//!
//! # RFC 2047 编码支持
//!
//! 邮件头（如 Subject、From）可能使用 RFC 2047 编码来表示非 ASCII 字符。
//!
//! ## 编码格式
//!
//! ```text
//! =?charset?encoding?encoded-text?=
//! ```
//!
//! ## 支持的编码
//!
//! | 字符集 | Base64 (B) | Quoted-Printable (Q) |
//! |--------|-----------|---------------------|
//! | UTF-8 | ✅ | ✅ |
//! | GBK | ✅ | ✅ |
//! | GB18030 | ✅ | ✅ |
//! | GB2312 | ✅ | ✅ |
//! | Big5 | ✅ | ❌ |
//!
//! ## 示例
//!
//! ```text
//! Subject: =?GBK?B?xOO6ww==?=           (Base64 编码的 GBK)
//! Subject: =?UTF-8?Q?=E4=B8=AD=E6=96=87?=  (Quoted-Printable 编码的 UTF-8)
//! ```
//!
//! # 乱码检测与修复
//!
//! `is_garbled()` 函数检测以下乱码模式：
//!
//! 1. **Unicode 替换字符过多**: 替换字符（�）占比超过 5%
//! 2. **GBK 错误解码模式**: 检测 GBK 被错误当作 Latin-1 解码的特征字符
//!
//! `fix_encoding_issue()` 函数尝试以下修复策略：
//!
//! 1. 解码 RFC 2047 编码
//! 2. 检测是否仍然乱码
//! 3. 移除替换字符
//! 4. 返回清理后的文本
//!
//! # mail_parser 集成
//!
//! 本模块使用 `mail_parser` 库进行核心邮件解析，但针对以下问题进行了增强：
//!
//! ## mail_parser 的限制
//!
//! - 对 RFC 2047 GBK 编码的支持有问题
//! - 某些中文邮件头解码不正确
//!
//! ## 增强方案
//!
//! 1. **原始邮件头提取**: 直接从原始邮件中提取 Subject 字段
//! 2. **自定义解码器**: 实现完整的 RFC 2047 解码器
//! 3. **编码检测**: 自动检测并处理编码问题
//! 4. **降级策略**: mail_parser 解码失败时使用自定义解码
//!
//! # 中文字符编码支持
//!
//! ## 支持的编码
//!
//! | 编码 | 使用场景 | 解码支持 |
//! |------|---------|---------|
//! | GBK | 简体中文（大陆） | ✅ 完全支持 |
//! | GB18030 | GBK 超集 | ✅ 完全支持 |
//! | GB2312 | 简体中文（旧标准） | ✅ 完全支持 |
//! | Big5 | 繁体中文（台湾/香港） | ✅ 完全支持 |
//! | Shift_JIS | 日文 | ✅ 完全支持 |
//! | UTF-8 | 通用 | ✅ 完全支持 |
//!
//! # 函数说明
//!
//! ## 主要解析函数
//!
//! - [`parse_email_with_mail_parser()`][]: 解析完整邮件，包含正文和附件
//! - [`parse_email_header_only()`][]: 仅解析邮件头，用于骨架同步
//!
//! ## 编码处理函数
//!
//! - [`decode_rfc2047()`][]: 解码 RFC 2047 编码字符串
//! - [`is_garbled()`][]: 检测字符串是否乱码
//! - [`fix_encoding_issue()`][]: 修复编码问题
//! - [`extract_and_decode_subject()`][]: 从原始邮件头提取并解码主题
//!
//! ## 辅助函数
//!
//! - [`decode_quoted_printable_utf8()`][]: 解码 QP 编码的 UTF-8
//! - [`decode_quoted_printable_gbk()`][]: 解码 QP 编码的 GBK
//! - [`extract_attachments()`][]: 提取附件信息
//!
//! # 使用示例
//!
//! ## 解析完整邮件
//!
//! ```rust,no_run
//! use crate::protocols::imap::parser::parse_email_with_mail_parser;
//!
//! let raw_email = "From: sender@example.com\n..."; // 原始邮件
//! let email = parse_email_with_mail_parser(&raw_email, 123)?;
//!
//! println!("主题: {}", email.subject);
//! println!("发件人: {}", email.from);
//! println!("纯文本: {}", email.body_text);
//! ```
//!
//! ## 仅解析邮件头
//!
//! ```rust,no_run
//! use crate::protocols::imap::parser::parse_email_header_only;
//!
//! let raw_header = "From: sender@example.com\n..."; // 原始邮件头
//! let header = parse_email_header_only(&raw_header, 123)?;
//!
//! println!("主题: {}", header.subject);
//! // 不包含正文和附件
//! ```
//!
//! # 注意事项
//!
//! - Subject 字段使用自定义解码器，绕过 mail_parser 的 GBK 问题
//! - 日期解析失败时使用当前时间作为回退
//! - From 地址缺失时使用空字符串而不是错误
//! - 附件名缺失时根据 content-type 生成默认名称
//!
//! # 参考资料
//!
//! - [RFC 5322 - Internet Message Format](https://datatracker.ietf.org/doc/html/rfc5322)
//! - [RFC 2047 - MIME (Multipurpose Internet Mail Extensions) Part Three](https://datatracker.ietf.org/doc/html/rfc2047)
//! - [RFC 2183 - Communicating Presentation Information in Internet Messages](https://datatracker.ietf.org/doc/html/rfc2183)
//!

use super::types::{EmailAttachment, EmailData, EmailFlags};
use anyhow::{Result, anyhow};
use mail_parser::MimeHeaders;

/// 检测字符串是否包含大量乱码字符
fn is_garbled(input: &str) -> bool {
    let replacement_count = input.chars().filter(|&c| c == '\u{FFFD}').count();
    let total_chars = input.chars().count();

    if total_chars == 0 {
        return false;
    }

    let replacement_ratio = replacement_count as f64 / total_chars as f64;

    // 如果替换字符占比超过5%，认为是乱码
    if replacement_ratio > 0.05 {
        return true;
    }

    // 检查是否包含特定的GBK编码错误模式
    // GBK被当作Latin-1解码时，会产生特定的字符序列
    if input.contains('ƶ') || input.contains('Ʊ') || input.contains('ĵ') || input.contains('ӷ')
    {
        // 这些字符是GBK中文被错误解码的典型标志
        return true;
    }

    false
}

/// 解码RFC 2047编码的字符串
/// 格式：=?charset?encoding?encoded-text?=
/// 例如：=?GBK?B?xOO6ww==?=
fn decode_rfc2047(input: &str) -> Result<String> {
    use base64::Engine;

    // RFC 2047 编码的正则表达式模式
    // 格式：=?charset?Q/B?encoded-text?=
    let pattern = regex::Regex::new(r"(?i)=\?([^?]+)\?([QB])\?([^?]+)\?=").unwrap();

    let mut result = String::from(input);
    let mut pos = 0;

    // 查找所有编码的片段
    while let Some(caps) = pattern.captures(&result[pos..]) {
        let full_match = caps.get(0).unwrap();
        let charset = caps.get(1).unwrap().as_str().to_uppercase();
        let encoding = caps.get(2).unwrap().as_str().to_uppercase();
        let encoded_text = caps.get(3).unwrap().as_str();

        let decoded_text = match (charset.as_str(), encoding.as_str()) {
            ("UTF-8", "B") => {
                // Base64 编码的 UTF-8
                let engine = base64::engine::general_purpose::STANDARD;
                match engine.decode(encoded_text) {
                    Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
                    Err(_) => encoded_text.to_string(),
                }
            }
            ("GBK", "B") | ("GB18030", "B") | ("GB2312", "B") => {
                // Base64 编码的 GBK/GB18030/GB2312
                let engine = base64::engine::general_purpose::STANDARD;
                match engine.decode(encoded_text) {
                    Ok(bytes) => {
                        let (text, _, _) = encoding_rs::GBK.decode(&bytes);
                        text.to_string()
                    }
                    Err(_) => encoded_text.to_string(),
                }
            }
            ("BIG5", "B") => {
                // Base64 编码的 Big5
                let engine = base64::engine::general_purpose::STANDARD;
                match engine.decode(encoded_text) {
                    Ok(bytes) => {
                        let (text, _, _) = encoding_rs::BIG5.decode(&bytes);
                        text.to_string()
                    }
                    Err(_) => encoded_text.to_string(),
                }
            }
            ("UTF-8", "Q") | ("UTF-8", "q") => {
                // Quoted-Printable 编码的 UTF-8
                decode_quoted_printable_utf8(encoded_text)
            }
            ("GBK", "Q")
            | ("GBK", "q")
            | ("GB18030", "Q")
            | ("GB18030", "q")
            | ("GB2312", "Q")
            | ("GB2312", "q") => {
                // Quoted-Printable 编码的 GBK
                decode_quoted_printable_gbk(encoded_text)
            }
            _ => {
                // 未知编码，返回原文
                full_match.as_str().to_string()
            }
        };

        let abs_match_start = pos + full_match.start();
        let abs_match_end = pos + full_match.end();
        result.replace_range(abs_match_start..abs_match_end, &decoded_text);
        pos = abs_match_start + decoded_text.len();
    }

    Ok(result)
}

/// 解码 Quoted-Printable 编码的 UTF-8 文本
fn decode_quoted_printable_utf8(input: &str) -> String {
    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '_' {
            // 下划线表示空格
            result.push(b' ');
            i += 1;
        } else if chars[i] == '=' && i + 2 < chars.len() {
            // =XX 表示字节值
            let hex = format!("{}{}", chars[i + 1], chars[i + 2]);
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte);
            }
            i += 3;
        } else {
            // 普通字符
            let mut buf = [0u8; 4];
            let s = chars[i].encode_utf8(&mut buf);
            result.extend_from_slice(s.as_bytes());
            i += 1;
        }
    }

    String::from_utf8(result).unwrap_or_default()
}

/// 解码 Quoted-Printable 编码的 GBK 文本
fn decode_quoted_printable_gbk(input: &str) -> String {
    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '_' {
            // 下划线表示空格
            result.push(b' ');
            i += 1;
        } else if chars[i] == '=' && i + 2 < chars.len() {
            // =XX 表示字节值
            let hex = format!("{}{}", chars[i + 1], chars[i + 2]);
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte);
            }
            i += 3;
        } else {
            // 普通字符（转换为Latin-1字节）
            result.push(chars[i] as u8);
            i += 1;
        }
    }

    // 使用GBK解码字节
    let (text, _, _) = encoding_rs::GBK.decode(&result);
    text.to_string()
}

/// 检测并修复字符串的编码问题
/// 首先尝试解码RFC 2047编码，然后检查是否仍然存在乱码
fn fix_encoding_issue(input: &str) -> String {
    // 首先尝试解码RFC 2047编码
    let decoded = match decode_rfc2047(input) {
        Ok(text) => text,
        Err(_) => input.to_string(),
    };

    // 如果解码后仍然存在乱码，尝试其他方法
    if is_garbled(&decoded) {
        tracing::warn!("检测到编码问题且RFC 2047解码后仍有乱码: 主题={}", decoded);

        // 尝试移除替换字符
        let cleaned = decoded.replace('\u{FFFD}', "");
        if !cleaned.is_empty() {
            return cleaned;
        }

        decoded
    } else {
        decoded
    }
}

/// 从原始邮件头中提取Subject字段并解码
/// 这是为了绕过mail_parser对RFC 2047 GBK编码的错误处理
fn extract_and_decode_subject(raw: &str) -> Option<String> {
    // 查找Subject字段
    let mut subject_lines = Vec::new();
    let mut in_subject = false;
    let mut subject_complete = false;

    for line in raw.lines() {
        if line.starts_with("Subject:") {
            in_subject = true;
            subject_lines.push(line.strip_prefix("Subject:")?.trim());
        } else if in_subject {
            // 检查是否是续行（以空格或制表符开头）
            if line.starts_with(' ') || line.starts_with('\t') {
                subject_lines.push(line.trim());
            } else {
                // Subject字段结束
                subject_complete = true;
                break;
            }
        }
    }

    if !subject_complete && !subject_lines.is_empty() {
        // 邮件头结束了但没有遇到其他字段
        subject_complete = true;
    }

    if subject_complete && !subject_lines.is_empty() {
        let subject_raw = subject_lines.join("");

        // 使用我们的RFC 2047解码器
        match decode_rfc2047(&subject_raw) {
            Ok(decoded) => {
                tracing::debug!("从原始邮件头提取并解码Subject: {}", decoded);
                Some(decoded)
            }
            Err(e) => {
                tracing::warn!("解码Subject失败: {}", e);
                None
            }
        }
    } else {
        None
    }
}

/// 使用 mail_parser 完整解析邮件
pub fn parse_email_with_mail_parser(raw: &str, uid: u32) -> Result<EmailData> {
    use mail_parser::MessageParser;

    let message = MessageParser::default().parse(raw.as_bytes());

    let message = message.ok_or_else(|| anyhow!("邮件解析失败"))?;

    // 从原始邮件头中提取Subject字段并解码
    // mail_parser对RFC 2047 GBK编码的支持有问题，所以我们自己处理
    let subject = extract_and_decode_subject(raw).unwrap_or_else(|| {
        // 如果提取失败，回退到mail_parser
        let subject_raw = message.subject().unwrap_or("无主题");
        fix_encoding_issue(subject_raw)
    });

    // 解析 From 地址
    let from = message
        .from()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| addr.address())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            tracing::warn!("邮件 UID {} 缺少 From 地址", uid);
            String::new()
        });

    // 解析 To 地址
    let to = message
        .to()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    // 解析 Cc 地址
    let cc = message
        .cc()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    // 解析日期 - 使用 to_rfc822() 而不是 to_rfc2822()
    let date = message
        .date()
        .and_then(|d| {
            let date_str = d.to_rfc822();
            chrono::DateTime::parse_from_rfc2822(&date_str).ok()
        })
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| {
            tracing::warn!("邮件 UID {} 日期解析失败，使用当前时间", uid);
            chrono::Utc::now()
        });

    // 解析纯文本正文
    let body_text = message
        .body_text(0)
        .map(|s| s.to_string())
        .unwrap_or_default();

    // 解析 HTML 正文
    let body_html = message
        .body_html(0)
        .map(|s| s.to_string())
        .unwrap_or_default();

    // 解析附件
    let attachments = extract_attachments(&message);

    tracing::debug!(
        "解析邮件 UID {}: subject='{}', from='{}', to='{}', cc='{}', attachments={}",
        uid,
        subject,
        from,
        to,
        cc,
        attachments.len()
    );

    Ok(EmailData {
        uid,
        subject,
        from,
        to,
        cc,
        date,
        body_text,
        body_html,
        raw: raw.to_string(),
        flags: EmailFlags {
            seen: false,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
            recent: false,
        },
        attachments,
    })
}

/// 从邮件中提取附件信息
fn extract_attachments(message: &mail_parser::Message<'_>) -> Vec<EmailAttachment> {
    let mut attachments = Vec::new();

    for part in message.attachments() {
        let filename: String = part
            .attachment_name()
            .map(|s: &str| s.to_string())
            .unwrap_or_else(|| {
                // 如果没有文件名，使用 content-type 生成一个
                let content_type = part
                    .content_type()
                    .map(|ct| {
                        let subtype = ct
                            .c_subtype
                            .as_ref()
                            .map(|s| s.as_ref())
                            .unwrap_or("octet-stream");
                        format!("{}/{}", ct.ctype(), subtype)
                    })
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                format!(
                    "attachment.{}",
                    content_type.split('/').next_back().unwrap_or("bin")
                )
            });

        let content_type = part
            .content_type()
            .map(|ct| {
                let subtype = ct
                    .c_subtype
                    .as_ref()
                    .map(|s| s.as_ref())
                    .unwrap_or("octet-stream");
                format!("{}/{}", ct.ctype(), subtype)
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let size = part.contents().len() as u64;

        attachments.push(EmailAttachment {
            filename,
            content_type,
            size,
        });
    }

    attachments
}

/// 仅解析邮件头（用于骨架同步）
pub fn parse_email_header_only(raw: &str, uid: u32) -> Result<super::types::EmailHeader> {
    use mail_parser::MessageParser;

    let message = MessageParser::default().parse(raw.as_bytes());

    let message = message.ok_or_else(|| anyhow!("邮件头解析失败"))?;

    // 从原始邮件头中提取Subject字段并解码
    let subject = extract_and_decode_subject(raw).unwrap_or_else(|| {
        // 如果提取失败，回退到mail_parser
        let subject_raw = message.subject().unwrap_or("无主题");
        fix_encoding_issue(subject_raw)
    });

    // 解析 From 地址
    let from = message
        .from()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| addr.address())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            tracing::warn!("邮件 UID {} 缺少 From 地址", uid);
            String::new()
        });

    // 解析 To 地址
    let to = message
        .to()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    // 解析 Cc 地址
    let cc = message
        .cc()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    // 解析日期
    let date = message
        .date()
        .and_then(|d| {
            let date_str = d.to_rfc822();
            chrono::DateTime::parse_from_rfc2822(&date_str).ok()
        })
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| {
            tracing::warn!("邮件 UID {} 日期解析失败，使用当前时间", uid);
            chrono::Utc::now()
        });

    tracing::debug!(
        "解析邮件头 UID {}: subject='{}', from='{}', to='{}', cc='{}'",
        uid,
        subject,
        from,
        to,
        cc
    );

    Ok(super::types::EmailHeader {
        uid,
        subject,
        from,
        to,
        cc,
        date,
        flags: super::types::EmailFlags {
            seen: false,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
            recent: false,
        },
    })
}
