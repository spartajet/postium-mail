use std::sync::Arc;

use postium_mail_lib::domain::auth::AuthManager;
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::infrastructure::storage::entities::emails;
use postium_mail_lib::service::email_service::EmailService;
use postium_mail_lib::service::{AccountService, LabelService};
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set};
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
        let auth = Arc::new(AuthManager::in_memory());

        Self {
            account_service: AccountService::new(db.clone(), auth.clone()),
            email_service: EmailService::new(auth.clone(), db.clone()),
            label_service: LabelService::new(db.clone()),
            db,
            auth,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TestEmail {
    pub folder: String,
    pub uid: u32,
    pub subject: Option<String>,
    pub sender_email: String,
    pub preview: Option<String>,
    pub body_text: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_deleted: bool,
    pub sent_at: i64,
}

#[allow(dead_code)]
impl TestEmail {
    pub fn new(uid: u32, subject: impl Into<String>) -> Self {
        let uid_i64 = i64::from(uid);

        Self {
            folder: "INBOX".to_string(),
            uid,
            subject: Some(subject.into()),
            sender_email: format!("sender-{}@example.com", uid),
            preview: Some(format!("Preview {}", uid)),
            body_text: Some(format!("Body {}", uid)),
            is_read: false,
            is_starred: false,
            is_deleted: false,
            sent_at: 1_800_000_000 - uid_i64,
        }
    }

    pub fn folder(mut self, folder: impl Into<String>) -> Self {
        self.folder = folder.into();
        self
    }

    pub fn sender_email(mut self, sender_email: impl Into<String>) -> Self {
        self.sender_email = sender_email.into();
        self
    }

    pub fn preview(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }

    pub fn body_text(mut self, body_text: impl Into<String>) -> Self {
        self.body_text = Some(body_text.into());
        self
    }

    pub fn read(mut self, is_read: bool) -> Self {
        self.is_read = is_read;
        self
    }

    pub fn starred(mut self, is_starred: bool) -> Self {
        self.is_starred = is_starred;
        self
    }

    pub fn deleted(mut self, is_deleted: bool) -> Self {
        self.is_deleted = is_deleted;
        self
    }

    pub fn sent_at(mut self, sent_at: i64) -> Self {
        self.sent_at = sent_at;
        self
    }
}

#[allow(dead_code)]
pub async fn insert_test_email(svc: &TestServices, account_id: i32, email: TestEmail) -> i32 {
    let now = chrono::Utc::now().timestamp();
    let inserted = emails::Entity::insert(emails::ActiveModel {
        account_id: Set(account_id),
        folder: Set(email.folder),
        uid: Set(email.uid),
        message_id: Set(Some(format!("<test-{}@example.com>", email.uid))),
        subject: Set(email.subject),
        sender_name: Set(Some("Test Sender".to_string())),
        sender_email: Set(email.sender_email),
        recipient_emails: Set("recipient@example.com".to_string()),
        cc_emails: Set(None),
        bcc_emails: Set(None),
        preview: Set(email.preview),
        body_text: Set(email.body_text),
        body_html: Set(Some("<p>Body</p>".to_string())),
        is_read: Set(Some(email.is_read)),
        is_starred: Set(Some(email.is_starred)),
        is_draft: Set(Some(false)),
        is_answered: Set(Some(false)),
        is_deleted: Set(Some(email.is_deleted)),
        sent_at: Set(email.sent_at),
        received_at: Set(email.sent_at),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .expect("插入测试邮件失败");

    inserted.last_insert_id
}
