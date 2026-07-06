use chrono::{TimeZone, Utc};
use postium_mail_lib::infrastructure::protocols::types::{
    AttachmentInfo, EmailFlags, EmailHeader, WholeEmailDto,
};
use postium_mail_lib::infrastructure::storage::database::DbConn;
use postium_mail_lib::infrastructure::storage::repository::email_repo;

async fn seed_account(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Test', 'test@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn seed_two_accounts(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, display_name, provider, auth_type, account_type, created_at, updated_at
            ) VALUES
                (1, 'Work', 'work@example.com', 'Work Mail', 'gmail', 'password', 'work', 1, 1),
                (2, 'Personal', 'personal@example.com', 'Personal Mail', 'outlook', 'password', 'personal', 2, 2)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn insert_repo_email(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: u32,
    subject: &str,
    sent_at: i64,
    is_read: bool,
    is_starred: bool,
) {
    let folder = folder.to_string();
    let subject = subject.to_string();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO emails (
                account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'Sender', 'sender@example.com',
                'to@example.com', '', '', '', ?6, ?7, 0, 0, 0, ?8, ?8, ?8, ?8)",
            rusqlite::params![
                account_id,
                folder,
                uid,
                format!("<repo-{account_id}-{uid}@example.com>"),
                subject,
                if is_read { 1 } else { 0 },
                if is_starred { 1 } else { 0 },
                sent_at,
            ],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

fn header(uid: u32, subject: &str, sent_at: i64) -> EmailHeader {
    EmailHeader {
        uid,
        subject: subject.to_string(),
        from: "Sender <sender@example.com>".to_string(),
        to: "to@example.com".to_string(),
        cc: String::new(),
        date: Utc.timestamp_opt(sent_at, 0).single().unwrap(),
        flags: EmailFlags {
            seen: false,
            flagged: false,
            answered: false,
            deleted: false,
            draft: false,
            recent: false,
        },
        attachments: vec![],
    }
}

fn whole_email(uid: u32, body_text: &str, body_html: &str) -> WholeEmailDto {
    WholeEmailDto {
        id: 0,
        account_id: 0,
        folder: "INBOX".to_string(),
        uid,
        message_id: Some(format!("<{uid}@example.com>")),
        sender_name: Some("Sender".to_string()),
        sender_email: "sender@example.com".to_string(),
        recipient_emails: "to@example.com".to_string(),
        cc_emails: None,
        bcc_emails: None,
        subject: Some(format!("subject-{uid}")),
        preview: Some(body_text.chars().take(20).collect()),
        body_text: Some(body_text.to_string()),
        body_html: Some(body_html.to_string()),
        attachments: vec![AttachmentInfo {
            filename: Some(format!("att-{uid}.txt")),
            content_type: "text/plain".to_string(),
            size: 12,
            section_path: "2".to_string(),
            disposition: Some("attachment".to_string()),
            content_id: None,
        }],
        is_read: false,
        is_starred: false,
        is_draft: false,
        is_answered: false,
        is_deleted: false,
        sent_at: 2_000 + i64::from(uid),
        received_at: 2_000 + i64::from(uid),
        created_at: 0,
    }
}

#[tokio::test]
async fn save_batch_email_headers_should_ignore_existing_account_folder_uid() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let first = vec![header(100, "first", 1000)];
    let duplicate = vec![header(100, "duplicate", 1000)];

    let first_count = email_repo::save_batch_email_headers(&db, 1, "INBOX", &first)
        .await
        .unwrap();
    let second_count = email_repo::save_batch_email_headers(&db, 1, "INBOX", &duplicate)
        .await
        .unwrap();

    let row_count = db
        .call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM emails WHERE account_id = 1 AND folder = 'INBOX' AND uid = 100",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .unwrap();

    assert_eq!(first_count, 1);
    assert_eq!(second_count, 0);
    assert_eq!(row_count, 1);
}

#[tokio::test]
async fn save_batch_emails_should_upsert_existing_header_and_replace_attachments() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    email_repo::save_batch_email_headers(&db, 1, "INBOX", &[header(200, "header only", 3000)])
        .await
        .unwrap();

    let email = whole_email(200, "body text", "<p>body text</p>");
    let saved = email_repo::save_batch_emails(&db, 1, "INBOX", &[email])
        .await
        .unwrap();

    let (body_text, body_html, attachment_count) = db
        .call(|conn| {
            conn.query_row(
                "SELECT body_text, body_html, (
                    SELECT COUNT(*) FROM attachments WHERE email_id = emails.id
                 ) AS attachment_count
                 FROM emails
                 WHERE account_id = 1 AND folder = 'INBOX' AND uid = 200",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
        })
        .await
        .unwrap();

    assert_eq!(saved, 1);
    assert_eq!(body_text.as_deref(), Some("body text"));
    assert_eq!(body_html.as_deref(), Some("<p>body text</p>"));
    assert_eq!(attachment_count, 1);
}

