mod common;

use common::{TestEmail, TestServices, insert_test_email};
use postium_mail_lib::infrastructure::storage::entities::{
    attachments, email_labels, emails, labels, sync_errors, sync_state,
};
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::label_service::CreateLabelRequest;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, PaginatorTrait, QueryFilter, Set,
    Statement,
};

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

#[tokio::test]
async fn test_delete_account_removes_local_account_data_without_foreign_keys() {
    let svc = TestServices::new().await;

    let account_a = svc
        .account_service
        .create(CreateAccountRequest {
            name: "Delete A".to_string(),
            email: "delete-a@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password-a".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        })
        .await
        .unwrap();
    let account_b = svc
        .account_service
        .create(CreateAccountRequest {
            name: "Keep B".to_string(),
            email: "keep-b@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password-b".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        })
        .await
        .unwrap();

    let email_a = insert_test_email(&svc, account_a.id, TestEmail::new(901, "delete me")).await;
    let email_b = insert_test_email(&svc, account_b.id, TestEmail::new(902, "keep me")).await;

    let label_a = svc
        .label_service
        .create_label(CreateLabelRequest {
            account_id: account_a.id,
            name: "delete label".to_string(),
            color: "#ff0000".to_string(),
        })
        .await
        .unwrap();
    let label_b = svc
        .label_service
        .create_label(CreateLabelRequest {
            account_id: account_b.id,
            name: "keep label".to_string(),
            color: "#00ff00".to_string(),
        })
        .await
        .unwrap();

    svc.label_service
        .add_label_to_email(email_a, label_a.id)
        .await
        .unwrap();
    svc.label_service
        .add_label_to_email(email_b, label_b.id)
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    attachments::Entity::insert(attachments::ActiveModel {
        email_id: Set(email_a),
        filename: Set(Some("delete.txt".to_string())),
        content_type: Set(Some("text/plain".to_string())),
        size: Set(12),
        section_path: Set("2".to_string()),
        disposition: Set(Some("attachment".to_string())),
        content_id: Set(None),
        path: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    attachments::Entity::insert(attachments::ActiveModel {
        email_id: Set(email_b),
        filename: Set(Some("keep.txt".to_string())),
        content_type: Set(Some("text/plain".to_string())),
        size: Set(34),
        section_path: Set("2".to_string()),
        disposition: Set(Some("attachment".to_string())),
        content_id: Set(None),
        path: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();

    sync_state::Entity::insert(sync_state::ActiveModel {
        account_id: Set(account_a.id),
        folder: Set("INBOX".to_string()),
        folder_nick_name: Set(None),
        uidvalidity: Set(Some(1)),
        uidnext: Set(Some(2)),
        synced_at: Set(Some(now)),
        last_sync_uid: Set(Some(1)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    sync_errors::Entity::insert(sync_errors::ActiveModel {
        account_id: Set(account_a.id),
        folder: Set(Some("INBOX".to_string())),
        error_type: Set("test".to_string()),
        error_message: Set("delete error row".to_string()),
        uid: Set(Some(1)),
        stack_trace: Set(None),
        resolved: Set(Some(false)),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();

    svc.account_service.delete(account_a.id).await.unwrap();

    assert!(svc.account_service.get(account_a.id).await.is_err());
    assert!(svc.auth.get_password(&account_a.email).is_err());

    assert_eq!(
        emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        attachments::Entity::find()
            .filter(attachments::Column::EmailId.eq(email_a))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        labels::Entity::find()
            .filter(labels::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        email_labels::Entity::find()
            .filter(email_labels::Column::EmailId.eq(email_a))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        email_labels::Entity::find()
            .filter(email_labels::Column::LabelId.eq(label_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sync_errors::Entity::find()
            .filter(sync_errors::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );

    assert!(svc.account_service.get(account_b.id).await.is_ok());
    assert_eq!(
        emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_b.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        attachments::Entity::find()
            .filter(attachments::Column::EmailId.eq(email_b))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        labels::Entity::find()
            .filter(labels::Column::AccountId.eq(account_b.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn test_delete_account_rolls_back_local_data_when_account_row_delete_fails() {
    let svc = TestServices::new().await;

    let account = svc
        .account_service
        .create(CreateAccountRequest {
            name: "Rollback Delete".to_string(),
            email: "rollback-delete@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        })
        .await
        .unwrap();
    let email_id =
        insert_test_email(&svc, account.id, TestEmail::new(903, "keep after rollback")).await;
    let label = svc
        .label_service
        .create_label(CreateLabelRequest {
            account_id: account.id,
            name: "rollback label".to_string(),
            color: "#0000ff".to_string(),
        })
        .await
        .unwrap();
    svc.label_service
        .add_label_to_email(email_id, label.id)
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    attachments::Entity::insert(attachments::ActiveModel {
        email_id: Set(email_id),
        filename: Set(Some("rollback.txt".to_string())),
        content_type: Set(Some("text/plain".to_string())),
        size: Set(56),
        section_path: Set("2".to_string()),
        disposition: Set(Some("attachment".to_string())),
        content_id: Set(None),
        path: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    sync_state::Entity::insert(sync_state::ActiveModel {
        account_id: Set(account.id),
        folder: Set("INBOX".to_string()),
        folder_nick_name: Set(None),
        uidvalidity: Set(Some(1)),
        uidnext: Set(Some(2)),
        synced_at: Set(Some(now)),
        last_sync_uid: Set(Some(1)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    sync_errors::Entity::insert(sync_errors::ActiveModel {
        account_id: Set(account.id),
        folder: Set(Some("INBOX".to_string())),
        error_type: Set("test".to_string()),
        error_message: Set("rollback error row".to_string()),
        uid: Set(Some(1)),
        stack_trace: Set(None),
        resolved: Set(Some(false)),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();

    svc.db
        .execute_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                r#"
            CREATE TRIGGER fail_delete_account
            BEFORE DELETE ON accounts
            WHEN OLD.id = {}
            BEGIN
                SELECT RAISE(ABORT, 'forced account delete failure');
            END;
            "#,
                account.id
            ),
        ))
        .await
        .unwrap();

    assert!(svc.account_service.delete(account.id).await.is_err());

    assert!(svc.account_service.get(account.id).await.is_ok());
    assert_eq!(
        emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        attachments::Entity::find()
            .filter(attachments::Column::EmailId.eq(email_id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        labels::Entity::find()
            .filter(labels::Column::AccountId.eq(account.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        email_labels::Entity::find()
            .filter(email_labels::Column::EmailId.eq(email_id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sync_errors::Entity::find()
            .filter(sync_errors::Column::AccountId.eq(account.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(svc.auth.get_password(&account.email).unwrap(), "password");
}
