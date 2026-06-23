mod common;

use common::TestServices;
use postium_mail_lib::domain::providers::pool::init_provider_pool;
use postium_mail_lib::infrastructure::storage::models::accounts;
use postium_mail_lib::infrastructure::testing::e2e_seed::seed_e2e_data;
use postium_mail_lib::service::email_service::EmailCategory;

#[tokio::test]
async fn test_seed_e2e_data_is_deterministic_and_idempotent() {
    let svc = TestServices::new().await;

    seed_e2e_data(&svc.db).await.unwrap();
    seed_e2e_data(&svc.db).await.unwrap();

    let account_count = count_all(&svc, "accounts").await;
    let email_count = count_all(&svc, "emails").await;

    assert_eq!(account_count, 2);
    assert_eq!(email_count, 48);

    let primary = account_by_email(&svc, "primary.e2e@postium.test").await;
    let secondary = account_by_email(&svc, "secondary.e2e@postium.test").await;

    assert_eq!(primary.provider, "custom");
    assert_eq!(secondary.provider, "custom");
    assert_eq!(primary.sync_enabled, Some(false));
    assert_eq!(secondary.sync_enabled, Some(false));
}

#[tokio::test]
async fn test_seed_data_supports_category_queries_and_search() {
    init_provider_pool();

    let svc = TestServices::new().await;
    seed_e2e_data(&svc.db).await.unwrap();

    let primary = account_by_email(&svc, "primary.e2e@postium.test").await;

    let inbox = svc
        .email_service
        .list_by_category(primary.id, EmailCategory::Inbox, 1, 30)
        .await
        .unwrap();
    assert_eq!(inbox.total, 24);
    assert_eq!(inbox.emails.len(), 24);
    assert_eq!(
        inbox
            .emails
            .first()
            .and_then(|email| email.subject.as_deref()),
        Some("Primary Inbox Message 01")
    );
    assert!(
        inbox
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Primary Inbox Message 24"))
    );

    let sent = svc
        .email_service
        .list_by_category(primary.id, EmailCategory::Sent, 1, 10)
        .await
        .unwrap();
    assert_eq!(sent.total, 5);
    assert!(
        sent.emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Sent Confirmation Message"))
    );

    let starred = svc
        .email_service
        .list_by_category(primary.id, EmailCategory::Starred, 1, 20)
        .await
        .unwrap();
    assert_eq!(starred.total, 8);
    assert!(
        starred
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Starred Reference Message"))
    );

    let account_id = primary.id;
    let quarterly_count: i64 = svc
        .db
        .call(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE account_id = ?1 AND subject LIKE ?2",
                rusqlite::params![account_id, "%Quarterly Planning%"],
                |row| row.get(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(quarterly_count, 3);

    let search_results = svc
        .email_service
        .search("Quarterly", Some(primary.id), Some(10))
        .await
        .unwrap();
    let subjects = search_results
        .iter()
        .filter_map(|email| email.subject.as_deref())
        .collect::<Vec<_>>();

    assert!(subjects.contains(&"Quarterly Planning Alpha"));
    assert!(subjects.contains(&"Quarterly Planning Beta"));
    assert!(subjects.contains(&"Quarterly Planning Archive"));
}

#[tokio::test]
async fn test_seed_data_is_isolated_by_account() {
    init_provider_pool();

    let svc = TestServices::new().await;
    seed_e2e_data(&svc.db).await.unwrap();

    let secondary = account_by_email(&svc, "secondary.e2e@postium.test").await;

    let inbox = svc
        .email_service
        .list_by_category(secondary.id, EmailCategory::Inbox, 1, 20)
        .await
        .unwrap();

    assert_eq!(inbox.total, 8);
    assert!(
        inbox
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Secondary Inbox Message 01"))
    );
    assert!(
        inbox
            .emails
            .iter()
            .all(|email| email.account_id == secondary.id)
    );
}

async fn count_all(svc: &TestServices, table: &'static str) -> i64 {
    svc.db
        .call(move |conn| {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            conn.query_row(&sql, [], |row| row.get(0))
        })
        .await
        .unwrap()
}

async fn account_by_email(svc: &TestServices, email: &'static str) -> accounts::Model {
    svc.db
        .call(move |conn| {
            conn.query_row(
                "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                        imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                        sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
                 FROM accounts
                 WHERE email = ?1",
                [email],
                |row| {
                    Ok(accounts::Model {
                        id: row.get("id")?,
                        name: row.get("name")?,
                        email: row.get("email")?,
                        display_name: row.get("display_name")?,
                        provider: row.get("provider")?,
                        imap_host: row.get("imap_host")?,
                        imap_port: row.get("imap_port")?,
                        imap_ssl: row
                            .get::<_, Option<i64>>("imap_ssl")?
                            .map(|value| value != 0),
                        imap_ssl_mode: row.get("imap_ssl_mode")?,
                        smtp_host: row.get("smtp_host")?,
                        smtp_port: row.get("smtp_port")?,
                        smtp_ssl: row
                            .get::<_, Option<i64>>("smtp_ssl")?
                            .map(|value| value != 0),
                        smtp_ssl_mode: row.get("smtp_ssl_mode")?,
                        color: row.get("color")?,
                        sync_enabled: row
                            .get::<_, Option<i64>>("sync_enabled")?
                            .map(|value| value != 0),
                        last_sync_at: row.get("last_sync_at")?,
                        auth_type: row.get("auth_type")?,
                        account_type: row.get("account_type")?,
                        created_at: row.get("created_at")?,
                        updated_at: row.get("updated_at")?,
                    })
                },
            )
        })
        .await
        .unwrap()
}
