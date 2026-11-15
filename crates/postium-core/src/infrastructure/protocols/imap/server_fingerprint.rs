//! IMAP 服务器软件指纹（provider 适配）
//!
//! 依据：网易邮箱大师 mastercore.dll `imap_session.cpp` 的 Connect 流程
//! （FUN_10932230 @ 0x10932230）：登录前把服务器 greeting 与一张硬编码的
//! 服务器软件名单做匹配，按命中结果设置会话级兼容开关。名单（字符串表
//! 0x1119b2bc 起，逐项以 NUL 分隔）：
//!
//! - "Coremail System IMap Server Ready"（Coremail：网易企业邮、腾讯企业邮等）
//! - "Courier-IMAP"
//! - "Hermes"
//! - "QIP IMAP server"
//! - "WinWebMail"
//! - "Microsoft Exchange IMAP4"
//!
//! 另有按域名的特殊处理名单（0x11110fac）：yahoo.com / qq.com / foxmail.com。
//!
//! 我们目前不照搬它的全部兼容开关（未观测到具体差异行为），只取两件可证实的：
//! 1. 记录命中的服务器软件，写入日志（连接问题可按服务器归类排查）；
//! 2. 用于连接建立阶段的诊断信息（`ServerFingerprint`）。

/// 服务器软件指纹：greeting 命中的已知服务器实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSoftware {
    Coremail,
    CourierImap,
    Hermes,
    Qip,
    WinWebMail,
    Exchange,
    /// 未命中任何已知名单（含 Gmail/Dovecot/QQ/163 等，行为按 RFC 走）
    Unknown,
}

/// 邮箱大师那张表，顺序即其在字符串表中的出现顺序。
const KNOWN: &[(&str, ServerSoftware)] = &[
    ("Coremail System IMap Server Ready", ServerSoftware::Coremail),
    ("Courier-IMAP", ServerSoftware::CourierImap),
    ("Hermes", ServerSoftware::Hermes),
    ("QIP IMAP server", ServerSoftware::Qip),
    ("WinWebMail", ServerSoftware::WinWebMail),
    ("Microsoft Exchange IMAP4", ServerSoftware::Exchange),
];

/// 对 greeting 文本做指纹识别（大小写不敏感的子串匹配，与邮箱大师一致）。
pub fn fingerprint_greeting(greeting: &str) -> ServerSoftware {
    for (needle, sw) in KNOWN {
        if greeting.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) {
            return *sw;
        }
    }
    ServerSoftware::Unknown
}

impl ServerSoftware {
    pub fn label(self) -> &'static str {
        match self {
            ServerSoftware::Coremail => "Coremail",
            ServerSoftware::CourierImap => "Courier-IMAP",
            ServerSoftware::Hermes => "Hermes",
            ServerSoftware::Qip => "QIP",
            ServerSoftware::WinWebMail => "WinWebMail",
            ServerSoftware::Exchange => "Microsoft Exchange",
            ServerSoftware::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coremail_greeting_matches() {
        // 真实 Coremail greeting 样式
        let g = "* OK Coremail System IMap Server Ready mimap17";
        assert_eq!(fingerprint_greeting(g), ServerSoftware::Coremail);
    }

    #[test]
    fn courier_and_exchange_match() {
        assert_eq!(fingerprint_greeting("* OK Courier-IMAP ready."), ServerSoftware::CourierImap);
        assert_eq!(
            fingerprint_greeting("* OK Microsoft Exchange IMAP4 ready"),
            ServerSoftware::Exchange
        );
    }

    #[test]
    fn gmail_and_dovecot_are_unknown() {
        assert_eq!(fingerprint_greeting("* OK Gimap ready"), ServerSoftware::Unknown);
        assert_eq!(fingerprint_greeting("* OK Dovecot ready."), ServerSoftware::Unknown);
        assert_eq!(fingerprint_greeting(""), ServerSoftware::Unknown);
    }
}
