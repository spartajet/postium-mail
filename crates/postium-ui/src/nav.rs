//! 导航目标：侧栏点击 → 主区域视图切换。

use postium_core::service::email_service::EmailCategory;

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
