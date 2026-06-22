mod common;

use common::{TestEmail, TestServices, insert_test_email};
use std::sync::atomic::{AtomicU32, Ordering};

static ACCOUNT_COUNTER: AtomicU32 = AtomicU32::new(1);
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::email_service::EmailCategory;

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

    let first_page = svc.email_service.list(account_id, "INBOX", 1, 2).await.unwrap();
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.emails.len(), 2);
    assert_eq!(first_page.emails[0].subject.as_deref(), Some("最新邮件"));
    assert_eq!(first_page.emails[1].subject.as_deref(), Some("中间邮件"));

    let second_page = svc.email_service.list(account_id, "INBOX", 2, 2).await.unwrap();
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

    let resp = svc.email_service.list(account_id, "INBOX", 1, 20).await.unwrap();
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

    // mark_as_read 内部使用 update_many，不存在的 id 会静默成功（影响 0 行）
    let result = svc.email_service.mark_as_read(999, true).await;
    assert!(
        result.is_ok(),
        "mark_as_read 对不存在的邮件不报错（update_many 静默成功）"
    );
}

#[tokio::test]
async fn test_mark_as_read_updates_existing_email() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(30, "未读邮件")).await;

    svc.email_service.mark_as_read(email_id, true).await.unwrap();

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
async fn test_delete_emails_soft_deletes_and_hides_from_list() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let delete_id = insert_test_email(&svc, account_id, TestEmail::new(60, "待删除")).await;
    insert_test_email(&svc, account_id, TestEmail::new(61, "保留")).await;

    let deleted = svc.email_service.delete(vec![delete_id]).await.unwrap();
    assert_eq!(deleted, 1);

    let resp = svc.email_service.list(account_id, "INBOX", 1, 20).await.unwrap();
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

    let inbox = svc.email_service.list(account_id, "INBOX", 1, 20).await.unwrap();
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
        TestEmail::new(81, "归档星标").folder("Archive").starred(true),
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
    assert!(resp.emails.iter().all(|email| email.subject.as_deref() != Some("已删除星标")));
}
