mod common;

use common::TestServices;
use postium_mail_lib::domain::providers::pool::init_provider_pool;
use postium_mail_lib::infrastructure::storage::models::{accounts, emails};
use postium_mail_lib::infrastructure::testing::e2e_seed::seed_e2e_data;
use postium_mail_lib::service::email_service::EmailCategory;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

#[tokio::test]
async fn test_seed_e2e_data_is_deterministic_and_idempotent() {
    let svc = TestServices::new().await;

    seed_e2e_data(&svc.db).await.unwrap();
    seed_e2e_data(&svc.db).await.unwrap();

    let account_count = accounts::Entity::find().count(&svc.db).await.unwrap();
    let email_count = emails::Entity::find().count(&svc.db).await.unwrap();

    assert_eq!(account_count, 2);
    assert_eq!(email_count, 48);

    let primary = accounts::Entity::find()
        .filter(accounts::Column::Email.eq("primary.e2e@postium.test"))
        .one(&svc.db)
        .await
        .unwrap()
        .unwrap();
    let secondary = accounts::Entity::find()
        .filter(accounts::Column::Email.eq("secondary.e2e@postium.test"))
        .one(&svc.db)
        .await
        .unwrap()
        .unwrap();

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

    let primary = accounts::Entity::find()
        .filter(accounts::Column::Email.eq("primary.e2e@postium.test"))
        .one(&svc.db)
        .await
        .unwrap()
        .unwrap();

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

    let quarterly_count = emails::Entity::find()
        .filter(emails::Column::AccountId.eq(primary.id))
        .filter(emails::Column::Subject.contains("Quarterly Planning"))
        .count(&svc.db)
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

    let secondary = accounts::Entity::find()
        .filter(accounts::Column::Email.eq("secondary.e2e@postium.test"))
        .one(&svc.db)
        .await
        .unwrap()
        .unwrap();

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
