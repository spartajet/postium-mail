use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Emails::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Emails::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Emails::AccountId).integer().not_null())
                    .col(ColumnDef::new(Emails::Folder).text().not_null())
                    .col(ColumnDef::new(Emails::Uid).integer())
                    .col(ColumnDef::new(Emails::MessageId).text().unique_key())
                    .col(ColumnDef::new(Emails::Subject).text())
                    .col(ColumnDef::new(Emails::SenderName).text())
                    .col(ColumnDef::new(Emails::SenderEmail).text().not_null())
                    .col(ColumnDef::new(Emails::RecipientEmails).text().not_null())
                    .col(ColumnDef::new(Emails::CcEmails).text())
                    .col(ColumnDef::new(Emails::BccEmails).text())
                    .col(ColumnDef::new(Emails::Preview).text())
                    .col(ColumnDef::new(Emails::BodyText).text())
                    .col(ColumnDef::new(Emails::BodyHtml).text())
                    .col(ColumnDef::new(Emails::IsRead).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsStarred).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsDraft).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsAnswered).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsDeleted).boolean().default(false))
                    .col(ColumnDef::new(Emails::SentAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::ReceivedAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::UpdatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_emails_account")
                            .from(Emails::Table, Emails::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_emails_account ON emails(account_id);",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder);",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_emails_sent_at ON emails(sent_at DESC);",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read);",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Emails::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Emails {
    Table,
    Id,
    AccountId,
    Folder,
    Uid,
    MessageId,
    Subject,
    SenderName,
    SenderEmail,
    RecipientEmails,
    CcEmails,
    BccEmails,
    Preview,
    BodyText,
    BodyHtml,
    IsRead,
    IsStarred,
    IsDraft,
    IsAnswered,
    IsDeleted,
    SentAt,
    ReceivedAt,
    CreatedAt,
    UpdatedAt,
}
