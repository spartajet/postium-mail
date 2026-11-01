//! 主题规范化与 Message-ID 解析的纯函数。

/// 主题归一化时要剥离的前缀表（小写匹配；中英文前缀各含全/半角冒号形态）。
const SUBJECT_PREFIXES: &[&str] = &[
    "re:",
    "fwd:",
    "fw:",
    "回复：",
    "回复:",
    "答复：",
    "答复:",
    "转发：",
    "转发:",
];

/// 规范化邮件主题，作为主题兜底归组的键。
///
/// 步骤：
/// 1. ASCII 小写化（`to_ascii_lowercase`，CJK 与土耳其语等 locale 不受影响）；
/// 2. 循环剥离前导空白与前缀表中的 `Re:`/`Fwd:`/`Fw:`/`回复:`/`答复:`/`转发:`
///    （全/半角冒号均识别，允许嵌套与混排）；
/// 3. 前后 trim，结果为空则返回空串；
/// 4. 连续空白折叠为单个空格。
pub fn normalize_subject(subject: &str) -> String {
    let lower = subject.to_ascii_lowercase();
    let mut rest = lower.as_str();
    loop {
        rest = rest.trim_start();
        match SUBJECT_PREFIXES
            .iter()
            .find_map(|prefix| rest.strip_prefix(prefix))
        {
            Some(stripped) => rest = stripped,
            None => break,
        }
    }
    let body = rest.trim();
    if body.is_empty() {
        return String::new();
    }
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 解析 `In-Reply-To` / `References` 头的原始值为 Message-ID 列表。
///
/// 优先按 `<...>` 尖括号成对提取（周围噪声忽略）；完全不存在成对尖括号时，
/// 按空白分割的裸 token 兜底。结果统一 ASCII 小写并去重（保首次出现顺序）。
pub fn parse_message_ids(raw_header_value: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut has_bracket_pair = false;
    let mut rest = raw_header_value;
    while let Some(open) = rest.find('<') {
        let after_open = &rest[open + 1..];
        match after_open.find('>') {
            Some(close) => {
                has_bracket_pair = true;
                let id = after_open[..close].trim();
                if !id.is_empty() {
                    ids.push(id.to_ascii_lowercase());
                }
                rest = &after_open[close + 1..];
            }
            // 未闭合的 '<'：不再继续找括号
            None => break,
        }
    }
    if !has_bracket_pair {
        ids.extend(
            raw_header_value
                .split_whitespace()
                .map(|token| token.to_ascii_lowercase()),
        );
    }
    // 去重保序
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(id.clone())).collect()
}

/// In-Reply-To 落库串的最大字节数（写侧与同步侧 `extract_in_reply_to` 同上限）。
pub const MAX_IN_REPLY_TO_STORED_BYTES: usize = 2048;

