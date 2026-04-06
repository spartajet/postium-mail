use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncErrors::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SyncErrors::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(SyncErrors::AccountId).integer().not_null())
                    .col(ColumnDef::new(SyncErrors::Folder).text())
                    .col(ColumnDef::new(SyncErrors::ErrorType).text().not_null())
                    .col(ColumnDef::new(SyncErrors::ErrorMessage).text().not_null())
                    .col(ColumnDef::new(SyncErrors::Uid).integer())
                    .col(ColumnDef::new(SyncErrors::StackTrace).text())
                    .col(ColumnDef::new(SyncErrors::Resolved).boolean().default(false))
                    .col(ColumnDef::new(SyncErrors::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sync_errors_account")
                            .from(SyncErrors::Table, SyncErrors::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_sync_errors_account ON sync_errors(account_id);",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_sync_errors_resolved ON sync_errors(resolved);",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SyncErrors::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum SyncErrors {
    Table,
    Id,
    AccountId,
    Folder,
    ErrorType,
    ErrorMessage,
    Uid,
    StackTrace,
    Resolved,
    CreatedAt,
}
