use postium_mail_lib::domain::providers::pool::init_provider_pool;
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::service::SyncService;
use postium_mail_lib::service::email_service::EmailCategory;
use std::sync::Arc;

async fn seed_account(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Gmail', 'user@gmail.com', 'gmail', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn history_state_should_reject_starred_category() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service.get_history_state(1, EmailCategory::Starred).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn history_state_should_return_folder_state_for_inbox() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service
        .get_history_state(1, EmailCategory::Inbox)
        .await
        .unwrap();

    assert_eq!(result.account_id, 1);
    assert_eq!(result.category, EmailCategory::Inbox);
    assert!(!result.folders.is_empty());
    assert!(!result.history_exhausted);
}

#[tokio::test]
async fn history_state_should_resolve_archive_alias_to_existing_local_folder() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    db.call(|conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, uidvalidity, uidnext, last_sync_uid, synced_at, created_at, updated_at
            ) VALUES (1, 'Archive', 1, 11, 10, 1, 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service
        .get_history_state(1, EmailCategory::Archive)
        .await
        .unwrap();

    assert_eq!(result.folders, vec!["Archive".to_string()]);
}

#[tokio::test]
async fn history_state_should_fallback_archive_alias_to_first_candidate() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service
        .get_history_state(1, EmailCategory::Archive)
        .await
        .unwrap();

    assert_eq!(result.folders, vec!["[Gmail]/All Mail".to_string()]);
}
