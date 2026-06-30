use crate::domain::folders::FolderCategory;

/// 关键词条目：关键词 → (类别, 分数)
struct KeywordEntry {
    keyword: &'static str,
    category: FolderCategory,
    score: u16,
}

/// 跨语言文件夹名关键词表。
///
/// 参考 FairEmail GUESS_FOLDER_TYPE 思路，并**补充中文关键词**（FairEmail 缺失）。
/// 匹配为单向 `name_lower.contains(keyword)`，杜绝双向子串误判。
fn keyword_table() -> &'static [KeywordEntry] {
    &[
        // ── Sent ──
        KeywordEntry { keyword: "已发送", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "已发邮件", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "发件箱", category: FolderCategory::Sent, score: 80 },
        KeywordEntry { keyword: "sent messages", category: FolderCategory::Sent, score: 95 },
        KeywordEntry { keyword: "sent items", category: FolderCategory::Sent, score: 95 },
        KeywordEntry { keyword: "sent", category: FolderCategory::Sent, score: 90 },
        KeywordEntry { keyword: "gesendet", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "envoy", category: FolderCategory::Sent, score: 90 },
        KeywordEntry { keyword: "отправлен", category: FolderCategory::Sent, score: 90 },
        // ── Drafts ──
        KeywordEntry { keyword: "草稿箱", category: FolderCategory::Drafts, score: 100 },
        KeywordEntry { keyword: "草稿", category: FolderCategory::Drafts, score: 90 },
        KeywordEntry { keyword: "drafts", category: FolderCategory::Drafts, score: 95 },
        KeywordEntry { keyword: "draft", category: FolderCategory::Drafts, score: 85 },
        KeywordEntry { keyword: "entwürfe", category: FolderCategory::Drafts, score: 100 },
        KeywordEntry { keyword: "brouillons", category: FolderCategory::Drafts, score: 100 },
        // ── Junk ──
        KeywordEntry { keyword: "垃圾邮件", category: FolderCategory::Junk, score: 100 },
        KeywordEntry { keyword: "junk", category: FolderCategory::Junk, score: 95 },
        KeywordEntry { keyword: "spam", category: FolderCategory::Junk, score: 95 },
        KeywordEntry { keyword: "bulk mail", category: FolderCategory::Junk, score: 90 },
        // ── Trash ──
        KeywordEntry { keyword: "已删除", category: FolderCategory::Trash, score: 100 },
        KeywordEntry { keyword: "废件箱", category: FolderCategory::Trash, score: 80 },
        KeywordEntry { keyword: "deleted messages", category: FolderCategory::Trash, score: 95 },
        KeywordEntry { keyword: "deleted", category: FolderCategory::Trash, score: 90 },
        KeywordEntry { keyword: "trash", category: FolderCategory::Trash, score: 95 },
        KeywordEntry { keyword: "papierkorb", category: FolderCategory::Trash, score: 100 },
        KeywordEntry { keyword: "корзин", category: FolderCategory::Trash, score: 90 },
        // ── Archive ──
        KeywordEntry { keyword: "所有邮件", category: FolderCategory::Archive, score: 80 },
        KeywordEntry { keyword: "归档", category: FolderCategory::Archive, score: 100 },
        KeywordEntry { keyword: "已归档", category: FolderCategory::Archive, score: 100 },
        KeywordEntry { keyword: "all mail", category: FolderCategory::Archive, score: 80 },
        KeywordEntry { keyword: "archived", category: FolderCategory::Archive, score: 90 },
        KeywordEntry { keyword: "archive", category: FolderCategory::Archive, score: 95 },
        KeywordEntry { keyword: "archiv", category: FolderCategory::Archive, score: 90 },
    ]
}

/// 对**已解码**的文件夹名做单向关键词猜测，返回最高分的 (类别, 分数)。
///
/// 多个关键词命中时取分数最高；同分按类别优先级（`FolderCategory::priority`）。
pub fn guess_category(decoded_name: &str) -> Option<(FolderCategory, u16)> {
    let name_lower = decoded_name.to_lowercase();
    keyword_table()
        .iter()
        .filter(|e| name_lower.contains(e.keyword))
        .max_by_key(|e| (e.score, u8::MAX - e.category.priority()))
        .map(|e| (e.category, e.score))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_chinese_sent() {
        assert_eq!(guess_category("已发送"), Some((FolderCategory::Sent, 100)));
    }

    #[test]
    fn matches_english_sent_messages_case_insensitive() {
        assert_eq!(guess_category("Sent Messages"), Some((FolderCategory::Sent, 95)));
    }

    #[test]
    fn matches_netease_drafts_utf7_decoded() {
        // 网易草稿箱 IMAP-UTF-7 "&g0l6P3ux-" 解码后为 "草稿箱"
        assert_eq!(guess_category("草稿箱"), Some((FolderCategory::Drafts, 100)));
    }

    #[test]
    fn matches_junk_not_confused_with_trash() {
        // "垃圾邮件" 应归 Junk，不是 Trash
        assert_eq!(guess_category("垃圾邮件"), Some((FolderCategory::Junk, 100)));
    }

    #[test]
    fn returns_none_for_unrelated_folder() {
        assert_eq!(guess_category("项目文档"), None);
    }

    #[test]
    fn higher_score_wins_on_multiple_matches() {
        // "已发送" 同时含 "sent"? 不含。构造一个含多词的：
        // "Sent 已发送" 含 "sent"(90) 和 "已发送"(100)，应取 100
        assert_eq!(guess_category("Sent 已发送"), Some((FolderCategory::Sent, 100)));
    }

    #[test]
    fn no_bidirectional_false_positive() {
        // 关键词单向匹配：一个叫 "Sent Archive Backup" 的自定义文件夹
        // 含 "sent"(90) 和 "archive"(95)，按分数 archive 胜，不会双向乱判
        let result = guess_category("Sent Archive Backup");
        assert_eq!(result.map(|(c, _)| c), Some(FolderCategory::Archive));
    }
}
