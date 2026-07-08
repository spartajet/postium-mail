# 邮件同步范围与历史回填 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 添加账号后让用户选择首次同步范围，并在邮件列表底部支持按当前文件夹/分类同步更久邮件。

**Architecture:** 后端把“同步新邮件”和“历史回填旧邮件”拆成两个独立状态：`last_sync_uid` 继续负责增量新邮件，新增 `history_synced_since` 和 `history_exhausted` 负责旧邮件回填边界。前端账号添加流程增加同步范围步骤；邮件列表底部先处理本地分页，再根据后端历史状态显示“同步更久邮件”。

**Tech Stack:** Rust 2024, Tauri v2, rusqlite/tokio-rusqlite, async-imap, Specta/Tauri Specta bindings, Svelte 5 runes, Vitest, Bun.

## Global Constraints

- 文档和用户可见说明用中文书写。
- 没有用户明确指令，不提交代码。
- Shell 命令使用 `rtk` 前缀。
- Svelte 文件变更后运行 Svelte autofixer 或至少运行 `bun run check`。
- Tauri 新命令必须注册到 `tauri_specta::collect_commands!`，否则前端调用会失败。
- 后端命令参数使用 owned types，避免 async command 借用参数。
- 初始同步范围固定为：最近 1 周、最近 1 个月、最近 3 个月、最近 1 年、全部邮件。
- 默认初始同步范围为最近 3 个月。
- “同步更久邮件”每次只针对当前列表对应的文件夹/分类，向前回填 3 个月。
- 搜索结果和星标邮件不显示“同步更久邮件”。
- 本地不足一页时，只要 `history_exhausted = false`，仍可显示“同步更久邮件”。

---

## File Structure

后端文件：

- Modify: `src-tauri/sql/schema.sql`
  - 给新库加入 `sync_state.history_synced_since`、`sync_state.history_exhausted` 和 `emails(account_id, folder, uid)` 唯一索引。
- Modify: `src-tauri/src/infrastructure/storage/database.rs`
  - 在初始化 schema 后执行幂等迁移：补字段、清理重复 UID、创建唯一索引。
- Modify: `src-tauri/src/infrastructure/storage/models/sync_state.rs`
  - 给模型增加历史边界字段。
- Modify: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
  - 读写历史同步状态，支持 `history_synced_since`、`history_exhausted`。
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 提供最早邮件时间查询；保存邮件头/完整邮件时按 `(account_id, folder, uid)` 幂等 upsert。
- Modify: `src-tauri/src/infrastructure/protocols/imap/search.rs`
  - 新增日期区间 UID 查询。
- Modify: `src-tauri/src/infrastructure/protocols/utils.rs`
  - 新增时间戳到 IMAP 日期格式、同步范围日期计算工具。
- Modify: `src-tauri/src/domain/sync/mod.rs`
  - 新增 `InitialSyncRange`、`SyncWindow` 类型。
- Modify: `src-tauri/src/service/sync_service.rs`
  - 新增面向 Tauri 绑定的 `HistorySyncState`、`OlderSyncResult` DTO。
- Create: `src-tauri/src/domain/sync/history.rs`
  - 纯函数：同步范围转窗口、旧账号边界初始化、3 个月历史回填窗口计算。
- Modify: `src-tauri/src/domain/sync/folder_sync_full.rs`
  - 全量同步接收 `SyncWindow`，不再硬编码近三个月。
- Modify: `src-tauri/src/domain/sync/folder_sync_dispatcher.rs`
  - 增加带初始范围同步入口。
- Modify: `src-tauri/src/service/sync_service.rs`
  - 暴露带范围初始同步、历史状态查询、历史回填服务方法。
- Modify: `src-tauri/src/command/sync.rs`
  - 新增 Tauri commands。
- Modify: `src-tauri/src/lib.rs`
  - 注册新增 commands，导出绑定。
- Test: `src-tauri/tests/schema.rs`
  - 数据库迁移测试。
- Test: `src-tauri/tests/email_commands.rs` 或新增 `src-tauri/tests/sync_history.rs`
  - 同步历史状态、窗口计算、非法分类测试。

前端文件：

- Modify: `src/lib/bindings.ts`
  - 由 Specta 导出更新，不手写。
- Modify: `src/lib/components/settings/account-add-flow.ts`
  - 把“进入主界面”和“开始同步”拆开，支持带范围同步。
- Modify: `src/lib/components/settings/AddAccountModal.svelte`
  - 增加同步范围步骤，账号创建/OAuth2 完成后进入该步骤。
- Modify: `src/lib/stores/sync.svelte.ts`
  - 增加历史状态查询、历史回填动作、回填中状态。
- Modify: `src/lib/stores/email.svelte.ts`
  - 增加本地下一页 append 加载方法。
- Modify: `src/lib/components/email/EmailList.svelte`
  - 底部显示“加载更多”或“同步更久邮件”。
- Modify: `src/lib/i18n/zh-CN.ts`
  - 增加中文文案。
- Modify: `src/lib/i18n/en-US.ts`
  - 增加英文文案。
- Test: `src/lib/__tests__/components/account-add-flow.test.ts`
  - 更新账号添加后同步流程测试。
- Test: `src/lib/__tests__/stores/email.test.ts`
  - 增加本地分页 append 调用测试。
- Create: `src/lib/__tests__/stores/sync-history.test.ts`
  - 测试新增 sync store 命令调用。
- Create or Modify: `src/lib/__tests__/components/EmailList.test.ts`
  - 测试底部按钮显示条件。

---

### Task 1: 数据库 schema、轻量迁移和 sync_state 模型

**Files:**
- Modify: `src-tauri/sql/schema.sql`
- Modify: `src-tauri/src/infrastructure/storage/database.rs`
- Modify: `src-tauri/src/infrastructure/storage/models/sync_state.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
- Test: `src-tauri/tests/schema.rs`

**Interfaces:**
- Produces: `sync_state::Model { history_synced_since: Option<i64>, history_exhausted: Option<bool> }`
- Produces: `sync_repo::update_history_state(db, account_id, folder, history_synced_since, history_exhausted) -> Result<(), MailError>`
- Produces: `sync_repo::get_sync_state(...)` now returns history fields.

- [ ] **Step 1: Write failing migration tests**

Append these tests to `src-tauri/tests/schema.rs`:

```rust
#[tokio::test]
async fn init_database_migrates_legacy_sync_state_history_columns() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("postium.sqlite");

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE sync_state (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                folder TEXT NOT NULL,
                folder_nick_name TEXT,
                uidvalidity INTEGER,
                uidnext INTEGER,
                synced_at INTEGER,
                last_sync_uid INTEGER,
                created_at INTEGER,
                updated_at INTEGER
            );
            ",
        )
        .unwrap();
    }

    let db = init_database(temp.path()).await.unwrap();

    let columns = db
        .call(|conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(sync_state)")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .unwrap();

    assert!(columns.contains(&"history_synced_since".to_string()));
    assert!(columns.contains(&"history_exhausted".to_string()));
}

