use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::email_repo::EmailWrite;

const PRIMARY_EMAIL: &str = "primary.e2e@postium.test";
const SECONDARY_EMAIL: &str = "secondary.e2e@postium.test";
const BASE_TS: i64 = 1_779_936_000;

pub async fn seed_e2e_data(db: &DbConn) -> Result<(), MailError> {
    let now = BASE_TS;
    db.transaction(move |tx| {
        insert_account(
            tx,
            AccountSeed {
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
        )?;

        insert_account(
            tx,
            AccountSeed {
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
        )?;

        let primary_id = account_id_by_email(tx, PRIMARY_EMAIL)?;
        let secondary_id = account_id_by_email(tx, SECONDARY_EMAIL)?;

        let mut fixtures = Vec::new();
        fixtures.extend(primary_emails(primary_id));
        fixtures.extend(secondary_emails(secondary_id));

        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO emails (
                account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )",
        )?;

        for fixture in &fixtures {
            insert_email(&mut stmt, fixture)?;
        }

        Ok(())
    })
    .await
}

struct AccountSeed {
    name: String,
    email: String,
    display_name: Option<String>,
    provider: String,
    imap_host: Option<String>,
    imap_port: Option<i32>,
    imap_ssl: Option<bool>,
    imap_ssl_mode: Option<String>,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_ssl: Option<bool>,
    smtp_ssl_mode: Option<String>,
    color: Option<String>,
    sync_enabled: Option<bool>,
    last_sync_at: Option<i64>,
    auth_type: Option<String>,
    account_type: String,
    created_at: i64,
    updated_at: i64,
}

fn insert_account(tx: &rusqlite::Transaction<'_>, account: AccountSeed) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO accounts (
            name, email, display_name, provider, imap_host, imap_port, imap_ssl,
            imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
            sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        rusqlite::params![
            account.name,
            account.email,
            account.display_name,
            account.provider,
            account.imap_host,
            account.imap_port,
            opt_bool_to_int(account.imap_ssl),
            account.imap_ssl_mode,
            account.smtp_host,
            account.smtp_port,
            opt_bool_to_int(account.smtp_ssl),
            account.smtp_ssl_mode,
            account.color,
            opt_bool_to_int(account.sync_enabled),
            account.last_sync_at,
            account.auth_type,
            account.account_type,
            account.created_at,
            account.updated_at,
        ],
    )?;
    Ok(())
}

fn account_id_by_email(tx: &rusqlite::Transaction<'_>, email: &str) -> rusqlite::Result<i32> {
    tx.query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
        row.get(0)
    })
}

fn insert_email(stmt: &mut rusqlite::Statement<'_>, email: &EmailWrite) -> rusqlite::Result<()> {
    stmt.execute(rusqlite::params![
        email.account_id,
        &email.folder,
        i64::from(email.uid),
        &email.message_id,
        &email.subject,
        &email.sender_name,
        &email.sender_email,
        &email.recipient_emails,
        &email.cc_emails,
        &email.bcc_emails,
        &email.preview,
        &email.body_text,
        &email.body_html,
        opt_bool_to_int(email.is_read),
        opt_bool_to_int(email.is_starred),
        opt_bool_to_int(email.is_draft),
        opt_bool_to_int(email.is_answered),
        opt_bool_to_int(email.is_deleted),
        email.sent_at,
        email.received_at,
        email.created_at,
        email.updated_at,
    ])?;
    Ok(())
}

fn opt_bool_to_int(value: Option<bool>) -> Option<i64> {
    value.map(i64::from)
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