#[tokio::test]
async fn save_batch_emails_should_return_affected_row_count_for_repeated_upserts() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let first_saved = email_repo::save_batch_emails(
        &db,
        1,
        "INBOX",
        &[whole_email(201, "first body", "<p>first body</p>")],
    )
    .await
    .unwrap();

    let second_saved = email_repo::save_batch_emails(
        &db,
        1,
        "INBOX",
        &[whole_email(201, "updated body", "<p>updated body</p>")],
    )
    .await
    .unwrap();

    let (row_count, body_text) = db
        .call(|conn| {
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM emails WHERE account_id = 1 AND folder = 'INBOX' AND uid = 201),
                    body_text
                 FROM emails
                 WHERE account_id = 1 AND folder = 'INBOX' AND uid = 201",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
        })
        .await
        .unwrap();

    assert_eq!(first_saved, 1);
    assert_eq!(second_saved, 1);
    assert_eq!(row_count, 1);
    assert_eq!(body_text.as_deref(), Some("updated body"));
}

#[tokio::test]
async fn earliest_sent_at_by_folder_should_return_oldest_local_email() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    email_repo::save_batch_email_headers(
        &db,
        1,
        "INBOX",
        &[header(101, "newer", 3000), header(102, "older", 1000)],
    )
    .await
    .unwrap();

    let earliest = email_repo::earliest_sent_at_by_folder(&db, 1, "INBOX")
        .await
        .unwrap();

    assert_eq!(earliest, Some(1000));
}

#[tokio::test]
async fn earliest_sent_at_by_folder_should_return_none_for_empty_folder() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let earliest = email_repo::earliest_sent_at_by_folder(&db, 1, "Archive")
        .await
        .unwrap();

    assert_eq!(earliest, None);
}

#[tokio::test]
async fn earliest_sent_at_by_folder_should_ignore_deleted_emails() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let mut deleted = header(301, "deleted", 500);
    deleted.flags.deleted = true;

    email_repo::save_batch_email_headers(
        &db,
        1,
        "INBOX",
        &[
            deleted,
            header(302, "kept", 1000),
            header(303, "newer", 3000),
        ],
    )
    .await
    .unwrap();

    let earliest = email_repo::earliest_sent_at_by_folder(&db, 1, "INBOX")
        .await
        .unwrap();

    assert_eq!(earliest, Some(1000));
}

#[tokio::test]
async fn min_uid_by_folder_should_return_smallest_local_uid() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    email_repo::save_batch_email_headers(
        &db,
        1,
        "INBOX",
        &[
            header(402, "newer uid", 3000),
            header(300, "older uid", 1000),
            header(350, "middle uid", 2000),
        ],
    )
    .await
    .unwrap();
    email_repo::save_batch_email_headers(&db, 1, "Archive", &[header(10, "archive", 1000)])
        .await
        .unwrap();

    let inbox_min_uid = email_repo::min_uid_by_folder(&db, 1, "INBOX")
        .await
        .unwrap();
    let archive_min_uid = email_repo::min_uid_by_folder(&db, 1, "Archive")
        .await
        .unwrap();
    let empty_min_uid = email_repo::min_uid_by_folder(&db, 1, "Sent").await.unwrap();

    assert_eq!(inbox_min_uid, Some(300));
    assert_eq!(archive_min_uid, Some(10));
    assert_eq!(empty_min_uid, None);
}

