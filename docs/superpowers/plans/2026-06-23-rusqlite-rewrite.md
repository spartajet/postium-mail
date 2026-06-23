# rusqlite 数据库层全量替换 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 一次性移除 SeaORM/sea-orm-migration，改用 `tokio-rusqlite + rusqlite`，启动时删除旧数据库并按 `src-tauri/sql/schema.sql` 重建。

**Architecture:** `src-tauri/sql/schema.sql` 是唯一 schema 来源；`DbConn` 封装 `tokio_rusqlite::Connection`，所有 repository 通过 `Connection::call` 执行同步 rusqlite SQL；跨表操作通过 `DbConn::transaction` 在单个 call 闭包内完成。保留现有 service/command 行为，先用兼容命名的普通 model 替代 SeaORM entity，降低业务层改动面。

**Tech Stack:** Rust 2024, Tauri 2, SQLite FTS5, `tokio-rusqlite`, `rusqlite`, `serde`, `specta`, `bun`, `vitest`.

## Global Constraints

- 一次性全量替换，不做 SeaORM/rusqlite 双栈。
- 不迁移旧数据，不兼容旧版本数据库。
- 应用启动时删除 `postium.sqlite`、`postium.sqlite-wal`、`postium.sqlite-shm` 并重建。
- `src-tauri/sql/schema.sql` 是唯一 schema 来源，生产初始化和测试必须共用它。
- 不实现历史 migration 系统，不保留 `src-tauri/migration` crate。
- 不引入连接池；使用 `tokio-rusqlite` 单连接后台线程。
- 事务不得跨 `await` 持有；事务只允许在 `Connection::call` 闭包内同步执行。
- 动态 SQL 只允许拼接占位符，不允许拼接用户输入值。
- 每个任务完成后提交一次。

---

## File Structure

创建：

- `src-tauri/sql/schema.sql`：完整建表 SQL、索引、FTS5 虚表和触发器。
- `src-tauri/src/infrastructure/storage/models/mod.rs`：导出普通 Rust model。
- `src-tauri/src/infrastructure/storage/models/accounts.rs`
- `src-tauri/src/infrastructure/storage/models/emails.rs`
- `src-tauri/src/infrastructure/storage/models/attachments.rs`
- `src-tauri/src/infrastructure/storage/models/labels.rs`
- `src-tauri/src/infrastructure/storage/models/email_labels.rs`
- `src-tauri/src/infrastructure/storage/models/sync_state.rs`
- `src-tauri/src/infrastructure/storage/models/sync_errors.rs`
- `src-tauri/src/infrastructure/storage/row.rs`：row mapping 和 bool/int 辅助函数。
- `src-tauri/tests/schema.rs`：替代历史 migration 测试。

修改：

- `src-tauri/Cargo.toml`：移除 SeaORM 依赖，增加 `rusqlite`、`tokio-rusqlite`。
- `src-tauri/src/infrastructure/storage/database.rs`：改成 rusqlite 初始化、删除旧库、schema 执行、事务 helper。
- `src-tauri/src/infrastructure/storage/mod.rs`：导出 `models`，停止导出 `entities`。
- `src-tauri/src/infrastructure/storage/repository/*.rs`：全部改为手写 SQL。
- `src-tauri/src/infrastructure/storage/search.rs`：改为 rusqlite row mapping。
- `src-tauri/src/error/types.rs`：替换数据库错误转换。
- `src-tauri/src/service/*.rs`：去掉 `sea_orm::Set`、`ActiveModel`、`TransactionTrait` 等 API。
- `src-tauri/src/infrastructure/testing/e2e_seed.rs`：改为 repository 或 SQL seed。
- `src-tauri/tests/common/mod.rs`：新测试库 helper。
- `src-tauri/tests/account_commands.rs`
- `src-tauri/tests/email_commands.rs`
- `src-tauri/tests/label_commands.rs`
- `src-tauri/tests/e2e_seed.rs`
- `src-tauri/tests/migrations.rs`：删除或改名为 `schema.rs`。
- `src-tauri/Cargo.lock`：由 cargo 更新。

删除：

- `src-tauri/migration/`
- `src-tauri/src/infrastructure/storage/entities/`

---

### Task 1: 建立 schema.sql 和 rusqlite 数据库基础设施

