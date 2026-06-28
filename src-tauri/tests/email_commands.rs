mod common;

use async_trait::async_trait;
use common::{TestEmail, TestServices, insert_test_email};
use postium_mail_lib::error::MailError;
use postium_mail_lib::infrastructure::storage::models::accounts;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

static ACCOUNT_COUNTER: AtomicU32 = AtomicU32::new(1);
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::email_service::EmailCategory;
use postium_mail_lib::service::mail_operation::MailRemoteOperator;

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

async fn create_remote_test_account(svc: &TestServices) -> i32 {
    let req = CreateAccountRequest {
        name: "Test Remote".to_string(),
        email: "test@example.com".to_string(),
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

#[derive(Default)]
struct FakeMailRemote {
    calls: Mutex<Vec<String>>,
    fail: bool,
}

impl FakeMailRemote {
    fn success() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: true,
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
    let account_id = create_test_account(&svc).await;
    let other_account_id = create_test_account(&svc).await;

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
        .list_by_category(account_id, EmailCategory::Starred, 1, 20)
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
        .list_by_category(account_id, EmailCategory::Starred, 1, 20)
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
