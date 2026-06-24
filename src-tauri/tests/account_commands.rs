mod common;

use async_trait::async_trait;
use common::{TestEmail, TestServices, count_where, insert_test_email};
use postium_mail_lib::domain::providers::ImapServerConfig;
use postium_mail_lib::error::MailError;
use postium_mail_lib::service::account_connection::ImapConnectionVerifier;
use postium_mail_lib::service::account_service::{CreateAccountRequest, CreateOAuth2AccountParams};
use postium_mail_lib::service::label_service::CreateLabelRequest;
use std::sync::Arc;

struct FailingImapVerifier;

#[async_trait]
impl ImapConnectionVerifier for FailingImapVerifier {
    async fn verify(
        &self,
        _config: &ImapServerConfig,
        _email: &str,
        _password: &str,
    ) -> Result<(), MailError> {
        Err(MailError::AuthFailed("forced imap failure".to_string()))
    }
}

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
async fn test_create_account_does_not_persist_when_imap_verification_fails() {
    let svc = TestServices::new_with_imap_verifier(Arc::new(FailingImapVerifier)).await;
    let req = CreateAccountRequest {
        name: "Invalid Account".to_string(),
        email: "invalid@gmail.com".to_string(),
        display_name: None,
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "bad_password".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };

    let result = svc.account_service.create(req).await;

    assert!(matches!(result, Err(MailError::AuthFailed(_))));
    let count = svc
        .db
        .call(|conn| {
            conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| {
                row.get::<_, i64>(0)
            })
        })
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(svc.auth.get_password("invalid@gmail.com").is_err());
}

#[tokio::test]
async fn test_create_account_paths_persist_ssl_defaults() {
    let svc = TestServices::new().await;

    let created = svc
        .account_service
        .create(CreateAccountRequest {
            name: "SSL Default".to_string(),
            email: "ssl-default@gmail.com".to_string(),
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
        })
        .await
        .unwrap();

    let oauth_created = svc
        .account_service
        .create_oauth2_account(CreateOAuth2AccountParams {
            email: "oauth-ssl-default@gmail.com".to_string(),
            display_name: None,
            provider_id: "gmail".to_string(),
            imap_host: "imap.gmail.com".to_string(),
            imap_port: 993,
            imap_ssl_mode: "TLS".to_string(),
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 465,
            smtp_ssl_mode: "TLS".to_string(),
            color: None,
        })
        .await
        .unwrap();

    let ssl_flags = svc
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT imap_ssl, smtp_ssl FROM accounts WHERE id IN (?1, ?2) ORDER BY id ASC",
            )?;
            stmt.query_map(rusqlite::params![created.id, oauth_created.id], |row| {
                Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        })
        .await
        .unwrap();

    assert_eq!(ssl_flags, vec![(Some(1), Some(1)), (Some(1), Some(1))]);
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
        provider: "yi".to_string(),
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
    insert_attachment(&svc, email_a, "delete.txt", 12, now).await;
    insert_attachment(&svc, email_b, "keep.txt", 34, now).await;
    insert_sync_state(&svc, account_a.id, now).await;
    insert_sync_error(&svc, account_a.id, "delete error row", now).await;

    svc.account_service.delete(account_a.id).await.unwrap();

    assert!(svc.account_service.get(account_a.id).await.is_err());
    assert!(svc.auth.get_password(&account_a.email).is_err());

    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM emails WHERE account_id = ?1",
            account_a.id
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM attachments WHERE email_id = ?1",
            email_a
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM labels WHERE account_id = ?1",
            account_a.id
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM email_labels WHERE email_id = ?1",
            email_a
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM email_labels WHERE label_id = ?1",
            label_a.id
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM sync_state WHERE account_id = ?1",
            account_a.id
        )
        .await,
        0
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM sync_errors WHERE account_id = ?1",
            account_a.id
        )
        .await,
        0
    );

    assert!(svc.account_service.get(account_b.id).await.is_ok());
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM emails WHERE account_id = ?1",
            account_b.id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM attachments WHERE email_id = ?1",
            email_b
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM labels WHERE account_id = ?1",
            account_b.id
        )
        .await,
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
    insert_attachment(&svc, email_id, "rollback.txt", 56, now).await;
    insert_sync_state(&svc, account.id, now).await;
    insert_sync_error(&svc, account.id, "rollback error row", now).await;

    svc.db
        .call(move |conn| {
            conn.execute_batch(&format!(
                "CREATE TRIGGER fail_delete_account
                 BEFORE DELETE ON accounts
                 WHEN OLD.id = {}
                 BEGIN
                     SELECT RAISE(ABORT, 'forced account delete failure');
                 END;",
                account.id
            ))?;
            Ok(())
        })
        .await
        .unwrap();

    assert!(svc.account_service.delete(account.id).await.is_err());

    assert!(svc.account_service.get(account.id).await.is_ok());
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM emails WHERE account_id = ?1",
            account.id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM attachments WHERE email_id = ?1",
            email_id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM labels WHERE account_id = ?1",
            account.id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM email_labels WHERE email_id = ?1",
            email_id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM sync_state WHERE account_id = ?1",
            account.id
        )
        .await,
        1
    );
    assert_eq!(
        count_where(
            &svc,
            "SELECT COUNT(*) FROM sync_errors WHERE account_id = ?1",
            account.id
        )
        .await,
        1
    );
    assert_eq!(svc.auth.get_password(&account.email).unwrap(), "password");
}

async fn insert_attachment(
    svc: &TestServices,
    email_id: i32,
    filename: &'static str,
    size: i32,
    now: i64,
) {
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
                ) VALUES (?1, ?2, 'text/plain', ?3, '2', 'attachment', NULL, NULL, ?4)",
                rusqlite::params![email_id, filename, size, now],
            )?;
            Ok(())
        })
        .await
        .unwrap();
}

async fn insert_sync_state(svc: &TestServices, account_id: i32, now: i64) {
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO sync_state (
                    account_id, folder, folder_nick_name, uidvalidity, uidnext,
                    synced_at, last_sync_uid, created_at, updated_at
                ) VALUES (?1, 'INBOX', NULL, 1, 2, ?2, 1, ?2, ?2)",
                rusqlite::params![account_id, now],
            )?;
            Ok(())
        })
        .await
        .unwrap();
}

async fn insert_sync_error(svc: &TestServices, account_id: i32, message: &'static str, now: i64) {
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO sync_errors (
                    account_id, folder, error_type, error_message, uid, stack_trace, resolved, created_at
                ) VALUES (?1, 'INBOX', 'test', ?2, 1, NULL, 0, ?3)",
                rusqlite::params![account_id, message, now],
            )?;
            Ok(())
        })
        .await
        .unwrap();
}
