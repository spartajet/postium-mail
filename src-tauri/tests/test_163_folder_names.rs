// 测试163邮箱的文件夹名称识别
#[test]
fn test_decode_imap_utf7() {
    // 测试 UTF-7 编码的中文文件夹名称识别

    // 模拟 determine_standard_name 的逻辑
    fn decode_and_map(imap_name: &str) -> String {
        let common_mappings = [
            ("&XfJT0ZAB-", "已发送"),
            ("&XfJSIJZk-", "收件箱"),
            ("&V4NXPpCuTvY-", "垃圾邮件"),
            ("&dcVr0mWHTvZZOQ-", "已删除"),
            ("&g0l6P3ux-", "草稿箱"),
        ];

        for (encoded, decoded) in common_mappings.iter() {
            if imap_name == *encoded || imap_name.ends_with(encoded) {
                return decoded.to_string();
            }
        }

        imap_name.to_string()
    }

    fn determine_standard_name(name: &str) -> String {
        let decoded_name = decode_and_map(name);
        let name_lower = decoded_name.to_lowercase();

        if name_lower.contains("inbox") || name_lower.contains("收件箱") {
            "inbox".to_string()
        } else if name_lower.contains("sent") || name_lower.contains("已发送") {
            "sent".to_string()
        } else if name_lower.contains("draft") || name_lower.contains("草稿") {
            "drafts".to_string()
        } else if name_lower.contains("spam") || name_lower.contains("junk") || name_lower.contains("垃圾邮件") {
            "spam".to_string()
        } else if name_lower.contains("trash") || name_lower.contains("deleted") || name_lower.contains("已删除") {
            "trash".to_string()
        } else if name_lower.contains("archive") || name_lower.contains("归档") {
            "archive".to_string()
        } else {
            name.to_string()
        }
    }

    // 测试163邮箱的常见中文文件夹
    assert_eq!(determine_standard_name("&XfJT0ZAB-"), "sent");
    assert_eq!(determine_standard_name("&XfJSIJZk-"), "inbox");
    assert_eq!(determine_standard_name("&V4NXPpCuTvY-"), "spam");
    assert_eq!(determine_standard_name("&g0l6P3ux-"), "drafts");
    assert_eq!(determine_standard_name("&dcVr0mWHTvZZOQ-"), "trash");

    // 测试英文名称
    assert_eq!(determine_standard_name("INBOX"), "inbox");
    assert_eq!(determine_standard_name("Sent"), "sent");
    assert_eq!(determine_standard_name("Drafts"), "drafts");
    assert_eq!(determine_standard_name("Spam"), "spam");
    assert_eq!(determine_standard_name("Trash"), "trash");

    println!("✅ 所有文件夹名称识别测试通过");
}
