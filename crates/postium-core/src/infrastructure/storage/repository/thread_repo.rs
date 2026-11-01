//! 会话线程仓库（Thread Repository）
//!
//! 负责 `threads` 表的存量回填：将历史上未归组的邮件（`thread_id IS NULL`）
//! 按“规范化主题 + 参与者交集”归入会话线程。归组决策复用领域层纯函数
//! [`crate::domain::threading::resolve_thread`]，本模块仅提供存储侧的
//! [`ThreadLookup`] 实现与会话写入。
//!
//! 运行期的新邮件归组（入库时分配 thread_id）不在本模块范围。

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
    let pending: Vec<PendingEmail> = {
        let mut stmt = tx.prepare(
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
    let mut lookup = TxThreadLookup { tx, error: None };

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
                merge_thread_participants(tx, thread_id, &input.participants, email.received_at)?;
                thread_id
            }
            ThreadResolution::New => {
                let subject_key = normalize_subject(input.subject.unwrap_or(""));
                let participants = join_capped(&input.participants);
                tx.execute(
                    "INSERT INTO threads (subject_key, participants, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)",
                    rusqlite::params![subject_key, participants, email.received_at],
                )?;
                stats.threads_created += 1;
                tx.last_insert_rowid()
            }
        };

        tx.execute(
            "UPDATE emails SET thread_id = ?1 WHERE id = ?2",
            rusqlite::params![thread_id, email.id],
        )?;
        stats.emails_assigned += 1;
    }

    Ok(stats)
}

/// [`ThreadLookup`] 的 SQLite 实现：在本回填事务内查询 threads / emails。
struct TxThreadLookup<'t, 'c> {
    tx: &'t rusqlite::Transaction<'c>,
    /// 查询出错时记录（trait 签名返回 Option），由调用方在决策后统一检查。
    error: Option<rusqlite::Error>,
}

impl TxThreadLookup<'_, '_> {
    /// 取出并清除已记录的查询错误。
    fn take_error(&mut self) -> Option<rusqlite::Error> {
        self.error.take()
    }
}

impl ThreadLookup for TxThreadLookup<'_, '_> {
    fn thread_of_message_id(&mut self, ids: &[String]) -> Option<i64> {
        for id in ids {
            match self
                .tx
                .query_row(
                    "SELECT thread_id FROM emails
                     WHERE message_id = ?1 AND thread_id IS NOT NULL
                     LIMIT 1",
                    [id],
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
            .tx
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
    tx: &rusqlite::Transaction<'_>,
    thread_id: i64,
    incoming: &BTreeSet<String>,
    activity_at: i64,
) -> rusqlite::Result<()> {
    let stored: String = tx.query_row(
        "SELECT participants FROM threads WHERE id = ?1",
        [thread_id],
        |row| row.get(0),
    )?;
    let merged = merge_capped(&stored, incoming);
    tx.execute(
        "UPDATE threads SET participants = ?1, updated_at = MAX(updated_at, ?2) WHERE id = ?3",
        rusqlite::params![merged, activity_at, thread_id],
    )?;
    Ok(())
}

/// 从发件人 + 收件人 + 抄送（均为逗号分隔）构造小写参与者集合。
fn collect_participants(sender: &str, recipients: &str, cc: Option<&str>) -> BTreeSet<String> {
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
}
