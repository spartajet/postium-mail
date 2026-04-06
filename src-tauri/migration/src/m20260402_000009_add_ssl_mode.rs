use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("accounts"))
                    .add_column(
                        ColumnDef::new(Alias::new("imap_ssl_mode"))
                            .text()
                            .default("Tls"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("accounts"))
                    .add_column(
                        ColumnDef::new(Alias::new("smtp_ssl_mode"))
                            .text()
                            .default("Tls"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 DROP COLUMN（3.35.0 之前），此处空操作
        let _ = manager;
        Ok(())
    }
}
