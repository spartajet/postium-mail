//! E2E 端到端测试的确定性数据播种器。
//!
//! 本模块为 Playwright/前端 E2E 测试提供**确定性**（可预测、可重复）的夹具数据：
//! 在数据库中插入 2 个测试账号以及约 40 封分布在不同文件夹的夹具邮件。
//! 所有邮件的 subject、UID、时间戳均按固定规则生成，便于 E2E 测试精确断言。
//!
//! # 触发方式
//!
//! 当启动应用时设置环境变量 `POSTIUM_E2E=1`，应用启动流程（见 `lib.rs`）
//! 会调用 [`seed_e2e_data`]。此时还需设置 `POSTIUM_DATA_DIR` 指定数据目录。
//!
//! # 夹具数据契约（E2E 测试依赖以下规律进行断言）
//!
//! ## 账号
//!
//! - `primary.e2e@postium.test` —— 主账号（颜色 `#2563eb`，蓝色）
//! - `secondary.e2e@postium.test` —— 副账号（颜色 `#16a34a`，绿色）
//!
//! ## 主账号邮件（[`primary_emails`]），各文件夹 UID 互不重叠
//!
//! | 文件夹 | UID 区间 | 数量 | 已读 | 星标 | sent_at |
//! |--------|----------|------|------|------|---------|
//! | INBOX | 1–24 | 24 | UID ≤ 16 已读 | UID ∈ {3,6,9,12} | `BASE_TS - UID` |
//! | Sent | 101–105 | 5 | 全部已读 | 无 | `BASE_TS - 100 - UID` |
//! | Archive | 201–204 | 4 | 全部已读 | 全部星标 | `BASE_TS - 200 - UID` |
//! | Drafts | 301–302 | 2 | 全部已读 | 无 | `BASE_TS - 300 - UID` |
//! | Trash | 401 | 1 | 已读 | 无 | `BASE_TS - 401` |
//!
//! 其中部分 INBOX 邮件有特殊 subject（搜索/归档测试用）：
//! UID 5 → `Quarterly Planning Alpha`、UID 8 → `Quarterly Planning Beta`、
//! UID 16 → `Quarterly Planning Archive`、UID 24 → `Primary Inbox Message 24`。
//!
//! ## 副账号邮件（[`secondary_emails`]），UID 偏移 1000
//!
//! | 文件夹 | UID 区间 | 数量 | 已读 | 星标 | sent_at |
//! |--------|----------|------|------|------|---------|
//! | INBOX | 1–8 | 8 | UID ≤ 4 已读 | UID = 2 | `BASE_TS - 1000 - UID` |
//! | Sent | 101–102 | 2 | 全部已读 | 无 | `BASE_TS - 1100 - UID` |
//! | Archive | 201 | 1 | 已读 | 星标 | `BASE_TS - 1201` |
//! | Drafts | 301 | 1 | 已读 | 无 | `BASE_TS - 1301` |
//!
//! # 布尔标记规律
//!
//! - `is_draft` —— 仅当文件夹为 `Drafts` 时为真。
//! - `is_deleted` —— 仅当文件夹为 `Trash` 时为真。
//! - `is_answered` —— 恒为假。
//! - `received_at` / `created_at` / `updated_at` —— 均等于 `sent_at`。

use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::email_repo::EmailWrite;

/// 主测试账号的邮箱地址。颜色标记为蓝色（`#2563eb`）。
const PRIMARY_EMAIL: &str = "primary.e2e@postium.test";

/// 副测试账号的邮箱地址。颜色标记为绿色（`#16a34a`）。
const SECONDARY_EMAIL: &str = "secondary.e2e@postium.test";

/// 所有夹具邮件的时间戳基准（Unix 秒）。
///
/// 各邮件的 `sent_at` = `BASE_TS - 偏移`，偏移随 UID 增大而增大，
/// 保证 UID 较小的邮件时间戳更大（更“新”），便于按时间排序断言。
/// 主账号 UID 段偏移直接取 UID；副账号在 UID 上额外偏移 1000。
const BASE_TS: i64 = 1_779_936_000;

/// 向数据库写入 E2E 确定性夹具数据（2 个账号 + 约 40 封邮件）。
///
/// 在单个事务中先插入两个测试账号，再为每个账号生成夹具邮件并批量写入。
/// 所有写入使用 `INSERT OR IGNORE`，因此重复播种是幂等的（不会报唯一约束错误）。
///
/// # 参数
///
/// - `db`: 数据库连接
///
/// # 返回
///
/// 成功返回 `Ok(())`；若任意插入或事务失败则返回对应错误。
///
/// # 触发条件
///
/// 仅当启动应用时设置环境变量 `POSTIUM_E2E=1` 时，由 `lib.rs` 的启动流程调用。
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