#[tokio::test]
async fn init_database_removes_duplicate_email_uids_before_unique_index() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("postium.sqlite");

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                provider TEXT NOT NULL,
                auth_type TEXT DEFAULT 'password',
                account_type TEXT NOT NULL DEFAULT 'personal',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE emails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                folder TEXT NOT NULL,
                uid INTEGER NOT NULL,
                message_id TEXT UNIQUE,
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
                sent_at INTEGER NOT NULL,
                received_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE attachments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
                filename TEXT,
                content_type TEXT,
                size INTEGER NOT NULL,
                section_path TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            INSERT INTO accounts (id, name, email, provider, auth_type, account_type, created_at, updated_at)
            VALUES (1, 'Legacy', 'legacy@example.com', 'custom', 'password', 'personal', 1, 1);
            INSERT INTO emails (id, account_id, folder, uid, subject, sender_email, recipient_emails, body_text, body_html, sent_at, received_at, created_at, updated_at)
            VALUES
                (1, 1, 'INBOX', 42, 'old', 'a@example.com', 'b@example.com', NULL, NULL, 10, 10, 1, 1),
                (2, 1, 'INBOX', 42, 'new', 'a@example.com', 'b@example.com', 'text', '<p>html</p>', 10, 10, 1, 2);
            INSERT INTO attachments (email_id, filename, content_type, size, section_path, created_at)
            VALUES (1, 'old.txt', 'text/plain', 1, '1', 1);
            ",
        )
        .unwrap();
    }

    let db = init_database(temp.path()).await.unwrap();

    let (email_count, remaining_subject, old_attachment_count) = db
        .call(|conn| {
            let email_count = conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE account_id = 1 AND folder = 'INBOX' AND uid = 42",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            let remaining_subject = conn.query_row(
                "SELECT subject FROM emails WHERE account_id = 1 AND folder = 'INBOX' AND uid = 42",
                [],
                |row| row.get::<_, String>(0),
            )?;
            let old_attachment_count = conn.query_row(
                "SELECT COUNT(*) FROM attachments WHERE email_id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, subject, sender_email, recipient_emails,
                    sent_at, received_at, created_at, updated_at
                ) VALUES (1, 'INBOX', 42, 'dup', 'a@example.com', 'b@example.com', 10, 10, 3, 3)",
                [],
            )
            .expect_err("unique index should reject duplicate account/folder/uid");
            Ok((email_count, remaining_subject, old_attachment_count))
        })
        .await
        .unwrap();

    assert_eq!(email_count, 1);
    assert_eq!(remaining_subject, "new");
    assert_eq!(old_attachment_count, 0);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run the two filters separately:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema init_database_migrates_legacy_sync_state_history_columns
rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema init_database_removes_duplicate_email_uids_before_unique_index
```

Expected: both tests fail because migration code and unique index do not exist yet.

- [ ] **Step 3: Update schema for new databases**

Modify `src-tauri/sql/schema.sql`:

```sql
CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT NOT NULL,
    folder_nick_name TEXT,
    uidvalidity INTEGER,
    uidnext INTEGER,
    synced_at INTEGER,
    last_sync_uid INTEGER,
    history_synced_since INTEGER,
    history_exhausted INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);
```

Add index near existing email indexes:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid ON emails(account_id, folder, uid);
```

- [ ] **Step 4: Implement migration helpers**

Modify `src-tauri/src/infrastructure/storage/database.rs`:

```rust
    async fn initialize_schema(&self) -> Result<(), MailError> {
        self.call(|conn| {
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.pragma_update(None, "busy_timeout", 5000)?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.execute_batch(SCHEMA_SQL)?;
            migrate_sync_state_history_columns(conn)?;
            migrate_email_uid_unique_index(conn)?;
            Ok(())
        })
        .await
    }
```

Add private helpers in the same file:

```rust
fn table_has_column(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|name| name == column))
}

fn migrate_sync_state_history_columns(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    if !table_has_column(conn, "sync_state", "history_synced_since")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_synced_since INTEGER",
            [],
        )?;
    }
    if !table_has_column(conn, "sync_state", "history_exhausted")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_exhausted INTEGER DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

fn index_exists(conn: &rusqlite::Connection, index_name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [index_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

fn migrate_email_uid_unique_index(conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    if index_exists(conn, "idx_emails_account_folder_uid")? {
        return Ok(());
    }

    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM emails
         WHERE id IN (
             SELECT duplicate.id
             FROM emails duplicate
             JOIN emails kept
               ON kept.account_id = duplicate.account_id
              AND kept.folder = duplicate.folder
              AND kept.uid = duplicate.uid
              AND kept.id != duplicate.id
             WHERE kept.id = (
                 SELECT candidate.id
                 FROM emails candidate
                 WHERE candidate.account_id = duplicate.account_id
                   AND candidate.folder = duplicate.folder
                   AND candidate.uid = duplicate.uid
                 ORDER BY
                   CASE
                     WHEN candidate.body_text IS NOT NULL OR candidate.body_html IS NOT NULL THEN 0
                     ELSE 1
                   END,
                   candidate.updated_at DESC,
                   candidate.id DESC
                 LIMIT 1
             )
             AND duplicate.id != kept.id
         )",
        [],
    )?;
    tx.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid
         ON emails(account_id, folder, uid)",
        [],
    )?;
    tx.commit()
}
```

- [ ] **Step 5: Update sync_state model and repository mapping**

Modify `src-tauri/src/infrastructure/storage/models/sync_state.rs`:

```rust
    /// 历史邮件已经同步到的最早时间戳（Unix 秒）
    pub history_synced_since: Option<i64>,
    /// 是否已确认没有更早的历史邮件
    pub history_exhausted: Option<bool>,
```

Modify `src-tauri/src/infrastructure/storage/repository/sync_repo.rs` mapping:

```rust
use crate::infrastructure::storage::row::opt_int_to_bool;
```

In `map_sync_state`:

```rust
        history_synced_since: row.get("history_synced_since")?,
        history_exhausted: opt_int_to_bool(row.get("history_exhausted")?),
```

Update SELECT columns in `get_sync_state`:

```sql
SELECT id, account_id, folder, folder_nick_name, uidvalidity, uidnext,
       synced_at, last_sync_uid, history_synced_since, history_exhausted,
       created_at, updated_at
```

Add helper:

```rust
pub async fn update_history_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    history_synced_since: Option<i64>,
    history_exhausted: bool,
) -> Result<(), MailError> {
    let folder = folder.to_string();
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, history_synced_since, history_exhausted, synced_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)
             ON CONFLICT(account_id, folder) DO UPDATE SET
                history_synced_since = excluded.history_synced_since,
                history_exhausted = excluded.history_exhausted,
                synced_at = excluded.synced_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                account_id,
                folder,
                history_synced_since,
                if history_exhausted { 1 } else { 0 },
                now,
            ],
        )?;
        Ok(())
    })
    .await
}
```

