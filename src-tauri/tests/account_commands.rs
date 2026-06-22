mod common;

use common::TestServices;
use postium_mail_lib::service::account_service::CreateAccountRequest;

#[tokio::test]
async fn test_create_account() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "Test Account".to_string(),
        email: "test@gmail.com".to_string(),
        display_name: Some("Test User".to_string()),
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "test_password".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: Some("#FF0000".to_string()),
        account_type: None,
    };

    let result = svc.account_service.create(req).await;
    assert!(result.is_ok(), "创建账号应成功: {:?}", result.err());
    let dto = result.unwrap();
    assert_eq!(dto.email, "test@gmail.com");
    assert_eq!(dto.name, "Test Account");
    assert!(dto.id > 0);
}

#[tokio::test]
async fn test_create_account_saves_password() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "Credential Account".to_string(),
        email: "credential@gmail.com".to_string(),
        display_name: None,
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "initial_password".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };

    let created = svc.account_service.create(req).await.unwrap();

    assert_eq!(
        svc.auth.get_password(&created.email).unwrap(),
        "initial_password"
    );
}

#[tokio::test]
async fn test_list_accounts_empty() {
    let svc = TestServices::new().await;
    let result = svc.account_service.list().await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_get_account_not_found() {
    let svc = TestServices::new().await;
    let result = svc.account_service.get(999).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_and_get_account() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "QQ Account".to_string(),
        email: "user@qq.com".to_string(),
        display_name: None,
        provider: "qq".to_string(),
        auth_type: "Password".to_string(),
        password: "password123".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };
    let created = svc.account_service.create(req).await.unwrap();
    let fetched = svc.account_service.get(created.id).await.unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.email, "user@qq.com");
}

#[tokio::test]
async fn test_update_account() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "Original".to_string(),
        email: "test@outlook.com".to_string(),
        display_name: None,
        provider: "outlook".to_string(),
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
    let created = svc.account_service.create(req).await.unwrap();

    use postium_mail_lib::service::account_service::UpdateAccountRequest;
    let update_req = UpdateAccountRequest {
        id: created.id,
        name: Some("Updated Name".to_string()),
        display_name: Some("Updated Display".to_string()),
        color: Some("#00FF00".to_string()),
        sync_enabled: None,
    };
    let updated = svc.account_service.update(update_req).await.unwrap();
    assert_eq!(updated.name, "Updated Name");
}

#[tokio::test]
async fn test_update_password_replaces_stored_password() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "Password Update".to_string(),
        email: "password-update@outlook.com".to_string(),
        display_name: None,
        provider: "outlook".to_string(),
        auth_type: "Password".to_string(),
        password: "old_password".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };
    let created = svc.account_service.create(req).await.unwrap();

    svc.account_service
        .update_password(created.id, "new_password".to_string())
        .await
        .unwrap();

    assert_eq!(
        svc.auth.get_password(&created.email).unwrap(),
        "new_password"
    );
}

#[tokio::test]
async fn test_delete_account() {
    let svc = TestServices::new().await;
    let req = CreateAccountRequest {
        name: "To Delete".to_string(),
        email: "delete@163.com".to_string(),
        display_name: None,
        provider: "163".to_string(),
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
    let created = svc.account_service.create(req).await.unwrap();
    let delete_result = svc.account_service.delete(created.id).await;
    assert!(
        delete_result.is_ok(),
        "删除账号应成功: {:?}",
        delete_result.err()
    );
    assert!(svc.account_service.get(created.id).await.is_err());
    assert!(svc.auth.get_password(&created.email).is_err());
}
