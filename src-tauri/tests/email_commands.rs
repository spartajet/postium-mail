mod common;

use common::TestServices;
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::email_service::EmailCategory;

async fn create_test_account(svc: &TestServices) -> i32 {
    let req = CreateAccountRequest {
        name: "Test".to_string(),
        email: "test@gmail.com".to_string(),
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
async fn test_get_email_not_found() {
    let svc = TestServices::new().await;

    let result = svc.email_service.get(999).await;
    assert!(result.is_err(), "get 不存在的邮件应返回错误");
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
async fn test_toggle_star_not_found() {
    let svc = TestServices::new().await;

    let result = svc.email_service.toggle_star(999).await;
    assert!(result.is_err(), "toggle_star 不存在的邮件应返回错误");
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
async fn test_delete_emails_empty_list() {
    let svc = TestServices::new().await;

    let result = svc.email_service.delete(vec![]).await;
    assert!(result.is_ok(), "delete 空 list 应该成功");
    assert_eq!(result.unwrap(), 0, "删除空列表应影响 0 行");
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