- [ ] **Step 6: Run tests and verify pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema
```

Expected: all schema tests pass.

- [ ] **Step 7: Review only**

Do not commit. Review `rtk git diff -- src-tauri/sql/schema.sql src-tauri/src/infrastructure/storage/database.rs src-tauri/src/infrastructure/storage/models/sync_state.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/tests/schema.rs`.

---

### Task 2: 邮件保存幂等性和最早邮件查询

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Test: `src-tauri/tests/email_commands.rs` or Create: `src-tauri/tests/email_repository.rs`

**Interfaces:**
- Produces: `email_repo::earliest_sent_at_by_folder(db, account_id, folder) -> Result<Option<i64>, MailError>`
- Produces: `save_batch_email_headers` and `save_batch_emails` are idempotent for existing `(account_id, folder, uid)`.

- [ ] **Step 1: Write failing repository tests**

Create `src-tauri/tests/email_repository.rs`:

```rust
use postium_mail_lib::infrastructure::protocols::types::{EmailFlags, EmailHeader};
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::infrastructure::storage::repository::email_repo;

async fn seed_account(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Test', 'test@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

fn header(uid: u32, subject: &str, sent_at: i64) -> EmailHeader {
    EmailHeader {
        uid,
        subject: subject.to_string(),
        from: "Sender <sender@example.com>".to_string(),
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        date: chrono::DateTime::from_timestamp(sent_at, 0).unwrap(),
        flags: EmailFlags {
            seen: false,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
            recent: false,
        },
        attachments: vec![],
    }
}

#[tokio::test]
async fn save_batch_email_headers_should_ignore_existing_account_folder_uid() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let first = vec![header(100, "first", 1000)];
    let duplicate = vec![header(100, "duplicate", 1000)];

    let first_count = email_repo::save_batch_email_headers(&db, 1, "INBOX", &first)
        .await
        .unwrap();
    let second_count = email_repo::save_batch_email_headers(&db, 1, "INBOX", &duplicate)
        .await
        .unwrap();

    let row_count = db
        .call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE account_id = 1 AND folder = 'INBOX' AND uid = 100",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(first_count, 1);
    assert_eq!(second_count, 0);
    assert_eq!(row_count, 1);
}

#[tokio::test]
async fn earliest_sent_at_by_folder_should_return_oldest_local_email() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    email_repo::save_batch_email_headers(
        &db,
        1,
        "INBOX",
        &[header(101, "newer", 3000), header(102, "older", 1000)],
    )
    .await
    .unwrap();

    let earliest = email_repo::earliest_sent_at_by_folder(&db, 1, "INBOX")
        .await
        .unwrap();

    assert_eq!(earliest, Some(1000));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_repository
```

Expected: fails because `earliest_sent_at_by_folder` does not exist and duplicate insert errors or counts wrong.

- [ ] **Step 3: Make header save idempotent**

In `src-tauri/src/infrastructure/storage/repository/email_repo.rs`, replace the insert statement used by `save_batch_email_headers` with an insert that ignores duplicate account/folder/uid:

```rust
const INSERT_EMAIL_HEADER_SQL: &str = "
INSERT INTO emails (
    account_id, folder, uid, message_id, subject, sender_name, sender_email,
    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
    is_read, is_starred, is_draft, is_answered, is_deleted,
    sent_at, received_at, created_at, updated_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7,
    ?8, ?9, ?10, ?11, ?12, ?13,
    ?14, ?15, ?16, ?17, ?18,
    ?19, ?20, ?21, ?22
)
ON CONFLICT(account_id, folder, uid) DO NOTHING";
```

In the header transaction, track affected rows:

```rust
let inserted = execute_email_insert(&mut email_stmt, email)?;
if inserted == 0 {
    continue;
}
let email_id = tx.last_insert_rowid() as i32;
```

If `execute_email_insert` currently returns `rusqlite::Result<()>`, change it to:

```rust
fn execute_email_insert(
    stmt: &mut rusqlite::Statement<'_>,
    email: &EmailWrite,
) -> rusqlite::Result<usize> {
    stmt.execute(rusqlite::params![
        email.account_id,
        email.folder,
        email.uid,
        email.message_id,
        email.subject,
        email.sender_name,
        email.sender_email,
        email.recipient_emails,
        email.cc_emails,
        email.bcc_emails,
        email.preview,
        email.body_text,
        email.body_html,
        opt_bool_to_int(email.is_read),
        opt_bool_to_int(email.is_starred),
        opt_bool_to_int(email.is_draft),
        opt_bool_to_int(email.is_answered),
        opt_bool_to_int(email.is_deleted),
        email.sent_at,
        email.received_at,
        email.created_at,
        email.updated_at,
    ])
}
```

- [ ] **Step 4: Make complete email save upsert body fields**

For `save_batch_emails`, use upsert so a later full fetch can fill body for an existing header:

```rust
const UPSERT_EMAIL_SQL: &str = "
INSERT INTO emails (
    account_id, folder, uid, message_id, subject, sender_name, sender_email,
    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
    is_read, is_starred, is_draft, is_answered, is_deleted,
    sent_at, received_at, created_at, updated_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7,
    ?8, ?9, ?10, ?11, ?12, ?13,
    ?14, ?15, ?16, ?17, ?18,
    ?19, ?20, ?21, ?22
)
ON CONFLICT(account_id, folder, uid) DO UPDATE SET
    message_id = COALESCE(excluded.message_id, emails.message_id),
    subject = excluded.subject,
    sender_name = excluded.sender_name,
    sender_email = excluded.sender_email,
    recipient_emails = excluded.recipient_emails,
    cc_emails = excluded.cc_emails,
    bcc_emails = excluded.bcc_emails,
    preview = excluded.preview,
    body_text = excluded.body_text,
    body_html = excluded.body_html,
    is_read = excluded.is_read,
    is_starred = excluded.is_starred,
    is_draft = excluded.is_draft,
    is_answered = excluded.is_answered,
    is_deleted = excluded.is_deleted,
    sent_at = excluded.sent_at,
    received_at = excluded.received_at,
    updated_at = excluded.updated_at";
