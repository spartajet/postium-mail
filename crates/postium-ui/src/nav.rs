//! 导航目标：侧栏点击 → 主区域视图切换。

use crate::i18n::t;
use postium_core::service::email_service::EmailCategory;
use postium_core::utf7_imap_folder_decode;

#[derive(Clone, PartialEq, Debug)]
pub enum NavTarget {
    /// 全部账号聚合收件箱
    AllInbox,
    /// 虚拟分类（星标/已发送/草稿/垃圾/回收站）
    Category(EmailCategory),
    /// 具体账号的具体文件夹
    AccountFolder { account_id: i32, folder: String },
    /// 标签
    Label(i32),
    Calendar,
    Workflow,
    /// 联系人
    Contacts,
    #[allow(dead_code)]
    Settings,
}

/// 分类对应的侧栏 i18n 键。
pub fn category_label_key(category: &EmailCategory) -> &'static str {
    match category {
        EmailCategory::Inbox => "sidebar.inbox",
        EmailCategory::Starred => "sidebar.starred",
        EmailCategory::Sent => "sidebar.sent",
        EmailCategory::Drafts => "sidebar.drafts",
        EmailCategory::Spam => "sidebar.spam",
        EmailCategory::Trash => "sidebar.trash",
        _ => "sidebar.inbox",
    }
}

/// 真实文件夹名 → 侧栏显示 i18n 键（不认识的返回 None，原样显示文件夹名）。
pub fn folder_label_key(folder: &str) -> Option<&'static str> {
    match folder.to_ascii_uppercase().as_str() {
        "INBOX" => Some("sidebar.inbox"),
        "SENT" | "SENT MESSAGES" => Some("sidebar.sent"),
        "DRAFTS" => Some("sidebar.drafts"),
        "JUNK" | "SPAM" => Some("sidebar.spam"),
        "TRASH" | "DELETED MESSAGES" => Some("sidebar.trash"),
        "ARCHIVE" => Some("sidebar.archive"),
        _ => None,
    }
}

/// 侧栏文件夹显示名（对照邮箱大师的两步法，mastercore 0x11162600 表 + mUTF-7 解码器，fnd_14）：
/// 1. wire 名命中已知表（含 mUTF-7 编码名 &XfJT0ZAB- 等）→ 直接给本地化显示名；
/// 2. 其余按 RFC 3502 modified UTF-7 解码（&Wo… → 中文），解码前后一致则原样返回。
///
/// wire 名仍用于 IMAP 操作（SELECT/FETCH），只有显示走这条路径。
pub fn folder_display_name(folder: &str) -> String {
    // 已知 wire 名 → 本地化名（大师做法：直接查表，不解码）。
    // mUTF-7 wire 名大小写敏感（base64）：先精确匹配，再匹配 ASCII 名
    // 的大写形式（INBOX/Sent 等服务器可能任意大小写）。
    let upper = folder.to_ascii_uppercase();
    let known: Option<String> = match folder {
        "&XfJT0ZAB-" | "&XfJ2fJAB-" | "&XfJSIJZk-" | "&XfJSKpZk-" => Some(t("sidebar.sent").to_string()),
        "&g0l6P3ux-" | "&Tgtm+DBN-" | "&kAFP4W4IMH8-" => Some(t("sidebar.drafts").to_string()),
        "&j,dg0TDhMPww6w-" | "&dcVr0mWHTvZZOQ-" | "&dcVr0mqUaEhZPg-" => {
            Some(t("sidebar.spam").to_string())
        }
        "&V4NXPpCuTvY-" | "&V4NXPpD1TvY-" | "&UkqWZG4IMH8-" => Some(t("sidebar.trash").to_string()),
        "&WkBHCG-7zTXhZg-" => Some(t("sidebar.archive").to_string()),
        _ => match upper.as_str() {
            "INBOX" => Some(t("sidebar.inbox").to_string()),
            "SENT" | "SENT MESSAGES" | "SENT ITEMS" | "SENT MAIL" => Some(t("sidebar.sent").to_string()),
            "DRAFTS" | "DRAFT" => Some(t("sidebar.drafts").to_string()),
            "JUNK" | "SPAM" => Some(t("sidebar.spam").to_string()),
            "TRASH" | "DELETED" | "DELETED MESSAGES" => Some(t("sidebar.trash").to_string()),
            "ARCHIVE" | "ALL MAIL" => Some(t("sidebar.archive").to_string()),
            _ => None,
        },
    };
    if let Some(label) = known {
        return label;
    }
    // 未命中：modified UTF-7 解码（&Wo…- 序列 → 中文），解码失败原样返回
    let decoded = utf7_imap_folder_decode(folder);
    if decoded == folder {
        folder.to_string()
    } else {
        decoded
    }
}

