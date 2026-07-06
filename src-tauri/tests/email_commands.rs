mod common;

use async_trait::async_trait;
use common::{
    RecordingSentArchiveWriter, RecordingSmtpSender, TestEmail, TestServices, insert_test_email,
};
use postium_mail_lib::domain::providers::SslMode;
use postium_mail_lib::error::MailError;
use postium_mail_lib::infrastructure::protocols::types::{
    AttachmentInfo, FetchedBodySection, WholeEmailDto,
};
use postium_mail_lib::infrastructure::storage::models::accounts;
use postium_mail_lib::infrastructure::storage::repository::sync_repo;
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::email_service::{
    EmailCategory, ReloadEmailResult, SendEmailRequest,
};
use postium_mail_lib::service::mail_operation::MailRemoteOperator;
use postium_mail_lib::service::mail_send::{
    build_email, smtp_config_from_account, validate_send_request,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

static ACCOUNT_COUNTER: AtomicU32 = AtomicU32::new(1);

fn account_model_with_smtp(
    provider: &str,
    smtp_host: Option<&str>,
    smtp_port: Option<i32>,
    smtp_ssl_mode: Option<&str>,
) -> accounts::Model {
    accounts::Model {
        id: 1,
        name: "Work".to_string(),
        email: "work@example.com".to_string(),
        display_name: Some("Work User".to_string()),
        provider: provider.to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl: Some(true),
        imap_ssl_mode: None,
        smtp_host: smtp_host.map(ToOwned::to_owned),
        smtp_port,
        smtp_ssl: Some(true),
        smtp_ssl_mode: smtp_ssl_mode.map(ToOwned::to_owned),
        color: None,
        sync_enabled: Some(true),
        last_sync_at: None,
        auth_type: Some("password".to_string()),
        account_type: "personal".to_string(),
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn smtp_config_from_account_prefers_manual_smtp_settings() {
    postium_mail_lib::domain::providers::pool::init_provider_pool();
    let account = account_model_with_smtp(
        "gmail",
        Some("smtp.manual.example.com"),
        Some(2525),
        Some("StartTls"),
    );

    let config = smtp_config_from_account(&account).unwrap();

    assert_eq!(config.host, "smtp.manual.example.com");
    assert_eq!(config.port, 2525);
    assert!(matches!(config.ssl, SslMode::StartTls));
}

#[test]
fn smtp_config_from_account_rejects_invalid_manual_port() {
    postium_mail_lib::domain::providers::pool::init_provider_pool();
    let account = account_model_with_smtp("gmail", Some("smtp.example.com"), Some(70000), None);

    let result = smtp_config_from_account(&account);

    assert!(
        matches!(result, Err(MailError::InvalidParam(message)) if message.contains("SMTP 端口"))
    );
}

#[test]
fn build_email_generates_message_id_and_raw_rfc822() {
    let account = account_model_with_smtp("gmail", None, None, None);
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let built = build_email(&account, &req).unwrap();
    let raw = String::from_utf8(built.raw).unwrap();

    assert!(built.message_id.starts_with('<'));
    assert!(built.message_id.ends_with('>'));
    assert!(raw.contains("Message-ID:"));
    assert!(raw.contains("Subject: Hello"));
    assert!(raw.contains("Content-Type: multipart/alternative"));
}

#[test]
fn send_validation_rejects_missing_recipients() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("收件人")));
}

#[test]
fn send_validation_rejects_empty_subject() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "   ".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("主题")));
}

#[test]
fn send_validation_rejects_empty_body_text() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p><br></p>".to_string(),
        body_text: " \n\t ".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("正文")));
}

async fn account_email(svc: &TestServices, account_id: i32) -> String {
    svc.account_service
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|account| account.id == account_id)
        .unwrap()
        .email
}

async fn create_test_account(svc: &TestServices) -> i32 {
    let index = ACCOUNT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let req = CreateAccountRequest {
        name: format!("Test {}", index),
        email: format!("test-{}@gmail.com", index),
        display_name: None,
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "pass".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };
    svc.account_service.create(req).await.unwrap().id
}

async fn create_test_account_with_display_name(svc: &TestServices, display_name: &str) -> i32 {
    let index = ACCOUNT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let req = CreateAccountRequest {
        name: format!("Test {}", index),
        email: format!("test-{}@gmail.com", index),
        display_name: Some(display_name.to_string()),
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "pass".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };
    svc.account_service.create(req).await.unwrap().id
}

fn send_request(account_id: i32) -> SendEmailRequest {
    SendEmailRequest {
        account_id,
        to: vec!["recipient@example.com".to_string()],
        cc: vec!["copy@example.com".to_string()],
        bcc: vec![],
        subject: "  Service Send  ".to_string(),
        body_html: "<p>Hello from service</p>".to_string(),
        body_text: "Hello from service".to_string(),
    }
}