```

After upsert, fetch the email id for attachments:

```rust
let email_id = tx.query_row(
    "SELECT id FROM emails WHERE account_id = ?1 AND folder = ?2 AND uid = ?3",
    rusqlite::params![email.account_id, email.folder, email.uid],
    |row| row.get::<_, i32>(0),
)?;
tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
```

- [ ] **Step 5: Add earliest sent_at query**

Add to `email_repo.rs`:

```rust
pub async fn earliest_sent_at_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<i64>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        match conn.query_row(
            "SELECT MIN(sent_at)
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND is_deleted = 0",
            rusqlite::params![account_id, folder],
            |row| row.get::<_, Option<i64>>(0),
        ) {
            Ok(value) => Ok(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}
```

- [ ] **Step 6: Run repository tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_repository
```

Expected: PASS.

- [ ] **Step 7: Run existing email tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands
```

Expected: PASS.

- [ ] **Step 8: Review only**

Do not commit. Review `rtk git diff -- src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/tests/email_repository.rs`.

---

### Task 3: 同步窗口领域类型和 IMAP 日期区间查询

**Files:**
- Modify: `src-tauri/src/domain/sync/mod.rs`
- Create: `src-tauri/src/domain/sync/history.rs`
- Modify: `src-tauri/src/infrastructure/protocols/utils.rs`
- Modify: `src-tauri/src/infrastructure/protocols/imap/search.rs`
- Test: unit tests in `src-tauri/src/domain/sync/history.rs`

**Interfaces:**
- Produces: `InitialSyncRange`
- Produces: `SyncWindow { start: Option<i64>, end: Option<i64> }`
- Produces: `older_window_from_boundary(boundary) -> SyncWindow`
- Produces: `ImapClient::list_uids_between(folder, start_date, end_date)`

- [ ] **Step 1: Write failing pure unit tests**

Create `src-tauri/src/domain/sync/history.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_sync_range_should_convert_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::Week, now),
            SyncWindow {
                start: Some(now - 7 * 24 * 60 * 60),
                end: None,
            },
        );
        assert_eq!(
            window_for_initial_range(InitialSyncRange::ThreeMonths, now),
            SyncWindow {
                start: Some(now - 90 * 24 * 60 * 60),
                end: None,
            },
        );
        assert_eq!(
            window_for_initial_range(InitialSyncRange::All, now),
            SyncWindow {
                start: None,
                end: None,
            },
        );
    }

    #[test]
    fn older_sync_window_should_move_boundary_back_three_months() {
        let boundary = 1_700_000_000;
        let window = older_window_from_boundary(boundary);

        assert_eq!(window.end, Some(boundary));
        assert_eq!(window.start, Some(boundary - 90 * 24 * 60 * 60));
    }

    #[test]
    fn legacy_boundary_should_prefer_local_earliest_email() {
        let now = 1_700_000_000;
        let boundary = initialize_legacy_boundary(Some(1_600_000_000), now);

        assert_eq!(boundary, 1_600_000_000);
    }

    #[test]
    fn legacy_boundary_should_default_to_three_months_when_folder_empty() {
        let now = 1_700_000_000;
        let boundary = initialize_legacy_boundary(None, now);

        assert_eq!(boundary, now - 90 * 24 * 60 * 60);
    }
}
```

- [ ] **Step 2: Run unit tests and verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml domain::sync::history
```

Expected: fails because module and types are incomplete.

- [ ] **Step 3: Add domain types**

Modify `src-tauri/src/domain/sync/mod.rs`:

```rust
pub mod history;
```

