use postium_mail_lib::infrastructure::storage::{
    database::{DbConn, init_database},
    repository::sync_repo,
};
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
async fn init_database_preserves_existing_database_data() {
    let temp = tempdir().unwrap();

    let first_db = init_database(temp.path()).await.unwrap();

    first_db
        .call(|conn| {
            conn.execute(
                "INSERT INTO accounts (
                    name, email, provider, auth_type, account_type, created_at, updated_at
                ) VALUES ('Persisted', 'persisted@example.com', 'custom', 'password', 'personal', 1, 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    drop(first_db);

    let reopened_db = init_database(temp.path()).await.unwrap();

    let account_count = reopened_db
        .call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM accounts WHERE email = 'persisted@example.com'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(account_count, 1);
}

#[tokio::test]
async fn init_database_creates_database_when_missing() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("postium.sqlite");

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
}

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
    assert!(columns.contains(&"history_before_uid".to_string()));
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

#[tokio::test]
async fn schema_allows_same_message_id_in_different_folders() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();

    let count = db
        .call(|conn| {
            conn.execute(
                "INSERT INTO accounts (
                    id, name, email, provider, auth_type, account_type, created_at, updated_at
                ) VALUES (1, 'Message Id Test', 'message-id@example.com', 'custom', 'password', 'personal', 1, 1)",
                [],
            )?;
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, message_id, subject, sender_email, recipient_emails,
                    sent_at, received_at, created_at, updated_at
                ) VALUES
                    (1, 'INBOX', 10, '<same@example.com>', 'Inbox copy', 'a@example.com', 'b@example.com', 10, 10, 1, 1),
                    (1, 'Archive', 20, '<same@example.com>', 'Archive copy', 'a@example.com', 'b@example.com', 10, 10, 1, 1)",
                [],
            )?;
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE message_id = '<same@example.com>'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(count, 2);
}

#[tokio::test]
async fn init_database_removes_legacy_message_id_unique_constraint() {
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
            INSERT INTO accounts (id, name, email, provider, auth_type, account_type, created_at, updated_at)
            VALUES (1, 'Legacy Message', 'legacy-message@example.com', 'custom', 'password', 'personal', 1, 1);
            INSERT INTO emails (
                id, account_id, folder, uid, message_id, subject, sender_email, recipient_emails,
                sent_at, received_at, created_at, updated_at
            ) VALUES (1, 1, 'INBOX', 10, '<same@example.com>', 'Inbox copy', 'a@example.com', 'b@example.com', 10, 10, 1, 1);
            ",
        )
        .unwrap();
    }

    let db = init_database(temp.path()).await.unwrap();

    let count = db
        .call(|conn| {
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, message_id, subject, sender_email, recipient_emails,
                    sent_at, received_at, created_at, updated_at
                ) VALUES (1, 'Archive', 20, '<same@example.com>', 'Archive copy', 'a@example.com', 'b@example.com', 10, 10, 2, 2)",
                [],
            )?;
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE message_id = '<same@example.com>'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(count, 2);
}

#[tokio::test]
async fn sync_repo_history_state_upserts_and_maps_history_fields() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();

    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Sync Test', 'sync-test@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    sync_repo::update_history_state(&db, 1, "INBOX", Some(1700000000), Some(500), false)
        .await
        .unwrap();

    let initial_state = sync_repo::get_sync_state(&db, 1, "INBOX")
        .await
        .unwrap()
        .expect("sync_state row should be created");

    assert_eq!(initial_state.history_synced_since, Some(1700000000));
    assert_eq!(initial_state.history_before_uid, Some(500));
    assert_eq!(initial_state.history_exhausted, Some(false));

    let row_count_after_first_write = db
        .call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sync_state WHERE account_id = 1 AND folder = 'INBOX'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(row_count_after_first_write, 1);

    sync_repo::update_history_state(&db, 1, "INBOX", Some(1600000000), Some(250), true)
        .await
        .unwrap();

    let updated_state = sync_repo::get_sync_state(&db, 1, "INBOX")
        .await
        .unwrap()
        .expect("sync_state row should be updated in place");

    assert_eq!(updated_state.id, initial_state.id);
    assert_eq!(updated_state.history_synced_since, Some(1600000000));
    assert_eq!(updated_state.history_before_uid, Some(250));
    assert_eq!(updated_state.history_exhausted, Some(true));

    let row_count_after_second_write = db
        .call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sync_state WHERE account_id = 1 AND folder = 'INBOX'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(row_count_after_second_write, 1);
}