/// 测试账号的写入模型，字段一一对应 `accounts` 表的列。
///
/// 作为 [`insert_account`] 的入参，集中承载播种一个账号所需的全部数据。
struct AccountSeed {
    /// 账号显示名称（如 "Primary E2E"）。
    name: String,
    /// 账号邮箱地址，唯一；用于通过 [`account_id_by_email`] 反查 id。
    email: String,
    /// 收件人展示用的显示名。
    display_name: Option<String>,
    /// 邮件服务商标识，夹具统一为 `"custom"`。
    provider: String,
    /// IMAP 服务器主机，夹具统一为 `"imap.postium.test"`。
    imap_host: Option<String>,
    /// IMAP 端口，夹具统一为 993。
    imap_port: Option<i32>,
    /// 是否启用 IMAP SSL，夹具统一为 `Some(true)`。
    imap_ssl: Option<bool>,
    /// IMAP SSL 模式字符串，夹具统一为 `"ssl"`。
    imap_ssl_mode: Option<String>,
    /// SMTP 服务器主机，夹具统一为 `"smtp.postium.test"`。
    smtp_host: Option<String>,
    /// SMTP 端口，夹具统一为 465。
    smtp_port: Option<i32>,
    /// 是否启用 SMTP SSL，夹具统一为 `Some(true)`。
    smtp_ssl: Option<bool>,
    /// SMTP SSL 模式字符串，夹具统一为 `"ssl"`。
    smtp_ssl_mode: Option<String>,
    /// UI 上的账号主题色（主账号 `#2563eb`、副账号 `#16a34a`）。
    color: Option<String>,
    /// 是否启用自动同步，夹具统一为 `Some(false)`（E2E 不触发真实同步）。
    sync_enabled: Option<bool>,
    /// 最近一次同步时间戳，夹具统一为 `None`。
    last_sync_at: Option<i64>,
    /// 鉴权方式，夹具统一为 `"Password"`。
    auth_type: Option<String>,
    /// 账号类型，夹具统一为 `"personal"`。
    account_type: String,
    /// 账号创建时间戳（主账号取 `BASE_TS`，副账号取 `BASE_TS + 1`）。
    created_at: i64,
    /// 账号更新时间戳（与 `created_at` 一致）。
    updated_at: i64,
}

/// 将一个 [`AccountSeed`] 插入 `accounts` 表。
///
/// 使用 `INSERT OR IGNORE`，故同名/同邮箱账号重复插入不会报错（幂等）。
/// 布尔类型的可选字段经 [`opt_bool_to_int`] 转为 `Option<i64>` 以匹配表中的整数列。
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

/// 按邮箱地址反查账号的自增 id。
///
/// 在两个账号插入完成后，用于获取它们的 `account_id`，供后续夹具邮件引用外键。
fn account_id_by_email(tx: &rusqlite::Transaction<'_>, email: &str) -> rusqlite::Result<i32> {
    tx.query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
        row.get(0)
    })
}

/// 用已预编译的语句执行单封邮件的插入（`INSERT OR IGNORE`）。
///
/// 接收外部预编译好的 [`rusqlite::Statement`]，在批量写入循环中复用，避免重复编译开销。
/// 布尔类型的可选字段经 [`opt_bool_to_int`] 转换为 `Option<i64>` 以匹配表中的整数列；
/// `uid` 由 `u32` 转为 `i64` 以匹配 SQLite 列类型。
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

/// 将 `Option<bool>` 转换为 `Option<i64>`（`Some(true)→1`、`Some(false)→0`、`None→None`）。
///
/// SQLite 表中布尔标记列以整数存储，此函数用于插入前转换。
fn opt_bool_to_int(value: Option<bool>) -> Option<i64> {
    value.map(i64::from)
}