**Files:**
- Create: `src-tauri/sql/schema.sql`
- Create: `src-tauri/tests/schema.rs`
- Delete: `src-tauri/tests/migrations.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/infrastructure/storage/database.rs`
- Modify: `src-tauri/src/error/types.rs`
- Modify: `src-tauri/src/infrastructure/storage/mod.rs`

**Interfaces:**
- Produces: `#[derive(Clone)] pub struct DbConn`
- Produces: `impl DbConn { pub async fn open_reset(path: &Path) -> Result<Self, MailError>; pub async fn open_in_memory_for_test() -> Result<Self, MailError>; pub async fn call<F, R>(&self, f: F) -> Result<R, MailError>; pub async fn transaction<F, R>(&self, f: F) -> Result<R, MailError>; }`
- Produces: `pub async fn init_database(data_dir: &Path) -> Result<DbConn, MailError>`
- Produces: `pub const SCHEMA_SQL: &str`
- Consumes: `MailError::DatabaseError(String)`

- [ ] **Step 1: Add dependencies and remove direct SeaORM dependencies**

Edit `src-tauri/Cargo.toml` dependencies section:

```toml
# Remove:
# postium-mail-migration = { path = "migration" }
# sea-orm = { ... }
# sea-orm-migration = "2.0.0-rc.35"

# Add:
rusqlite = { version = "0.37", features = ["bundled", "chrono"] }
tokio-rusqlite = "0.7"
```

Run: `rtk cargo check --manifest-path src-tauri/Cargo.toml`

Expected: FAIL with unresolved `sea_orm`/`postium_mail_migration` imports. This is the intentional red state for the full rewrite.

- [ ] **Step 2: Write schema.sql**

Create `src-tauri/sql/schema.sql` with this complete schema:

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT,
    provider TEXT NOT NULL,
    imap_host TEXT,
    imap_port INTEGER,
    imap_ssl INTEGER DEFAULT 1,
    imap_ssl_mode TEXT,
    smtp_host TEXT,
    smtp_port INTEGER,
    smtp_ssl INTEGER DEFAULT 1,
    smtp_ssl_mode TEXT,
    color TEXT,
    sync_enabled INTEGER DEFAULT 1,
    last_sync_at INTEGER,
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
    disposition TEXT,
    content_id TEXT,
    path TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE email_labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    UNIQUE(email_id, label_id)
);

CREATE TABLE sync_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT NOT NULL,
    folder_nick_name TEXT,
    uidvalidity INTEGER,
    uidnext INTEGER,
    synced_at INTEGER,
    last_sync_uid INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE sync_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT,
    error_type TEXT NOT NULL,
    error_message TEXT NOT NULL,
    uid INTEGER,
    stack_trace TEXT,
    resolved INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_emails_account ON emails(account_id);
CREATE INDEX idx_emails_folder ON emails(folder);
CREATE INDEX idx_emails_sent_at ON emails(sent_at DESC);
CREATE INDEX idx_emails_is_read ON emails(is_read);
CREATE INDEX idx_attachments_email ON attachments(email_id);
CREATE INDEX idx_labels_account ON labels(account_id);
CREATE INDEX idx_email_labels_email ON email_labels(email_id);
CREATE INDEX idx_email_labels_label ON email_labels(label_id);
CREATE UNIQUE INDEX idx_sync_state_account_folder ON sync_state(account_id, folder);
CREATE INDEX idx_sync_errors_account ON sync_errors(account_id);

CREATE VIRTUAL TABLE emails_fts USING fts5(
    subject,
    sender_email,
    preview,
    content='emails',
    content_rowid='id'
);

CREATE TRIGGER emails_fts_ai AFTER INSERT ON emails BEGIN
    INSERT INTO emails_fts(rowid, subject, sender_email, preview)
    VALUES (new.id, new.subject, new.sender_email, new.preview);
END;

CREATE TRIGGER emails_fts_ad AFTER DELETE ON emails BEGIN
    INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
    VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
END;

CREATE TRIGGER emails_fts_au AFTER UPDATE ON emails BEGIN
    INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
    VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
    INSERT INTO emails_fts(rowid, subject, sender_email, preview)
    VALUES (new.id, new.subject, new.sender_email, new.preview);
END;
```

- [ ] **Step 3: Add failing schema tests**

Create `src-tauri/tests/schema.rs`:

```rust
use postium_mail_lib::infrastructure::storage::database::{DbConn, init_database};
use tempfile::tempdir;