async fn sent_email_count(svc: &TestServices, account_id: i32) -> i64 {
    svc.db
        .call(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE account_id = ?1 AND folder = ?2",
                rusqlite::params![account_id, "[Gmail]/Sent Mail"],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn send_email_writes_local_sent_and_archives_remote() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(None),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(None),
    });
    let svc = TestServices::new_with_send_dependencies(smtp.clone(), archiver.clone()).await;
    let account_id = create_test_account_with_display_name(&svc, "Sender Name").await;

    let response = svc
        .email_service
        .send(send_request(account_id))
        .await
        .unwrap();

    assert_eq!(
        smtp.sent_message_ids.lock().unwrap().as_slice(),
        &[response.message_id.clone()]
    );
    assert_eq!(
        archiver.archived_message_ids.lock().unwrap().as_slice(),
        &[response.message_id.clone()]
    );
    assert!(response.remote_archived);
    assert!(response.remote_archive_error.is_none());
    let detail = svc
        .email_service
        .get(response.local_email_id)
        .await
        .unwrap();
    assert_eq!(detail.email.folder, "[Gmail]/Sent Mail");
    assert_eq!(detail.email.subject.as_deref(), Some("Service Send"));
    assert_eq!(
        detail.email.sender_email,
        account_email(&svc, account_id).await
    );
    assert_eq!(detail.recipient_emails, "recipient@example.com");
    assert_eq!(detail.cc_emails.as_deref(), Some("copy@example.com"));
    assert_eq!(
        sent_email_count(&svc, account_id).await,
        1,
        "SMTP 成功后应写入本地 Sent"
    );
}

#[tokio::test]
async fn send_email_does_not_write_sent_when_smtp_fails() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(Some(MailError::SmtpSendFailed(
            "simulated smtp failure".to_string(),
        ))),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(None),
    });
    let svc = TestServices::new_with_send_dependencies(smtp, archiver.clone()).await;
    let account_id = create_test_account(&svc).await;

    let result = svc.email_service.send(send_request(account_id)).await;

    assert!(
        matches!(result, Err(MailError::SmtpSendFailed(message)) if message.contains("simulated smtp failure"))
    );
    assert!(archiver.archived_message_ids.lock().unwrap().is_empty());
    assert_eq!(
        sent_email_count(&svc, account_id).await,
        0,
        "SMTP 失败不能写入本地 Sent"
    );
}

#[tokio::test]
async fn send_email_keeps_local_sent_when_remote_archive_fails() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(None),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(Vec::new()),
        fail_with: Mutex::new(Some(MailError::ImapConnectionFailed(
            "simulated append failure".to_string(),
        ))),
    });
    let svc = TestServices::new_with_send_dependencies(smtp.clone(), archiver).await;
    let account_id = create_test_account(&svc).await;

    let response = svc
        .email_service
        .send(send_request(account_id))
        .await
        .unwrap();

    assert_eq!(
        smtp.sent_message_ids.lock().unwrap().as_slice(),
        &[response.message_id.clone()]
    );
    assert!(!response.remote_archived);
    assert!(
        response
            .remote_archive_error
            .as_deref()
            .is_some_and(|message| message.contains("simulated append failure"))
    );
    svc.email_service
        .get(response.local_email_id)
        .await
        .unwrap();
    assert_eq!(
        sent_email_count(&svc, account_id).await,
        1,
        "远端归档失败仍应保留本地 Sent"
    );
}

async fn create_remote_test_account(svc: &TestServices) -> i32 {
    create_remote_test_account_with_display_name(svc, None).await
}

async fn create_remote_test_account_with_display_name(
    svc: &TestServices,
    display_name: Option<&str>,
) -> i32 {
    let req = CreateAccountRequest {
        name: "Test Remote".to_string(),
        email: "test@example.com".to_string(),
        display_name: display_name.map(str::to_string),
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "pass".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };
    svc.account_service.create(req).await.unwrap().id
}

fn remote_email(uid: u32, subject: &str, body: &str) -> WholeEmailDto {
    WholeEmailDto {
        id: 0,
        account_id: 0,
        folder: "INBOX".to_string(),
        uid,
        message_id: Some(format!("<reload-{uid}@example.com>")),
        sender_name: Some("Reload Sender".to_string()),
        sender_email: "reload@example.com".to_string(),
        recipient_emails: "recipient@example.com".to_string(),
        cc_emails: Some("copy@example.com".to_string()),
        bcc_emails: None,
        subject: Some(subject.to_string()),
        preview: Some(body.chars().take(200).collect()),
        body_text: Some(body.to_string()),
        body_html: Some(format!("<p>{body}</p>")),
        attachments: vec![AttachmentInfo {
            filename: Some("reload.pdf".to_string()),
            content_type: "application/pdf".to_string(),
            size: 42,
            section_path: "2".to_string(),
            disposition: Some("attachment".to_string()),
            content_id: None,
        }],
        is_read: true,
        is_starred: true,
        is_draft: false,
        is_answered: true,
        is_deleted: false,
        sent_at: 1_800_000_100,
        received_at: 1_800_000_101,
        created_at: 0,
    }
}

struct FakeMailRemote {
    calls: Mutex<Vec<String>>,
    fail: bool,
    reload_result: Mutex<Option<Result<Option<WholeEmailDto>, MailError>>>,
    attachment_section: Mutex<Option<Result<Option<FetchedBodySection>, MailError>>>,
}

