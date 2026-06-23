mod common;

use common::create_test_db;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DbErr, QueryResult, Statement, TryGetable,
};
use sea_orm_migration::MigratorTrait;

async fn fts_match_count(db: &sea_orm::DatabaseConnection, query: &str) -> Result<i64, DbErr> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "SELECT COUNT(*) FROM emails_fts WHERE emails_fts MATCH ?",
        [query.into()],
    );
    let row = db
        .query_one_raw(stmt)
        .await?
        .expect("FTS count query should return one row");

    Ok(i64::try_get_by_index(&row, 0)?)
}

async fn fts_trigger_sql(
    db: &sea_orm::DatabaseConnection,
    trigger_name: &str,
) -> Result<String, DbErr> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?",
        [trigger_name.into()],
    );
    let row: QueryResult = db
        .query_one_raw(stmt)
        .await?
        .unwrap_or_else(|| panic!("trigger {trigger_name} should exist"));

    Ok(String::try_get_by_index(&row, 0)?)
}

async fn insert_migration_test_account_and_email(db: &sea_orm::DatabaseConnection) {
    db.execute_unprepared(
        "INSERT INTO accounts (
            id, name, email, provider, auth_type, account_type, created_at, updated_at
        ) VALUES (
            1, 'Migration Test', 'migration.e2e@postium.test', 'custom', 'password',
            'personal', 1, 1
        );",
    )
    .await
    .unwrap();

    db.execute_unprepared(
        "INSERT INTO emails (
            id, account_id, folder, uid, message_id, subject, sender_name,
            sender_email, recipient_emails, preview, body_text, body_html,
            is_read, is_starred, is_draft, is_answered, is_deleted,
            sent_at, received_at, created_at, updated_at
        ) VALUES (
            1, 1, 'INBOX', 1, 'migration-message-1', 'Migration Search Alpha',
            'Migration Sender', 'sender@postium.test', 'user@postium.test',
            'Alpha preview', 'Alpha body', '<p>Alpha body</p>',
            0, 0, 0, 0, 0, 1, 1, 1, 1
        );",
    )
    .await
    .unwrap();
}

async fn create_email_fts_with_stale_content_table(db: &sea_orm::DatabaseConnection) {
    db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ai;")
        .await
        .unwrap();
    db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ad;")
        .await
        .unwrap();
    db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_au;")
        .await
        .unwrap();
    db.execute_unprepared("DROP TABLE IF EXISTS emails_fts;")
        .await
        .unwrap();
    db.execute_unprepared(
        "CREATE TABLE _emails_old (
            id INTEGER PRIMARY KEY,
            subject TEXT,
            sender_email TEXT,
            preview TEXT
        );",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE VIRTUAL TABLE emails_fts USING fts5(
            subject, sender_email, preview,
            content=_emails_old, content_rowid=id
        );",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE TRIGGER emails_fts_ad AFTER DELETE ON emails BEGIN
            INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
            VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
        END;",
    )
    .await
    .unwrap();
    db.execute_unprepared("DROP TABLE _emails_old;")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_full_migration_creates_working_email_fts_triggers() {
    let db = create_test_db().await;

    let delete_trigger = fts_trigger_sql(&db, "emails_fts_ad").await.unwrap();
    let update_trigger = fts_trigger_sql(&db, "emails_fts_au").await.unwrap();

    assert!(delete_trigger.contains("'delete'"));
    assert!(update_trigger.contains("'delete'"));

    insert_migration_test_account_and_email(&db).await;

    assert_eq!(fts_match_count(&db, "Alpha").await.unwrap(), 1);

    db.execute_unprepared(
        "UPDATE emails
         SET subject = 'Migration Search Beta', preview = 'Beta preview'
         WHERE id = 1;",
    )
    .await
    .unwrap();

    assert_eq!(fts_match_count(&db, "Alpha").await.unwrap(), 0);
    assert_eq!(fts_match_count(&db, "Beta").await.unwrap(), 1);

    db.execute_unprepared("DELETE FROM emails WHERE id = 1;")
        .await
        .unwrap();

    assert_eq!(fts_match_count(&db, "Beta").await.unwrap(), 0);
}

#[tokio::test]
async fn test_pending_migration_repairs_email_fts_stale_content_table() {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    postium_mail_migration::Migrator::up(&db, Some(14))
        .await
        .unwrap();
    insert_migration_test_account_and_email(&db).await;
    create_email_fts_with_stale_content_table(&db).await;

    postium_mail_migration::Migrator::up(&db, None)
        .await
        .unwrap();

    db.execute_unprepared("DELETE FROM emails WHERE id = 1;")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_migration_from_thirteen_to_latest_repairs_email_fts() {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    postium_mail_migration::Migrator::up(&db, Some(13))
        .await
        .unwrap();

    postium_mail_migration::Migrator::up(&db, None)
        .await
        .unwrap();
    insert_migration_test_account_and_email(&db).await;

    db.execute_unprepared("DELETE FROM emails WHERE id = 1;")
        .await
        .unwrap();
}
