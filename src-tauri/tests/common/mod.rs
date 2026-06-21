use std::sync::Arc;

use postium_mail_lib::domain::auth::AuthManager;
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::service::email_service::EmailService;
use postium_mail_lib::service::{AccountService, LabelService};
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

/// Create an in-memory SQLite database with all migrations applied.
pub async fn create_test_db() -> DbConn {
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    postium_mail_migration::Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    // Disable foreign-key enforcement so that CASCADE deletes triggered by
    // FTS5 triggers do not reference the stale _emails_old shadow table left
    // behind by migration 13 (which rebuilt the emails table via rename).
    use sea_orm::ConnectionTrait;
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .ok();

    db
}

/// Holds ready-to-use service instances backed by an in-memory test database.
#[allow(dead_code)]
pub struct TestServices {
    pub db: DbConn,
    pub auth: Arc<AuthManager>,
    pub account_service: AccountService,
    pub email_service: EmailService,
    pub label_service: LabelService,
}

#[allow(dead_code)]
impl TestServices {
    /// Create a full set of services backed by a fresh in-memory database.
    pub async fn new() -> Self {
        let db = create_test_db().await;
        let auth = Arc::new(AuthManager::default());

        Self {
            account_service: AccountService::new(db.clone(), auth.clone()),
            email_service: EmailService::new(auth.clone(), db.clone()),
            label_service: LabelService::new(db.clone()),
            db,
            auth,
        }
    }
}
