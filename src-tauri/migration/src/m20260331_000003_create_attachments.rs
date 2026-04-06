use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Attachments::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Attachments::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Attachments::EmailId).integer().not_null())
                    .col(ColumnDef::new(Attachments::Filename).text().not_null())
                    .col(ColumnDef::new(Attachments::ContentType).text())
                    .col(ColumnDef::new(Attachments::Size).integer().not_null())
                    .col(ColumnDef::new(Attachments::Path).text())
                    .col(ColumnDef::new(Attachments::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_attachments_email")
                            .from(Attachments::Table, Attachments::EmailId)
                            .to(Emails::Table, Emails::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attachments_email")
                    .table(Attachments::Table)
                    .col(Attachments::EmailId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Attachments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Emails {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Attachments {
    Table,
    Id,
    EmailId,
    Filename,
    ContentType,
    Size,
    Path,
    CreatedAt,
}