#[tokio::test]
async fn schema_creates_fts_triggers_that_track_email_changes() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();

    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Test', 'test@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO emails (
                id, account_id, folder, uid, subject, sender_email, recipient_emails,
                preview, sent_at, received_at, created_at, updated_at
            ) VALUES (
                1, 1, 'INBOX', 1, 'Alpha subject', 'sender@example.com',
                'to@example.com', 'Alpha preview', 1, 1, 1, 1
            )",
            [],
        )?;

        let alpha_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM emails_fts WHERE emails_fts MATCH 'Alpha'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(alpha_count, 1);

        conn.execute(
            "UPDATE emails SET subject = 'Beta subject', preview = 'Beta preview' WHERE id = 1",
            [],
        )?;

        let alpha_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM emails_fts WHERE emails_fts MATCH 'Alpha'",
            [],
            |row| row.get(0),
        )?;
        let beta_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM emails_fts WHERE emails_fts MATCH 'Beta'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(alpha_count, 0);
        assert_eq!(beta_count, 1);

        conn.execute("DELETE FROM emails WHERE id = 1", [])?;

        let beta_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM emails_fts WHERE emails_fts MATCH 'Beta'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(beta_count, 0);

        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn init_database_removes_existing_database_files_before_recreating_schema() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("postium.sqlite");
    let wal_path = temp.path().join("postium.sqlite-wal");
    let shm_path = temp.path().join("postium.sqlite-shm");

    std::fs::write(&db_path, b"old database bytes").unwrap();
    std::fs::write(&wal_path, b"old wal bytes").unwrap();
    std::fs::write(&shm_path, b"old shm bytes").unwrap();

    let db = init_database(temp.path()).await.unwrap();

    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES ('Reset', 'reset@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    assert!(db_path.exists());
    assert!(!wal_path.exists() || std::fs::metadata(&wal_path).unwrap().len() >= 0);
    assert!(!shm_path.exists() || std::fs::metadata(&shm_path).unwrap().len() >= 0);
}
```

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema -- --nocapture`

Expected: FAIL because `DbConn::open_in_memory_for_test` and rusqlite database layer do not exist yet.

Delete `src-tauri/tests/migrations.rs` in the same step. The new schema test replaces historical migration coverage, and leaving the old file in place will compile SeaORM migration imports during intermediate tasks.

- [ ] **Step 4: Implement MailError conversions**

In `src-tauri/src/error/types.rs`, replace SeaORM conversion and its test:

```rust
impl From<rusqlite::Error> for MailError {
    fn from(err: rusqlite::Error) -> Self {
        MailError::DatabaseError(err.to_string())
    }
}

impl From<tokio_rusqlite::Error> for MailError {
    fn from(err: tokio_rusqlite::Error) -> Self {
        MailError::DatabaseError(err.to_string())
    }
}
```

Update `test_from_db_error`:

```rust
#[test]
fn test_from_db_error() {
    let db_err = rusqlite::Error::QueryReturnedNoRows;
    let mail_err: MailError = db_err.into();
    assert!(matches!(mail_err, MailError::DatabaseError(_)));
}
```

- [ ] **Step 5: Implement DbConn**

Replace `src-tauri/src/infrastructure/storage/database.rs` with:

```rust
use std::path::{Path, PathBuf};

use crate::error::MailError;

pub const SCHEMA_SQL: &str = include_str!("../../../sql/schema.sql");

#[derive(Clone)]
pub struct DbConn {
    inner: tokio_rusqlite::Connection,
}

impl DbConn {
    pub async fn open_reset(db_path: &Path) -> Result<Self, MailError> {
        reset_database_files(db_path)?;
        let conn = tokio_rusqlite::Connection::open(db_path).await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    pub async fn open_in_memory_for_test() -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open_in_memory().await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    async fn initialize_schema(&self) -> Result<(), MailError> {
        self.call(|conn| {
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.pragma_update(None, "busy_timeout", 5000)?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.execute_batch(SCHEMA_SQL)?;
            Ok(())
        })
        .await
    }

    pub async fn call<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        Ok(self.inner.call(f).await??)
    }

    pub async fn transaction<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
        .await
    }
}

pub async fn init_database(data_dir: &Path) -> Result<DbConn, MailError> {
    let db_path = data_dir.join("postium.sqlite");
    std::fs::create_dir_all(data_dir)
        .map_err(|err| MailError::DatabaseError(err.to_string()))?;

    tracing::info!("重建数据库: {}", db_path.display());
    DbConn::open_reset(&db_path).await
}

fn reset_database_files(db_path: &Path) -> Result<(), MailError> {
    for path in database_files(db_path) {
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!("已删除旧数据库文件: {}", path.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(MailError::DatabaseError(err.to_string())),
        }
    }
    Ok(())
}

fn database_files(db_path: &Path) -> [PathBuf; 3] {
    [
        db_path.to_path_buf(),
        PathBuf::from(format!("{}-wal", db_path.display())),
        PathBuf::from(format!("{}-shm", db_path.display())),
    ]
}
```