impl FakeMailRemote {
    fn success() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: false,
            reload_result: Mutex::new(Some(Ok(None))),
            attachment_section: Mutex::new(Some(Ok(None))),
        })
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: true,
            reload_result: Mutex::new(Some(Err(MailError::ImapConnectionFailed(
                "fake remote failure".to_string(),
            )))),
            attachment_section: Mutex::new(Some(Err(MailError::ImapConnectionFailed(
                "fake remote failure".to_string(),
            )))),
        })
    }

    fn with_reload(result: Result<Option<WholeEmailDto>, MailError>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: false,
            reload_result: Mutex::new(Some(result)),
            attachment_section: Mutex::new(Some(Ok(None))),
        })
    }

    fn with_attachment_section(body: Vec<u8>, transfer_encoding: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: false,
            reload_result: Mutex::new(Some(Ok(None))),
            attachment_section: Mutex::new(Some(Ok(Some(FetchedBodySection {
                body,
                transfer_encoding,
            })))),
        })
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn maybe_fail(&self) -> Result<(), MailError> {
        if self.fail {
            return Err(MailError::ImapConnectionFailed(
                "fake remote failure".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl MailRemoteOperator for FakeMailRemote {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("mark_seen:{}:{folder}:{uid}:{seen}", account.email));
        self.maybe_fail()
    }

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError> {
        self.calls.lock().unwrap().push(format!(
            "set_flagged:{}:{folder}:{uid}:{flagged}",
            account.email
        ));
        self.maybe_fail()
    }

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        self.calls.lock().unwrap().push(format!(
            "move_to_folder:{}:{folder}:{uid}:{target_folder}",
            account.email
        ));
        self.maybe_fail()
    }

    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("reload:{}:{folder}:{uid}", account.email));
        self.reload_result
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(None))
    }

    async fn fetch_attachment_section(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError> {
        self.calls.lock().unwrap().push(format!(
            "fetch_attachment_section:{}:{folder}:{uid}:{section_path}",
            account.email
        ));
        self.attachment_section
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(None))
    }
}

async fn insert_test_attachment(
    svc: &TestServices,
    email_id: i32,
    filename: &str,
    size: i64,
    section_path: &str,
) -> i32 {
    let filename = filename.to_string();
    let section_path = section_path.to_string();
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                rusqlite::params![
                    email_id,
                    filename,
                    "text/plain",
                    size,
                    section_path,
                    "attachment",
                    Option::<String>::None,
                    chrono::Utc::now().timestamp()
                ],
            )?;
            Ok(conn.last_insert_rowid() as i32)
        })
        .await
        .unwrap()
}

async fn attachment_path(svc: &TestServices, attachment_id: i32) -> Option<String> {
    svc.db
        .call(move |conn| {
            conn.query_row(
                "SELECT path FROM attachments WHERE id = ?1",
                [attachment_id],
                |row| row.get::<_, Option<String>>(0),
            )
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn ensure_cached_downloads_small_attachment_and_updates_path() {
    let remote = FakeMailRemote::with_attachment_section(
        b"hello attachment".to_vec(),
        Some("7bit".to_string()),
    );
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id =
        insert_test_email(&svc, account_id, TestEmail::new(3101, "small attachment")).await;
    let attachment_id = insert_test_attachment(&svc, email_id, "hello.txt", 16, "2").await;
    let cache_root = tempfile::tempdir().unwrap();

    let service = postium_mail_lib::service::AttachmentService::new_with_remote(
        svc.db.clone(),
        remote.clone(),
        cache_root.path().to_path_buf(),
    );

    let dto = service.ensure_cached(attachment_id).await.unwrap();

    assert!(dto.is_cached);
    let cache_path = dto.cache_path.unwrap();
    assert_eq!(std::fs::read(&cache_path).unwrap(), b"hello attachment");
    let calls = remote.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].ends_with(":INBOX:3101:2"));
}

#[tokio::test]
async fn ensure_cached_decodes_base64_attachment_with_mime_whitespace() {
    let remote = FakeMailRemote::with_attachment_section(
        b"aGVs\r\nbG8g\r\nYXR0YWNobWVudA==".to_vec(),
        Some("base64".to_string()),
    );
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id =
        insert_test_email(&svc, account_id, TestEmail::new(3104, "base64 attachment")).await;
    let attachment_id = insert_test_attachment(&svc, email_id, "base64.txt", 16, "2").await;
    let cache_root = tempfile::tempdir().unwrap();

    let service = postium_mail_lib::service::AttachmentService::new_with_remote(
        svc.db.clone(),
        remote,
        cache_root.path().to_path_buf(),
    );

    let dto = service.ensure_cached(attachment_id).await.unwrap();

    let cache_path = dto.cache_path.unwrap();
    assert_eq!(std::fs::read(&cache_path).unwrap(), b"hello attachment");
}

#[tokio::test]
async fn save_as_copies_small_attachment_from_cache_and_updates_path() {
    let remote =
        FakeMailRemote::with_attachment_section(b"cached copy".to_vec(), Some("7bit".to_string()));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(3102, "save small")).await;
    let attachment_id = insert_test_attachment(&svc, email_id, "copy.txt", 11, "2").await;
    let cache_root = tempfile::tempdir().unwrap();
    let output_dir = tempfile::tempdir().unwrap();
    let target_path = output_dir.path().join("saved-copy.txt");
    let service = postium_mail_lib::service::AttachmentService::new_with_remote(
        svc.db.clone(),
        remote.clone(),
        cache_root.path().to_path_buf(),
    );

    service
        .save_as(attachment_id, target_path.to_string_lossy().to_string())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&target_path).unwrap(), b"cached copy");
    assert!(attachment_path(&svc, attachment_id).await.is_some());
    let calls = remote.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].ends_with(":INBOX:3102:2"));
}

