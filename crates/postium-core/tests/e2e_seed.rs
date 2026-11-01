mod common;

use common::TestServices;
use postium_core::domain::providers::pool::init_provider_pool;
use postium_core::infrastructure::storage::models::accounts;
use postium_core::infrastructure::testing::e2e_seed::seed_e2e_data;
use postium_core::service::email_service::EmailCategory;

#[tokio::test]
async fn test_seed_e2e_data_is_deterministic_and_idempotent() {
    let svc = TestServices::new().await;

    seed_e2e_data(&svc.db).await.unwrap();
    seed_e2e_data(&svc.db).await.unwrap();

    let account_count = count_all(&svc, "accounts").await;
    let email_count = count_all(&svc, "emails").await;

    assert_eq!(account_count, 2);
    // 48 固定夹具 + 5 封“今天锚定”日历夹具（UID 501–505）
    assert_eq!(email_count, 53);

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
        .list_by_category(primary.id, EmailCategory::Inbox, 1, 30, false)
        .await
        .unwrap();
    // 24 固定 INBOX + 5 封今日锚定日历夹具
    assert_eq!(inbox.total, 29);
    assert_eq!(inbox.emails.len(), 29);
    // 按时间倒序：最新一封是“后天 15:00”与“本月 15 日 12:00”两夹具中较晚者
    // （月初 1–12 号运行时本月 15 日更晚，月中之后“后天”更晚——期望随运行日期计算）
    assert_eq!(
        inbox
            .emails
            .first()
            .and_then(|email| email.subject.as_deref()),
        Some(expected_latest_calendar_subject())
    );
    assert!(
        inbox
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Primary Inbox Message 24"))
    );

    let sent = svc
        .email_service
        .list_by_category(primary.id, EmailCategory::Sent, 1, 10, false)
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
        .list_by_category(primary.id, EmailCategory::Starred, 1, 20, false)
        .await
        .unwrap();
    // 固定星标 8 封（INBOX 4 + Archive 4）+ 日历夹具 Beta 1 封
    assert_eq!(starred.total, 9);
    assert!(
        starred
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Starred Reference Message"))
    );
    assert!(
        starred
            .emails
            .iter()
            .any(|email| email.subject.as_deref() == Some("Calendar Today Beta Starred"))
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
        .list_by_category(secondary.id, EmailCategory::Inbox, 1, 20, false)
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

/// 按 [`seed_e2e_data`] 日历夹具的时间规则（后天 15:00 vs 本月 15 日 12:00），
/// 计算当前运行日期下 INBOX 最新一封日历夹具的主题。
fn expected_latest_calendar_subject() -> &'static str {
    use chrono::{Datelike, Duration, Local};

    let today = Local::now().date_naive();
    let midmonth = today.with_day(15).expect("构造本月 15 日失败");
    let midmonth_ts = midmonth
        .and_hms_opt(12, 0, 0)
        .expect("构造夹具时间戳失败");
    let future_ts = (today + Duration::days(2))
        .and_hms_opt(15, 0, 0)
        .expect("构造夹具时间戳失败");
    if midmonth_ts > future_ts {
        "Calendar Midmonth Epsilon"
    } else {
        "Calendar Future Delta"
    }
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
