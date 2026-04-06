use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncState::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SyncState::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(SyncState::AccountId).integer().not_null())
                    .col(ColumnDef::new(SyncState::Folder).text().not_null())
                    .col(ColumnDef::new(SyncState::FolderNickName).text())
                    .col(ColumnDef::new(SyncState::Uidvalidity).big_integer())
                    .col(ColumnDef::new(SyncState::Uidnext).big_integer())
                    .col(ColumnDef::new(SyncState::SyncedAt).big_integer())
                    .col(ColumnDef::new(SyncState::LastSyncUid).big_integer())
                    .col(ColumnDef::new(SyncState::CreatedAt).big_integer())
                    .col(ColumnDef::new(SyncState::UpdatedAt).big_integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sync_state_account")
                            .from(SyncState::Table, SyncState::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .col(SyncState::AccountId)
                            .col(SyncState::Folder),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SyncState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum SyncState {
    Table,
    Id,
    AccountId,
    Folder,
    FolderNickName,
    Uidvalidity,
    Uidnext,
    SyncedAt,
    LastSyncUid,
    CreatedAt,
    UpdatedAt,
}
