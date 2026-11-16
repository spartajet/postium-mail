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

/// 会话级兼容开关组（邮箱大师 Connect FUN_10932230 在 greeting 指纹后
/// 写入 session 对象 7 个布尔位；本结构体是它的可读版本，位名即大师位）。
///
/// 逆向证据（mastercore.dll 5.5.3.1009，见 MECHANICS.md §9）：
///
/// | 大师位 | 匹配串 | 读点（已证实的用途） |
/// |---|---|---|
/// | `is_coremail` (0xfb) | "Coremail System IMap Server Ready" | 见 fetch 旗标清理路径 FUN_10acbd40（与 yahoo/qq 域名位共用清旗标逻辑） |
/// | `is_courier` (0x106) | "Courier-IMAP" | FUN_109372a0：Courier 连接清理时对挂起命令强制收尾（FUN_10a971a0，对响应流 `poll`<0 时销毁内部读缓冲） |
/// | `is_hermes` (0xf6) | "Hermes" | FUN_10931bf0：能力协商——**非 Hermes 且服务器宣告 CONDSTORE(0x1000) 才启用 CONDSTORE**（Hermes 上禁用） |
/// | `is_qip` (0xf8) | "QIP IMAP server" | 未找到独立读点（写入后无 imap_session 内消费者） |
/// | `is_webmail_cn` (0xf9) | 域名 yahoo.com/qq.com/foxmail.com（9 字节串 0x11110fac） | FUN_10acbd40/10acc380：响应到达时清对应延迟位 |
/// | `is_winwebmail` (0x10d) | "WinWebMail"（**反相**：非命中才置位） | 读点仅见于 Qt 树模型（0x103edc88，QStandardItem，非本会话对象）——判定为无效位 |
/// | `is_exchange` (0xfc) | "Microsoft Exchange IMAP4" | FUN_10acbd40/10acc380：0xfc 非零时走单独通知分支（错误码 0x2714 网络重试路径） |
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerQuirks {
    /// 0xfb：Coremail 系（网易/腾讯企业邮等）
    pub is_coremail: bool,
    /// 0x106：Courier-IMAP（挂起命令需强制收尾）
    pub is_courier: bool,
    /// 0xf6：Hermes（禁用 CONDSTORE）
    pub is_hermes: bool,
    /// 0xf8：QIP IMAP server
    pub is_qip: bool,
    /// 0xf9：域名 yahoo.com / qq.com / foxmail.com
    pub is_webmail_cn: bool,
    /// 0x10d：WinWebMail（大师为反相位；未证实有效用途）
    pub is_winwebmail: bool,
    /// 0xfc：Microsoft Exchange IMAP4（单独通知分支）
    pub is_exchange: bool,
}

impl ServerQuirks {
    /// greeting 指纹 + 域名双条件 → 开关组（与大师 Connect 相同的输入）。
    pub fn detect(greeting: &str, domain: Option<&str>) -> Self {
        let sw = fingerprint_greeting(greeting);
        let mut q = Self::default();
        match sw {
            ServerSoftware::Coremail => q.is_coremail = true,
            ServerSoftware::CourierImap => q.is_courier = true,
            ServerSoftware::Hermes => q.is_hermes = true,
            ServerSoftware::Qip => q.is_qip = true,
            // 大师 0x10d 是反相位：非 WinWebMail 才置位。postium 只在命中
            // 时标记（反相位未证实有消费者，不模拟）
            ServerSoftware::WinWebMail => q.is_winwebmail = true,
            ServerSoftware::Exchange => q.is_exchange = true,
            ServerSoftware::Unknown => {}
        }
        if let Some(d) = domain {
            let dl = d.to_ascii_lowercase();
            // 域名位（大师串表 0x11110fac：yahoo.com/qq.com/foxmail.com）：
            // 按片段匹配，覆盖 imap.qq.com、mail.foxmail.com.cn 等变体
            if dl.contains("qq.com") || dl.contains("foxmail") || dl.contains("yahoo.com") {
                q.is_webmail_cn = true;
            }
        }
        q
    }

    /// 大师 FUN_10931bf0 能力协商的门控规则：CONDSTORE 仅在「服务器宣告
    /// CONDSTORE 且非 Hermes」时启用。
    pub fn enable_condstore(&self, server_declares_condstore: bool) -> bool {
        server_declares_condstore && !self.is_hermes
    }
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

    #[test]
    fn quirks_detect_matches_master_table() {
        // 六串各自点亮对应位（大师 FUN_10932230 的写入方向）
        let coremail = ServerQuirks::detect("* OK Coremail System IMap Server Ready", None);
        assert!(coremail.is_coremail && !coremail.is_exchange);
        let exchange = ServerQuirks::detect("* OK Microsoft Exchange IMAP4 ready", None);
        assert!(exchange.is_exchange && !exchange.is_coremail);
        let courier = ServerQuirks::detect("* OK Courier-IMAP ready.", None);
        assert!(courier.is_courier);
        // 域名位：yahoo/qq/foxmail（0x11110fac）
        let qq = ServerQuirks::detect("* OK QQMail IMAP4 Server ready", Some("imap.qq.com"));
        assert!(qq.is_webmail_cn && !qq.is_coremail);
        let fox = ServerQuirks::detect("* OK ready", Some("mail.foxmail.com.cn"));
        assert!(fox.is_webmail_cn);
        let other = ServerQuirks::detect("* OK Dovecot ready.", Some("imap.gmail.com"));
        assert!(!other.is_webmail_cn && !other.is_exchange);
    }

    #[test]
    fn condstore_gating_follows_hermes_rule() {
        // 大师 FUN_10931bf0：CONDSTORE = 服务器宣告 && 非 Hermes
        let hermes = ServerQuirks::detect("* OK Hermes IMAP ready", None);
        assert!(!hermes.enable_condstore(true)); // Hermes 即使宣告也禁用
        let plain = ServerQuirks::detect("* OK Dovecot ready.", None);
        assert!(plain.enable_condstore(true));
        assert!(!plain.enable_condstore(false)); // 未宣告不启用
    }
}