/// 把 In-Reply-To 头原始值归一为落库形态：`<id>` 小写逗号串（去重保序），
/// 超 2048 字节按字节截断到最近字符边界（对齐同步侧 `extract_in_reply_to`
/// 的落库形态与截断语义）。写侧（草稿/发送本地行）落库前统一走本函数，
/// 保证与同步拉回的行形态一致。None/空白/无可解析 id → None。
pub fn normalize_in_reply_to_stored_value(raw: Option<&str>) -> Option<String> {
    let joined = parse_message_ids(raw?.trim())
        .iter()
        .map(|id| format!("<{id}>"))
        .collect::<Vec<_>>()
        .join(",");
    if joined.is_empty() {
        return None;
    }
    if joined.len() <= MAX_IN_REPLY_TO_STORED_BYTES {
        return Some(joined);
    }
    let mut end = MAX_IN_REPLY_TO_STORED_BYTES;
    while !joined.is_char_boundary(end) {
        end -= 1;
    }
    Some(joined[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- normalize_subject：前缀剥离（brief 逐字用例） ----------

    #[test]
    fn normalize_strips_stacked_prefixes() {
        assert_eq!(normalize_subject("Re: 回复： FWD: 项目评审"), "项目评审");
        assert_eq!(normalize_subject("  Hello   World "), "hello world");
        assert_eq!(normalize_subject(""), "");
    }

    // ---------- normalize_subject：表驱动 ----------

    #[test]
    fn normalize_prefix_table_ascii() {
        let cases = [
            ("Re: hello", "hello"),
            ("RE: hello", "hello"),
            ("re: hello", "hello"),
            ("Fwd: hello", "hello"),
            ("FWD: hello", "hello"),
            ("FW: hello", "hello"),
            ("fw: hello", "hello"),
            ("Re:hello", "hello"), // 前缀后无空格也剥
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_subject(input), expected, "input={input:?}");
        }
    }

    #[test]
    fn normalize_prefix_table_cjk() {
        let cases = [
            ("回复：项目评审", "项目评审"),
            ("回复:项目评审", "项目评审"),
            ("答复：会议纪要", "会议纪要"),
            ("答复:会议纪要", "会议纪要"),
            ("转发：周报", "周报"),
            ("转发:周报", "周报"),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_subject(input), expected, "input={input:?}");
        }
    }

    #[test]
    fn normalize_nested_mixed_prefixes() {
        assert_eq!(normalize_subject("Re: 回复： Re: x"), "x");
        assert_eq!(normalize_subject("fw: 答复: fwd: 转发： deep"), "deep");
        assert_eq!(normalize_subject("RE:FW:Re: tight"), "tight");
    }

    #[test]
    fn normalize_whitespace_handling() {
        // 前后空白剥离 + 连续空白折叠
        assert_eq!(normalize_subject("  \tHello \r\n World  "), "hello world");
        assert_eq!(normalize_subject("Re:\t 项目  评审 "), "项目 评审");
        assert_eq!(normalize_subject("a  b\t\tc\r\nd"), "a b c d");
    }

    #[test]
    fn normalize_ascii_lowercase_only() {
        // ASCII 小写化
        assert_eq!(normalize_subject("Hello WORLD"), "hello world");
        // CJK 天然不受影响
        assert_eq!(normalize_subject("项目评审"), "项目评审");
        // 混合
        assert_eq!(normalize_subject("Re: Q3 报告 Review"), "q3 报告 review");
    }

    #[test]
    fn normalize_empty_and_prefix_only() {
        assert_eq!(normalize_subject(""), "");
        assert_eq!(normalize_subject("   "), "");
        assert_eq!(normalize_subject("Re:"), "");
        assert_eq!(normalize_subject("Re:    "), "");
        assert_eq!(normalize_subject("回复："), "");
        assert_eq!(normalize_subject("  Re:  转发：  "), "");
    }

    #[test]
    fn normalize_keeps_non_prefix_lookalikes() {
        // 不是前缀表中的形态就不剥
        assert_eq!(normalize_subject("reply: hello"), "reply: hello");
        assert_eq!(normalize_subject("reviews: top"), "reviews: top");
    }

    // ---------- normalize_in_reply_to_stored_value ----------

    #[test]
    fn stored_value_normalizes_to_bracketed_lowercase_comma_form() {
        assert_eq!(
            normalize_in_reply_to_stored_value(Some("  <A@B>  <C@D> ")),
            Some("<a@b>,<c@d>".to_string())
        );
        assert_eq!(
            normalize_in_reply_to_stored_value(Some("<id1@x.com>")),
            Some("<id1@x.com>".to_string())
        );
        // 裸 token 兜底也归一
        assert_eq!(
            normalize_in_reply_to_stored_value(Some("x@y z@w")),
            Some("<x@y>,<z@w>".to_string())
        );
    }

    #[test]
    fn stored_value_none_for_missing_or_blank_input() {
        assert_eq!(normalize_in_reply_to_stored_value(None), None);
        assert_eq!(normalize_in_reply_to_stored_value(Some("")), None);
        assert_eq!(normalize_in_reply_to_stored_value(Some("   ")), None);
        assert_eq!(normalize_in_reply_to_stored_value(Some("<>")), None);
    }

    #[test]
    fn stored_value_truncates_over_2048_bytes_on_char_boundary() {
        let long = (0..200)
            .map(|i| format!("<id{i:03}@example.com>"))
            .collect::<Vec<_>>()
            .join(",");
        let stored =
            normalize_in_reply_to_stored_value(Some(&long)).expect("超长链仍应产出截断值");
        assert!(
            stored.len() <= 2048,
            "应截断到 2048 字节内: len={}",
            stored.len()
        );
        assert!(stored.starts_with("<id000@example.com>"), "截断保头部");
    }

    // ---------- parse_message_ids ----------

    #[test]
    fn parse_bracketed_ids() {
        assert_eq!(
            parse_message_ids("<a@b> <c@d>"),
            vec!["a@b".to_string(), "c@d".to_string()]
        );
        assert_eq!(
            parse_message_ids("<a@b>\r\n  <c@d>\n<e@f>"),
            vec!["a@b".to_string(), "c@d".to_string(), "e@f".to_string()]
        );
    }

    #[test]
    fn parse_bare_ids_fallback() {
        assert_eq!(
            parse_message_ids("a@b c@d"),
            vec!["a@b".to_string(), "c@d".to_string()]
        );
        assert_eq!(
            parse_message_ids("a@b\tc@d\ne@f"),
            vec!["a@b".to_string(), "c@d".to_string(), "e@f".to_string()]
        );
    }

    #[test]
    fn parse_dedup_and_lowercase() {
        assert_eq!(
            parse_message_ids("<A@B> <a@b> <C@d> <c@D>"),
            vec!["a@b".to_string(), "c@d".to_string()]
        );
        assert_eq!(
            parse_message_ids("X@Y x@y"),
            vec!["x@y".to_string()]
        );
        // 去重保序：首次出现的位置
        assert_eq!(
            parse_message_ids("<c@d> <a@b> <C@D>"),
            vec!["c@d".to_string(), "a@b".to_string()]
        );
    }

    #[test]
    fn parse_empty_input() {
        assert_eq!(parse_message_ids(""), Vec::<String>::new());
        assert_eq!(parse_message_ids("   \r\n \t "), Vec::<String>::new());
    }

    #[test]
    fn parse_brackets_take_priority_over_bare_tokens() {
        // 有成对尖括号时只取尖括号内容，周围噪声忽略
        assert_eq!(
            parse_message_ids("noise <a@b> more noise"),
            vec!["a@b".to_string()]
        );
    }
}
