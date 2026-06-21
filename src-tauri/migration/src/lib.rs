pub use sea_orm_migration::prelude::*;

mod m20260331_000001_create_accounts;
mod m20260331_000002_create_emails;
mod m20260331_000003_create_attachments;
mod m20260331_000004_create_sync_state;
mod m20260331_000005_create_sync_errors;
mod m20260331_000006_create_folders;
mod m20260331_000007_create_fts;
mod m20260402_000008_create_labels;
mod m20260402_000009_add_ssl_mode;
mod m20260403_000010_drop_oauth_provider;
mod m20260404_000011_drop_folders;
mod m20260405_000012_refactor_attachments;
mod m20260405_000013_make_email_uid_not_null;
mod m20260621_000014_rebuild_email_fts;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260331_000001_create_accounts::Migration),
            Box::new(m20260331_000002_create_emails::Migration),
            Box::new(m20260331_000003_create_attachments::Migration),
            Box::new(m20260331_000004_create_sync_state::Migration),
            Box::new(m20260331_000005_create_sync_errors::Migration),
            Box::new(m20260331_000006_create_folders::Migration),
            Box::new(m20260331_000007_create_fts::Migration),
            Box::new(m20260402_000008_create_labels::Migration),
            Box::new(m20260402_000009_add_ssl_mode::Migration),
            Box::new(m20260403_000010_drop_oauth_provider::Migration),
            Box::new(m20260404_000011_drop_folders::Migration),
            Box::new(m20260405_000012_refactor_attachments::Migration),
            Box::new(m20260405_000013_make_email_uid_not_null::Migration),
            Box::new(m20260621_000014_rebuild_email_fts::Migration),
        ]
    }
}
