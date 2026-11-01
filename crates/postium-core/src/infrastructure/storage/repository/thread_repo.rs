//! 会话线程仓库（Thread Repository）
//!
//! 负责 `threads` 表的会话归组与聚合查询：将邮件按归组决策写入会话线程，
//! 并提供会话列表 / 会话内邮件的聚合查询。
//! 归组决策复用领域层纯函数 [`crate::domain::threading::resolve_thread`]，
//! 本模块提供存储侧的 [`ThreadLookup`] 实现与会话写入：
//!
//! - 存量回填 [`backfill_threading`]：历史上未归组的邮件（`thread_id IS NULL`）
//!   按“规范化主题 + 参与者交集”批量归组；
//! - 运行期单封归组 [`assign`]：入库时在调用方事务内决定归属线程并返回
//!   `thread_id`（调用方写入邮件行）；
//! - 会话聚合查询 [`list_conversations`] / [`conversation_emails`]：按
//!   [`ConversationFilter`] 可见性过滤（账号 / 分类 / 文件夹 / 仅未读）
//!   聚合线程列表（封数 / 未读 / 星标 + 最新一封列表头）与会话内邮件时间线。

use std::collections::BTreeSet;

use crate::domain::folders::FolderCategory;
use crate::domain::threading::{
    ThreadLookup, ThreadResolution, ThreadingInput, normalize_subject, resolve_thread,
};
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::emails;
use crate::infrastructure::storage::repository::email_repo::map_email;
use crate::service::email_service::{ConversationFilter, ConversationSummary, EmailCategory};
use rusqlite::OptionalExtension;
use rusqlite::types::Value;

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

// ═════════════════════════════════════════════════════════════════════════
// 会话聚合查询（list_conversations / conversation_emails）
// ═════════════════════════════════════════════════════════════════════════

/// 会话可见性过滤的已解析形态（owned，供 async 闭包按值捕获）。
///
/// 由 [`ConversationFilter`] 一次性解析而来；分类 → 条件的映射**复用既有
/// 同一套规则**，不另建第二份映射表：
///
/// - `Starred` → 跨文件夹 `is_starred = 1` 条件（与 `email_repo::list_starred*`
///   的 WHERE 构造同一规则——星标是标志条件，不是文件夹名表）；
/// - 其他分类 → 经 [`FolderCategory::from_email_category`] 归一后，与同步期
///   `FolderRegistry` 落库到 `sync_state.folder_category` 的**同一份文件夹分类**
///   （写入侧即 `FolderCategory::as_str()`，见 folder_sync_dispatcher）对齐：
///   `folder IN (SELECT folder FROM sync_state WHERE folder_category = ? AND
///   account_id = <本行账号>)`。服务层 `list_by_category` 的 FolderRegistry
///   运行时解析同样以这份持久化分类为输入（`local_folder_registry_inputs`），
///   两侧消费同一来源；
/// - `unread_only` → `(? = 0 OR is_read = 0 OR is_read IS NULL)`，与
///   `email_repo` 既有 list* 查询的未读条件写法一致（`is_read` 可空）。
struct VisibilitySpec {
    account_id: Option<i32>,
    folder: Option<String>,
    /// Starred 分类：`is_starred = 1` 条件（跨文件夹）。
    starred_only: bool,
    /// 非 Starred 分类的文件夹类别（`FolderCategory::as_str()` 持久化形态）。
    folder_category: Option<&'static str>,
    unread_only: bool,
}

impl From<&ConversationFilter<'_>> for VisibilitySpec {
    fn from(filter: &ConversationFilter<'_>) -> Self {
        Self {
            account_id: filter.account_id,
            folder: filter.folder.map(str::to_string),
            starred_only: filter.category.as_ref() == Some(&EmailCategory::Starred),
            folder_category: filter
                .category
                .as_ref()
                .and_then(FolderCategory::from_email_category)
                .map(FolderCategory::as_str),
            unread_only: filter.unread_only,
        }
    }
}