#[tokio::test]
async fn save_as_writes_large_attachment_directly_without_updating_path() {
    let remote =
        FakeMailRemote::with_attachment_section(b"direct large".to_vec(), Some("7bit".to_string()));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(3103, "save large")).await;
    let attachment_id = insert_test_attachment(
        &svc,
        email_id,
        "large.bin",
        postium_mail_lib::service::attachment_service::SMALL_ATTACHMENT_LIMIT_BYTES + 1,
        "2",
    )
    .await;
    let cache_root = tempfile::tempdir().unwrap();
    let output_dir = tempfile::tempdir().unwrap();
    let target_path = output_dir.path().join("large.bin");
    let service = postium_mail_lib::service::AttachmentService::new_with_remote(
        svc.db.clone(),
        remote.clone(),
        cache_root.path().to_path_buf(),
    );

    service
        .save_as(attachment_id, target_path.to_string_lossy().to_string())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&target_path).unwrap(), b"direct large");
    assert!(attachment_path(&svc, attachment_id).await.is_none());
    let calls = remote.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].ends_with(":INBOX:3103:2"));
}

#[tokio::test]
async fn test_mark_as_read_remote_success_updates_local() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(501, "远端已读")).await;

    svc.email_service
        .mark_as_read(email_id, true)
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(detail.email.is_read);
    assert_eq!(
        remote.calls(),
        vec!["mark_seen:test@example.com:INBOX:501:true"]
    );
}

#[tokio::test]
async fn test_mark_as_read_remote_failure_keeps_local_state() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(502, "远端失败")).await;

    let result = svc.email_service.mark_as_read(email_id, true).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(!detail.email.is_read);
    assert_eq!(
        remote.calls(),
        vec!["mark_seen:test@example.com:INBOX:502:true"]
    );
}

#[tokio::test]
async fn test_toggle_star_remote_success_updates_local() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(503, "远端星标")).await;

    let new_state = svc.email_service.toggle_star(email_id).await.unwrap();

    assert!(new_state);
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(detail.email.is_starred);
    assert_eq!(
        remote.calls(),
        vec!["set_flagged:test@example.com:INBOX:503:true"]
    );
}

#[tokio::test]
async fn test_toggle_star_remote_failure_keeps_local_state() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(504, "星标失败")).await;

    let result = svc.email_service.toggle_star(email_id).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(!detail.email.is_starred);
    assert_eq!(
        remote.calls(),
        vec!["set_flagged:test@example.com:INBOX:504:true"]
    );
}

#[tokio::test]
async fn test_move_to_folder_remote_success_updates_local_folder() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(505, "移动邮件")).await;

    svc.email_service
        .move_to_folder(email_id, "Work")
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "Work");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:505:Work"]
    );
}

#[tokio::test]
async fn test_move_to_folder_remote_failure_keeps_local_folder() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(506, "移动失败")).await;

    let result = svc.email_service.move_to_folder(email_id, "Work").await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "INBOX");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:506:Work"]
    );
}

#[tokio::test]
async fn test_archive_moves_remote_to_provider_archive_folder() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(507, "归档邮件")).await;

    svc.email_service.archive(email_id).await.unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "[Gmail]/All Mail");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:507:[Gmail]/All Mail"]
    );
}

#[tokio::test]
async fn test_delete_moves_remote_to_provider_trash_folder_without_soft_delete() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(508, "删除邮件")).await;

    let count = svc.email_service.delete(vec![email_id]).await.unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(detail.email.folder, "[Gmail]/Trash");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:508:[Gmail]/Trash"]
    );
}

#[tokio::test]
async fn test_delete_remote_failure_keeps_email_in_original_folder() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(509, "删除失败")).await;

    let result = svc.email_service.delete(vec![email_id]).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "INBOX");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:509:[Gmail]/Trash"]
    );
}

#[tokio::test]
async fn test_delete_email_already_in_trash_returns_error_without_remote_call() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(510, "已在垃圾箱").folder("[Gmail]/Trash"),
    )
    .await;

    let result = svc.email_service.delete(vec![email_id]).await;

    assert!(matches!(result, Err(MailError::InvalidParam(_))));
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "[Gmail]/Trash");
    assert!(remote.calls().is_empty());
}