Add types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InitialSyncRange {
    Week,
    Month,
    ThreeMonths,
    Year,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncWindow {
    pub start: Option<i64>,
    pub end: Option<i64>,
}

```

- [ ] **Step 4: Implement history pure functions**

In `src-tauri/src/domain/sync/history.rs`:

```rust
use crate::domain::sync::{InitialSyncRange, SyncWindow};

const DAY_SECONDS: i64 = 24 * 60 * 60;
const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;

pub fn window_for_initial_range(range: InitialSyncRange, now: i64) -> SyncWindow {
    let start = match range {
        InitialSyncRange::Week => Some(now - 7 * DAY_SECONDS),
        InitialSyncRange::Month => Some(now - MONTH_SECONDS),
        InitialSyncRange::ThreeMonths => Some(now - 3 * MONTH_SECONDS),
        InitialSyncRange::Year => Some(now - 365 * DAY_SECONDS),
        InitialSyncRange::All => None,
    };

    SyncWindow { start, end: None }
}

pub fn older_window_from_boundary(boundary: i64) -> SyncWindow {
    SyncWindow {
        start: Some(boundary - 3 * MONTH_SECONDS),
        end: Some(boundary),
    }
}

pub fn initialize_legacy_boundary(local_earliest_sent_at: Option<i64>, now: i64) -> i64 {
    local_earliest_sent_at.unwrap_or(now - 3 * MONTH_SECONDS)
}
```

- [ ] **Step 5: Add IMAP date formatter and between search**

In `src-tauri/src/infrastructure/protocols/utils.rs` add:

```rust
pub fn unix_seconds_to_imap_date(timestamp: i64) -> String {
    let date = chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_else(chrono::Utc::now)
        .date_naive();
    date.format("%d-%b-%Y").to_string()
}
```

In `src-tauri/src/infrastructure/protocols/imap/search.rs` add:

```rust
    pub async fn list_uids_between(
        &mut self,
        folder: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<u32>, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let search_cmd = format!("SINCE {} BEFORE {}", start_date, end_date);
        tracing::info!("📤 使用 IMAP 历史区间搜索命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        Ok(uid_list)
    }
```

- [ ] **Step 6: Run tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml domain::sync::history
```

Expected: PASS.

- [ ] **Step 7: Review only**

Do not commit. Review `rtk git diff -- src-tauri/src/domain/sync/mod.rs src-tauri/src/domain/sync/history.rs src-tauri/src/infrastructure/protocols/utils.rs src-tauri/src/infrastructure/protocols/imap/search.rs`.

---

### Task 4: 后端同步服务支持初始范围和历史回填

**Files:**
- Modify: `src-tauri/src/domain/sync/folder_sync_full.rs`
- Modify: `src-tauri/src/domain/sync/folder_sync_dispatcher.rs`
- Modify: `src-tauri/src/service/sync_service.rs`
- Modify: `src-tauri/src/command/sync.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/sync_history.rs`

**Interfaces:**
- Produces: `SyncService::sync_account_with_range(app_handle, account_id, range) -> Result<(), MailError>`
- Produces: `SyncService::get_history_state(account_id, category) -> Result<HistorySyncState, MailError>`
- Produces: `SyncService::sync_older_emails(account_id, category) -> Result<OlderSyncResult, MailError>`
- Produces commands: `sync_account_with_range`, `get_sync_history_state`, `sync_older_emails`

- [ ] **Step 1: Write service-level failing tests for category support**

Create `src-tauri/tests/sync_history.rs`:

```rust
use postium_mail_lib::domain::providers::pool::init_provider_pool;
use postium_mail_lib::domain::sync::InitialSyncRange;
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::service::email_service::EmailCategory;
use postium_mail_lib::service::SyncService;
use std::sync::Arc;

async fn seed_account(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Gmail', 'user@gmail.com', 'gmail', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn history_state_should_reject_starred_category() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service.get_history_state(1, EmailCategory::Starred).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn history_state_should_return_folder_state_for_inbox() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service.get_history_state(1, EmailCategory::Inbox).await.unwrap();

    assert_eq!(result.account_id, 1);
    assert_eq!(result.category, EmailCategory::Inbox);
    assert!(!result.folders.is_empty());
    assert!(!result.history_exhausted);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test sync_history
```

Expected: fails because service methods do not exist.

- [ ] **Step 3: Refactor full sync to accept SyncWindow**

Modify `sync_folder_full` signature:

```rust
pub async fn sync_folder_full(
    db: DbConn,
    account_id: i32,
    folder: &str,
    uidvalidity: u32,
    imap_client: &mut ImapClient,
    window: SyncWindow,
) -> Result<SyncResult, MailError>
```

Replace hard-coded date:

```rust
let uids = if let Some(start) = window.start {
    let date_since = unix_seconds_to_imap_date(start);
    imap_client.list_uids_since(folder, &date_since).await?
} else {
    imap_client.list_uids_since_uid(folder, 1).await?
};
```

After successful sync state upsert, update history state:

```rust
if let Some(start) = window.start {
    sync_repo::update_history_state(&db, account_id, folder, Some(start), false).await?;
} else {
    sync_repo::update_history_state(&db, account_id, folder, None, true).await?;
}
```

- [ ] **Step 4: Add range-aware account sync**

In `SyncOrchestrator`, add:

```rust
pub async fn sync_account_with_range(
    &self,
    account_id: i32,
    range: InitialSyncRange,
) -> Result<SyncResult, MailError> {
    let now = chrono::Utc::now().timestamp();
    let initial_window = crate::domain::sync::history::window_for_initial_range(range, now);
    self.sync_account_with_initial_window(account_id, Some(initial_window)).await
}
```

Extract existing `sync_account` body into:

```rust
async fn sync_account_with_initial_window(
    &self,
    account_id: i32,
    initial_window: Option<SyncWindow>,
) -> Result<SyncResult, MailError>
```

When matching full sync:

```rust
let window = initial_window.unwrap_or_else(|| {
    crate::domain::sync::history::window_for_initial_range(
        InitialSyncRange::ThreeMonths,
        chrono::Utc::now().timestamp(),
    )
});
sync_folder_full(self.db.clone(), account_id, folder, uidvalidity as u32, &mut client, window).await?
```

Keep `sync_account(account_id)` calling `sync_account_with_initial_window(account_id, None)` so existing refresh behavior remains compatible.

- [ ] **Step 5: Add history state and older sync service methods**

In `src-tauri/src/service/sync_service.rs`, add DTOs near `SyncService`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct HistorySyncState {
    pub account_id: i32,
    pub category: crate::service::email_service::EmailCategory,
    pub history_synced_since: Option<i64>,
    pub history_exhausted: bool,
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OlderSyncResult {
    pub new_emails: u64,
    pub updated_emails: u64,
    pub window_start: i64,
    pub window_end: i64,
    pub history_exhausted: bool,
    pub folders: Vec<String>,
}
```

Then add methods in `SyncService`:

```rust
pub async fn sync_account_with_range(
    &self,
    app_handle: tauri::AppHandle,
    account_id: i32,
    range: InitialSyncRange,
) -> Result<(), MailError> {
    let emitter = SyncProgressEmitter::new(app_handle);
    emitter.emit(SyncProgress {
        account_id,
        stage: SyncStage::Connecting,
        folder: None,
        current: 0,
        total: 0,
        message: "正在连接...".into(),
    });

    let orchestrator =
        SyncOrchestrator::new(self.db.clone(), self.auth.clone()).with_emitter(emitter);
    orchestrator.sync_account_with_range(account_id, range).await.map(|_| ())
}

pub async fn get_history_state(
    &self,
    account_id: i32,
    category: crate::service::email_service::EmailCategory,
) -> Result<HistorySyncState, MailError> {
    if category == crate::service::email_service::EmailCategory::Starred {
        return Err(MailError::InvalidParam("星标邮件不支持历史回填".into()));
    }

    let account = account_repo::get_by_id(&self.db, account_id)
        .await?
        .ok_or(MailError::AccountNotFound(account_id))?;
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported("未找到provider pool".into()))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let folders = category.resolve_folders(&provider.folder_mapping());
    if folders.is_empty() {
        return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
    }

    let mut history_synced_since = None;
    let mut history_exhausted = true;
    for folder in &folders {
        let state = sync_repo::get_sync_state(&self.db, account_id, folder).await?;
        history_synced_since = match (history_synced_since, state.as_ref().and_then(|s| s.history_synced_since)) {
            (Some(existing), Some(next)) => Some(existing.min(next)),
            (None, next) => next,
            (existing, None) => existing,
        };
        history_exhausted &= state.and_then(|s| s.history_exhausted).unwrap_or(false);
    }

    Ok(HistorySyncState {
        account_id,
        category,
        history_synced_since,
        history_exhausted,
        folders,
    })
}
```

Use the existing `MailError::InvalidParam` variant for unsupported categories.

- [ ] **Step 6: Implement older sync orchestration in SyncService**

Add `SyncService::sync_older_emails(account_id, category) -> Result<OlderSyncResult, MailError>`. Keep this in the service layer because it returns service DTOs and already resolves provider/category state.

First resolve the account, provider, credentials, IMAP config, and folders using the same patterns as `SyncOrchestrator::sync_account`:

```rust
let account = account_repo::get_by_id(&self.db, account_id)
    .await?
    .ok_or(MailError::AccountNotFound(account_id))?;
let provider_pool = PROVIDER_POOL
    .get()
    .ok_or(MailError::ProviderNotSupported("未找到provider pool".into()))?
    .clone();
let provider = provider_pool
    .get(&account.provider)
    .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
let folders = category.resolve_folders(&provider.folder_mapping());
if folders.is_empty() || category == EmailCategory::Starred {
    return Err(MailError::InvalidParam("当前分类不支持历史回填".into()));
}
let credentials = self
    .auth
    .get_credentials(&account.email, &provider.provider_info().auth_type, Some(&account.provider))
    .await?;
let imap_config = crate::service::account_connection::imap_config_from_account(&account)?;
let mut client = match credentials {
    crate::domain::auth::manager::Credentials::Password(password) => {
        ImapClient::connect(&imap_config, &account.email, &password).await?
    }
    crate::domain::auth::manager::Credentials::OAuth2 { access_token } => {
        ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await?
    }
};
```

Then for each folder, use these concrete rules:

```rust
let local_earliest = email_repo::earliest_sent_at_by_folder(&self.db, account_id, folder).await?;
let boundary = state
    .and_then(|s| s.history_synced_since)
    .unwrap_or_else(|| history::initialize_legacy_boundary(local_earliest, now));
let window = history::older_window_from_boundary(boundary);
let start_date = unix_seconds_to_imap_date(window.start.unwrap());
let end_date = unix_seconds_to_imap_date(window.end.unwrap());
let uids = client.list_uids_between(folder, &start_date, &end_date).await?;
```

For each folder:

```rust
for group in uids.chunks(10) {
    let email_headers = client
        .batch_fetch_email_headers(folder, group[0], group[group.len() - 1])
        .await?;
    new_emails += email_repo::save_batch_email_headers(&self.db, account_id, folder, &email_headers).await?;
}
sync_repo::update_history_state(&self.db, account_id, folder, window.start, false).await?;
```

Return:

```rust
Ok(OlderSyncResult {
    new_emails: new_emails as u64,
    updated_emails: 0,
    window_start: min_window_start,
    window_end: max_window_end,
    history_exhausted: false,
    folders,
})
```

Do not mark `history_exhausted = true` for an empty three-month window unless implementing a reliable minimum UID probe in the same task.

- [ ] **Step 7: Add Tauri commands**

In `src-tauri/src/command/sync.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn sync_account_with_range(
    service: tauri::State<'_, crate::service::SyncService>,
    app_handle: tauri::AppHandle,
    account_id: i32,
    range: InitialSyncRange,
) -> Result<(), MailError> {
    service.sync_account_with_range(app_handle, account_id, range).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_sync_history_state(
    service: tauri::State<'_, crate::service::SyncService>,
    account_id: i32,
    category: EmailCategory,
) -> Result<HistorySyncState, MailError> {
    service.get_history_state(account_id, category).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_older_emails(
    service: tauri::State<'_, crate::service::SyncService>,
    account_id: i32,
    category: EmailCategory,
) -> Result<OlderSyncResult, MailError> {
    service.sync_older_emails(account_id, category).await
}
```

Add imports:

```rust
use crate::domain::sync::InitialSyncRange;
use crate::service::email_service::EmailCategory;
use crate::service::sync_service::{HistorySyncState, OlderSyncResult};
```

- [ ] **Step 8: Register commands**

Modify `src-tauri/src/lib.rs` command collection:

```rust
            command::sync::sync_account_with_range,
            command::sync::get_sync_history_state,
            command::sync::sync_older_emails,
```

- [ ] **Step 9: Run backend tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test sync_history
rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema
```

Expected: PASS.

- [ ] **Step 10: Check bindings compile**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS and debug binding export compiles.

- [ ] **Step 11: Review only**

Do not commit. Review backend diffs for command registration and error serialization.

---

### Task 5: 前端绑定刷新和 sync store 历史接口

**Files:**
- Modify: `src/lib/bindings.ts`
- Modify: `src/lib/stores/sync.svelte.ts`
- Create: `src/lib/__tests__/stores/sync-history.test.ts`

**Interfaces:**
- Consumes commands: `syncAccountWithRange(accountId, range)`, `getSyncHistoryState(accountId, category)`, `syncOlderEmails(accountId, category)`
- Produces: `syncStore.historyStates`
- Produces: `syncStore.loadHistoryState(accountId, category)`
- Produces: `syncStore.syncOlderEmails(accountId, category)`

- [ ] **Step 1: Regenerate bindings**

Run a debug check that triggers Specta export:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: `src/lib/bindings.ts` contains:

```ts
syncAccountWithRange
getSyncHistoryState
syncOlderEmails
export type InitialSyncRange
export type HistorySyncState
export type OlderSyncResult
```

- [ ] **Step 2: Write failing sync store tests**

Create `src/lib/__tests__/stores/sync-history.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createMockResult, mockInvoke } from "../mocks/tauri";

const { invoke } = await import("@tauri-apps/api/core");

describe("同步历史命令调用", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("get_sync_history_state 使用账号和分类参数", async () => {
    mockInvoke.mockResolvedValue(
      createMockResult({
        account_id: 1,
        category: "inbox",
        history_synced_since: 1700000000,
        history_exhausted: false,
        folders: ["INBOX"],
      }),
    );

    const result = await invoke("get_sync_history_state", {
      accountId: 1,
      category: "inbox",
    });

    expect(result).toMatchObject({ status: "ok" });
    expect(mockInvoke).toHaveBeenCalledWith("get_sync_history_state", {
      accountId: 1,
      category: "inbox",
    });
  });

  it("sync_older_emails 使用账号和分类参数", async () => {
    mockInvoke.mockResolvedValue(
      createMockResult({
        new_emails: 3,
        updated_emails: 0,
        window_start: 1600000000,
        window_end: 1700000000,
        history_exhausted: false,
        folders: ["INBOX"],
      }),
    );

    const result = await invoke("sync_older_emails", {
      accountId: 1,
      category: "inbox",
    });

    expect(result).toMatchObject({ status: "ok" });
    expect(mockInvoke).toHaveBeenCalledWith("sync_older_emails", {
      accountId: 1,
      category: "inbox",
    });
  });
});
```

- [ ] **Step 3: Run tests and verify failure if bindings missing**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/sync-history.test.ts
```

Expected: PASS for raw invoke tests after bindings exist. If import path setup fails, fix the test to match existing `email.test.ts` mock style.

- [ ] **Step 4: Add sync store state and methods**

Modify `src/lib/stores/sync.svelte.ts`:

```ts
import type {
  SyncProgress,
  FolderStat,
  EmailCategory,
  HistorySyncState,
  OlderSyncResult,
  InitialSyncRange,
} from "$lib/bindings";
```

Inside `SyncState`:

```ts
  historyStates = $state<Record<string, HistorySyncState>>({});
  olderSyncingKeys = $state<Set<string>>(new Set());

  private historyKey(accountId: number, category: EmailCategory) {
    return `${accountId}:${category}`;
  }

  async syncAccountWithRange(accountId: number, range: InitialSyncRange) {
    this.syncing = true;
    this.error = null;
    try {
      const result = await commands.syncAccountWithRange(accountId, range);
      if (result.status === "error") {
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      this.error = formatError(e);
    } finally {
      this.syncing = false;
    }
  }

  async loadHistoryState(accountId: number, category: EmailCategory) {
    const result = await commands.getSyncHistoryState(accountId, category);
    if (result.status === "error") {
      this.error = result.error.message as string;
      return null;
    }
    this.historyStates = {
      ...this.historyStates,
      [this.historyKey(accountId, category)]: result.data,
    };
    return result.data;
  }

  async syncOlderEmails(
    accountId: number,
    category: EmailCategory,
  ): Promise<OlderSyncResult | null> {
    const key = this.historyKey(accountId, category);
    this.olderSyncingKeys = new Set([...this.olderSyncingKeys, key]);
    this.error = null;
    try {
      const result = await commands.syncOlderEmails(accountId, category);
      if (result.status === "error") {
        this.error = result.error.message as string;
        return null;
      }
      await this.loadHistoryState(accountId, category);
      return result.data;
    } catch (e: unknown) {
      this.error = formatError(e);
      return null;
    } finally {
      const next = new Set(this.olderSyncingKeys);
      next.delete(key);
      this.olderSyncingKeys = next;
    }
  }

  isOlderSyncing(accountId: number, category: EmailCategory) {
    return this.olderSyncingKeys.has(this.historyKey(accountId, category));
  }

  getHistoryState(accountId: number, category: EmailCategory) {
    return this.historyStates[this.historyKey(accountId, category)] ?? null;
  }
```

- [ ] **Step 5: Run frontend tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/sync-history.test.ts
rtk bun run check
```

Expected: PASS.

- [ ] **Step 6: Review only**

Do not commit. Review `rtk git diff -- src/lib/bindings.ts src/lib/stores/sync.svelte.ts src/lib/__tests__/stores/sync-history.test.ts`.

---

### Task 6: 账号添加流程增加同步范围步骤

**Files:**
- Modify: `src/lib/components/settings/account-add-flow.ts`
- Modify: `src/lib/components/settings/AddAccountModal.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Modify: `src/lib/__tests__/components/account-add-flow.test.ts`

**Interfaces:**
- Consumes: `syncStore.syncAccountWithRange(accountId, range)`
- Produces: `continueAfterAccountAdded` no longer starts sync immediately; new helper `startInitialSyncAfterAccountAdded`.

- [ ] **Step 1: Update helper tests first**

Replace current test in `src/lib/__tests__/components/account-add-flow.test.ts` with:

```ts
import { describe, it, expect, vi } from "vitest";
import {
  continueAfterAccountAdded,
  startInitialSyncAfterAccountAdded,
} from "$lib/components/settings/account-add-flow";

describe("account add flow", () => {
  it("添加账号后只进入主界面，不立即同步", async () => {
    const calls: string[] = [];

    const result = await continueAfterAccountAdded({
      accountId: 7,
      loadAccounts: async () => {
        calls.push("loadAccounts");
      },
      setActive: (accountId) => {
        calls.push(`setActive:${accountId}`);
      },
      close: () => {
        calls.push("close");
      },
      goHome: async () => {
        calls.push("goHome");
      },
    });

    expect(calls).toEqual(["loadAccounts", "setActive:7", "close", "goHome"]);
    expect(result.readyForInitialSync).toBe(true);
  });

  it("用户选择范围后启动带范围首次同步", async () => {
    const syncAccountWithRange = vi.fn(async () => undefined);

    await startInitialSyncAfterAccountAdded({
      accountId: 7,
      range: "three_months",
      syncAccountWithRange,
    });

    expect(syncAccountWithRange).toHaveBeenCalledWith(7, "three_months");
  });
});
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/account-add-flow.test.ts
```

Expected: fails because helper signatures have not changed.

- [ ] **Step 3: Update account-add-flow helper**

Modify `src/lib/components/settings/account-add-flow.ts`:

```ts
import type { InitialSyncRange } from "$lib/bindings";

type ContinueAfterAccountAddedOptions = {
  accountId: number;
  loadAccounts: () => Promise<void>;
  setActive: (accountId: number) => void;
  close: () => void;
  goHome: () => Promise<void>;
};

export async function continueAfterAccountAdded({
  accountId,
  loadAccounts,
  setActive,
  close,
  goHome,
}: ContinueAfterAccountAddedOptions) {
  await loadAccounts();
  setActive(accountId);
  close();
  await goHome();

  return { readyForInitialSync: true };
}

type StartInitialSyncOptions = {
  accountId: number;
  range: InitialSyncRange;
  syncAccountWithRange: (
    accountId: number,
    range: InitialSyncRange,
  ) => Promise<void>;
};

export async function startInitialSyncAfterAccountAdded({
  accountId,
  range,
  syncAccountWithRange,
}: StartInitialSyncOptions) {
  await syncAccountWithRange(accountId, range);
  return { syncStarted: true };
}
```

- [ ] **Step 4: Add i18n labels**

In `zh-CN.ts`, add under account/sync area matching existing structure:

```ts
syncScopeTitle: "选择要同步的邮件范围",
syncScopeHint: "稍后可以在邮件列表底部继续同步更早邮件。",
syncRangeWeek: "最近 1 周",
syncRangeMonth: "最近 1 个月",
syncRangeThreeMonths: "最近 3 个月",
syncRangeYear: "最近 1 年",
syncRangeAll: "全部邮件",
syncRangeAllHint: "全部邮件可能耗时较长。",
startSync: "开始同步",
```

In `en-US.ts`:

```ts
syncScopeTitle: "Choose mail sync range",
syncScopeHint: "You can sync older mail later from the bottom of the mail list.",
syncRangeWeek: "Last 1 week",
syncRangeMonth: "Last 1 month",
syncRangeThreeMonths: "Last 3 months",
syncRangeYear: "Last 1 year",
syncRangeAll: "All mail",
syncRangeAllHint: "All mail may take longer.",
startSync: "Start sync",
```

- [ ] **Step 5: Update AddAccountModal state**

In `AddAccountModal.svelte`:

```ts
import type { InitialSyncRange, ProviderInfo } from "$lib/bindings";
import { continueAfterAccountAdded, startInitialSyncAfterAccountAdded } from "./account-add-flow";
```

Change step type:

```ts
let step = $state<"select" | "credentials" | "sync-scope" | "done">("select");
let pendingInitialSyncAccountId = $state<number | null>(null);
let selectedInitialSyncRange = $state<InitialSyncRange>("three_months");
```

Replace `enterMainAndStartSync` with:

```ts
async function enterSyncScope(accountId: number) {
  await accountStore.loadAccounts();
  accountStore.setActive(accountId);
  pendingInitialSyncAccountId = accountId;
  selectedInitialSyncRange = "three_months";
  step = "sync-scope";
}

async function startSelectedInitialSync() {
  if (pendingInitialSyncAccountId === null) return;
  const accountId = pendingInitialSyncAccountId;
  await continueAfterAccountAdded({
    accountId,
    loadAccounts: () => accountStore.loadAccounts(),
    setActive: (id) => accountStore.setActive(id),
    close,
    goHome: () => goto("/"),
  });
  void startInitialSyncAfterAccountAdded({
    accountId,
    range: selectedInitialSyncRange,
    syncAccountWithRange: (id, range) => syncStore.syncAccountWithRange(id, range),
  });
}
```

In password submit success:

```ts
await enterSyncScope(result.data.id);
```

In OAuth2 completed success:

```ts
await enterSyncScope(accountId);
```

Reset in `close()`:

```ts
pendingInitialSyncAccountId = null;
selectedInitialSyncRange = "three_months";
```

- [ ] **Step 6: Add sync-scope markup**

Add a step block:

```svelte
{:else if step === "sync-scope"}
  <div class="p-6">
    <h3 class="text-base font-semibold text-foreground">
      {t.account.syncScopeTitle}
    </h3>
    <p class="mt-1 text-sm text-muted-foreground">
      {t.account.syncScopeHint}
    </p>

    <div class="mt-5 space-y-2">
      {#each [
        { value: "week", label: t.account.syncRangeWeek },
        { value: "month", label: t.account.syncRangeMonth },
        { value: "three_months", label: t.account.syncRangeThreeMonths },
        { value: "year", label: t.account.syncRangeYear },
        { value: "all", label: t.account.syncRangeAll, hint: t.account.syncRangeAllHint },
      ] as option}
        <label class="flex cursor-pointer items-start gap-3 rounded-lg border border-border px-3 py-2 hover:bg-glass-hover">
          <input
            type="radio"
            name="initial-sync-range"
            value={option.value}
            bind:group={selectedInitialSyncRange}
            class="mt-1"
          />
          <span class="min-w-0">
            <span class="block text-sm font-medium text-foreground">{option.label}</span>
            {#if option.hint}
              <span class="block text-xs text-muted-foreground">{option.hint}</span>
            {/if}
          </span>
        </label>
      {/each}
    </div>

    <button
      class="mt-6 w-full rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
      onclick={startSelectedInitialSync}
    >
      {t.account.startSync}
    </button>
  </div>
```

- [ ] **Step 7: Run Svelte checks and tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/account-add-flow.test.ts
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/settings/AddAccountModal.svelte --svelte-version 5
rtk bun run check
```

Expected: PASS. Autofixer should report no blocking Svelte issues.

- [ ] **Step 8: Review only**

Do not commit. Review frontend account add diffs.

---

### Task 7: 邮件列表本地分页和同步更久入口

**Files:**
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src/lib/components/email/EmailList.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Modify: `src/lib/__tests__/stores/email.test.ts`
- Create or Modify: `src/lib/__tests__/components/EmailList.test.ts`

**Interfaces:**
- Consumes: `syncStore.getHistoryState`, `syncStore.loadHistoryState`, `syncStore.syncOlderEmails`, `syncStore.isOlderSyncing`
- Produces: `EmailState.loadNextPage(accountId) -> Promise<void>`

- [ ] **Step 1: Add email store pagination test**

Append to `src/lib/__tests__/stores/email.test.ts`:

```ts
it("loadNextPage 调用下一页分类邮件参数", async () => {
  mockInvoke.mockResolvedValue(
    createMockResult({ emails: [], total: 100, page: 2, limit: 50 }),
  );

  await invokeMock<EmailList>("list_emails_by_category", {
    accountId: 1,
    category: "inbox",
    page: 2,
    limit: 50,
  });

  expect(mockInvoke).toHaveBeenCalledWith("list_emails_by_category", {
    accountId: 1,
    category: "inbox",
    page: 2,
    limit: 50,
  });
});
```

This test validates command shape. If adding direct `EmailState` unit tests is feasible in this codebase, replace this with a direct state test for append behavior.

- [ ] **Step 2: Add EmailState.loadNextPage**

In `src/lib/stores/email.svelte.ts`:

```ts
  async loadNextPage(accountId: number) {
    if (this.loading || this.emails.length >= this.total) return;

    const nextPage = this.page + 1;
    this.loading = true;
    try {
      const result = await commands.listEmailsByCategory(
        accountId,
        this.currentFolder,
        nextPage,
        this.limit,
      );
      if (result.status === "ok") {
        this.emails = [...this.emails, ...result.data.emails];
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      this.setError(e, "Failed to load next email page");
    } finally {
      this.loading = false;
    }
  }
```

- [ ] **Step 3: Add i18n labels**

In `zh-CN.ts` under email:

```ts
loadMore: "加载更多",
syncOlder: "同步更久邮件",
syncingOlder: "正在同步更久...",
```

In `en-US.ts`:

```ts
loadMore: "Load more",
syncOlder: "Sync older mail",
syncingOlder: "Syncing older mail...",
```

- [ ] **Step 4: Update EmailList imports**

In `EmailList.svelte`, import sync store:

```ts
import { getSyncState } from "$lib/stores/sync.svelte";
```

Initialize:

```ts
const syncStore = getSyncState();
```

Add derived helpers:

```ts
let supportsOlderSync = $derived(
  searchResults === null && emailState.currentFolder !== "starred",
);
let localHasMore = $derived(emailState.emails.length < emailState.total);
let historyState = $derived(
  accountStore.activeAccountId
    ? syncStore.getHistoryState(accountStore.activeAccountId, emailState.currentFolder)
    : null,
);
let canSyncOlder = $derived(
  supportsOlderSync &&
    !localHasMore &&
    historyState !== null &&
    !historyState.history_exhausted,
);
let olderSyncing = $derived(
  accountStore.activeAccountId
    ? syncStore.isOlderSyncing(accountStore.activeAccountId, emailState.currentFolder)
    : false,
);
```

Add effect to load history state:

```ts
$effect(() => {
  const accountId = accountStore.activeAccountId;
  const category = emailState.currentFolder;
  const searchingNow = searchResults !== null;
  if (accountId && !searchingNow && category !== "starred") {
    void syncStore.loadHistoryState(accountId, category);
  }
});
```

- [ ] **Step 5: Add click handlers**

In `EmailList.svelte`:

```ts
async function handleLoadMore() {
  if (accountStore.activeAccountId) {
    await emailState.loadNextPage(accountStore.activeAccountId);
  }
}

async function handleSyncOlder() {
  const accountId = accountStore.activeAccountId;
  if (!accountId) return;
  const result = await syncStore.syncOlderEmails(accountId, emailState.currentFolder);
  if (result) {
    await emailState.loadEmailsByCategory(accountId, emailState.currentFolder, 1);
  }
}
```

- [ ] **Step 6: Add footer markup**

Inside the scrollable list after the `{#each}` block and before closing the scroll container:

```svelte
{#if searchResults === null && emailState.emails.length > 0}
  <div class="border-t border-border p-3">
    {#if localHasMore}
      <button
        class="w-full rounded-md border border-border px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
        onclick={handleLoadMore}
        disabled={emailState.loading}
      >
        {t.email.loadMore}
      </button>
    {:else if canSyncOlder}
      <button
        class="w-full rounded-md border border-border px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground disabled:opacity-60"
        onclick={handleSyncOlder}
        disabled={olderSyncing}
      >
        {olderSyncing ? t.email.syncingOlder : t.email.syncOlder}
      </button>
    {/if}
  </div>
{/if}
```

For empty state, also render a footer button below the empty message when `canSyncOlder` is true:

```svelte
{#if canSyncOlder}
  <button
    class="mt-4 rounded-md border border-border px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground disabled:opacity-60"
    onclick={handleSyncOlder}
    disabled={olderSyncing}
  >
    {olderSyncing ? t.email.syncingOlder : t.email.syncOlder}
  </button>
{/if}
```

- [ ] **Step 7: Run tests and checks**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/email.test.ts
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/email/EmailList.svelte --svelte-version 5
rtk bun run check
```

Expected: PASS. If Svelte reports invalid `$derived` dependencies around `searchResults`, convert helper expressions into plain functions called from markup.

- [ ] **Step 8: Review only**

Do not commit. Review EmailList and store diffs.

---

### Task 8: Final verification and integration pass

**Files:**
- All modified files from Tasks 1-7.

**Interfaces:**
- Verifies all new commands are registered, bindings compile, Rust tests pass, frontend tests pass.

- [ ] **Step 1: Run Rust formatting**

Run:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml
```

Expected: no errors.

- [ ] **Step 2: Run Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests**

Run:

```bash
rtk bun run test:frontend
```

Expected: PASS.

- [ ] **Step 4: Run Svelte/type check**

Run:

```bash
rtk bun run check
```

Expected: PASS.

- [ ] **Step 5: Inspect generated bindings**

Run:

```bash
rtk rg "syncAccountWithRange|getSyncHistoryState|syncOlderEmails|InitialSyncRange|HistorySyncState|OlderSyncResult" src/lib/bindings.ts
```

Expected: all six names appear.

- [ ] **Step 6: Manual smoke path**

Run the app only if the user asks for manual verification. If running locally, use:

```bash
rtk bun run tauri dev
```

Expected manual behavior:

1. Add account after validation.
2. Modal shows sync range step with “最近 3 个月” selected.
3. Choosing a range enters main view and starts sync.
4. Email list bottom shows “加载更多” while local pages remain.
5. After local pages are exhausted, inbox shows “同步更久邮件” when history is not exhausted.
6. Search and starred views do not show “同步更久邮件”.

- [ ] **Step 7: Final review**

Run:

```bash
rtk git status --short
rtk git diff --stat
```

Expected: only files listed in this plan changed. Do not commit unless the user explicitly instructs it.
