use std::sync::Arc;

use async_trait::async_trait;
use postium_mail_lib::domain::auth::AuthManager;
use postium_mail_lib::domain::providers::pool::init_provider_pool;
use postium_mail_lib::error::MailError;
use postium_mail_lib::infrastructure::protocols::types::{FetchedBodySection, WholeEmailDto};
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::infrastructure::storage::models::accounts;
use postium_mail_lib::service::account_connection::{
    ImapConnectionVerifier, NoopImapConnectionVerifier,
};
use postium_mail_lib::service::email_service::EmailService;
use postium_mail_lib::service::mail_operation::MailRemoteOperator;
use postium_mail_lib::service::{AccountService, LabelService, SyncService};

pub mod real_mail;

/// Create an in-memory SQLite database with all migrations applied.
pub async fn create_test_db() -> DbConn {
    DbConn::open_in_memory_for_test()
        .await
        .expect("Failed to create in-memory SQLite test database")
}

/// Holds ready-to-use service instances backed by an in-memory test database.
#[allow(dead_code)]
pub struct TestServices {
    pub db: DbConn,
    pub auth: Arc<AuthManager>,
    pub account_service: AccountService,
    pub email_service: EmailService,
    pub label_service: LabelService,
    pub sync_service: SyncService,
}

struct NoopMailRemoteOperator;

#[async_trait]
impl MailRemoteOperator for NoopMailRemoteOperator {
    async fn mark_seen(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
        _seen: bool,
    ) -> Result<(), MailError> {
        Ok(())
    }

    async fn set_flagged(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
        _flagged: bool,
    ) -> Result<(), MailError> {
        Ok(())
    }

    async fn move_to_folder(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
        _target_folder: &str,
    ) -> Result<(), MailError> {
        Ok(())
    }

    async fn reload_email(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        Ok(None)
    }

    async fn fetch_attachment_section(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
        _section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError> {
        Ok(None)
    }
}

#[allow(dead_code)]
impl TestServices {
    /// Create a full set of services backed by a fresh in-memory database.
    pub async fn new() -> Self {
        Self::new_with_imap_verifier(Arc::new(NoopImapConnectionVerifier)).await
    }

    pub async fn new_with_imap_verifier(imap_verifier: Arc<dyn ImapConnectionVerifier>) -> Self {
        init_provider_pool();
        let db = create_test_db().await;
        let auth = Arc::new(AuthManager::in_memory());
        let mail_remote = Arc::new(NoopMailRemoteOperator);

        Self {
            account_service: AccountService::new_with_imap_verifier(
                db.clone(),
                auth.clone(),
                imap_verifier,
            ),
            email_service: EmailService::new_with_mail_remote(
                auth.clone(),
                db.clone(),
                mail_remote,
            ),
            label_service: LabelService::new(db.clone()),
            sync_service: SyncService::new(db.clone(), auth.clone()),
            db,
            auth,
        }
    }

    pub async fn new_with_mail_remote(mail_remote: Arc<dyn MailRemoteOperator>) -> Self {
        init_provider_pool();
        let db = create_test_db().await;
        let auth = Arc::new(AuthManager::in_memory());

        Self {
            account_service: AccountService::new_with_imap_verifier(
                db.clone(),
                auth.clone(),
                Arc::new(NoopImapConnectionVerifier),
            ),
            email_service: EmailService::new_with_mail_remote(
                auth.clone(),
                db.clone(),
                mail_remote,
            ),
            label_service: LabelService::new(db.clone()),
            sync_service: SyncService::new(db.clone(), auth.clone()),
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
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, ?11, ?12, ?13, 0, 0, ?14, ?15, ?16, ?17, ?18)",
                rusqlite::params![
                    account_id,
                    email.folder,
                    email.uid,
                    format!("<test-{}@example.com>", email.uid),
                    email.subject,
                    "Test Sender",
                    email.sender_email,
                    "recipient@example.com",
                    email.preview,
                    email.body_text,
                    Some("<p>Body</p>".to_string()),
                    email.is_read as i64,
                    email.is_starred as i64,
                    email.is_deleted as i64,
                    email.sent_at,
                    email.sent_at,
                    now,
                    now,
                ],
            )?;
            Ok(conn.last_insert_rowid() as i32)
        })
        .await
        .expect("插入测试邮件失败")
}

#[allow(dead_code)]
pub async fn count_where(svc: &TestServices, sql: &'static str, value: i32) -> i64 {
    svc.db
        .call(move |conn| conn.query_row(sql, [value], |row| row.get(0)))
        .await
        .expect("count query failed")
}