#[tokio::test]
async fn test_delete_email_in_alternate_trash_folder_returns_error_without_remote_call() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(511, "中文垃圾箱").folder("[Gmail]/&V4NXPpCuTvY-"),
    )
    .await;

    let result = svc.email_service.delete(vec![email_id]).await;

    assert!(matches!(result, Err(MailError::InvalidParam(_))));
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "[Gmail]/&V4NXPpCuTvY-");
    assert!(remote.calls().is_empty());
}

#[tokio::test]
async fn test_delete_batch_returns_error_without_remote_or_local_changes() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let first_id = insert_test_email(&svc, account_id, TestEmail::new(512, "第一封")).await;
    let second_id = insert_test_email(&svc, account_id, TestEmail::new(513, "第二封")).await;

    let result = svc.email_service.delete(vec![first_id, second_id]).await;

    assert!(matches!(result, Err(MailError::InvalidParam(_))));
    assert_eq!(
        svc.email_service.get(first_id).await.unwrap().email.folder,
        "INBOX"
    );
    assert_eq!(
        svc.email_service.get(second_id).await.unwrap().email.folder,
        "INBOX"
    );
    assert!(remote.calls().is_empty());
}

#[tokio::test]
async fn test_reload_email_replaces_local_email_when_remote_exists() {
    let remote =
        FakeMailRemote::with_reload(Ok(Some(remote_email(514, "远端新主题", "远端新正文"))));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id =
        create_remote_test_account_with_display_name(&svc, Some("Reload Account")).await;
    let email_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(514, "本地旧主题").body_text("本地旧正文"),
    )
    .await;

    let result = svc.email_service.reload_email(email_id).await.unwrap();

    let ReloadEmailResult::Reloaded { email } = result else {
        panic!("expected reloaded result");
    };
    assert_eq!(email.email.id, email_id);
    assert_eq!(email.email.subject.as_deref(), Some("远端新主题"));
    assert_eq!(email.body_text.as_deref(), Some("远端新正文"));
    assert_eq!(email.body_html.as_deref(), Some("<p>远端新正文</p>"));
    assert_eq!(email.recipient_emails, "recipient@example.com");
    assert_eq!(email.cc_emails.as_deref(), Some("copy@example.com"));
    assert!(email.email.is_read);
    assert!(email.email.is_starred);
    assert!(email.email.has_attachments);
    assert_eq!(email.email.sender_email, "reload@example.com");
    assert_eq!(
        email.email.account_email.as_deref(),
        Some("test@example.com")
    );
    assert_eq!(
        email.email.account_display_name.as_deref(),
        Some("Reload Account")
    );
    assert_eq!(remote.calls(), vec!["reload:test@example.com:INBOX:514"]);
}

#[tokio::test]
async fn test_reload_email_removes_local_email_when_remote_uid_missing() {
    let remote = FakeMailRemote::with_reload(Ok(None));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(515, "远端不存在")).await;

    let result = svc.email_service.reload_email(email_id).await.unwrap();

    assert!(matches!(
        result,
        ReloadEmailResult::Removed { email_id: removed_id } if removed_id == email_id
    ));
    assert!(matches!(
        svc.email_service.get(email_id).await,
        Err(MailError::EmailNotFound(id)) if id == email_id
    ));
    assert_eq!(remote.calls(), vec!["reload:test@example.com:INBOX:515"]);
}

#[tokio::test]
async fn test_reload_email_remote_error_keeps_local_email() {
    let remote =
        FakeMailRemote::with_reload(Err(MailError::ImapError("remote failed".to_string())));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(516, "保留主题").body_text("保留正文"),
    )
    .await;

    let result = svc.email_service.reload_email(email_id).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.subject.as_deref(), Some("保留主题"));
    assert_eq!(detail.body_text.as_deref(), Some("保留正文"));
    assert_eq!(remote.calls(), vec!["reload:test@example.com:INBOX:516"]);
}

#[tokio::test]
async fn test_list_emails_empty() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let result = svc.email_service.list(account_id, "INBOX", 1, 20).await;
    assert!(result.is_ok(), "list 应该成功: {:?}", result.err());
    let resp = result.unwrap();
    assert_eq!(resp.emails.len(), 0);
    assert_eq!(resp.total, 0);
}

#[tokio::test]
async fn test_list_emails_account_not_found() {
    let svc = TestServices::new().await;

    // 对不存在的账号调用 list，应该返回空结果（不会报错，因为没有 JOIN 约束）
    let result = svc.email_service.list(999, "INBOX", 1, 20).await;
    assert!(result.is_ok(), "list 对不存在的账号应返回空结果");
    let resp = result.unwrap();
    assert_eq!(resp.emails.len(), 0);
    assert_eq!(resp.total, 0);
}

#[tokio::test]
async fn test_list_emails_paginates_and_orders_by_sent_at_desc() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(1, "较旧邮件").sent_at(1_700_000_000),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(2, "最新邮件").sent_at(1_700_000_200),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(3, "中间邮件").sent_at(1_700_000_100),
    )
    .await;

    let first_page = svc
        .email_service
        .list(account_id, "INBOX", 1, 2)
        .await
        .unwrap();
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.emails.len(), 2);
    assert_eq!(first_page.emails[0].subject.as_deref(), Some("最新邮件"));
    assert_eq!(first_page.emails[1].subject.as_deref(), Some("中间邮件"));

    let second_page = svc
        .email_service
        .list(account_id, "INBOX", 2, 2)
        .await
        .unwrap();
    assert_eq!(second_page.emails.len(), 1);
    assert_eq!(second_page.emails[0].subject.as_deref(), Some("较旧邮件"));
}

