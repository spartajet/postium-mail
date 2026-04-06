use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
                subject, sender_email, preview,
                content=emails, content_rowid=id
            );",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_ai AFTER INSERT ON emails BEGIN
                INSERT INTO emails_fts(rowid, subject, sender_email, preview)
                VALUES (new.id, new.subject, new.sender_email, new.preview);
            END;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_ad AFTER DELETE ON emails BEGIN
                DELETE FROM emails_fts WHERE rowid = old.id;
            END;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_au AFTER UPDATE ON emails BEGIN
                UPDATE emails_fts SET subject = new.subject,
                    sender_email = new.sender_email, preview = new.preview
                WHERE rowid = new.id;
            END;",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_au;")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ad;")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ai;")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS emails_fts;")
            .await?;
        Ok(())
    }
}
