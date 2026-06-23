use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::{
    account_repo::{self, AccountWrite},
    email_repo::{self, EmailWrite},
};

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
    let primary = account_repo::create(
        db,
        AccountWrite {
            name: "Primary E2E".to_string(),
            email: PRIMARY_EMAIL.to_string(),
            display_name: Some("Primary E2E".to_string()),
            provider: "custom".to_string(),
            imap_host: Some("imap.postium.test".to_string()),
            imap_port: Some(993),
            imap_ssl: Some(true),
            imap_ssl_mode: Some("ssl".to_string()),
            smtp_host: Some("smtp.postium.test".to_string()),
            smtp_port: Some(465),
            smtp_ssl: Some(true),
            smtp_ssl_mode: Some("ssl".to_string()),
            color: Some("#2563eb".to_string()),
            sync_enabled: Some(false),
            last_sync_at: None,
            auth_type: Some("Password".to_string()),
            account_type: "personal".to_string(),
            created_at: now,
            updated_at: now,
        },
    )
    .await?;

    let secondary = account_repo::create(
        db,
        AccountWrite {
            name: "Secondary E2E".to_string(),
            email: SECONDARY_EMAIL.to_string(),
            display_name: Some("Secondary E2E".to_string()),
            provider: "custom".to_string(),
            imap_host: Some("imap.postium.test".to_string()),
            imap_port: Some(993),
            imap_ssl: Some(true),
            imap_ssl_mode: Some("ssl".to_string()),
            smtp_host: Some("smtp.postium.test".to_string()),
            smtp_port: Some(465),
            smtp_ssl: Some(true),
            smtp_ssl_mode: Some("ssl".to_string()),
            color: Some("#16a34a".to_string()),
            sync_enabled: Some(false),
            last_sync_at: None,
            auth_type: Some("Password".to_string()),
            account_type: "personal".to_string(),
            created_at: now + 1,
            updated_at: now + 1,
        },
    )
    .await?;

    let mut fixtures = Vec::new();
    fixtures.extend(primary_emails(primary.id));
    fixtures.extend(secondary_emails(secondary.id));

    email_repo::bulk_insert(db, fixtures).await?;
    Ok(())
}

fn primary_emails(account_id: i32) -> Vec<EmailWrite> {
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

fn secondary_emails(account_id: i32) -> Vec<EmailWrite> {
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
) -> EmailWrite {
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

    EmailWrite {
        account_id,
        folder: folder.to_string(),
        uid,
        message_id: Some(format!("<e2e-{account_id}-{folder}-{uid}@postium.test>")),
        subject: Some(subject.to_string()),
        sender_name: Some(sender_name.to_string()),
        sender_email: sender_email.to_string(),
        recipient_emails: "e2e.user@postium.test".to_string(),
        cc_emails: None,
        bcc_emails: None,
        preview: Some(format!("Preview for {subject}")),
        body_text: Some(body.clone()),
        body_html: Some(format!("<p>{body}</p>")),
        is_read: Some(is_read),
        is_starred: Some(is_starred),
        is_draft: Some(folder == "Drafts"),
        is_answered: Some(false),
        is_deleted: Some(folder == "Trash"),
        sent_at,
        received_at: sent_at,
        created_at: sent_at,
        updated_at: sent_at,
    }
}
