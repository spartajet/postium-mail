//! 邮件会话线程化：归组决策纯函数。
//!
//! 三分支短路序：头链（In-Reply-To/References）命中 → 主题兜底（规范化主题 +
//! 参与者交集）→ 新建会话。

pub mod normalize;

pub use normalize::{
    MAX_IN_REPLY_TO_STORED_BYTES, normalize_in_reply_to_stored_value, normalize_subject,
    parse_message_ids,
};

/// 归组决策的输入快照。
pub struct ThreadingInput<'a> {
    /// 本封 Message-ID（不含尖括号语义也可，统一小写）。
    pub message_id: Option<&'a str>,
    /// 原始主题（规范化在 [`resolve_thread`] 内部完成）。
    pub subject: Option<&'a str>,
    /// In-Reply-To + References 的 Message-ID 并集（小写、含尖括号剥离）。
    pub chain_ids: Vec<String>,
    /// 本封 发+收+抄 邮箱小写。
    pub participants: std::collections::BTreeSet<String>,
}

/// 归组决策结果：归入既有会话（携带 thread_id）或新建会话。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadResolution {
    /// 归入既有会话。
    Existing(i64),
    /// 新建会话。
    New,
}

/// 归组所需的存量数据查找接口（由存储层实现）。
pub trait ThreadLookup {
    /// 头链：任一 id 命中存量邮件的 message_id → 其 thread_id
    fn thread_of_message_id(&mut self, ids: &[String]) -> Option<i64>;
    /// 主题兜底：subject_key 相同的候选线程（updated_at DESC）中参与者有交集者
    fn thread_by_subject_and_participant(
        &mut self,
        subject_key: &str,
        participants: &std::collections::BTreeSet<String>,
    ) -> Option<i64>;
}

