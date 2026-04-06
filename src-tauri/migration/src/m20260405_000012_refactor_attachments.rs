use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 ALTER COLUMN 修改类型或可空性，
        // 因此采用经典的四步重建策略：
        //   1. 将旧表重命名为临时表
        //   2. 创建新表
        //   3. 从临时表迁移数据
        //   4. 删除临时表

        let db = manager.get_connection();

        // Step 1: 重命名旧表
        db.execute_unprepared("ALTER TABLE attachments RENAME TO _attachments_old")
            .await?;

        // Step 2: 创建新表
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("attachments"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("email_id")).integer().not_null())
                    // filename: NOT NULL → 可空（某些附件没有文件名）
                    .col(ColumnDef::new(Alias::new("filename")).text().null())
                    .col(ColumnDef::new(Alias::new("content_type")).text().null())
                    // size: integer → big_integer（支持 >2GB 附件）
                    .col(ColumnDef::new(Alias::new("size")).big_integer().not_null())
                    // 新增：MIME section 路径，用于按需下载附件
                    .col(ColumnDef::new(Alias::new("section_path")).text().not_null())
                    // 新增：Content-Disposition（attachment / inline）
                    .col(ColumnDef::new(Alias::new("disposition")).text().null())
                    // 新增：Content-ID，用于 HTML 邮件内嵌图片 cid: 引用
                    .col(ColumnDef::new(Alias::new("content_id")).text().null())
                    .col(ColumnDef::new(Alias::new("path")).text().null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_attachments_email")
                            .from(Alias::new("attachments"), Alias::new("email_id"))
                            .to(Alias::new("emails"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 3: 迁移数据
        // 新增列使用默认值：section_path = "0"，disposition / content_id = NULL
        db.execute_unprepared(
            "INSERT INTO attachments (id, email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at)
             SELECT id, email_id, filename, content_type, size, '0', NULL, NULL, path, created_at
             FROM _attachments_old",
        )
        .await?;

        // Step 4: 删除旧表
        db.execute_unprepared("DROP TABLE _attachments_old").await?;

        // 重建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attachments_email")
                    .table(Alias::new("attachments"))
                    .col(Alias::new("email_id"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚：同样的四步重建策略，还原为旧表结构

        let db = manager.get_connection();

        // Step 1: 重命名当前表
        db.execute_unprepared("ALTER TABLE attachments RENAME TO _attachments_new")
            .await?;

        // Step 2: 创建旧表结构
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("attachments"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("email_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("filename")).text().not_null())
                    .col(ColumnDef::new(Alias::new("content_type")).text().null())
                    .col(ColumnDef::new(Alias::new("size")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("path")).text().null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_attachments_email")
                            .from(Alias::new("attachments"), Alias::new("email_id"))
                            .to(Alias::new("emails"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 3: 迁移数据（丢弃新增列，filename 为空时用默认值填充以满足 NOT NULL）
        db.execute_unprepared(
            "INSERT INTO attachments (id, email_id, filename, content_type, size, path, created_at)
             SELECT id, email_id,
                    COALESCE(filename, 'unnamed_attachment'),
                    content_type, size, path, created_at
             FROM _attachments_new",
        )
        .await?;

        // Step 4: 删除临时表
        db.execute_unprepared("DROP TABLE _attachments_new").await?;

        // 重建索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_attachments_email")
                    .table(Alias::new("attachments"))
                    .col(Alias::new("email_id"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