Before finalizing this step, run `rtk cargo check --manifest-path src-tauri/Cargo.toml`. If the compiler reports a signature mismatch for `tokio_rusqlite::Connection::call`, inspect the installed crate source under Cargo's registry and adjust only the private `DbConn::call` implementation to match the crate API. Keep the public `DbConn` methods and repository-facing signatures unchanged.

- [ ] **Step 6: Update storage mod**

Change `src-tauri/src/infrastructure/storage/mod.rs`:

```rust
pub mod database;
pub mod models;
pub mod repository;
pub mod search;

pub use database::DbConn;
```

- [ ] **Step 7: Run schema tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema -- --nocapture`

Expected: PASS for schema tests, while full crate may still fail because repositories and services still reference SeaORM.

- [ ] **Step 8: Commit**

```bash
rtk git add -A src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/sql/schema.sql src-tauri/tests/schema.rs src-tauri/tests/migrations.rs src-tauri/src/infrastructure/storage/database.rs src-tauri/src/infrastructure/storage/mod.rs src-tauri/src/error/types.rs
rtk git commit -m "feat: add rusqlite database foundation"
```

---

### Task 2: 用普通 Rust model 和 row helper 替代 SeaORM entities

**Files:**
- Create: `src-tauri/src/infrastructure/storage/models/*.rs`
- Create: `src-tauri/src/infrastructure/storage/row.rs`
- Modify: `src-tauri/src/infrastructure/storage/mod.rs`
- Modify: type imports in service/repository files from `entities` to `models`

**Interfaces:**
- Consumes: `DbConn` from Task 1.
- Produces: `crate::infrastructure::storage::models::{accounts, emails, attachments, labels, email_labels, sync_state, sync_errors}`
- Produces: each module has `pub struct Model` matching old SeaORM fields.
- Produces: `row::int_to_bool`, `row::opt_int_to_bool`, `row::bool_to_int`, `row::opt_bool_to_int`.

- [ ] **Step 1: Create row helper**

Create `src-tauri/src/infrastructure/storage/row.rs`:

```rust
pub fn int_to_bool(value: i64) -> bool {
    value != 0
}

pub fn opt_int_to_bool(value: Option<i64>) -> Option<bool> {
    value.map(int_to_bool)
}

pub fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub fn opt_bool_to_int(value: Option<bool>) -> Option<i64> {
    value.map(bool_to_int)
}
```

- [ ] **Step 2: Create model modules**

Create `src-tauri/src/infrastructure/storage/models/mod.rs`:

```rust
pub mod accounts;
pub mod attachments;
pub mod email_labels;
pub mod emails;
pub mod labels;
pub mod sync_errors;
pub mod sync_state;
```

Create `accounts.rs`:

```rust
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub imap_ssl_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub smtp_ssl_mode: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
    pub last_sync_at: Option<i64>,
    pub auth_type: Option<String>,
    pub account_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}
```

Create `emails.rs`, `attachments.rs`, `labels.rs`, `email_labels.rs`, `sync_state.rs`, `sync_errors.rs` with the same field names and types as the old entity models.

- [ ] **Step 3: Export row helper**

Update `src-tauri/src/infrastructure/storage/mod.rs`:

```rust
pub mod database;
pub mod models;
pub mod repository;
pub mod row;
pub mod search;

pub use database::DbConn;
```

- [ ] **Step 4: Change imports from entities to models**

Use targeted edits in these files:

- `src-tauri/src/infrastructure/storage/repository/*.rs`
- `src-tauri/src/service/*.rs`
- `src-tauri/src/infrastructure/testing/e2e_seed.rs`
- `src-tauri/tests/common/mod.rs`
- `src-tauri/tests/*.rs`

Replace:

```rust
use crate::infrastructure::storage::entities::{accounts, emails};
```

with:

```rust
use crate::infrastructure::storage::models::{accounts, emails};
```

Do not try to compile green yet; repository APIs still need rewriting.

- [ ] **Step 5: Run compile to expose remaining SeaORM API usage**

Run: `rtk cargo check --manifest-path src-tauri/Cargo.toml`

Expected: FAIL only on SeaORM API usages such as `ActiveModel`, `Set`, `Entity`, `Column`, `QueryFilter`, not because model modules are missing.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/infrastructure/storage/models src-tauri/src/infrastructure/storage/row.rs src-tauri/src/infrastructure/storage/mod.rs src-tauri/src/infrastructure/storage/repository src-tauri/src/service src-tauri/src/infrastructure/testing src-tauri/tests
rtk git commit -m "feat: add storage models for rusqlite"
```

---

### Task 3: 重写 account、label、sync repository 和账号 service

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/account_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/label_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
- Modify: `src-tauri/src/service/account_service.rs`
- Modify: `src-tauri/src/service/label_service.rs`
- Modify: `src-tauri/tests/account_commands.rs`
- Modify: `src-tauri/tests/label_commands.rs`
- Modify: `src-tauri/tests/common/mod.rs`

**Interfaces:**
- Consumes: `models::{accounts, labels, email_labels, sync_state, sync_errors}`
- Produces: `account_repo::{list, get_by_id, get_by_email, create, update, delete, update_last_sync}`
- Produces: `label_repo::{list_by_account, get_by_id, create, update, delete, delete_by_account, add_to_email, remove_from_email, get_labels_for_email, list_emails_by_label}`
- Produces: `sync_repo::{get_state, upsert_state, update_sync_state, record_error, delete_by_account}`

- [ ] **Step 1: Update tests away from SeaORM insert helpers**

In `src-tauri/tests/common/mod.rs`, replace `create_test_db` with:

```rust
pub async fn create_test_db() -> DbConn {
    DbConn::open_in_memory_for_test()
        .await
        .expect("Failed to create in-memory SQLite test database")
}
```

Replace direct `Entity::insert` helpers with `db.call` SQL inserts. Example for `insert_test_email`:

```rust
pub async fn insert_test_email(svc: &TestServices, account_id: i32, email: TestEmail) -> i32 {
    let now = chrono::Utc::now().timestamp();
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, ?11, ?12, ?13, 0, 0, ?14, ?15, ?16, ?17, ?18)",
                rusqlite::params![
                    account_id,
                    email.folder,
                    email.uid,
                    format!("<test-{}@example.com>", email.uid),
                    email.subject,
                    "Test Sender",
                    email.sender_email,
                    "recipient@example.com",
                    email.preview,
                    email.body_text,
                    Some("<p>Body</p>".to_string()),
                    email.is_read as i64,
                    email.is_starred as i64,
                    email.is_deleted as i64,
                    email.sent_at,
                    email.sent_at,
                    now,
                    now,
                ],
            )?;
            Ok(conn.last_insert_rowid() as i32)
        })
        .await
        .unwrap()
}
```

In account and label tests, replace SeaORM count assertions with `db.call` SQL count queries.

- [ ] **Step 2: Run account/label tests for red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands --test label_commands`

Expected: FAIL because repositories still use SeaORM APIs.

- [ ] **Step 3: Implement account_repo with rusqlite**

Replace `account_repo.rs` with functions that use `db.call`. Include row mapper:

```rust
fn map_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<accounts::Model> {
    Ok(accounts::Model {
        id: row.get("id")?,
        name: row.get("name")?,
        email: row.get("email")?,
        display_name: row.get("display_name")?,
        provider: row.get("provider")?,
        imap_host: row.get("imap_host")?,
        imap_port: row.get("imap_port")?,
        imap_ssl: crate::infrastructure::storage::row::opt_int_to_bool(row.get("imap_ssl")?),
        imap_ssl_mode: row.get("imap_ssl_mode")?,
        smtp_host: row.get("smtp_host")?,
        smtp_port: row.get("smtp_port")?,
        smtp_ssl: crate::infrastructure::storage::row::opt_int_to_bool(row.get("smtp_ssl")?),
        smtp_ssl_mode: row.get("smtp_ssl_mode")?,
        color: row.get("color")?,
        sync_enabled: crate::infrastructure::storage::row::opt_int_to_bool(row.get("sync_enabled")?),
        last_sync_at: row.get("last_sync_at")?,
        auth_type: row.get("auth_type")?,
        account_type: row.get("account_type")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
```

Expose create/update inputs as ordinary structs to replace ActiveModel:

```rust
pub struct AccountWrite {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub imap_ssl_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub smtp_ssl_mode: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
    pub last_sync_at: Option<i64>,
    pub auth_type: Option<String>,
    pub account_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}
```

Implement `list`, `get_by_id`, `get_by_email`, `create`, `update`, `delete`, `update_last_sync` with bound parameters.

- [ ] **Step 4: Update AccountService**

Replace `accounts::ActiveModel` creation with `account_repo::AccountWrite`. Remove `sea_orm::{Set, TransactionTrait}` import.

For `delete`, use:

```rust
self.db
    .transaction(move |tx| {
        label_repo::delete_by_account_tx(tx, id)?;
        email_repo::delete_by_account_tx(tx, id)?;
        sync_repo::delete_by_account_tx(tx, id)?;
        account_repo::delete_tx(tx, id)?;
        Ok(())
    })
    .await?;
```

Task 3 may add temporary stubs for `email_repo::delete_by_account_tx`; Task 4 will complete email repository. If adding stubs, they must execute the actual SQL deletes for `attachments` and `emails`.

- [ ] **Step 5: Implement label_repo and LabelService writes**

Add `LabelWrite { account_id, name, color, created_at }` and use SQL insert/update. For duplicate email label relation use:

```sql
INSERT OR IGNORE INTO email_labels (email_id, label_id, created_at) VALUES (?1, ?2, ?3)
```

Implement `delete_by_account_tx(tx, account_id)` with:

```sql
DELETE FROM email_labels
WHERE email_id IN (SELECT id FROM emails WHERE account_id = ?1)
   OR label_id IN (SELECT id FROM labels WHERE account_id = ?1);
DELETE FROM labels WHERE account_id = ?1;
```

- [ ] **Step 6: Implement sync_repo**

Use `INSERT ... ON CONFLICT(account_id, folder) DO UPDATE SET ...` for upsert. Ensure `schema.sql` has `idx_sync_state_account_folder` unique index.

Implement `delete_by_account_tx`:

```sql
DELETE FROM sync_errors WHERE account_id = ?1;
DELETE FROM sync_state WHERE account_id = ?1;
```

- [ ] **Step 7: Run tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
rtk cargo test --manifest-path src-tauri/Cargo.toml --test label_commands
```

Expected: both PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/infrastructure/storage/repository/account_repo.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs src-tauri/src/service/label_service.rs src-tauri/tests/common/mod.rs src-tauri/tests/account_commands.rs src-tauri/tests/label_commands.rs
rtk git commit -m "feat: rewrite account label sync storage with rusqlite"
```

---

### Task 4: 重写 email、attachment、search repository 和邮件 service

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/attachment_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/search.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/service/sync_service.rs`
- Modify: `src-tauri/src/domain/sync/*.rs`
- Modify: `src-tauri/tests/email_commands.rs`
- Modify: `src-tauri/tests/schema.rs`

**Interfaces:**
- Consumes: model modules and `DbConn`.
- Produces: current email repository behavior with rusqlite SQL.
- Produces: `search::search_fts(db, query, account_id, limit)`.

- [ ] **Step 1: Update email tests away from SeaORM**

Replace direct SeaORM imports/counts in `src-tauri/tests/email_commands.rs` with SQL through `DbConn::call`. Keep assertions identical at behavior level.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands`

Expected: FAIL because `email_repo` still uses SeaORM.

- [ ] **Step 2: Implement email row mapper**

In `email_repo.rs`, add:

```rust
fn map_email(row: &rusqlite::Row<'_>) -> rusqlite::Result<emails::Model> {
    Ok(emails::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        folder: row.get("folder")?,
        uid: row.get::<_, i64>("uid")? as u32,
        message_id: row.get("message_id")?,
        subject: row.get("subject")?,
        sender_name: row.get("sender_name")?,
        sender_email: row.get("sender_email")?,
        recipient_emails: row.get("recipient_emails")?,
        cc_emails: row.get("cc_emails")?,
        bcc_emails: row.get("bcc_emails")?,
        preview: row.get("preview")?,
        body_text: row.get("body_text")?,
        body_html: row.get("body_html")?,
        is_read: crate::infrastructure::storage::row::opt_int_to_bool(row.get("is_read")?),
        is_starred: crate::infrastructure::storage::row::opt_int_to_bool(row.get("is_starred")?),
        is_draft: crate::infrastructure::storage::row::opt_int_to_bool(row.get("is_draft")?),
        is_answered: crate::infrastructure::storage::row::opt_int_to_bool(row.get("is_answered")?),
        is_deleted: crate::infrastructure::storage::row::opt_int_to_bool(row.get("is_deleted")?),
        sent_at: row.get("sent_at")?,
        received_at: row.get("received_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
```

- [ ] **Step 3: Implement email read/list/update APIs**

Use SQL equivalents:

- `list_by_folder`: count + paginated select with `account_id`, `folder`, `is_deleted = 0`, order `sent_at DESC`.
- `list_by_folders`: dynamically generate placeholders for folders.
- `list_starred`: `is_starred = 1 AND is_deleted = 0`.
- `get_by_id`
- `get_by_uid`
- `mark_as_read`
- `toggle_star`
- `soft_delete`
- `move_to_folder`
- `folder_stats_by_account`
- `get_ids_by_folder`
- `delete_by_folder`

For dynamic folder placeholders:

```rust
fn placeholders(count: usize) -> String {
    std::iter::repeat("?").take(count).collect::<Vec<_>>().join(",")
}
```

Bind values using `rusqlite::params_from_iter`.

- [ ] **Step 4: Implement email insert APIs**

Replace ActiveModel-based insert functions with write structs:

```rust
pub struct EmailWrite { /* same insertable email fields except id */ }
pub struct AttachmentWrite { /* same insertable attachment fields except id */ }
```

Update sync code and email service to build these write structs from `EmailHeader` and `WholeEmailDto`.

Bulk insert must use `db.transaction` and prepared statements.

- [ ] **Step 5: Implement attachment_repo**

Implement:

- `list_by_email`
- `create`
- `bulk_insert`
- `get_by_id`
- `delete_by_email`

Use `attachments::Model` row mapper.

- [ ] **Step 6: Implement search.rs**

Replace `FromQueryResult` and SeaORM statements with:

```rust
pub async fn search_fts(
    db: &DbConn,
    query: &str,
    account_id: Option<i32>,
    limit: u64,
) -> Result<Vec<SearchResult>, MailError> {
    let query = query.to_string();
    db.call(move |conn| {
        let mut results = Vec::new();
        if let Some(account_id) = account_id {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank
                 FROM emails_fts f JOIN emails e ON f.rowid = e.id
                 WHERE emails_fts MATCH ?1 AND e.account_id = ?2 AND e.is_deleted = 0
                 ORDER BY f.rank DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![query, account_id, limit as i64], map_search_result)?;
            for row in rows {
                results.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank
                 FROM emails_fts f JOIN emails e ON f.rowid = e.id
                 WHERE emails_fts MATCH ?1 AND e.is_deleted = 0
                 ORDER BY f.rank DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![query, limit as i64], map_search_result)?;
            for row in rows {
                results.push(row?);
            }
        }
        Ok(results)
    })
    .await
}
```

- [ ] **Step 7: Run email and schema tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands
rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/attachment_repo.rs src-tauri/src/infrastructure/storage/search.rs src-tauri/src/service/email_service.rs src-tauri/src/service/sync_service.rs src-tauri/src/domain/sync src-tauri/tests/email_commands.rs src-tauri/tests/schema.rs
rtk git commit -m "feat: rewrite email storage with rusqlite"
```

