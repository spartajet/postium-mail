use anyhow::{anyhow, Result};
use super::types::{EmailData, EmailFlags};

/// 使用 mail_parser 完整解析邮件
pub fn parse_email_with_mail_parser(raw: &str, uid: u32) -> Result<EmailData> {
    use mail_parser::MessageParser;

    let message = MessageParser::default().parse(raw.as_bytes());

    let message = message.ok_or_else(|| anyhow!("邮件解析失败"))?;

    // 解析主题
    let subject = message
        .subject()
        .unwrap_or("无主题")
        .to_string();

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
        .map(|addrs| addrs.iter()
            .filter_map(|addr| addr.address())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", "))
        .unwrap_or_default();

    // 解析 Cc 地址
    let cc = message
        .cc()
        .map(|addrs| addrs.iter()
            .filter_map(|addr| addr.address())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", "))
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

    tracing::debug!(
        "解析邮件 UID {}: subject='{}', from='{}', to='{}', cc='{}'",
        uid,
        subject,
        from,
        to,
        cc
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
        },
    })
}
