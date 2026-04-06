use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Accounts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Accounts::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Accounts::Name).text().not_null())
                    .col(ColumnDef::new(Accounts::Email).text().not_null().unique_key())
                    .col(ColumnDef::new(Accounts::DisplayName).text())
                    .col(ColumnDef::new(Accounts::Provider).text().not_null())
                    .col(ColumnDef::new(Accounts::ImapHost).text())
                    .col(ColumnDef::new(Accounts::ImapPort).integer())
                    .col(ColumnDef::new(Accounts::ImapSsl).boolean().default(true))
                    .col(ColumnDef::new(Accounts::SmtpHost).text())
                    .col(ColumnDef::new(Accounts::SmtpPort).integer())
                    .col(ColumnDef::new(Accounts::SmtpSsl).boolean().default(true))
                    .col(ColumnDef::new(Accounts::Color).text())
                    .col(ColumnDef::new(Accounts::SyncEnabled).boolean().default(true))
                    .col(ColumnDef::new(Accounts::LastSyncAt).big_integer())
                    .col(ColumnDef::new(Accounts::AuthType).text().default("password"))
                    .col(ColumnDef::new(Accounts::OauthProvider).text())
                    .col(ColumnDef::new(Accounts::OauthExpiresAt).big_integer())
                    .col(ColumnDef::new(Accounts::AccountType).text().not_null().default("personal"))
                    .col(ColumnDef::new(Accounts::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Accounts::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_accounts_email")
                    .table(Accounts::Table)
                    .col(Accounts::Email)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Accounts::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
    Name,
    Email,
    DisplayName,
    Provider,
    ImapHost,
    ImapPort,
    ImapSsl,
    SmtpHost,
    SmtpPort,
    SmtpSsl,
    Color,
    SyncEnabled,
    LastSyncAt,
    AuthType,
    OauthProvider,
    OauthExpiresAt,
    AccountType,
    CreatedAt,
    UpdatedAt,
}