#[tokio::test]
async fn test_list_emails_filters_folder_account_and_deleted_messages() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let other_account_id = create_test_account(&svc).await;

    insert_test_email(&svc, account_id, TestEmail::new(10, "收件箱邮件")).await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(11, "已删除邮件").deleted(true),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(12, "已发送邮件").folder("Sent"),
    )
    .await;
    insert_test_email(&svc, other_account_id, TestEmail::new(13, "其他账号邮件")).await;

    let resp = svc
        .email_service
        .list(account_id, "INBOX", 1, 20)
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.emails[0].subject.as_deref(), Some("收件箱邮件"));
}

#[tokio::test]
async fn test_list_emails_returns_real_attachment_state() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let with_attachment_id =
        insert_test_email(&svc, account_id, TestEmail::new(3002, "列表附件")).await;
    let without_attachment_id =
        insert_test_email(&svc, account_id, TestEmail::new(3003, "列表无附件")).await;

    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                rusqlite::params![
                    with_attachment_id,
                    "report.pdf",
                    "application/pdf",
                    1234_i64,
                    "2",
                    "attachment",
                    Option::<String>::None,
                    chrono::Utc::now().timestamp()
                ],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let resp = svc
        .email_service
        .list(account_id, "INBOX", 1, 20)
        .await
        .unwrap();

    let with_attachment = resp
        .emails
        .iter()
        .find(|email| email.id == with_attachment_id)
        .unwrap();
    let without_attachment = resp
        .emails
        .iter()
        .find(|email| email.id == without_attachment_id)
        .unwrap();
    assert!(with_attachment.has_attachments);
    assert!(!without_attachment.has_attachments);
}

#[tokio::test]
async fn test_get_email_not_found() {
    let svc = TestServices::new().await;

    let result = svc.email_service.get(999).await;
    assert!(result.is_err(), "get 不存在的邮件应返回错误");
}

#[tokio::test]
async fn test_get_email_returns_detail_fields() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(20, "详情邮件")
            .sender_email("detail@example.com")
            .preview("详情预览")
            .body_text("详情正文")
            .read(true)
            .starred(true),
    )
    .await;

    let detail = svc.email_service.get(email_id).await.unwrap();

    assert_eq!(detail.email.subject.as_deref(), Some("详情邮件"));
    assert_eq!(detail.email.sender_email, "detail@example.com");
    assert_eq!(detail.email.preview.as_deref(), Some("详情预览"));
    assert_eq!(detail.body_text.as_deref(), Some("详情正文"));
    assert!(detail.email.is_read);
    assert!(detail.email.is_starred);
}

#[tokio::test]
async fn get_email_returns_real_attachments() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id =
        insert_test_email(&svc, account_id, TestEmail::new(3001, "with attachment")).await;

    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                rusqlite::params![
                    email_id,
                    "report.pdf",
                    "application/pdf",
                    1234_i64,
                    "2",
                    "attachment",
                    Option::<String>::None,
                    chrono::Utc::now().timestamp()
                ],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();

    assert!(detail.email.has_attachments);
    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(detail.attachments[0].filename, "report.pdf");
    assert_eq!(detail.attachments[0].content_type, "application/pdf");
    assert_eq!(detail.attachments[0].size, 1234);
    assert!(!detail.attachments[0].is_inline);
    assert!(!detail.attachments[0].is_cached);
}

#[tokio::test]
async fn test_mark_as_read_not_found() {
    let svc = TestServices::new().await;

    let result = svc.email_service.mark_as_read(999, true).await;
    assert!(
        matches!(result, Err(MailError::EmailNotFound(999))),
        "mark_as_read 对不存在的邮件应返回 EmailNotFound"
    );
}

#[tokio::test]
async fn test_mark_as_read_updates_existing_email() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(30, "未读邮件")).await;

    svc.email_service
        .mark_as_read(email_id, true)
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(detail.email.is_read);
}

#[tokio::test]
async fn test_toggle_star_not_found() {
    let svc = TestServices::new().await;

    let result = svc.email_service.toggle_star(999).await;
    assert!(result.is_err(), "toggle_star 不存在的邮件应返回错误");
}

#[tokio::test]
async fn test_toggle_star_flips_existing_email_state() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(40, "星标邮件")).await;

    let starred = svc.email_service.toggle_star(email_id).await.unwrap();
    assert!(starred);

    let unstarred = svc.email_service.toggle_star(email_id).await.unwrap();
    assert!(!unstarred);
}

