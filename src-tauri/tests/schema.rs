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
