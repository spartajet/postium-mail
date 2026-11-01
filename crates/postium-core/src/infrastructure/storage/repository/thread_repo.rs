//! 会话线程仓库（Thread Repository）
//!
//! 负责 `threads` 表的会话归组：将邮件按归组决策写入会话线程。
//! 归组决策复用领域层纯函数 [`crate::domain::threading::resolve_thread`]，
//! 本模块提供存储侧的 [`ThreadLookup`] 实现与会话写入：
//!
//! - 存量回填 [`backfill_threading`]：历史上未归组的邮件（`thread_id IS NULL`）
//!   按“规范化主题 + 参与者交集”批量归组；
//! - 运行期单封归组 [`assign`]：入库时在调用方事务内决定归属线程并返回
//!   `thread_id`（调用方写入邮件行）。

use std::collections::BTreeSet;

use crate::domain::threading::{
    ThreadLookup, ThreadResolution, ThreadingInput, normalize_subject, resolve_thread,
};
use rusqlite::OptionalExtension;

/// `threads.participants` 持久化集合的大小上限：达到后不再追加新参与者。
const MAX_THREAD_PARTICIPANTS: usize = 64;

/// 一次回填的统计结果。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackfillStats {
    /// 新建的线程数。
    pub threads_created: usize,
    /// 被归组（写入 thread_id）的邮件数。
    pub emails_assigned: usize,
}

/// 对存量邮件执行会话线程回填。
///
/// 处理对象：`is_draft = 0 AND thread_id IS NULL` 的邮件（草稿不参与线程化），
/// 按 `received_at` 升序逐封归组，保证线程的 `created_at`/`updated_at`
/// 与邮件时间线一致。
///
/// 归组依据：存量邮件没有已落库的 In-Reply-To/References 头，`chain_ids`
/// 恒为空、仅走主题兜底路径——同一规范化主题的候选线程（`updated_at DESC`）
/// 中，参与者（发+收+抄，小写邮箱）有交集者归入既有线程（参与者合并入线程，
/// 受 [`MAX_THREAD_PARTICIPANTS`] 上限约束），否则新建线程。
///
/// 事务：内部通过 [`rusqlite::Connection::unchecked_transaction`] 自包一个事务，
/// 全部成功才提交、任一步失败整体回滚。**必须独立调用**——不得在外层事务中
/// 调用（SQLite 不支持嵌套 `BEGIN`，会直接报错）。
///
/// 幂等：已归组邮件（thread_id 非空）不再处理，重复调用返回零统计。
pub fn backfill_threading(conn: &rusqlite::Connection) -> rusqlite::Result<BackfillStats> {
    let tx = conn.unchecked_transaction()?;
    let stats = backfill_pending_emails(&tx)?;
    tx.commit()?;
    Ok(stats)
}

/// 单封邮件归属（在调用方事务内执行）：三分支决策 + 建线程/并线程。
/// 返回 thread_id 供邮件行写入。草稿由调用方跳过（不调本函数）。
///
/// 决策由 [`resolve_thread`] 完成：头链（In-Reply-To/References）命中 →
/// 主题 + 参与者交集 → 新建。本函数只写 `threads` 表（`Existing` 合并
/// 参与者并刷新 `updated_at`；`New` 建线程），**不写邮件行**——调用方拿到
/// 返回的 thread_id 后自行写入 `emails.thread_id`。
///
/// 不自开事务：SQL 直接在传入连接上执行。SQLite 不支持嵌套 `BEGIN`，
/// 需要原子性时由调用方包事务后传入。
pub fn assign(
    conn: &rusqlite::Connection,
    input: &ThreadingInput<'_>,
    received_at: i64,
) -> rusqlite::Result<i64> {
    let mut lookup = SqlThreadLookup { conn, error: None };
    let resolution = resolve_thread(input, &mut lookup);
    // trait 签名无法返回 Result，查询错误先记录，决策后统一上抛
    if let Some(err) = lookup.take_error() {
        return Err(err);
    }
    match resolution {
        ThreadResolution::Existing(thread_id) => {
            merge_thread_participants(conn, thread_id, &input.participants, received_at)?;
            Ok(thread_id)
        }
        ThreadResolution::New => create_thread(conn, input, received_at),
    }
}