#[tokio::test]
async fn test_search_emails_empty() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let result = svc
        .email_service
        .search("test query", Some(account_id), Some(10))
        .await;
    assert!(result.is_ok(), "search 应该成功: {:?}", result.err());
    let results = result.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_search_emails_matches_subject_preview_and_account_scope() {
    let svc = TestServices::new().await;
    let account_id = create_test_account_with_display_name(&svc, "Search Account").await;
    let other_account_id = create_test_account(&svc).await;
    let expected_email = account_email(&svc, account_id).await;

    let matching_id = insert_test_email(
        &svc,
        account_id,
        TestEmail::new(50, "Quarterly budget review").preview("contains pipeline signal"),
    )
    .await;
    insert_test_email(
        &svc,
        other_account_id,
        TestEmail::new(51, "Quarterly budget review").preview("other account should not appear"),
    )
    .await;

    let scoped_results = svc
        .email_service
        .search("budget", Some(account_id), Some(10))
        .await
        .unwrap();

    assert_eq!(scoped_results.len(), 1);
    assert_eq!(scoped_results[0].id, matching_id);
    assert_eq!(
        scoped_results[0].account_email.as_deref(),
        Some(expected_email.as_str())
    );
    assert_eq!(
        scoped_results[0].account_display_name.as_deref(),
        Some("Search Account")
    );

    let preview_results = svc
        .email_service
        .search("pipeline", Some(account_id), Some(10))
        .await
        .unwrap();
    assert_eq!(preview_results.len(), 1);
    assert_eq!(preview_results[0].id, matching_id);
}

#[tokio::test]
async fn test_delete_emails_empty_list() {
    let svc = TestServices::new().await;

    let result = svc.email_service.delete(vec![]).await;
    assert!(result.is_ok(), "delete 空 list 应该成功");
    assert_eq!(result.unwrap(), 0, "删除空列表应影响 0 行");
}

#[tokio::test]
async fn test_delete_emails_moves_to_trash_and_hides_from_inbox() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let delete_id = insert_test_email(&svc, account_id, TestEmail::new(60, "待删除")).await;
    insert_test_email(&svc, account_id, TestEmail::new(61, "保留")).await;

    let deleted = svc.email_service.delete(vec![delete_id]).await.unwrap();
    assert_eq!(deleted, 1);

    let resp = svc
        .email_service
        .list(account_id, "INBOX", 1, 20)
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.emails[0].subject.as_deref(), Some("保留"));
}

#[tokio::test]
async fn test_move_to_folder_hides_from_old_folder_and_shows_in_new_folder() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(70, "可移动邮件")).await;

    svc.email_service
        .move_to_folder(email_id, "Archive")
        .await
        .unwrap();

    let inbox = svc
        .email_service
        .list(account_id, "INBOX", 1, 20)
        .await
        .unwrap();
    assert_eq!(inbox.total, 0);

    let archive = svc
        .email_service
        .list(account_id, "Archive", 1, 20)
        .await
        .unwrap();
    assert_eq!(archive.total, 1);
    assert_eq!(archive.emails[0].id, email_id);
}

#[tokio::test]
async fn test_state_updates_preserve_updated_at() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let original_updated_at = 1_234_567;

    let read_id = insert_test_email(&svc, account_id, TestEmail::new(71, "标记已读")).await;
    let star_id = insert_test_email(&svc, account_id, TestEmail::new(72, "切换星标")).await;
    let delete_id = insert_test_email(&svc, account_id, TestEmail::new(73, "软删除")).await;
    let move_id = insert_test_email(&svc, account_id, TestEmail::new(74, "移动文件夹")).await;

    for id in [read_id, star_id, delete_id, move_id] {
        svc.db
            .call(move |conn| {
                conn.execute(
                    "UPDATE emails SET updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![original_updated_at, id],
                )?;
                Ok(())
            })
            .await
            .unwrap();
    }

    svc.email_service.mark_as_read(read_id, true).await.unwrap();
    svc.email_service.toggle_star(star_id).await.unwrap();
    svc.email_service.delete(vec![delete_id]).await.unwrap();
    svc.email_service
        .move_to_folder(move_id, "Archive")
        .await
        .unwrap();

    let updated_at_values = svc
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT updated_at FROM emails WHERE id IN (?1, ?2, ?3, ?4)")?;
            stmt.query_map(
                rusqlite::params![read_id, star_id, delete_id, move_id],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .await
        .unwrap();

    assert_eq!(updated_at_values, vec![original_updated_at; 4]);
}

#[tokio::test]
async fn test_list_by_category_inbox_empty() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    // list_by_category Inbox 需要查找 provider pool 映射
    // 对于测试环境没有初始化 provider pool，所以会返回 ProviderNotSupported 错误
    // 使用 Starred 类别（不走 provider pool）来验证空结果
    let result = svc
        .email_service
        .list_by_category(account_id, EmailCategory::Starred, 1, 20, false)
        .await;
    assert!(
        result.is_ok(),
        "list_by_category Starred 应该成功: {:?}",
        result.err()
    );
    let resp = result.unwrap();
    assert_eq!(resp.emails.len(), 0);
    assert_eq!(resp.total, 0);
}

