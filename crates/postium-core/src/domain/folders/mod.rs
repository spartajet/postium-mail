//! 文件夹识别引擎模块
//!
//! 用三层递进策略（SPECIAL-USE + 跨语言关键词 + provider 候选名）将 IMAP 真实
//! 文件夹名映射到标准类别，统一取代旧的三套 resolve_* 逻辑。

pub mod category;
pub mod keyword_table;
pub mod registry;
pub mod special_use;

pub use category::FolderCategory;
pub use registry::{FolderRegistry, RemoteFolder};
pub use special_use::SpecialUseFlag;

/// IMAP 文件夹名归一（RFC 3501 §5.1：`INBOX` 是大小写不敏感的保留名，
/// 规范形态大写）。
///
/// 落库与查询的统一出口（线上反馈⑧）：不同 IMAP 服务器 LIST 返回的收件箱
/// 形态不一（163/Outlook 实测 `inbox` 小写、保留名常作 `INBOX`），若按
/// 原名入库，`(account_id, folder, uid)` 唯一键把同一封邮件判成两条
/// （全量同步写 LIST 原名、IDLE 定向同步写保留名 → 双行）。所有写入
/// emails/sync_state 的 folder 值与按名查询的入参都必须先过本函数；
/// 其余文件夹名保持服务器原名（大小写有语义的服务器存在，不盲归一）。
pub fn normalize_imap_folder(name: &str) -> String {
    if name.eq_ignore_ascii_case("INBOX") {
        "INBOX".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inbox_variants_normalize_to_canonical() {
        for variant in ["INBOX", "inbox", "Inbox", "iNbOx"] {
            assert_eq!(normalize_imap_folder(variant), "INBOX");
        }
    }

    #[test]
    fn non_inbox_folders_keep_original_form() {
        // 大小写有语义的服务器存在（如 macOS Server 的 "Sent" vs "sent"）：
        // 仅 INBOX 保留名归一
        for kept in ["&XfJT0ZAB-", "Sent Messages", "[Gmail]/All Mail", "Archive"] {
            assert_eq!(normalize_imap_folder(kept), kept);
        }
    }
}