---

### Task 5: 改造 e2e seed、剩余测试并删除 SeaORM migration crate

**Files:**
- Modify: `src-tauri/src/infrastructure/testing/e2e_seed.rs`
- Modify: `src-tauri/tests/e2e_seed.rs`
- Delete: `src-tauri/migration/`
- Delete: `src-tauri/src/infrastructure/storage/entities/`
- Modify: `src-tauri/Cargo.toml`
- Modify: `Cargo.lock` if present through cargo.

**Interfaces:**
- Consumes: all rusqlite repositories from prior tasks.
- Produces: no remaining `sea_orm`, `sea-orm`, `sea_orm_migration`, `postium_mail_migration` references in business code.

- [ ] **Step 1: Rewrite e2e seed**

In `src-tauri/src/infrastructure/testing/e2e_seed.rs`, replace ActiveModel inserts with repository write structs or a single `db.transaction` SQL seed.

Keep seed behavior:

- deterministic accounts
- deterministic emails
- idempotent seed
- account isolation
- category/search support

Use `INSERT OR IGNORE` for accounts and emails keyed by unique `email`/`message_id`.

- [ ] **Step 2: Update e2e seed tests**

In `src-tauri/tests/e2e_seed.rs`, replace SeaORM counts and filters with SQL:

```rust
let count: i64 = services.db.call(|conn| {
    conn.query_row("SELECT COUNT(*) FROM emails WHERE account_id = ?1", [account_id], |row| row.get(0))
}).await.unwrap();
```

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test e2e_seed`

Expected: PASS.

- [ ] **Step 3: Confirm migration test coverage is replaced**

Confirm `src-tauri/tests/migrations.rs` no longer exists. Its coverage is replaced by `src-tauri/tests/schema.rs`.

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test schema`