impl VisibilitySpec {
    /// 把可见性过滤下推为 emails 行（以 `alias` 为表别名）的条件片段。
    ///
    /// 返回 `(条件串, 绑定值)`；无条件时条件串为空。片段内 `?` 与绑定值
    /// 按序对应，同一 spec 对不同别名生成的绑定值完全相同（仅别名文字不同）。
    fn conditions(&self, alias: &str) -> (String, Vec<Value>) {
        let mut conds: Vec<String> = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        if let Some(account_id) = self.account_id {
            conds.push(format!("{alias}.account_id = ?"));
            values.push(Value::from(account_id));
        }
        if let Some(folder) = &self.folder {
            conds.push(format!("{alias}.folder = ?"));
            values.push(Value::from(folder.clone()));
        }
        if self.starred_only {
            conds.push(format!("{alias}.is_starred = 1"));
        }
        if let Some(category) = self.folder_category {
            // 关联子查询按本行账号取该账号自己的文件夹分类，账号过滤为 None
            // （全账号）时仍然逐行对齐到所属账号的分类
            conds.push(format!(
                "{alias}.folder IN (
                    SELECT s.folder FROM sync_state s
                    WHERE s.folder_category = ? AND s.account_id = {alias}.account_id
                )"
            ));
            values.push(Value::from(category.to_string()));
        }
        if self.unread_only {
            conds.push(format!(
                "(? = 0 OR {alias}.is_read = 0 OR {alias}.is_read IS NULL)"
            ));
            values.push(Value::from(1));
        }
        (conds.join(" AND "), values)
    }
}

/// 条件串拼进 `JOIN … ON` / `WHERE` 尾部的前缀（空条件则空串）。
fn conjunct(suffix_conditions: &str) -> String {
    if suffix_conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {suffix_conditions}")
    }
}

/// [`list_conversations`] 的同步内核：单条聚合查询 + 同条件总数。
///
/// SQL 形态对齐 brief 骨架：聚合侧 `JOIN emails e` 与最新一封选取子查询
/// 应用同一可见性过滤；`ORDER BY t.updated_at DESC` 加 `t.id DESC`、
/// 最新一封选取加 `id DESC` 作平局稳定化（对齐 email_repo 既有 list*
/// 的 `sent_at DESC, id DESC` 惯例，保证分页确定性）。
fn list_conversations_impl(
    conn: &rusqlite::Connection,
    spec: &VisibilitySpec,
    page: usize,
    limit: usize,
) -> rusqlite::Result<(Vec<ConversationSummary>, u64)> {
    let (e_cond, e_values) = spec.conditions("e");
    let (le_cond, le_values) = spec.conditions("le");

    // 总数：同 FROM/WHERE 的 COUNT(DISTINCT t.id)（e_last 由 e 的存在性保证，无需再 JOIN）
    let count_sql = format!(
        "SELECT COUNT(DISTINCT t.id)
         FROM threads t
         JOIN emails e ON e.thread_id = t.id AND e.is_draft = 0 AND e.is_deleted = 0{}",
        conjunct(&e_cond)
    );
    let total = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(e_values.iter()),
        |row| row.get::<_, i64>(0),
    )? as u64;

    let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
    let select_sql = format!(
        "SELECT t.id AS thread_id,
                latest.subject, latest.sender_name, latest.sender_email, latest.preview,
                t.updated_at AS received_at,
                COUNT(e.id) AS total,
                SUM(CASE WHEN e.is_read = 0 OR e.is_read IS NULL THEN 1 ELSE 0 END) AS unread,
                MAX(e.is_starred) AS starred_any
         FROM threads t
         JOIN emails e ON e.thread_id = t.id AND e.is_draft = 0 AND e.is_deleted = 0{}
         JOIN emails latest ON latest.id = (
             SELECT le.id FROM emails le
             WHERE le.thread_id = t.id AND le.is_draft = 0 AND le.is_deleted = 0{}
             ORDER BY le.received_at DESC, le.id DESC
             LIMIT 1
         )
         GROUP BY t.id
         ORDER BY t.updated_at DESC, t.id DESC
         LIMIT ? OFFSET ?",
        conjunct(&e_cond),
        conjunct(&le_cond)
    );
    let mut values = e_values;
    values.extend(le_values);
    values.push(Value::from(limit as i64));
    values.push(Value::from(offset));
    let mut stmt = conn.prepare(&select_sql)?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok(ConversationSummary {
                thread_id: row.get("thread_id")?,
                subject: row.get("subject")?,
                sender_name: row.get("sender_name")?,
                sender_email: row.get("sender_email")?,
                preview: row.get("preview")?,
                received_at: row.get("received_at")?,
                total: row.get::<_, i64>("total")? as u64,
                unread: row.get::<_, Option<i64>>("unread")?.unwrap_or(0) as u64,
                starred_any: row.get::<_, Option<i64>>("starred_any")?.unwrap_or(0) != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((items, total))
}

/// [`conversation_emails`] 的同步内核：会话内可见邮件按时间升序。
fn conversation_emails_impl(
    conn: &rusqlite::Connection,
    thread_id: i64,
    spec: &VisibilitySpec,
) -> rusqlite::Result<Vec<emails::Model>> {
    let (e_cond, e_values) = spec.conditions("e");
    let sql = format!(
        "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted, thread_id, in_reply_to,
                sent_at, received_at, created_at, updated_at
         FROM emails e
         WHERE e.thread_id = ? AND e.is_draft = 0 AND e.is_deleted = 0{}
         ORDER BY e.received_at ASC, e.id ASC",
        conjunct(&e_cond)
    );
    let mut values = vec![Value::from(thread_id)];
    values.extend(e_values);
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), map_email)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

