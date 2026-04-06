use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Emails {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Labels {
    Table,
    Id,
    AccountId,
    Name,
    Color,
    CreatedAt,
}

#[derive(DeriveIden)]
enum EmailLabels {
    Table,
    Id,
    EmailId,
    LabelId,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Labels::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Labels::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Labels::AccountId).integer().not_null())
                    .col(ColumnDef::new(Labels::Name).text().not_null())
                    .col(ColumnDef::new(Labels::Color).text().not_null())
                    .col(ColumnDef::new(Labels::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_labels_account")
                            .from(Labels::Table, Labels::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EmailLabels::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(EmailLabels::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(EmailLabels::EmailId).integer().not_null())
                    .col(ColumnDef::new(EmailLabels::LabelId).integer().not_null())
                    .col(ColumnDef::new(EmailLabels::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_email_labels_email")
                            .from(EmailLabels::Table, EmailLabels::EmailId)
                            .to(Emails::Table, Emails::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_email_labels_label")
                            .from(EmailLabels::Table, EmailLabels::LabelId)
                            .to(Labels::Table, Labels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .if_not_exists()
                            .name("idx_email_labels_unique")
                            .table(EmailLabels::Table)
                            .col(EmailLabels::EmailId)
                            .col(EmailLabels::LabelId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EmailLabels::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Labels::Table).to_owned())
            .await
    }
}