/// 按三分支短路序决策会话归组：
///
/// 1. **头链命中**：`chain_ids` 非空且任一 id 命中存量邮件 → 其 thread_id，
///    不再走主题查找；
/// 2. **主题兜底**：规范化后的主题键非空时，查同主题候选线程中参与者有交集者；
/// 3. **新建**：以上均未命中，或主题键为空（None 或规范化后空串恒不参与主题匹配）。
pub fn resolve_thread<'a>(
    input: &ThreadingInput<'a>,
    lookup: &mut dyn ThreadLookup,
) -> ThreadResolution {
    if !input.chain_ids.is_empty() {
        if let Some(thread_id) = lookup.thread_of_message_id(&input.chain_ids) {
            return ThreadResolution::Existing(thread_id);
        }
    }
    let subject_key = normalize_subject(input.subject.unwrap_or(""));
    if subject_key.is_empty() {
        return ThreadResolution::New;
    }
    match lookup.thread_by_subject_and_participant(&subject_key, &input.participants) {
        Some(thread_id) => ThreadResolution::Existing(thread_id),
        None => ThreadResolution::New,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// brief 规定的 MockLookup：记录调用以断言短路语义。
    #[derive(Default)]
    struct MockLookup {
        chain_hit: Option<i64>,
        subject_hit: Option<i64>,
        chain_calls: usize,
        subject_calls: usize,
        last_subject_key: Option<String>,
        last_participants: Option<BTreeSet<String>>,
    }

    impl ThreadLookup for MockLookup {
        fn thread_of_message_id(&mut self, _ids: &[String]) -> Option<i64> {
            self.chain_calls += 1;
            self.chain_hit
        }

        fn thread_by_subject_and_participant(
            &mut self,
            subject_key: &str,
            participants: &BTreeSet<String>,
        ) -> Option<i64> {
            self.subject_calls += 1;
            self.last_subject_key = Some(subject_key.to_string());
            self.last_participants = Some(participants.clone());
            self.subject_hit
        }
    }

    fn input_with_chain(chain: Vec<&str>) -> ThreadingInput<'static> {
        ThreadingInput {
            message_id: None,
            subject: None,
            chain_ids: chain.into_iter().map(str::to_string).collect(),
            participants: BTreeSet::new(),
        }
    }

    fn input_with_subject(subject: &'static str) -> ThreadingInput<'static> {
        ThreadingInput {
            message_id: None,
            subject: Some(subject),
            chain_ids: Vec::new(),
            participants: BTreeSet::new(),
        }
    }

    fn participants_from(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // ---------- 分支 1：头链命中 ----------

    #[test]
    fn resolve_prefers_chain_and_short_circuits() {
        let mut lk = MockLookup::default(); // 记录调用
        lk.chain_hit = Some(7);
        let res = resolve_thread(&input_with_chain(vec!["<a@b>"]), &mut lk);
        assert!(matches!(res, ThreadResolution::Existing(7)));
        assert_eq!(lk.subject_calls, 0, "链命中后不得再走主题查找");
        assert_eq!(lk.chain_calls, 1);
    }

    #[test]
    fn resolve_empty_chain_skips_lookup_entirely() {
        // 空 chain 不触发任何 lookup 调用（无可归组依据）
        let mut lk = MockLookup::default();
        lk.chain_hit = Some(7);
        let res = resolve_thread(&input_with_subject("hello"), &mut lk);
        assert!(matches!(res, ThreadResolution::New));
        assert_eq!(lk.chain_calls, 0, "空链不应触发头链查找");
    }

    // ---------- 分支 2：链未中 + 空主题键 ----------

    #[test]
    fn resolve_chain_miss_with_empty_subject_key_is_new() {
        let mut lk = MockLookup::default();
        lk.chain_hit = None;
        lk.subject_hit = Some(9); // 即使 mock 声称主题可命中
        let mut input = input_with_chain(vec!["<a@b>"]);
        input.subject = Some("  Re:   "); // 规范化后为空串
        let res = resolve_thread(&input, &mut lk);
        assert!(matches!(res, ThreadResolution::New));
        assert_eq!(lk.subject_calls, 0, "空主题键恒不参与主题匹配");
    }

    #[test]
    fn resolve_empty_subject_never_matches_by_subject() {
        // subject 为空串（非 None）：规范化后为空键 → New，恒不触发主题查找
        let mut lk = MockLookup::default();
        lk.subject_hit = Some(9);
        let res = resolve_thread(&input_with_subject(""), &mut lk);
        assert!(matches!(res, ThreadResolution::New));
        assert_eq!(lk.subject_calls, 0, "空主题键恒不参与主题匹配");
    }

    #[test]
    fn resolve_subject_none_chain_miss_is_new() {
        // subject 为 None + 链 miss：unwrap_or("") 归一为空主题键 → New，且不得触发主题查找。
        // 若实现中的 unwrap_or("") 被改坏（如改成 panic/跳过规范化），本测试即失败。
        let mut lk = MockLookup::default();
        lk.chain_hit = None;
        lk.subject_hit = Some(9); // 即使 mock 声称主题可命中也不得被调用
        let input = input_with_chain(vec!["<miss@x>"]); // subject 即为 None
        assert_eq!(input.subject, None, "前置：确认本用例真正持有 None 主题");
        let res = resolve_thread(&input, &mut lk);
        assert!(matches!(res, ThreadResolution::New));
        assert_eq!(lk.chain_calls, 1);
        assert_eq!(
            lk.subject_calls, 0,
            "None 主题经 unwrap_or(\"\") 后为空键，不得触发主题查找"
        );
    }

    // ---------- 分支 3：主题兜底 ----------

    #[test]
    fn resolve_subject_hit_without_intersection_is_new() {
        // lookup 判定同主题候选线程中无参与者交集 → New
        let mut lk = MockLookup::default();
        lk.subject_hit = None;
        let res = resolve_thread(&input_with_subject("项目评审"), &mut lk);
        assert!(matches!(res, ThreadResolution::New));
        assert_eq!(lk.subject_calls, 1);
    }

    #[test]
    fn resolve_subject_with_intersection_is_existing() {
        let mut lk = MockLookup::default();
        lk.subject_hit = Some(5);
        let res = resolve_thread(&input_with_subject("Re: 项目评审"), &mut lk);
        assert!(matches!(res, ThreadResolution::Existing(5)));
        // 传给 lookup 的是规范化后的主题键
        assert_eq!(lk.last_subject_key.as_deref(), Some("项目评审"));
    }

    #[test]
    fn resolve_chain_miss_falls_back_to_subject() {
        let mut lk = MockLookup::default();
        lk.chain_hit = None;
        lk.subject_hit = Some(3);
        let mut input = input_with_chain(vec!["<miss@x>"]);
        input.subject = Some("Re: hello");
        let res = resolve_thread(&input, &mut lk);
        assert!(matches!(res, ThreadResolution::Existing(3)));
        assert_eq!(lk.chain_calls, 1);
        assert_eq!(lk.subject_calls, 1);
        assert_eq!(lk.last_subject_key.as_deref(), Some("hello"));
    }

    #[test]
    fn resolve_forwards_participants_to_subject_lookup() {
        let mut lk = MockLookup::default();
        lk.subject_hit = Some(2);
        let mut input = input_with_subject("hello world");
        input.participants = participants_from(&["a@x", "b@y"]);
        let res = resolve_thread(&input, &mut lk);
        assert!(matches!(res, ThreadResolution::Existing(2)));
        assert_eq!(
            lk.last_participants.as_ref().unwrap(),
            &participants_from(&["a@x", "b@y"])
        );
    }

    #[test]
    fn resolve_multi_candidate_decided_by_lookup_latest() {
        // 多候选时由 lookup 决定（mock 模拟"updated_at DESC 取第一个有交集者"返回最新）
        let mut lk = MockLookup::default();
        lk.chain_hit = None;
        lk.subject_hit = Some(42); // mock 内部从 [42, 40, 39] 挑了最新的 42
        let mut input = input_with_chain(vec!["<miss@x>", "<miss2@y>"]);
        input.subject = Some("转发：周报");
        input.participants = participants_from(&["p@q"]);
        let res = resolve_thread(&input, &mut lk);
        assert!(matches!(res, ThreadResolution::Existing(42)));
        assert_eq!(lk.last_subject_key.as_deref(), Some("周报"));
    }
}