/// 分页查询会话列表（聚合查询的 async 包装，经 [`DbConn::call`] 调度）。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `filter`: 可见性过滤（账号 / 分类 / 文件夹 / 仅未读）
/// - `page`: 页码（从 1 开始）
/// - `limit`: 每页数量
///
/// # 返回
///
/// `(会话摘要列表, 符合条件的会话总数)`。列表按 `threads.updated_at` 降序。
pub async fn list_conversations(
    db: &DbConn,
    filter: &ConversationFilter<'_>,
    page: usize,
    limit: usize,
) -> Result<(Vec<ConversationSummary>, u64), MailError> {
    let spec = VisibilitySpec::from(filter);
    db.call(move |conn| list_conversations_impl(conn, &spec, page, limit))
        .await
}

/// 查询一个会话内的可见邮件（非草稿、未软删除 + 同 [`ConversationFilter`]
/// 可见性过滤），按 `received_at` 升序返回完整 [`emails::Model`]。
pub async fn conversation_emails(
    db: &DbConn,
    thread_id: i64,
    filter: &ConversationFilter<'_>,
) -> Result<Vec<emails::Model>, MailError> {
    let spec = VisibilitySpec::from(filter);
    db.call(move |conn| conversation_emails_impl(conn, thread_id, &spec))
        .await
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

    /// 手工建最小 emails + threads + sync_state 表（不依赖 SCHEMA_SQL 全库）。
    ///
    /// emails 列集合与生产 schema 对齐（省略 FK 与唯一索引——夹具不启用
    /// foreign_keys，且旧行不带 uid 不能加 `(account_id, folder, uid)` 唯一约束），
    /// 供归组测试与 [`super::list_conversations_impl`] 聚合查询测试共用。
    fn minimal_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE emails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL DEFAULT 1,
                folder TEXT NOT NULL DEFAULT 'INBOX',
                uid INTEGER NOT NULL DEFAULT 0,
                message_id TEXT,
                subject TEXT,
                sender_name TEXT,
                sender_email TEXT NOT NULL,
                recipient_emails TEXT NOT NULL,
                cc_emails TEXT,
                bcc_emails TEXT,
                preview TEXT,
                body_text TEXT,
                body_html TEXT,
                is_read INTEGER DEFAULT 0,
                is_starred INTEGER DEFAULT 0,
                is_draft INTEGER DEFAULT 0,
                is_answered INTEGER DEFAULT 0,
                is_deleted INTEGER DEFAULT 0,
                in_reply_to TEXT,
                sent_at INTEGER NOT NULL DEFAULT 0,
                received_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL DEFAULT 0,
                thread_id INTEGER
            );
            CREATE TABLE threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                subject_key TEXT NOT NULL,
                participants TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE sync_state (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                folder TEXT NOT NULL,
                folder_category TEXT
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

    // ---------- list_conversations / conversation_emails 夹具 ----------

    /// 直接落一封已归组、带可见性字段的邮件（布尔标记按 0/1 落库，uid 自增保唯一）。
    #[allow(clippy::too_many_arguments)]
    fn seed_conv_email(
        conn: &rusqlite::Connection,
        thread_id: i64,
        account_id: i64,
        folder: &str,
        subject: Option<&str>,
        sender_name: Option<&str>,
        sender_email: &str,
        preview: Option<&str>,
        is_read: bool,
        is_starred: bool,
        is_deleted: bool,
        received_at: i64,
    ) -> i64 {
        let uid = conn
            .query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM emails", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO emails (
                account_id, folder, uid, subject, sender_name, sender_email,
                recipient_emails, preview, is_read, is_starred, is_deleted,
                is_draft, sent_at, received_at, created_at, updated_at, thread_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'r@x', ?7, ?8, ?9, ?10, 0, ?11, ?11, ?11, ?11, ?12)",
            rusqlite::params![
                account_id,
                folder,
                uid,
                subject,
                sender_name,
                sender_email,
                preview,
                i64::from(is_read),
                i64::from(is_starred),
                i64::from(is_deleted),
                received_at,
                thread_id
            ],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    /// 落一条 sync_state 文件夹分类（同步期 FolderRegistry 落库的持久化形态）。
    fn seed_sync_category(
        conn: &rusqlite::Connection,
        account_id: i64,
        folder: &str,
        category: &str,
    ) {
        conn.execute(
            "INSERT INTO sync_state (account_id, folder, folder_category) VALUES (?1, ?2, ?3)",
            rusqlite::params![account_id, folder, category],
        )
        .unwrap();
    }

    fn base_filter() -> ConversationFilter<'static> {
        ConversationFilter::default()
    }

    fn conv_ids(rows: &[ConversationSummary]) -> Vec<i64> {
        rows.iter().map(|row| row.thread_id).collect()
    }

    // ---------- list_conversations：排序 / 聚合 / 可见性 ----------

    #[test]
    fn list_conversations_orders_threads_by_updated_at_desc() {
        let conn = minimal_conn();
        let t_old = seed_thread(&conn, "old", "a@x", 100, 100);
        let t_new = seed_thread(&conn, "new", "a@x", 100, 300);
        let t_mid = seed_thread(&conn, "mid", "a@x", 100, 200);
        for (tid, at) in [(t_old, 100), (t_new, 300), (t_mid, 200)] {
            seed_conv_email(
                &conn,
                tid,
                1,
                "INBOX",
                Some("s"),
                None,
                "a@x",
                None,
                true,
                false,
                false,
                at,
            );
        }

        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&base_filter()), 1, 50).unwrap();

        assert_eq!(total, 3);
        assert_eq!(
            conv_ids(&rows),
            vec![t_new, t_mid, t_old],
            "按 updated_at DESC 排序"
        );
        assert_eq!(rows[0].received_at, 300, "received_at 取线程 updated_at");
    }

    #[test]
    fn list_conversations_aggregates_counts_and_latest_email_summary() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "quarterly", "a@x,b@x", 100, 500);
        seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("Kickoff"),
            Some("Alice"),
            "alice@x",
            Some("第一封"),
            false,
            false,
            false,
            100,
        );
        // 最新的可见邮件（t=300）：列表头字段应取这一封
        seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("Re: Kickoff"),
            Some("Bob"),
            "bob@x",
            Some("最新一封"),
            false,
            false,
            false,
            300,
        );
        seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("Re: Kickoff"),
            Some("Carol"),
            "carol@x",
            Some("星标已读"),
            true,
            true,
            false,
            200,
        );
        // 软删除/草稿不进聚合，也不得影响"最新可见邮件"的选取（t 更大也不算）
        seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("已删除"),
            None,
            "d@x",
            None,
            false,
            false,
            true,
            400,
        );
        conn.execute(
            "INSERT INTO emails (account_id, folder, uid, subject, sender_email, recipient_emails,
             is_draft, sent_at, received_at, created_at, updated_at, thread_id)
             VALUES (1, 'INBOX', 99, '草稿', 'd@x', 'r@x', 1, 500, 500, 500, 500, ?1)",
            [tid],
        )
        .unwrap();

        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&base_filter()), 1, 50).unwrap();

        assert_eq!(total, 1);
        let row = &rows[0];
        assert_eq!(row.thread_id, tid);
        assert_eq!(row.total, 3, "软删除/草稿不计入封数");
        assert_eq!(row.unread, 2, "未读数只数可见邮件中的未读");
        assert!(row.starred_any, "任一可见邮件星标则 starred_any");
        assert_eq!(
            row.subject.as_deref(),
            Some("Re: Kickoff"),
            "主题取最新可见邮件"
        );
        assert_eq!(
            row.sender_name.as_deref(),
            Some("Bob"),
            "发件人名取最新可见邮件"
        );
        assert_eq!(row.sender_email, "bob@x");
        assert_eq!(
            row.preview.as_deref(),
            Some("最新一封"),
            "预览取最新可见邮件"
        );
        assert_eq!(
            row.received_at, 500,
            "received_at 取线程 updated_at（=最新邮件时间）"
        );
    }

    #[test]
    fn list_conversations_thread_with_only_deleted_emails_is_hidden() {
        let conn = minimal_conn();
        let t_alive = seed_thread(&conn, "alive", "a@x", 100, 100);
        let t_dead = seed_thread(&conn, "dead", "a@x", 100, 200);
        seed_conv_email(
            &conn,
            t_alive,
            1,
            "INBOX",
            Some("alive"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            100,
        );
        seed_conv_email(
            &conn,
            t_dead,
            1,
            "INBOX",
            Some("dead"),
            None,
            "a@x",
            None,
            true,
            false,
            true,
            200,
        );

        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&base_filter()), 1, 50).unwrap();

        assert_eq!(total, 1, "全部软删除的线程不进列表与总数");
        assert_eq!(conv_ids(&rows), vec![t_alive]);
    }

    #[test]
    fn list_conversations_inbox_category_uses_persisted_folder_categories() {
        let conn = minimal_conn();
        seed_sync_category(&conn, 1, "INBOX", "inbox");
        seed_sync_category(&conn, 1, "Archive", "archive");
        let t_inbox = seed_thread(&conn, "in", "a@x", 100, 200);
        let t_archive = seed_thread(&conn, "arch", "a@x", 100, 300);
        seed_conv_email(
            &conn,
            t_inbox,
            1,
            "INBOX",
            Some("in"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            200,
        );
        seed_conv_email(
            &conn,
            t_archive,
            1,
            "Archive",
            Some("arch"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );

        let mut filter = base_filter();
        filter.category = Some(EmailCategory::Inbox);
        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&filter), 1, 50).unwrap();

        assert_eq!(total, 1, "Inbox 分类只含持久化分类为 inbox 的文件夹线程");
        assert_eq!(
            conv_ids(&rows),
            vec![t_inbox],
            "Archive 线程不得混入 Inbox 分类"
        );
    }

    #[test]
    fn list_conversations_starred_category_crosses_folders_and_picks_latest_visible() {
        let conn = minimal_conn();
        let t_starred = seed_thread(&conn, "star", "a@x", 100, 200);
        let t_plain = seed_thread(&conn, "plain", "a@x", 100, 300);
        // 星标线程：可见（星标）邮件只有 t=200 这封；t=300 的未星标邮件不可见
        seed_conv_email(
            &conn,
            t_starred,
            1,
            "INBOX",
            Some("被星标的回信"),
            Some("Bob"),
            "bob@x",
            Some("星标预览"),
            false,
            true,
            false,
            200,
        );
        seed_conv_email(
            &conn,
            t_starred,
            1,
            "INBOX",
            Some("后到但未星标"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );
        seed_conv_email(
            &conn,
            t_plain,
            1,
            "Sent",
            Some("plain"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );

        let mut filter = base_filter();
        filter.category = Some(EmailCategory::Starred);
        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&filter), 1, 50).unwrap();

        assert_eq!(total, 1, "Starred 只含任一可见邮件星标的线程（跨文件夹）");
        assert_eq!(conv_ids(&rows), vec![t_starred]);
        let row = &rows[0];
        assert!(row.starred_any);
        assert_eq!(row.total, 1, "封数只数可见（星标）邮件");
        assert_eq!(
            row.subject.as_deref(),
            Some("被星标的回信"),
            "列表头取最新一封可见（星标）邮件"
        );
        assert_eq!(row.preview.as_deref(), Some("星标预览"));
    }

    #[test]
    fn list_conversations_unread_only_keeps_threads_with_unread() {
        let conn = minimal_conn();
        let t_unread = seed_thread(&conn, "unread", "a@x", 100, 100);
        let t_read = seed_thread(&conn, "read", "a@x", 100, 300);
        seed_conv_email(
            &conn,
            t_unread,
            1,
            "INBOX",
            Some("unread"),
            None,
            "a@x",
            None,
            false,
            false,
            false,
            100,
        );
        seed_conv_email(
            &conn,
            t_read,
            1,
            "INBOX",
            Some("read"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );

        let mut filter = base_filter();
        filter.unread_only = true;
        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&filter), 1, 50).unwrap();

        assert_eq!(total, 1);
        assert_eq!(
            conv_ids(&rows),
            vec![t_unread],
            "unread_only 只留含未读邮件的线程"
        );
    }

    #[test]
    fn list_conversations_filters_by_account_and_folder() {
        let conn = minimal_conn();
        let t_a1_inbox = seed_thread(&conn, "a1-in", "a@x", 100, 100);
        let t_a1_sent = seed_thread(&conn, "a1-sent", "a@x", 100, 200);
        let t_a2_inbox = seed_thread(&conn, "a2-in", "a@x", 100, 300);
        seed_conv_email(
            &conn,
            t_a1_inbox,
            1,
            "INBOX",
            Some("a1"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            100,
        );
        seed_conv_email(
            &conn,
            t_a1_sent,
            1,
            "Sent",
            Some("a1s"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            200,
        );
        seed_conv_email(
            &conn,
            t_a2_inbox,
            2,
            "INBOX",
            Some("a2"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );

        let mut by_account = base_filter();
        by_account.account_id = Some(2);
        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&by_account), 1, 50).unwrap();
        assert_eq!(
            (total, conv_ids(&rows)),
            (1, vec![t_a2_inbox]),
            "account_id 过滤"
        );

        let mut by_folder = base_filter();
        by_folder.account_id = Some(1);
        by_folder.folder = Some("INBOX");
        let (rows, total) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&by_folder), 1, 50).unwrap();
        assert_eq!(
            (total, conv_ids(&rows)),
            (1, vec![t_a1_inbox]),
            "单账号文件夹过滤"
        );
    }

    #[test]
    fn list_conversations_paginates_with_stable_total() {
        let conn = minimal_conn();
        let mut tids = Vec::new();
        for (i, updated) in [100, 300, 200].into_iter().enumerate() {
            let tid = seed_thread(&conn, &format!("t{i}"), "a@x", 100, updated);
            seed_conv_email(
                &conn,
                tid,
                1,
                "INBOX",
                Some("s"),
                None,
                "a@x",
                None,
                true,
                false,
                false,
                updated,
            );
            tids.push(tid);
        }
        let newest_first = vec![tids[1], tids[2], tids[0]];

        let (page1, total1) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&base_filter()), 1, 2).unwrap();
        let (page2, total2) =
            list_conversations_impl(&conn, &VisibilitySpec::from(&base_filter()), 2, 2).unwrap();

        assert_eq!((page1.len(), total1), (2, 3));
        assert_eq!((page2.len(), total2), (1, 3), "总数不随页变化");
        assert_eq!(conv_ids(&page1), newest_first[..2]);
        assert_eq!(conv_ids(&page2), vec![tids[0]]);
    }

    // ---------- conversation_emails ----------

    #[test]
    fn conversation_emails_orders_ascending_and_excludes_deleted() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "chrono", "a@x", 100, 300);
        let e_mid = seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("mid"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            200,
        );
        let e_last = seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("last"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            300,
        );
        let e_first = seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("first"),
            None,
            "a@x",
            None,
            false,
            false,
            false,
            100,
        );
        let deleted = seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("gone"),
            None,
            "a@x",
            None,
            true,
            false,
            true,
            400,
        );
        let _ = deleted;
        conn.execute(
            "UPDATE emails SET in_reply_to = '<root@x>' WHERE id = ?1",
            [e_first],
        )
        .unwrap();

        let rows =
            conversation_emails_impl(&conn, tid, &VisibilitySpec::from(&base_filter())).unwrap();

        assert_eq!(
            rows.iter().map(|m| m.id as i64).collect::<Vec<_>>(),
            vec![e_first, e_mid, e_last],
            "received_at 升序，软删除不返回"
        );
        assert_eq!(
            rows[0].in_reply_to.as_deref(),
            Some("<root@x>"),
            "Model 透传 in_reply_to"
        );
        assert!(rows.iter().all(|m| m.thread_id == Some(tid)));
    }

    #[test]
    fn conversation_emails_applies_visibility_filter() {
        let conn = minimal_conn();
        let tid = seed_thread(&conn, "cross", "a@x,b@x", 100, 300);
        let mine = seed_conv_email(
            &conn,
            tid,
            1,
            "INBOX",
            Some("mine"),
            None,
            "a@x",
            None,
            true,
            false,
            false,
            100,
        );
        let other = seed_conv_email(
            &conn,
            tid,
            2,
            "INBOX",
            Some("other"),
            None,
            "b@x",
            None,
            true,
            false,
            false,
            200,
        );

        let mut filter = base_filter();
        filter.account_id = Some(1);
        let rows = conversation_emails_impl(&conn, tid, &VisibilitySpec::from(&filter)).unwrap();

        assert_eq!(
            rows.iter().map(|m| m.id as i64).collect::<Vec<_>>(),
            vec![mine]
        );
        let _ = other;
    }
}
