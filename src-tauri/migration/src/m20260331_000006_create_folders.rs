use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Folders::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Folders::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Folders::AccountId).integer().not_null())
                    .col(ColumnDef::new(Folders::Name).text().not_null())
                    .col(ColumnDef::new(Folders::Delimiter).text())
                    .col(ColumnDef::new(Folders::ParentId).integer())
                    .col(ColumnDef::new(Folders::StandardFolder).text())
                    .col(ColumnDef::new(Folders::UnreadCount).integer().default(0))
                    .col(ColumnDef::new(Folders::TotalCount).integer().default(0))
                    .col(ColumnDef::new(Folders::SortOrder).integer().default(0))
                    .col(ColumnDef::new(Folders::CreatedAt).big_integer())
                    .col(ColumnDef::new(Folders::UpdatedAt).big_integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_folders_account")
                            .from(Folders::Table, Folders::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Folders::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    Id,
    AccountId,
    Name,
    Delimiter,
    ParentId,
    StandardFolder,
    UnreadCount,
    TotalCount,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}