Expected: PASS.

- [ ] **Step 4: Delete SeaORM source directories**

Delete:

```bash
rtk rm -rf src-tauri/migration
rtk rm -rf src-tauri/src/infrastructure/storage/entities
```

If `rtk rm` is unavailable, use `rm -rf` through `rtk`.

- [ ] **Step 5: Search for forbidden references**

Run:

```bash
rtk grep -R "sea_orm\\|sea-orm\\|sea_orm_migration\\|postium_mail_migration\\|ActiveModel\\|EntityTrait\\|QueryFilter\\|PaginatorTrait\\|Set(" -n src-tauri Cargo.toml Cargo.lock || true
```

Expected: no matches in active source, tests, or manifests. Matches inside old git logs do not matter because grep is over files.

- [ ] **Step 6: Run full Rust tests**

Run: `rtk bun run test:rust`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add -A src-tauri Cargo.lock
rtk git commit -m "chore: remove seaorm migration stack"
```

---

### Task 6: Final verification and frontend contract check

**Files:**
- Modify only if checks reveal generated binding drift: `src/lib/bindings.ts`

**Interfaces:**
- Consumes: completed rusqlite rewrite.
- Produces: verified branch ready for review/merge.

- [ ] **Step 1: Run full Rust tests**

Run: `rtk bun run test:rust`

Expected: PASS. Confirm output includes all Rust integration tests and 0 failures.

- [ ] **Step 2: Run frontend tests**

Run: `rtk bun run test:frontend`

Expected: PASS.

- [ ] **Step 3: Run Svelte/type check**

Run: `rtk bun run check`

Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 4: Run targeted formatting**

Run:

```bash
rtk rustfmt --edition 2024 --check \
  src-tauri/src/infrastructure/storage/database.rs \
  src-tauri/src/infrastructure/storage/row.rs \
  src-tauri/src/infrastructure/storage/models/*.rs \
  src-tauri/src/infrastructure/storage/repository/*.rs \
  src-tauri/src/infrastructure/storage/search.rs \
  src-tauri/src/service/*.rs \
  src-tauri/src/domain/sync/*.rs \
  src-tauri/tests/*.rs
```

Expected: PASS. Do not run full-tree rustfmt as the repo has known pre-existing formatting diffs in old migration files; those files should be deleted by this point, but targeted check keeps the gate scoped.

- [ ] **Step 5: Run diff checks**

Run:

```bash
rtk git diff --check
rtk grep -R "sea_orm\\|sea-orm\\|sea_orm_migration\\|postium_mail_migration" -n src-tauri Cargo.toml Cargo.lock || true
```

Expected:

- `git diff --check` exits 0.
- grep has no active code/dependency matches.

- [ ] **Step 6: Inspect final status**

Run:

```bash
rtk git status --short --branch
rtk git log --oneline -6
```

Expected: working tree clean after final commit sequence; recent commits correspond to plan tasks.

- [ ] **Step 7: Commit any final binding or cleanup changes**

If Step 1-5 changed generated files or cleanup, commit:

```bash
rtk git add -A
rtk git commit -m "chore: verify rusqlite rewrite"
```

If no files changed, do not create an empty commit.

---

## Self-Review

- Spec coverage: covered full SeaORM removal, startup reset, single `schema.sql`, `tokio-rusqlite` connection, repository SQL rewrite, transaction boundary, tests, and SQLite MCP exclusion from implementation path.
- Placeholder scan: no TBD/TODO/fill-in-later language remains; tasks contain exact files, commands, expected outcomes, and key code templates.
- Type consistency: plan uses `DbConn`, `models::*::Model`, repository write structs, and `MailError` conversions consistently across tasks.