/// 侧栏文件夹排序（对照大师 Qt 树模型角色键 0x113e2230，fnd_14）：
/// INBOX=10 → Sent=30 → Drafts=32 → Trash=40 → Junk=50 → Archive=55 →
/// 未知文件夹在已分类之后，同组内按解码名排序。
/// 注意：wire 名比较用小写（&XfJT0ZAB- 经 to_ascii_lowercase）。
pub fn folder_sort_key(folder: &str) -> (u8, String) {
    let lower = folder.to_ascii_lowercase();
    let rank: u8 = if lower == "inbox" {
        10
    } else if lower.contains("sent")
        || matches!(lower.as_str(), "&xfjt0zab-" | "&xfj2fjab-" | "&xfjsijzk-" | "&xfjskpzk-")
    {
        30
    } else if lower.contains("draft")
        || matches!(lower.as_str(), "&g0l6p3ux-" | "&tgtm+dbn-" | "&kafp4w4imh8-")
    {
        32
    } else if lower.contains("trash")
        || lower.contains("deleted")
        || matches!(lower.as_str(), "&v4nxppcutvy-" | "&v4nxppd1tvy-" | "&ukqwzg4imh8-")
    {
        40
    } else if lower == "junk"
        || lower == "spam"
        || matches!(
            lower.as_str(),
            "&j,dg0tdhmpww6w-" | "&dcvr0mwhtvzzoq-" | "&dcvr0mquaehzpg-"
        )
    {
        50
    } else if lower == "archive" || lower == "all mail" || lower == "&wkbhcg-7ztxhzg-" {
        55
    } else {
        90 // 未知/自定义文件夹：排标准文件夹之后（大师 90=广告位）
    };
    (rank, folder_display_name(folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 网易系 wire 名（大师 0x11162600 表，fnd_14）→ 本地化显示名
    #[test]
    fn netease_wire_names_display_as_localized() {
        assert_eq!(folder_display_name("&XfJT0ZAB-"), t("sidebar.sent").to_string());
        assert_eq!(folder_display_name("&g0l6P3ux-"), t("sidebar.drafts").to_string());
        assert_eq!(
            folder_display_name("&j,dg0TDhMPww6w-"),
            t("sidebar.spam").to_string()
        );
        assert_eq!(
            folder_display_name("&V4NXPpCuTvY-"),
            t("sidebar.trash").to_string()
        );
        // 已发送变体与俄语垃圾箱 wire 名也命中
        assert_eq!(folder_display_name("&XfJ2fJAB-"), t("sidebar.sent").to_string());
    }

    /// 表外的 mUTF-7 编码名 → 解码显示（&Wo…- 不再原样漏出）
    #[test]
    fn unknown_utf7_folder_decodes_for_display() {
        // &ZeVnLIqe- 经 modified UTF-7 解码为 "日本語"（测试确定性基准）
        assert_eq!(folder_display_name("&ZeVnLIqe-"), "日本語");
        // 纯 ASCII 名不解码（前后一致原样返回）
        assert_eq!(folder_display_name("Projects"), "Projects");
    }

    /// 排序对照大师角色键：INBOX(10) < Sent(30) < Drafts(32) < Trash(40) < Junk(50) < 自定义(90)
    #[test]
    fn sort_follows_master_role_keys() {
        let mut names = vec![
            "自定义项目".to_string(),
            "&XfJT0ZAB-".to_string(), // 已发送
            "&j,dg0TDhMPww6w-".to_string(), // 垃圾邮件
            "INBOX".to_string(),
            "&g0l6P3ux-".to_string(), // 草稿箱
            "&V4NXPpCuTvY-".to_string(), // 已删除
        ];
        names.sort_by_key(|n| folder_sort_key(n));
        assert_eq!(
            names,
            vec![
                "INBOX".to_string(),
                "&XfJT0ZAB-".to_string(),
                "&g0l6P3ux-".to_string(),
                "&V4NXPpCuTvY-".to_string(),
                "&j,dg0TDhMPww6w-".to_string(),
                "自定义项目".to_string(),
            ]
        );
    }
}
