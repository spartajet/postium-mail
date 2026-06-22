mod common;

use common::TestServices;
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::label_service::{CreateLabelRequest, UpdateLabelRequest};

/// Helper: create a test account and return its id.
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
async fn test_create_label() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let req = CreateLabelRequest {
        account_id,
        name: "Important".to_string(),
        color: "#ff5722".to_string(),
    };
    let result = svc.label_service.create_label(req).await;
    assert!(result.is_ok(), "创建标签应成功: {:?}", result.err());

    let label = result.unwrap();
    assert!(label.id > 0, "标签 ID 应为正数");
    assert_eq!(label.account_id, account_id);
    assert_eq!(label.name, "Important");
    assert_eq!(label.color, "#ff5722");
}

#[tokio::test]
async fn test_list_labels_empty() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let labels = svc.label_service.list_labels(account_id).await.unwrap();
    assert!(labels.is_empty(), "新账号不应有任何标签");
}

#[tokio::test]
async fn test_update_label() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let create_req = CreateLabelRequest {
        account_id,
        name: "Work".to_string(),
        color: "#2196f3".to_string(),
    };
    let created = svc.label_service.create_label(create_req).await.unwrap();

    let update_req = UpdateLabelRequest {
        name: Some("Personal".to_string()),
        color: Some("#4caf50".to_string()),
    };
    let updated = svc
        .label_service
        .update_label(created.id, update_req)
        .await
        .unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "Personal");
    assert_eq!(updated.color, "#4caf50");
    assert_eq!(updated.account_id, account_id);
}

#[tokio::test]
async fn test_delete_label() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let create_req = CreateLabelRequest {
        account_id,
        name: "To Delete".to_string(),
        color: "#e91e63".to_string(),
    };
    let created = svc.label_service.create_label(create_req).await.unwrap();

    let delete_result = svc.label_service.delete_label(created.id).await;
    assert!(delete_result.is_ok(), "删除标签应成功: {:?}", delete_result.err());

    let labels = svc.label_service.list_labels(account_id).await.unwrap();
    assert!(labels.is_empty(), "删除后标签列表应为空");
}

#[tokio::test]
async fn test_label_not_found() {
    let svc = TestServices::new().await;

    // update_label on a non-existent label should return LabelNotFound error
    let update_req = UpdateLabelRequest {
        name: Some("DoesNotExist".to_string()),
        color: None,
    };
    let result = svc.label_service.update_label(99999, update_req).await;
    assert!(result.is_err(), "更新不存在的标签应返回错误");
}