#[tokio::test]
async fn distinct_folders_by_account_should_return_existing_non_deleted_folders() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;

    let mut deleted = header(401, "deleted archive", 500);
    deleted.flags.deleted = true;
    email_repo::save_batch_email_headers(&db, 1, "INBOX", &[header(402, "kept", 1000)])
        .await
        .unwrap();
    email_repo::save_batch_email_headers(&db, 1, "Archive", &[deleted])
        .await
        .unwrap();

    let folders = email_repo::distinct_folders_by_account(&db, 1)
        .await
        .unwrap();

    assert_eq!(folders, vec!["INBOX".to_string()]);
}

#[tokio::test]
async fn list_by_account_folder_filters_should_merge_sort_and_page_across_accounts() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(&db, 1, "INBOX", 101, "older work inbox", 100, false, false).await;
    insert_repo_email(
        &db,
        2,
        "Inbox",
        201,
        "newer personal inbox",
        300,
        false,
        false,
    )
    .await;
    insert_repo_email(&db, 1, "Sent", 102, "work sent ignored", 400, false, false).await;

    let (items, total) = email_repo::list_by_account_folder_filters(
        &db,
        vec![
            email_repo::AccountFolderFilter {
                account_id: 1,
                folders: vec!["INBOX".to_string()],
            },
            email_repo::AccountFolderFilter {
                account_id: 2,
                folders: vec!["Inbox".to_string()],
            },
        ],
        1,
        1,
        false,
    )
    .await
    .unwrap();

    assert_eq!(total, 2);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("newer personal inbox"));
    assert_eq!(items[0].account_id, 2);
}

#[tokio::test]
async fn list_by_account_folder_filters_should_apply_unread_filter() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(&db, 1, "INBOX", 101, "read work inbox", 100, true, false).await;
    insert_repo_email(
        &db,
        2,
        "Inbox",
        201,
        "unread personal inbox",
        200,
        false,
        false,
    )
    .await;
    insert_repo_email(
        &db,
        2,
        "Archive",
        202,
        "unread archive ignored",
        300,
        false,
        false,
    )
    .await;

    let (items, total) = email_repo::list_by_account_folder_filters(
        &db,
        vec![
            email_repo::AccountFolderFilter {
                account_id: 1,
                folders: vec!["INBOX".to_string()],
            },
            email_repo::AccountFolderFilter {
                account_id: 2,
                folders: vec!["Inbox".to_string()],
            },
        ],
        1,
        50,
        true,
    )
    .await
    .unwrap();

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("unread personal inbox"));
    assert_eq!(items[0].account_id, 2);
}

#[tokio::test]
async fn list_by_account_folder_filters_should_use_stable_tie_breaker_for_equal_sent_at() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(
        &db,
        1,
        "INBOX",
        101,
        "older inserted inbox",
        200,
        false,
        false,
    )
    .await;
    insert_repo_email(
        &db,
        2,
        "Inbox",
        201,
        "newer inserted inbox",
        200,
        false,
        false,
    )
    .await;

    let (items, total) = email_repo::list_by_account_folder_filters(
        &db,
        vec![
            email_repo::AccountFolderFilter {
                account_id: 1,
                folders: vec!["INBOX".to_string()],
            },
            email_repo::AccountFolderFilter {
                account_id: 2,
                folders: vec!["Inbox".to_string()],
            },
        ],
        1,
        1,
        false,
    )
    .await
    .unwrap();

    assert_eq!(total, 2);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("newer inserted inbox"));
    assert_eq!(items[0].account_id, 2);
}

#[tokio::test]
async fn list_starred_all_accounts_should_apply_unread_filter() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(&db, 1, "INBOX", 101, "read starred", 100, true, true).await;
    insert_repo_email(&db, 2, "Archive", 201, "unread starred", 200, false, true).await;

    let (items, total) = email_repo::list_starred_all_accounts(&db, 1, 50, true)
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("unread starred"));
    assert_eq!(items[0].account_id, 2);
}