/// 一封待归组邮件的快照（先整体读出再逐封处理，避免游标与回填写互相干扰）。
struct PendingEmail {
    id: i64,
    message_id: Option<String>,
    subject: Option<String>,
    sender_email: String,
    recipient_emails: String,
    cc_emails: Option<String>,
    received_at: i64,
}

/// 在既有事务内执行回填主体（事务边界见 [`backfill_threading`]）。
fn backfill_pending_emails(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<BackfillStats> {
    // 归组辅助（lookup/合并/建线程）统一按裸连接编写，与运行期 [`assign`]
    // 共用同一套存储侧实现；事务边界仍由 `tx` 持有，此处只是借用。
    let conn: &rusqlite::Connection = tx;
    let pending: Vec<PendingEmail> = {
        let mut stmt = conn.prepare(
            "SELECT id, message_id, subject, sender_email, recipient_emails, cc_emails, received_at
             FROM emails
             WHERE is_draft = 0 AND thread_id IS NULL
             ORDER BY received_at ASC, id ASC",
        )?;
        let rows = stmt.query_map(
            [],
            |row| {
                Ok(PendingEmail {
                    id: row.get(0)?,
                    message_id: row.get(1)?,
                    subject: row.get(2)?,
                    sender_email: row.get(3)?,
                    recipient_emails: row.get(4)?,
                    cc_emails: row.get(5)?,
                    received_at: row.get(6)?,
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut stats = BackfillStats::default();
    let mut lookup = SqlThreadLookup { conn, error: None };

    for email in &pending {
        let participants = collect_participants(
            &email.sender_email,
            &email.recipient_emails,
            email.cc_emails.as_deref(),
        );
        let input = ThreadingInput {
            message_id: email.message_id.as_deref(),
            subject: email.subject.as_deref(),
            // 存量邮件没有落库的 In-Reply-To/References 头，头链恒为空，
            // 归组只走主题兜底（resolve_thread 对空 chain_ids 不会查头链）。
            chain_ids: Vec::new(),
            participants,
        };

        let resolution = resolve_thread(&input, &mut lookup);
        // trait 签名无法返回 Result，查询错误先记录，决策后统一上抛
        if let Some(err) = lookup.take_error() {
            return Err(err);
        }

        let thread_id = match resolution {
            ThreadResolution::Existing(thread_id) => {
                merge_thread_participants(conn, thread_id, &input.participants, email.received_at)?;
                thread_id
            }
            ThreadResolution::New => {
                let thread_id = create_thread(conn, &input, email.received_at)?;
                stats.threads_created += 1;
                thread_id
            }
        };

        conn.execute(
            "UPDATE emails SET thread_id = ?1 WHERE id = ?2",
            rusqlite::params![thread_id, email.id],
        )?;
        stats.emails_assigned += 1;
    }

    Ok(stats)
}

/// [`ThreadLookup`] 的 SQLite 实现：在给定连接（裸连接或事务借出的连接）上
/// 查询 threads / emails，供回填与单封归组共用。
struct SqlThreadLookup<'c> {
    conn: &'c rusqlite::Connection,
    /// 查询出错时记录（trait 签名返回 Option），由调用方在决策后统一检查。
    error: Option<rusqlite::Error>,
}

impl SqlThreadLookup<'_> {
    /// 取出并清除已记录的查询错误。
    fn take_error(&mut self) -> Option<rusqlite::Error> {
        self.error.take()
    }
}

/// 归一 Message-ID 用于查找比较：剥两端尖括号 + ASCII 小写。
///
/// 引擎侧的链 id 本就是该形态，此处为防御性再归一（容忍调用方传入未归一 id），
/// 保证参数侧恒定；DB 侧由 SQL 的 `LOWER(TRIM(...))` 对齐到同一形态。
fn normalize_message_id_for_lookup(id: &str) -> String {
    id.trim_matches(|c| c == '<' || c == '>')
        .to_ascii_lowercase()
}

impl ThreadLookup for SqlThreadLookup<'_> {
    fn thread_of_message_id(&mut self, ids: &[String]) -> Option<i64> {
        for id in ids {
            // 两侧格式统一：DB 存的 message_id 常带尖括号原样大小写（如 <AbC@x>），
            // 引擎链 id 为剥括号小写（abc@x）。SQL 侧把存储值剥两端 <、> 字符
            // （TRIM 第二参是字符集合语义）并 ASCII 小写后与归一参数比较。
            let needle = normalize_message_id_for_lookup(id);
            match self
                .conn
                .query_row(
                    "SELECT thread_id FROM emails
                     WHERE LOWER(TRIM(message_id, '<>')) = ?1 AND thread_id IS NOT NULL
                     LIMIT 1",
                    [needle],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
            {
                Ok(Some(thread_id)) => return Some(thread_id),
                Ok(None) => continue,
                Err(err) => {
                    if self.error.is_none() {
                        self.error = Some(err);
                    }
                    return None;
                }
            }
        }
        None
    }

    fn thread_by_subject_and_participant(
        &mut self,
        subject_key: &str,
        participants: &BTreeSet<String>,
    ) -> Option<i64> {
        let candidates = self
            .conn
            .prepare(
                // 主题键已在写入前规范化，直接精确匹配（避免 LIKE 通配符误伤
                // 含 %/_ 的主题）；候选按 updated_at DESC 排序后逐个内存求交
                "SELECT id, participants FROM threads
                 WHERE subject_key = ?1
                 ORDER BY updated_at DESC, id DESC",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map([subject_key], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            });
        let candidates = match candidates {
            Ok(candidates) => candidates,
            Err(err) => {
                if self.error.is_none() {
                    self.error = Some(err);
                }
                return None;
            }
        };

        candidates
            .iter()
            .find(|(_, stored)| stored_participants_intersect(stored, participants))
            .map(|(id, _)| *id)
    }
}

/// 已存线程参与者（逗号分隔）与当前邮件参与者是否有交集。
fn stored_participants_intersect(stored: &str, participants: &BTreeSet<String>) -> bool {
    stored
        .split(',')
        .map(str::trim)
        .any(|candidate| !candidate.is_empty() && participants.contains(candidate))
}

/// 将本封邮件的参与者合并进既有线程（受 64 上限约束），并把线程
/// `updated_at` 刷新到不早于本封邮件的接收时间。
fn merge_thread_participants(
    conn: &rusqlite::Connection,
    thread_id: i64,
    incoming: &BTreeSet<String>,
    activity_at: i64,
) -> rusqlite::Result<()> {
    let stored: String = conn.query_row(
        "SELECT participants FROM threads WHERE id = ?1",
        [thread_id],
        |row| row.get(0),
    )?;
    let merged = merge_capped(&stored, incoming);
    conn.execute(
        "UPDATE threads SET participants = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
        rusqlite::params![merged, activity_at, thread_id],
    )?;
    Ok(())
}

/// 按 [`ThreadResolution::New`] 建线程：subject_key = 规范化主题（空主题落空串），
/// participants 截前 64，created_at/updated_at 均取 received_at。返回新线程 id。
fn create_thread(
    conn: &rusqlite::Connection,
    input: &ThreadingInput<'_>,
    received_at: i64,
) -> rusqlite::Result<i64> {
    let subject_key = normalize_subject(input.subject.unwrap_or(""));
    let participants = join_capped(&input.participants);
    conn.execute(
        "INSERT INTO threads (subject_key, participants, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?3)",
        rusqlite::params![subject_key, participants, received_at],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 从发件人 + 收件人 + 抄送（均为逗号分隔）构造小写参与者集合。
///
/// 供本模块（回填/单封归属）与 email_repo（入库接线）共用，
/// 保证写入线程与查找交集的两侧参与者集合构造规则一致。
pub(crate) fn collect_participants(
    sender: &str,
    recipients: &str,
    cc: Option<&str>,
) -> BTreeSet<String> {
    let mut participants = BTreeSet::new();
    let mut add_field = |field: &str| {
        for candidate in field.split(',') {
            let candidate = candidate.trim().to_ascii_lowercase();
            if !candidate.is_empty() {
                participants.insert(candidate);
            }
        }
    };
    add_field(sender);
    add_field(recipients);
    if let Some(cc) = cc {
        add_field(cc);
    }
    participants
}

/// 把逗号分隔的已存参与者解析回集合（容忍空白与空段）。
fn parse_stored_participants(stored: &str) -> BTreeSet<String> {
    stored
        .split(',')
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(str::to_string)
        .collect()
}

/// 合并已存参与者与新增参与者，集合达到 [`MAX_THREAD_PARTICIPANTS`] 后不再追加。
fn merge_capped(stored: &str, incoming: &BTreeSet<String>) -> String {
    let mut merged = parse_stored_participants(stored);
    for candidate in incoming {
        if merged.len() >= MAX_THREAD_PARTICIPANTS {
            break;
        }
        merged.insert(candidate.clone());
    }
    merged
        .iter()
        .take(MAX_THREAD_PARTICIPANTS)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

/// 序列化参与者集合为小写邮箱逗号串（入集合时已统一小写），
/// 最多保留 [`MAX_THREAD_PARTICIPANTS`] 个。
fn join_capped(participants: &BTreeSet<String>) -> String {
    participants
        .iter()
        .take(MAX_THREAD_PARTICIPANTS)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手工建最小 emails + threads 表（不依赖 SCHEMA_SQL 全库）。
    fn minimal_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE emails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT,
                subject TEXT,
                sender_email TEXT NOT NULL,
                recipient_emails TEXT NOT NULL,
                cc_emails TEXT,
                is_draft INTEGER DEFAULT 0,
                received_at INTEGER NOT NULL,
                thread_id INTEGER
            );
            CREATE TABLE threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                subject_key TEXT NOT NULL,
                participants TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
        conn
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_email(
        conn: &rusqlite::Connection,
        subject: &str,
        sender: &str,
        recipients: &str,
        cc: Option<&str>,
        is_draft: bool,
        received_at: i64,
    ) -> i64 {
        conn.execute(
            "INSERT INTO emails (
                subject, sender_email, recipient_emails, cc_emails, is_draft, received_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![subject, sender, recipients, cc, i64::from(is_draft), received_at],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn thread_id_of(conn: &rusqlite::Connection, id: i64) -> Option<i64> {
        conn.query_row("SELECT thread_id FROM emails WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn thread_row(conn: &rusqlite::Connection, id: i64) -> (String, String, i64, i64) {
        conn.query_row(
            "SELECT subject_key, participants, created_at, updated_at FROM threads WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap()
    }

    #[test]
    fn backfill_groups_same_subject_replies_into_single_thread() {
        let conn = minimal_conn();
        let e1 = insert_email(&conn, "Quarterly Review", "alice@corp.com", "bob@corp.com", None, false, 100);
        let e2 = insert_email(&conn, "Re: Quarterly Review", "bob@corp.com", "alice@corp.com", None, false, 200);
        let e3 = insert_email(
            &conn,
            "quarterly review",
            "alice@corp.com",
            "bob@corp.com,carol@corp.com",
            Some("dave@corp.com"),
            false,
            300,
        );

        let stats = backfill_threading(&conn).unwrap();

        assert_eq!(stats.threads_created, 1, "三个同主题邮件应只建一条线程");
        assert_eq!(stats.emails_assigned, 3);

        let (t1, t2, t3) = (
            thread_id_of(&conn, e1).expect("e1 应已归组"),
            thread_id_of(&conn, e2).expect("e2 应已归组"),
            thread_id_of(&conn, e3).expect("e3 应已归组"),
        );
        assert_eq!(t1, t2, "Re: 前缀不影响同主题归组");
        assert_eq!(t2, t3, "大小写/空白差异不影响同主题归组");

        let (subject_key, participants, created_at, updated_at) = thread_row(&conn, t1);
        assert_eq!(subject_key, "quarterly review", "主题键应为规范化后的值");
        let participant_set: std::collections::BTreeSet<&str> =
            participants.split(',').collect();
        for expected in ["alice@corp.com", "bob@corp.com", "carol@corp.com", "dave@corp.com"] {
            assert!(
                participant_set.contains(expected),
                "participants 应合并包含 {expected}：{participants}"
            );
        }
        assert_eq!(created_at, 100, "线程 created_at 应为首封邮件时间");
        assert_eq!(updated_at, 300, "线程 updated_at 应刷新为最新邮件时间");
    }

    #[test]
    fn backfill_same_subject_disjoint_participants_split_into_two_threads() {
        let conn = minimal_conn();
        let e1 = insert_email(&conn, "Lunch plan", "a@x.com", "b@x.com", None, false, 100);
        let e2 = insert_email(&conn, "Re: Lunch plan", "c@y.com", "d@y.com", None, false, 200);
        // 组 A 的后续邮件：候选按 updated_at DESC 逐个求交，跳过无交集的组 B 线程
        let e3 = insert_email(&conn, "Re: Lunch plan", "b@x.com", "a@x.com", None, false, 300);

        let stats = backfill_threading(&conn).unwrap();

        assert_eq!(stats.threads_created, 2, "参与者无交集应拆成两条线程");
        assert_eq!(stats.emails_assigned, 3);
        assert_eq!(
            thread_id_of(&conn, e1).unwrap(),
            thread_id_of(&conn, e3).unwrap(),
            "组 A 前后两封应归同一线程"
        );
        assert_ne!(
            thread_id_of(&conn, e1).unwrap(),
            thread_id_of(&conn, e2).unwrap(),
            "组 B 应独立成线"
        );
    }

    #[test]
    fn backfill_skips_drafts_and_leaves_thread_id_null() {
        let conn = minimal_conn();
        let normal = insert_email(&conn, "Report", "a@x.com", "b@x.com", None, false, 100);
        let draft = insert_email(&conn, "Re: Report", "b@x.com", "a@x.com", None, true, 200);

        let stats = backfill_threading(&conn).unwrap();

        assert_eq!(stats.threads_created, 1);
        assert_eq!(stats.emails_assigned, 1, "草稿不参与回填");
        assert!(thread_id_of(&conn, normal).is_some());
        assert!(thread_id_of(&conn, draft).is_none(), "草稿 thread_id 应保持 NULL");
    }

    #[test]
    fn backfill_rerun_is_idempotent_with_zero_stats() {
        let conn = minimal_conn();
        let e1 = insert_email(&conn, "Hello", "a@x.com", "b@x.com", None, false, 100);
        let e2 = insert_email(&conn, "Re: Hello", "b@x.com", "a@x.com", None, false, 200);

        let first = backfill_threading(&conn).unwrap();
        assert_eq!(
            (first.threads_created, first.emails_assigned),
            (1, 2),
            "首轮应建 1 线程归组 2 封"
        );

        let ids_before = (thread_id_of(&conn, e1), thread_id_of(&conn, e2));
        let thread_count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))
            .unwrap();

        let second = backfill_threading(&conn).unwrap();
        assert_eq!(
            (second.threads_created, second.emails_assigned),
            (0, 0),
            "重复回填应零统计"
        );
        assert_eq!(
            (thread_id_of(&conn, e1), thread_id_of(&conn, e2)),
            ids_before,
            "重复回填不得改变既有归组"
        );
        let thread_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(thread_count_before, thread_count_after, "不得新增线程");
    }

    // ---------- assign 夹具 ----------

    /// 直接落一条既有线程（participants 原样写入，逗号分隔）。
    fn seed_thread(
        conn: &rusqlite::Connection,
        subject_key: &str,
        participants: &str,
        created_at: i64,
        updated_at: i64,
    ) -> i64 {
        conn.execute(
            "INSERT INTO threads (subject_key, participants, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![subject_key, participants, created_at, updated_at],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    /// 直接落一封已归组的存量邮件（message_id 原样写入，可带尖括号/大小写混排）。
    fn seed_threaded_email(
        conn: &rusqlite::Connection,
        message_id: &str,
        thread_id: i64,
        received_at: i64,
    ) -> i64 {
        conn.execute(
            "INSERT INTO emails (
                message_id, subject, sender_email, recipient_emails, cc_emails,
                is_draft, received_at, thread_id
            ) VALUES (?1, NULL, 's@x', 'r@x', NULL, 0, ?2, ?3)",
            rusqlite::params![message_id, received_at, thread_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    /// 构造 assign 输入（participants 视为已小写，chain 原样转 String）。
    fn assign_input<'a>(
        subject: Option<&'a str>,
        chain: &[&str],
        participants: &[&str],
    ) -> ThreadingInput<'a> {
        ThreadingInput {
            message_id: None,
            subject,
            chain_ids: chain.iter().map(|id| (*id).to_string()).collect(),
            participants: participants.iter().map(|p| (*p).to_string()).collect(),
        }
    }

    fn thread_count(conn: &rusqlite::Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))
            .unwrap()
    }

    // ---------- assign：头链路径（含 message_id 两侧格式统一） ----------

    #[test]
    fn assign_chain_hits_thread_despite_stored_message_id_format() {
        // 交接坑位锁定：DB 存「带尖括号 + 原样大小写」（<AbC@x.com>），
        // 引擎链 id 为「剥括号小写」（abc@x.com）→ 归一后必须命中。
        // 参与者与既有线程刻意无交集：若链路径失效，主题兜底不会命中
        // （同主题但无交集 → 新建），本测试即失败——锁定命中的确来自链。
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "kickoff", "boss@x.com,peer@x.com", 100, 100);
        seed_threaded_email(&conn, "<AbC@x.com>", tid, 100);

        let input = assign_input(Some("Re: Kickoff"), &["abc@x.com"], &["newbie@x.com"]);
        let got = assign(&conn, &input, 200).unwrap();

        assert_eq!(got, tid, "格式归一后链 id 应命中既有线程");
        assert_eq!(thread_count(&conn), 1, "链命中不得新建线程");

        let (_, participants, created_at, updated_at) = thread_row(&conn, tid);
        assert!(
            participants.contains("newbie@x.com"),
            "命中后应合并参与者：{participants}"
        );
        assert_eq!(created_at, 100, "既有线程 created_at 不变");
        assert_eq!(updated_at, 200, "updated_at 应刷新为本封 received_at");
    }

    #[test]
    fn assign_chain_message_id_lookup_tolerates_format_variants() {
        // 表驱动：两侧各种括号/大小写形态组合均应命中。
        let cases: &[(&str, &str)] = &[
            ("<AbC@x>", "abc@x"), // DB 带括号混排，链 id 规范形态
            ("AbC@x", "abc@x"),   // DB 裸串混排
            ("<abc@x>", "ABC@X"), // 链 id 未归一（参数侧防御性归一）
        ];
        for (stored, chain) in cases {
            let conn = minimal_conn();
            let tid = seed_thread(&conn, "t", "a@x", 1, 1);
            seed_threaded_email(&conn, stored, tid, 1);

            let input = assign_input(None, &[chain], &["a@x"]);
            assert_eq!(
                assign(&conn, &input, 2).unwrap(),
                tid,
                "stored={stored:?} chain={chain:?} 应命中"
            );
        }
    }

    #[test]
    fn assign_chain_multiple_ids_hits_any_and_full_miss_creates_new() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "t", "a@x", 1, 1);
        seed_threaded_email(&conn, "<parent@x>", tid, 1);

        // 链上多个 id：先 miss 后命中 → 仍归既有线程
        let hit = assign_input(None, &["zz@miss", "parent@x"], &["a@x"]);
        assert_eq!(assign(&conn, &hit, 5).unwrap(), tid);

        // 全部 miss + 空主题（None）→ 新建，不得误吞既有线程
        let miss = assign_input(None, &["other@miss"], &["a@x"]);
        let new_tid = assign(&conn, &miss, 6).unwrap();
        assert_ne!(new_tid, tid);
        assert_eq!(thread_count(&conn), 2);
    }

    // ---------- assign：主题兜底路径 ----------

    #[test]
    fn assign_subject_with_participant_intersection_joins_existing_thread() {
        let conn = minimal_conn();
        let first = assign_input(Some("Quarterly Review"), &[], &["alice@x.com", "bob@x.com"]);
        let t1 = assign(&conn, &first, 100).unwrap();
        let (subject_key, participants, created_at, updated_at) = thread_row(&conn, t1);
        assert_eq!(subject_key, "quarterly review", "主题键应规范化落库");
        assert_eq!(participants, "alice@x.com,bob@x.com");
        assert_eq!((created_at, updated_at), (100, 100));

        // 回信：链 miss，主题规范化同键（前缀/大小写/空白折叠），参与者交集 bob
        let reply = assign_input(
            Some("Re:   quarterly review"),
            &["<miss@z>"],
            &["bob@x.com", "carol@x.com"],
        );
        let t2 = assign(&conn, &reply, 200).unwrap();
        assert_eq!(t2, t1, "主题同键 + 参与者交集应归入既有线程");
        assert_eq!(thread_count(&conn), 1);

        let (_, participants, created_at, updated_at) = thread_row(&conn, t1);
        assert_eq!(
            participants, "alice@x.com,bob@x.com,carol@x.com",
            "应合并新参与者"
        );
        assert_eq!(created_at, 100, "既有线程 created_at 不变");
        assert_eq!(updated_at, 200);
    }

    #[test]
    fn assign_subject_without_intersection_creates_separate_thread() {
        let conn = minimal_conn();
        let a = assign_input(Some("Lunch plan"), &[], &["a@x.com", "b@x.com"]);
        let t1 = assign(&conn, &a, 100).unwrap();

        let b = assign_input(Some("Re: Lunch plan"), &[], &["c@y.com", "d@y.com"]);
        let t2 = assign(&conn, &b, 200).unwrap();

        assert_ne!(t1, t2, "无参与者交集不得并入");
        assert_eq!(thread_count(&conn), 2);
        let (subject_key, participants, created_at, updated_at) = thread_row(&conn, t2);
        assert_eq!(subject_key, "lunch plan");
        assert_eq!(participants, "c@y.com,d@y.com");
        assert_eq!(
            (created_at, updated_at),
            (200, 200),
            "新线程时间戳取 received_at"
        );
    }

    #[test]
    fn assign_empty_subject_always_creates_new_thread_with_empty_key() {
        // 空主题（None / 规范化后为空）恒新建，subject_key 落空串（与 backfill 同语义）
        let conn = minimal_conn();
        let first = assign_input(Some("Re:"), &[], &["a@x"]);
        let t1 = assign(&conn, &first, 100).unwrap();
        let (subject_key, ..) = thread_row(&conn, t1);
        assert_eq!(subject_key, "", "空主题键应落空串");

        let second = assign_input(None, &[], &["a@x"]);
        let t2 = assign(&conn, &second, 200).unwrap();
        assert_ne!(t1, t2, "空主题恒不参与主题匹配");
        assert_eq!(thread_count(&conn), 2);
    }

    // ---------- assign：participants 合并（去重 / 64 上限） ----------

    #[test]
    fn assign_existing_merges_participants_without_duplicates() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "sync", "a@x,b@x", 100, 100);
        seed_threaded_email(&conn, "<p@x>", tid, 100);

        let input = assign_input(Some("Re: Sync"), &["p@x"], &["b@x", "c@x"]);
        assign(&conn, &input, 150).unwrap();

        let (_, participants, ..) = thread_row(&conn, tid);
        assert_eq!(participants, "a@x,b@x,c@x", "合并去重：b 不得重复出现");
    }

    #[test]
    fn assign_existing_caps_merged_participants_at_64() {
        let conn = minimal_conn();
        let base: Vec<String> = (0..64).map(|i| format!("p{i:02}@x")).collect();
        let tid = seed_thread(&conn, "cap", &base.join(","), 100, 100);
        seed_threaded_email(&conn, "<p@x>", tid, 100);

        // 已达 64 上限：新参与者不再追加，既有 64 个原样保留
        let input = assign_input(Some("Re: Cap"), &["p@x"], &["p00@x", "new@x"]);
        assign(&conn, &input, 150).unwrap();

        let (_, participants, ..) = thread_row(&conn, tid);
        let stored: Vec<&str> = participants.split(',').collect();
        assert_eq!(stored.len(), 64, "不得超过 64 上限");
        assert!(
            !stored.contains(&"new@x"),
            "达上限后不得追加新参与者：{participants}"
        );
        assert_eq!(stored.first(), Some(&"p00@x"));
        assert_eq!(stored.last(), Some(&"p63@x"));
    }

    #[test]
    fn assign_new_thread_caps_participants_at_64() {
        let conn = minimal_conn();
        let many: Vec<String> = (0..70).map(|i| format!("u{i:02}@x")).collect();
        let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let input = assign_input(Some("Fresh"), &[], &many_refs);

        let tid = assign(&conn, &input, 100).unwrap();

        let (_, participants, ..) = thread_row(&conn, tid);
        let stored: Vec<&str> = participants.split(',').collect();
        assert_eq!(stored.len(), 64, "新线程参与者截前 64");
        assert_eq!(stored.last(), Some(&"u63@x"), "按集合序保留前 64");
        assert!(
            !stored.contains(&"u64@x"),
            "第 65 个起不得落库：{participants}"
        );
    }

    // ---------- assign：updated_at 只增不减 ----------

    #[test]
    fn assign_refreshes_updated_at_never_decreasing() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "clock", "a@x", 100, 500);

        // 更旧的邮件并入：updated_at 不得回退
        let older = assign_input(Some("Re: Clock"), &[], &["a@x", "old@x"]);
        assign(&conn, &older, 200).unwrap();
        let (_, _, created_at, updated_at) = thread_row(&conn, tid);
        assert_eq!(updated_at, 500, "updated_at 只增不减");
        assert_eq!(created_at, 100, "created_at 不受并入影响");

        // 更新的邮件并入：刷新到 received_at，且两轮参与者合并均生效
        let newer = assign_input(Some("Re: Clock"), &[], &["a@x", "new@x"]);
        assign(&conn, &newer, 900).unwrap();
        let (_, participants, _, updated_at) = thread_row(&conn, tid);
        assert_eq!(updated_at, 900);
        assert_eq!(participants, "a@x,new@x,old@x");
    }
}