/// 生成主账号的全部夹具邮件（共 36 封）。
///
/// 这是 E2E 测试契约的核心。各文件夹 UID 互不重叠，便于断言：
///
/// # UID / 文件夹 / 时间戳编号方案
///
/// - **INBOX**（24 封，UID 1–24）
///     - 已读：UID ≤ 16；未读：UID 17–24。
///     - 星标：UID ∈ {3, 6, 9, 12}。
///     - `sent_at = BASE_TS - UID`。
///     - 特殊 subject（搜索/归档测试断点）：
///       UID 5 → `Quarterly Planning Alpha`、UID 8 → `Quarterly Planning Beta`、
///       UID 16 → `Quarterly Planning Archive`、UID 24 → `Primary Inbox Message 24`。
///       其余 UID 的 subject 为 `Primary Inbox Message NN`（零填充），
///       其中 UID 1 特殊化为 `Primary Inbox Message 01`。
/// - **Sent**（5 封，UID 101–105）：全部已读、无星标；`sent_at = BASE_TS - 100 - UID`。
///   UID 101 的 subject 为 `Sent Confirmation Message`，其余为 `Primary Sent Message NN`。
/// - **Archive**（4 封，UID 201–204）：全部已读且星标；`sent_at = BASE_TS - 200 - UID`。
///   UID 201 的 subject 为 `Starred Reference Message`，其余为 `Primary Starred Message NN`。
/// - **Drafts**（2 封，UID 301–302）：全部已读、无星标；`sent_at = BASE_TS - 300 - UID`。
///   UID 301 的 subject 为 `Draft Proposal Outline`，UID 302 为 `Primary Draft Message 02`。
/// - **Trash**（1 封，UID 401）：已读、无星标；subject 为 `Trash Cleanup Notice`；
///   `sent_at = BASE_TS - 401`。
///
/// # 参数
///
/// - `account_id`: 主账号的数据库 id
///
/// # 返回
///
/// 包含 36 封 [`EmailWrite`] 的向量，调用方将其批量插入 `emails` 表。
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

/// 生成副账号的全部夹具邮件（共 12 封）。
///
/// UID 编号方案与 [`primary_emails`] 结构一致，但时间戳整体再偏移 1000，
/// 使副账号邮件全部比主账号“更旧”，便于按账号/时间排序断言。
///
/// # UID / 文件夹 / 时间戳编号方案
///
/// - **INBOX**（8 封，UID 1–8）：已读 UID ≤ 4；星标仅 UID 2；
///   `sent_at = BASE_TS - 1_000 - UID`；subject 为 `Secondary Inbox Message NN`。
/// - **Sent**（2 封，UID 101–102）：全部已读、无星标；`sent_at = BASE_TS - 1_100 - UID`；
///   subject 为 `Secondary Sent Message NN`。
/// - **Archive**（1 封，UID 201）：已读且星标；subject 为 `Secondary Starred Message 01`；
///   `sent_at = BASE_TS - 1_201`。
/// - **Drafts**（1 封，UID 301）：已读、无星标；subject 为 `Secondary Draft Message 01`；
///   `sent_at = BASE_TS - 1_301`。
///
/// # 参数
///
/// - `account_id`: 副账号的数据库 id
///
/// # 返回
///
/// 包含 12 封 [`EmailWrite`] 的向量，调用方将其批量插入 `emails` 表。
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

/// 构造单封夹具邮件的 [`EmailWrite`] 模型。
///
/// 根据 subject 前缀推断发件人（`Secondary…` → 副账号发件人，其余 → 主账号发件人），
/// 并按固定规则生成 `message_id`、预览、正文、HTML 及各类标记：
///
/// - `message_id` 格式为 `<e2e-{account_id}-{folder}-{uid}@postium.test>`，全局唯一。
/// - `preview` 为 `Preview for {subject}`。
/// - 正文（`body_text`）为 `This is deterministic E2E body content for {subject}.`，
///   `body_html` 为其包裹一层 `<p>`。
/// - `is_draft` 仅当文件夹为 `Drafts` 时为真；`is_deleted` 仅当文件夹为 `Trash` 时为真；
///   `is_answered` 恒为假。
/// - `received_at` / `created_at` / `updated_at` 均等于传入的 `sent_at`。
///
/// # 参数
///
/// - `account_id`: 所属账号 id
/// - `folder`: 文件夹名（`INBOX` / `Sent` / `Archive` / `Drafts` / `Trash`）
/// - `uid`: 邮件 UID（同一账号内各文件夹互不重叠，见 [`primary_emails`] / [`secondary_emails`]）
/// - `subject`: 邮件主题
/// - `is_read`: 是否已读
/// - `is_starred`: 是否星标
/// - `sent_at`: 发送时间戳（夹具中 `received_at` / `created_at` / `updated_at` 均取此值）
///
/// # 返回
///
/// 填充好全部字段的 [`EmailWrite`]，可直接用于插入。
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
