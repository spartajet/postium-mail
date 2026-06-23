use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ai;")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ad;")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_au;")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS emails_fts;")
            .await?;

        db.execute_unprepared(
            "CREATE VIRTUAL TABLE emails_fts USING fts5(
                subject, sender_email, preview,
                content=emails, content_rowid=id
            );",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER emails_fts_ai AFTER INSERT ON emails BEGIN
                INSERT INTO emails_fts(rowid, subject, sender_email, preview)
                VALUES (new.id, new.subject, new.sender_email, new.preview);
            END;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER emails_fts_ad AFTER DELETE ON emails BEGIN
                INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
                VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
            END;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER emails_fts_au AFTER UPDATE ON emails BEGIN
                INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
                VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
                INSERT INTO emails_fts(rowid, subject, sender_email, preview)
                VALUES (new.id, new.subject, new.sender_email, new.preview);
            END;",
        )
        .await?;

        db.execute_unprepared("INSERT INTO emails_fts(emails_fts) VALUES ('rebuild');")
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
