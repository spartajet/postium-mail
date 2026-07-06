mod common;

use common::{TestEmail, TestServices, insert_test_email};
use postium_mail_lib::error::MailError;
use postium_mail_lib::service::account_service::CreateAccountRequest;

static ACCOUNT_EMAILS: &[&str] = &["all-work@gmail.com", "all-personal@gmail.com"];

async fn create_account(svc: &TestServices, index: usize) -> i32 {
    let req = CreateAccountRequest {
        name: format!("All Account {}", index),
        email: ACCOUNT_EMAILS[index].to_string(),
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

#[tokio::test]
async fn get_folder_stats_for_all_accounts_should_sum_sidebar_categories() {
    let svc = TestServices::new().await;
    let work_id = create_account(&svc, 0).await;
    let personal_id = create_account(&svc, 1).await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(10, "work unread")
            .folder("INBOX")
            .read(false),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(20, "personal read")
            .folder("INBOX")
            .read(true),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(21, "personal starred")
            .folder("INBOX")
            .starred(true)
            .read(false),
    )
    .await;

    let stats = svc
        .sync_service
        .get_folder_stats_for_all_accounts()
        .await
        .unwrap();
    let inbox = stats.iter().find(|stat| stat.folder == "inbox").unwrap();
    let starred = stats.iter().find(|stat| stat.folder == "starred").unwrap();

    assert_eq!(inbox.total, 3);
    assert_eq!(inbox.unread, 2);
    assert_eq!(starred.total, 1);
    assert_eq!(starred.unread, 1);
}

async fn set_account_provider(svc: &TestServices, account_id: i32, provider: &str) {
    let provider = provider.to_string();
    svc.db
        .call(move |conn| {
            conn.execute(
                "UPDATE accounts SET provider = ?1 WHERE id = ?2",
                rusqlite::params![provider, account_id],
            )?;
            Ok(())
        })
        .await
        .expect("更新账号 provider 失败");
}

#[tokio::test]
async fn get_folder_stats_for_all_accounts_should_skip_failed_accounts_when_some_accounts_fail() {
    let svc = TestServices::new().await;
    let work_id = create_account(&svc, 0).await;
    let broken_id = create_account(&svc, 1).await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(10, "work unread")
            .folder("INBOX")
            .read(false),
    )
    .await;
    set_account_provider(&svc, broken_id, "unknown-provider").await;

    let stats = svc
        .sync_service
        .get_folder_stats_for_all_accounts()
        .await
        .unwrap();

    assert_eq!(stats.len(), 1);
    let inbox = stats.iter().find(|stat| stat.folder == "inbox").unwrap();
    assert_eq!(inbox.total, 1);
    assert_eq!(inbox.unread, 1);
}

#[tokio::test]
async fn get_folder_stats_for_all_accounts_should_return_error_when_all_accounts_fail() {
    let svc = TestServices::new().await;
    let work_id = create_account(&svc, 0).await;
    let personal_id = create_account(&svc, 1).await;

    set_account_provider(&svc, work_id, "unknown-provider-a").await;
    set_account_provider(&svc, personal_id, "unknown-provider-b").await;

    let err = svc
        .sync_service
        .get_folder_stats_for_all_accounts()
        .await
        .unwrap_err();

    assert!(matches!(err, MailError::ProviderNotSupported(_)));
}
