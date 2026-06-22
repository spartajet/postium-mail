use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::entities::{accounts, emails};
use crate::infrastructure::storage::repository::account_repo;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, EntityTrait, Set};

const PRIMARY_EMAIL: &str = "primary.e2e@postium.test";
const SECONDARY_EMAIL: &str = "secondary.e2e@postium.test";
const BASE_TS: i64 = 1_779_936_000;

pub async fn seed_e2e_data(db: &DbConn) -> Result<(), MailError> {
    if account_repo::get_by_email(db, PRIMARY_EMAIL)
        .await?
        .is_some()
    {
        return Ok(());
    }

    let now = BASE_TS;
    let primary = accounts::ActiveModel {
        id: NotSet,
        name: Set("Primary E2E".to_string()),
        email: Set(PRIMARY_EMAIL.to_string()),
        display_name: Set(Some("Primary E2E".to_string())),
        provider: Set("custom".to_string()),
        imap_host: Set(Some("imap.postium.test".to_string())),
        imap_port: Set(Some(993)),
        imap_ssl: Set(Some(true)),
        imap_ssl_mode: Set(Some("ssl".to_string())),
        smtp_host: Set(Some("smtp.postium.test".to_string())),
        smtp_port: Set(Some(465)),
        smtp_ssl: Set(Some(true)),
        smtp_ssl_mode: Set(Some("ssl".to_string())),
        color: Set(Some("#2563eb".to_string())),
        sync_enabled: Set(Some(false)),
        last_sync_at: Set(None),
        auth_type: Set(Some("Password".to_string())),
        account_type: Set("personal".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;

    let secondary = accounts::ActiveModel {
        id: NotSet,
        name: Set("Secondary E2E".to_string()),
        email: Set(SECONDARY_EMAIL.to_string()),
        display_name: Set(Some("Secondary E2E".to_string())),
        provider: Set("custom".to_string()),
        imap_host: Set(Some("imap.postium.test".to_string())),
        imap_port: Set(Some(993)),
        imap_ssl: Set(Some(true)),
        imap_ssl_mode: Set(Some("ssl".to_string())),
        smtp_host: Set(Some("smtp.postium.test".to_string())),
        smtp_port: Set(Some(465)),
        smtp_ssl: Set(Some(true)),
        smtp_ssl_mode: Set(Some("ssl".to_string())),
        color: Set(Some("#16a34a".to_string())),
        sync_enabled: Set(Some(false)),
        last_sync_at: Set(None),
        auth_type: Set(Some("Password".to_string())),
        account_type: Set("personal".to_string()),
        created_at: Set(now + 1),
        updated_at: Set(now + 1),
    }
    .insert(db)
    .await?;

    let mut fixtures = Vec::new();
    fixtures.extend(primary_emails(primary.id));
    fixtures.extend(secondary_emails(secondary.id));

    emails::Entity::insert_many(fixtures).exec(db).await?;
    Ok(())
}

fn primary_emails(account_id: i32) -> Vec<emails::ActiveModel> {
    let mut items = Vec::new();

    let special_subjects = [
        (5, "Quarterly Planning Alpha"),
        (8, "Quarterly Planning Beta"),
        (16, "Quarterly Planning Archive"),
        (24, "Primary Inbox Message 24"),
    ];

    for idx in 1..=24 {
        let subject = special_subjects
            .iter()
            .find_map(|(special_idx, subject)| (*special_idx == idx).then_some(*subject))
            .unwrap_or_else(|| {
                if idx == 1 {
                    "Primary Inbox Message 01"
                } else {
                    "Primary Inbox Message"
                }
            });
        let subject = if subject == "Primary Inbox Message" {
            format!("Primary Inbox Message {idx:02}")
        } else {
            subject.to_string()
        };

        items.push(email_model(
            account_id,
            "INBOX",
            idx,
            &subject,
            idx <= 16,
            matches!(idx, 3 | 6 | 9 | 12),
            BASE_TS - i64::from(idx),
        ));
    }

    for idx in 1..=5 {
        let subject = if idx == 1 {
            "Sent Confirmation Message".to_string()
        } else {
            format!("Primary Sent Message {idx:02}")
        };
        items.push(email_model(
            account_id,
            "Sent",
            100 + idx,
            &subject,
            true,
            false,
            BASE_TS - 100 - i64::from(idx),
        ));
    }

    for idx in 1..=4 {
        let subject = if idx == 1 {
            "Starred Reference Message".to_string()
        } else {
            format!("Primary Starred Message {idx:02}")
        };
        items.push(email_model(
            account_id,
            "Archive",
            200 + idx,
            &subject,
            true,
            true,
            BASE_TS - 200 - i64::from(idx),
        ));
    }

    for idx in 1..=2 {
        let subject = if idx == 1 {
            "Draft Proposal Outline".to_string()
        } else {
            "Primary Draft Message 02".to_string()
        };
        items.push(email_model(
            account_id,
            "Drafts",
            300 + idx,
            &subject,
            true,
            false,
            BASE_TS - 300 - i64::from(idx),
        ));
    }

    items.push(email_model(
        account_id,
        "Trash",
        401,
        "Trash Cleanup Notice",
        true,
        false,
        BASE_TS - 401,
    ));

    items
}

fn secondary_emails(account_id: i32) -> Vec<emails::ActiveModel> {
    let mut items = Vec::new();

    for idx in 1..=8 {
        items.push(email_model(
            account_id,
            "INBOX",
            idx,
            &format!("Secondary Inbox Message {idx:02}"),
            idx <= 4,
            idx == 2,
            BASE_TS - 1_000 - i64::from(idx),
        ));
    }

    for idx in 1..=2 {
        items.push(email_model(
            account_id,
            "Sent",
            100 + idx,
            &format!("Secondary Sent Message {idx:02}"),
            true,
            false,
            BASE_TS - 1_100 - i64::from(idx),
        ));
    }

    items.push(email_model(
        account_id,
        "Archive",
        201,
        "Secondary Starred Message 01",
        true,
        true,
        BASE_TS - 1_201,
    ));

    items.push(email_model(
        account_id,
        "Drafts",
        301,
        "Secondary Draft Message 01",
        true,
        false,
        BASE_TS - 1_301,
    ));

    items
}

fn email_model(
    account_id: i32,
    folder: &str,
    uid: u32,
    subject: &str,
    is_read: bool,
    is_starred: bool,
    sent_at: i64,
) -> emails::ActiveModel {
    let is_secondary = subject.starts_with("Secondary");
    let sender_name = if is_secondary {
        "Secondary Sender"
    } else {
        "Primary Sender"
    };
    let sender_email = if is_secondary {
        "sender.secondary@postium.test"
    } else {
        "sender.primary@postium.test"
    };
    let body = format!("This is deterministic E2E body content for {subject}.");

    emails::ActiveModel {
        id: NotSet,
        account_id: Set(account_id),
        folder: Set(folder.to_string()),
        uid: Set(uid),
        message_id: Set(Some(format!(
            "<e2e-{account_id}-{folder}-{uid}@postium.test>"
        ))),
        subject: Set(Some(subject.to_string())),
        sender_name: Set(Some(sender_name.to_string())),
        sender_email: Set(sender_email.to_string()),
        recipient_emails: Set("e2e.user@postium.test".to_string()),
        cc_emails: Set(None),
        bcc_emails: Set(None),
        preview: Set(Some(format!("Preview for {subject}"))),
        body_text: Set(Some(body.clone())),
        body_html: Set(Some(format!("<p>{body}</p>"))),
        is_read: Set(Some(is_read)),
        is_starred: Set(Some(is_starred)),
        is_draft: Set(Some(folder == "Drafts")),
        is_answered: Set(Some(false)),
        is_deleted: Set(Some(folder == "Trash")),
        sent_at: Set(sent_at),
        received_at: Set(sent_at),
        created_at: Set(sent_at),
        updated_at: Set(sent_at),
    }
}
