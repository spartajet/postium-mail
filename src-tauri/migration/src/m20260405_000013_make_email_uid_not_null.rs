use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // SQLite 不支持 ALTER COLUMN 添加 NOT NULL 约束，
        // 采用四步重建策略：rename → create → copy → drop

        // Step 1: 重命名旧表
        manager
            .execute(
                Table::rename()
                    .table(Alias::new("emails"), Alias::new("_emails_old"))
                    .to_owned(),
            )
            .await?;

        // Step 2: 创建新表（uid 改为 NOT NULL）
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("emails"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("account_id"))
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("folder")).text().not_null())
                    // uid: nullable → NOT NULL
                    .col(ColumnDef::new(Alias::new("uid")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("message_id")).text().unique_key())
                    .col(ColumnDef::new(Alias::new("subject")).text())
                    .col(ColumnDef::new(Alias::new("sender_name")).text())
                    .col(ColumnDef::new(Alias::new("sender_email")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("recipient_emails"))
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("cc_emails")).text())
                    .col(ColumnDef::new(Alias::new("bcc_emails")).text())
                    .col(ColumnDef::new(Alias::new("preview")).text())
                    .col(ColumnDef::new(Alias::new("body_text")).text())
                    .col(ColumnDef::new(Alias::new("body_html")).text())
                    .col(
                        ColumnDef::new(Alias::new("is_read"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_starred"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_draft"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_answered"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_deleted"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("sent_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("received_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_emails_account")
                            .from(Alias::new("emails"), Alias::new("account_id"))
                            .to(Alias::new("accounts"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 3: 迁移数据（uid 为 NULL 时用 0 兜底）
        db.execute_unprepared(
            "INSERT INTO emails (id, account_id, folder, uid, message_id, subject, sender_name, sender_email, recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html, is_read, is_starred, is_draft, is_answered, is_deleted, sent_at, received_at, created_at, updated_at)
             SELECT id, account_id, folder, COALESCE(uid, 0), message_id, subject, sender_name, sender_email, recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html, is_read, is_starred, is_draft, is_answered, is_deleted, sent_at, received_at, created_at, updated_at
             FROM _emails_old",
        )
        .await?;

        // Step 4: 删除旧表
        manager
            .execute(Table::drop().table(Alias::new("_emails_old")).to_owned())
            .await?;

        // 重建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_account")
                    .table(Alias::new("emails"))
                    .col(Alias::new("account_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_folder")
                    .table(Alias::new("emails"))
                    .col(Alias::new("folder"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_sent_at")
                    .table(Alias::new("emails"))
                    .col(Alias::new("sent_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_is_read")
                    .table(Alias::new("emails"))
                    .col(Alias::new("is_read"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 回滚：uid 恢复为可空

        // Step 1: 重命名当前表
        manager
            .execute(
                Table::rename()
                    .table(Alias::new("emails"), Alias::new("_emails_new"))
                    .to_owned(),
            )
            .await?;

        // Step 2: 创建旧表结构（uid 可空）
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("emails"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("account_id"))
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("folder")).text().not_null())
                    .col(ColumnDef::new(Alias::new("uid")).integer())
                    .col(ColumnDef::new(Alias::new("message_id")).text().unique_key())
                    .col(ColumnDef::new(Alias::new("subject")).text())
                    .col(ColumnDef::new(Alias::new("sender_name")).text())
                    .col(ColumnDef::new(Alias::new("sender_email")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("recipient_emails"))
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("cc_emails")).text())
                    .col(ColumnDef::new(Alias::new("bcc_emails")).text())
                    .col(ColumnDef::new(Alias::new("preview")).text())
                    .col(ColumnDef::new(Alias::new("body_text")).text())
                    .col(ColumnDef::new(Alias::new("body_html")).text())
                    .col(
                        ColumnDef::new(Alias::new("is_read"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_starred"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_draft"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_answered"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_deleted"))
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("sent_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("received_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_emails_account")
                            .from(Alias::new("emails"), Alias::new("account_id"))
                            .to(Alias::new("accounts"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 3: 迁移数据
        db.execute_unprepared(
            "INSERT INTO emails (id, account_id, folder, uid, message_id, subject, sender_name, sender_email, recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html, is_read, is_starred, is_draft, is_answered, is_deleted, sent_at, received_at, created_at, updated_at)
             SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email, recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html, is_read, is_starred, is_draft, is_answered, is_deleted, sent_at, received_at, created_at, updated_at
             FROM _emails_new",
        )
        .await?;

        // Step 4: 删除临时表
        manager
            .execute(Table::drop().table(Alias::new("_emails_new")).to_owned())
            .await?;

        // 重建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_account")
                    .table(Alias::new("emails"))
                    .col(Alias::new("account_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_folder")
                    .table(Alias::new("emails"))
                    .col(Alias::new("folder"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_sent_at")
                    .table(Alias::new("emails"))
                    .col(Alias::new("sent_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_emails_is_read")
                    .table(Alias::new("emails"))
                    .col(Alias::new("is_read"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