#[tokio::test]
async fn test_list_by_category_starred_filters_starred_and_deleted_messages() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(80, "收件箱星标").starred(true),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(81, "归档星标")
            .folder("Archive")
            .starred(true),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(82, "普通邮件").starred(false),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(83, "已删除星标").starred(true).deleted(true),
    )
    .await;

    let resp = svc
        .email_service
        .list_by_category(account_id, EmailCategory::Starred, 1, 20, false)
        .await
        .unwrap();

    assert_eq!(resp.total, 2);
    assert!(resp.emails.iter().all(|email| email.is_starred));
    assert!(
        resp.emails
            .iter()
            .all(|email| email.subject.as_deref() != Some("已删除星标"))
    );
}

#[tokio::test]
async fn list_by_category_starred_can_filter_unread_messages() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(85, "未读星标").starred(true).read(false),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(86, "已读星标").starred(true).read(true),
    )
    .await;
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(87, "未读非星标").starred(false).read(false),
    )
    .await;

    let resp = svc
        .email_service
        .list_by_category(account_id, EmailCategory::Starred, 1, 20, true)
        .await
        .unwrap();

    assert_eq!(resp.total, 1);
    assert_eq!(resp.emails[0].subject.as_deref(), Some("未读星标"));
    assert!(resp.emails.iter().all(|email| !email.is_read));
}

#[tokio::test]
async fn list_by_category_uses_persisted_folder_category_when_name_has_no_keyword() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    sync_repo::upsert_folder_category(&svc.db, account_id, "VendorFolder", "sent")
        .await
        .unwrap();
    insert_test_email(
        &svc,
        account_id,
        TestEmail::new(84, "SPECIAL-USE sent").folder("VendorFolder"),
    )
    .await;

    let resp = svc
        .email_service
        .list_by_category(account_id, EmailCategory::Sent, 1, 20, false)
        .await
        .unwrap();

    assert_eq!(resp.total, 1);
    assert_eq!(resp.emails[0].subject.as_deref(), Some("SPECIAL-USE sent"));
}

#[tokio::test]
async fn list_by_category_for_all_accounts_should_merge_inbox_across_accounts() {
    let svc = TestServices::new().await;
    let work_id = create_test_account_with_display_name(&svc, "Work Mail").await;
    let personal_id = create_test_account_with_display_name(&svc, "Personal Mail").await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(100, "work inbox")
            .folder("INBOX")
            .sent_at(100),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(200, "personal inbox")
            .folder("INBOX")
            .sent_at(300),
    )
    .await;
    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(101, "work sent")
            .folder("[Gmail]/Sent Mail")
            .sent_at(400),
    )
    .await;

    let response = svc
        .email_service
        .list_by_category_for_all_accounts(EmailCategory::Inbox, 1, 50, false)
        .await
        .unwrap();

    assert_eq!(response.total, 2);
    assert_eq!(response.emails.len(), 2);
    assert_eq!(
        response.emails[0].subject.as_deref(),
        Some("personal inbox")
    );
    assert_eq!(response.emails[0].account_id, personal_id);
    let expected_personal_email = account_email(&svc, personal_id).await;
    assert_eq!(
        response.emails[0].account_email.as_deref(),
        Some(expected_personal_email.as_str())
    );
    assert_eq!(
        response.emails[0].account_display_name.as_deref(),
        Some("Personal Mail")
    );
    assert_eq!(response.emails[1].subject.as_deref(), Some("work inbox"));
    assert_eq!(response.emails[1].account_id, work_id);
    let expected_work_email = account_email(&svc, work_id).await;
    assert_eq!(
        response.emails[1].account_email.as_deref(),
        Some(expected_work_email.as_str())
    );
    assert_eq!(
        response.emails[1].account_display_name.as_deref(),
        Some("Work Mail")
    );
}

#[tokio::test]
async fn list_by_category_for_all_accounts_should_query_starred_across_folders() {
    let svc = TestServices::new().await;
    let work_id = create_test_account_with_display_name(&svc, "Work Mail").await;
    let personal_id = create_test_account_with_display_name(&svc, "Personal Mail").await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(300, "work starred")
            .folder("INBOX")
            .starred(true)
            .sent_at(100),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(400, "personal starred")
            .folder("Archive")
            .starred(true)
            .sent_at(200),
    )
    .await;

    let response = svc
        .email_service
        .list_by_category_for_all_accounts(EmailCategory::Starred, 1, 50, false)
        .await
        .unwrap();

    assert_eq!(response.total, 2);
    assert_eq!(
        response.emails[0].subject.as_deref(),
        Some("personal starred")
    );
    assert_eq!(response.emails[0].account_id, personal_id);
    let expected_personal_email = account_email(&svc, personal_id).await;
    assert_eq!(
        response.emails[0].account_email.as_deref(),
        Some(expected_personal_email.as_str())
    );
    assert_eq!(
        response.emails[0].account_display_name.as_deref(),
        Some("Personal Mail")
    );
    assert_eq!(response.emails[1].subject.as_deref(), Some("work starred"));
    assert_eq!(response.emails[1].account_id, work_id);
    let expected_work_email = account_email(&svc, work_id).await;
    assert_eq!(
        response.emails[1].account_email.as_deref(),
        Some(expected_work_email.as_str())
    );
    assert_eq!(
        response.emails[1].account_display_name.as_deref(),
        Some("Work Mail")
    );
}
