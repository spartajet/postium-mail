//! 会话线程化服务层 API 测试（EmailService 会话变体）。
//!
//! 起库与夹具形态跟随 `tests/email_commands.rs` 先例：
//! `common::TestServices`（内存库 + Noop 远端）+ `account_service.create` 建账号；
//! 邮件经 `email_repo::save_batch_emails` 落库（入库事务内真实归组线程），
//! 文件夹分类（sync_state.folder_category）按同步期持久化形态直插。

mod common;

use common::TestServices;
use postium_core::infrastructure::protocols::types::WholeEmailDto;
use postium_core::infrastructure::storage::repository::email_repo;
use postium_core::service::account_service::CreateAccountRequest;
use postium_core::service::email_service::EmailCategory;
use std::sync::atomic::{AtomicU32, Ordering};

static ACCOUNT_COUNTER: AtomicU32 = AtomicU32::new(1);

async fn create_test_account(svc: &TestServices) -> i32 {
    let index = ACCOUNT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let req = CreateAccountRequest {
        name: format!("Conv Test {}", index),
        email: format!("conv-{}@gmail.com", index),
        display_name: None,
        provider: "gmail".to_string(),
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
    svc.account_service.create(req).await.unwrap().id
}

/// 一封参与会话归组的完整邮件（最小字段 + 线程化/状态参数）。
#[allow(clippy::too_many_arguments)]
fn conv_email(
    uid: u32,
    subject: &str,
    sender: &str,
    recipient: &str,
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    received_at: i64,
    is_read: bool,
    is_starred: bool,
) -> WholeEmailDto {
    WholeEmailDto {
        id: 0,
        account_id: 0,
        folder: "INBOX".to_string(),
        uid,
        message_id: message_id.map(str::to_string),
        in_reply_to: in_reply_to.map(str::to_string),
        sender_name: None,
        sender_email: sender.to_string(),
        recipient_emails: recipient.to_string(),
        cc_emails: None,
        bcc_emails: None,
        subject: Some(subject.to_string()),
        preview: None,
        body_text: None,
        body_html: None,
        attachments: vec![],
        is_read,
        is_starred,
        is_draft: false,
        is_answered: false,
        is_deleted: false,
        sent_at: received_at,
        received_at,
        created_at: received_at,
    }
}

/// 落一封邮件（经 save_batch_emails 真实归组）并返回 (邮件 id, thread_id)。
async fn seed(
    svc: &TestServices,
    account_id: i32,
    folder: &str,
    email: WholeEmailDto,
) -> (i32, i64) {
    let uid = email.uid;
    email_repo::save_batch_emails(&svc.db, account_id, folder, &[email])
        .await
        .unwrap();
    let model = email_repo::get_by_uid(&svc.db, account_id, folder, uid)
        .await
        .unwrap()
        .expect("seed 邮件应可按 uid 回读");
    let thread_id = model.thread_id.expect("非草稿 seed 邮件应已归组");
    (model.id, thread_id)
}

/// 按同步期持久化形态落一条文件夹分类（sync_state.folder_category）。
async fn seed_folder_category(svc: &TestServices, account_id: i32, folder: &str, category: &str) {
    let folder = folder.to_string();
    let category = category.to_string();
    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO sync_state (account_id, folder, folder_category)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![account_id, folder, category],
            )?;
            Ok(())
        })
        .await
        .unwrap();
}

fn thread_ids(resp: &postium_core::service::email_service::ConversationListResponse) -> Vec<i64> {
    resp.conversations.iter().map(|c| c.thread_id).collect()
}

// ─── 三个 list 变体过滤 ───

#[tokio::test]
async fn list_conversations_variants_filter_correctly() {
    let svc = TestServices::new().await;
    let a1 = create_test_account(&svc).await;
    let a2 = create_test_account(&svc).await;

    // 账号 a1：INBOX 一条线程（头链两封）、Archive 一条线程（最新一封星标）
    let (root_id, t_inbox) = seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            1,
            "Quarterly Review",
            "alice@x.com",
            "bob@x.com",
            Some("<q1@x.com>"),
            None,
            100,
            false,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            2,
            "Re: Quarterly Review",
            "bob@x.com",
            "alice@x.com",
            Some("<q2@x.com>"),
            Some("<q1@x.com>"),
            200,
            false,
            false,
        ),
    )
    .await;
    let (starred_id, t_archive) = seed(
        &svc,
        a1,
        "Archive",
        conv_email(
            3,
            "Old ledger",
            "carol@x.com",
            "alice@x.com",
            Some("<l1@x.com>"),
            None,
            300,
            true,
            true,
        ),
    )
    .await;
    // 账号 a2：INBOX 一条线程
    let (_, t_a2_inbox) = seed(
        &svc,
        a2,
        "INBOX",
        conv_email(
            4,
            "Other account",
            "dave@x.com",
            "eve@x.com",
            Some("<o1@x.com>"),
            None,
            150,
            true,
            false,
        ),
    )
    .await;

    // 变体 1：单账号 + 文件夹
    let resp = svc
        .email_service
        .list_conversations(a1, "INBOX", 1, 50, false)
        .await
        .unwrap();
    assert_eq!((resp.total, thread_ids(&resp)), (1, vec![t_inbox]));
    assert_eq!(resp.conversations[0].total, 2, "线程内两封");
    let _ = root_id;

    // 变体 2/3 依赖同步期持久化的文件夹分类
    seed_folder_category(&svc, a1, "INBOX", "inbox").await;
    seed_folder_category(&svc, a1, "Archive", "archive").await;
    seed_folder_category(&svc, a2, "INBOX", "inbox").await;

    // 变体 2：单账号 + 分类（Inbox 只含持久化分类为 inbox 的文件夹）
    let resp = svc
        .email_service
        .list_conversations_by_category(a1, EmailCategory::Inbox, 1, 50, false)
        .await
        .unwrap();
    assert_eq!(
        (resp.total, thread_ids(&resp)),
        (1, vec![t_inbox]),
        "Archive 线程不得混入 Inbox 分类"
    );

    // 变体 2：Starred 分类跨文件夹（命中 Archive 里的星标线程）
    let resp = svc
        .email_service
        .list_conversations_by_category(a1, EmailCategory::Starred, 1, 50, false)
        .await
        .unwrap();
    assert_eq!(
        (resp.total, thread_ids(&resp)),
        (1, vec![t_archive]),
        "Starred 应跨文件夹命中唯一星标线程"
    );
    let _ = starred_id;

    // 变体 3：全账号 + 分类（两账号的 Inbox 线程都在）
    let resp = svc
        .email_service
        .list_conversations_by_category_for_all_accounts(EmailCategory::Inbox, 1, 50, false)
        .await
        .unwrap();
    assert_eq!(resp.total, 2);
    let mut ids = thread_ids(&resp);
    ids.sort_unstable();
    let mut expected = vec![t_inbox, t_a2_inbox];
    expected.sort_unstable();
    assert_eq!(ids, expected, "全账号 Inbox 应含两账号各自的线程");
}

// ─── unread_only 过滤下推（三变体统一） ───

#[tokio::test]
async fn list_conversations_unread_only_keeps_threads_with_unread() {
    let svc = TestServices::new().await;
    let a1 = create_test_account(&svc).await;

    // 线程 A：两封全未读；线程 B：两封全已读；线程 C：一读一未读（混合）
    let (_, t_a) = seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            51,
            "Unread thread",
            "alice@x.com",
            "bob@x.com",
            Some("<u1@x.com>"),
            None,
            100,
            false,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            52,
            "Re: Unread thread",
            "bob@x.com",
            "alice@x.com",
            Some("<u2@x.com>"),
            Some("<u1@x.com>"),
            200,
            false,
            false,
        ),
    )
    .await;
    let (_, t_b) = seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            53,
            "Read thread",
            "carol@x.com",
            "alice@x.com",
            Some("<u3@x.com>"),
            None,
            150,
            true,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            54,
            "Re: Read thread",
            "alice@x.com",
            "carol@x.com",
            Some("<u4@x.com>"),
            Some("<u3@x.com>"),
            250,
            true,
            false,
        ),
    )
    .await;
    let (_, t_c) = seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            55,
            "Mixed thread",
            "dave@x.com",
            "alice@x.com",
            Some("<u5@x.com>"),
            None,
            175,
            true,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        a1,
        "INBOX",
        conv_email(
            56,
            "Re: Mixed thread",
            "alice@x.com",
            "dave@x.com",
            Some("<u6@x.com>"),
            Some("<u5@x.com>"),
            275,
            false,
            false,
        ),
    )
    .await;
    seed_folder_category(&svc, a1, "INBOX", "inbox").await;

    // 对照：不过滤时三条线程齐全
    let all = svc
        .email_service
        .list_conversations(a1, "INBOX", 1, 50, false)
        .await
        .unwrap();
    let mut expected_all = vec![t_a, t_b, t_c];
    expected_all.sort_unstable();
    let mut got_all = thread_ids(&all);
    got_all.sort_unstable();
    assert_eq!((all.total, got_all), (3, expected_all));

    // 变体 1：unread_only=true 只剩含未读的线程（A 全未读、C 混合），
    // 全已读的 B 排除；混合线程聚合口径 = 过滤后可见封数（1 封未读）
    let unread = svc
        .email_service
        .list_conversations(a1, "INBOX", 1, 50, true)
        .await
        .unwrap();
    let mut expected_unread = vec![t_a, t_c];
    expected_unread.sort_unstable();
    let mut got_unread = thread_ids(&unread);
    got_unread.sort_unstable();
    assert_eq!(
        (unread.total, got_unread),
        (2, expected_unread),
        "unread_only 只保留含未读邮件的线程"
    );
    let mixed = unread
        .conversations
        .iter()
        .find(|c| c.thread_id == t_c)
        .unwrap();
    assert_eq!(
        (mixed.total, mixed.unread),
        (1, 1),
        "混合线程在 unread_only 下的聚合数 = 过滤后可见（未读）封数"
    );

    // 变体 2/3：分类入口同样下推
    let by_category = svc
        .email_service
        .list_conversations_by_category(a1, EmailCategory::Inbox, 1, 50, true)
        .await
        .unwrap();
    assert_eq!(by_category.total, 2, "单账号分类入口同样过滤");

    let all_accounts = svc
        .email_service
        .list_conversations_by_category_for_all_accounts(EmailCategory::Inbox, 1, 50, true)
        .await
        .unwrap();
    assert_eq!(all_accounts.total, 2, "全账号分类入口同样过滤");
}

// ─── conversation_emails 时间序 + DTO 透出 ───

#[tokio::test]
async fn conversation_emails_returns_chronological_dtos_with_headers() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    // 乱序落库：received_at 300 → 100 → 200，头链全部指向首封
    let (first_id, thread_id) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            11,
            "Kickoff",
            "alice@x.com",
            "bob@x.com",
            Some("<k1@x.com>"),
            None,
            300,
            false,
            false,
        ),
    )
    .await;
    let (reply_id, _) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            12,
            "Re: Kickoff",
            "bob@x.com",
            "alice@x.com",
            Some("<k2@x.com>"),
            Some("<k1@x.com>"),
            100,
            false,
            false,
        ),
    )
    .await;
    let (late_id, _) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            13,
            "Re: Kickoff",
            "alice@x.com",
            "bob@x.com",
            Some("<k3@x.com>"),
            Some("<k1@x.com>"),
            200,
            false,
            false,
        ),
    )
    .await;

    let dtos = svc.email_service.conversation_emails(thread_id).await.unwrap();

    assert_eq!(
        dtos.iter().map(|dto| dto.id).collect::<Vec<_>>(),
        vec![reply_id, late_id, first_id],
        "会话内邮件按 received_at 升序"
    );
    assert_eq!(
        dtos.iter()
            .filter(|dto| dto.message_id.as_deref() == Some("<k1@x.com>"))
            .count(),
        1,
        "EmailDto 应透出 message_id"
    );
    let reply_dto = dtos.iter().find(|dto| dto.id == reply_id).unwrap();
    assert_eq!(
        reply_dto.in_reply_to.as_deref(),
        Some("<k1@x.com>"),
        "EmailDto 应透出 in_reply_to"
    );
}

// ─── mark_conversation_read 全读后 unread 归零 ───

#[tokio::test]
async fn mark_conversation_read_clears_unread_count() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    let (_, thread_id) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            21,
            "Release plan",
            "alice@x.com",
            "bob@x.com",
            Some("<r1@x.com>"),
            None,
            100,
            true,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            22,
            "Re: Release plan",
            "bob@x.com",
            "alice@x.com",
            Some("<r2@x.com>"),
            Some("<r1@x.com>"),
            200,
            false,
            false,
        ),
    )
    .await;
    seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            23,
            "Re: Release plan",
            "alice@x.com",
            "bob@x.com",
            Some("<r3@x.com>"),
            Some("<r1@x.com>"),
            300,
            false,
            false,
        ),
    )
    .await;

    let before = svc
        .email_service
        .list_conversations(account_id, "INBOX", 1, 50, false)
        .await
        .unwrap();
    assert_eq!(before.conversations[0].unread, 2, "标记前应有 2 封未读");

    svc.email_service.mark_conversation_read(thread_id).await.unwrap();

    let after = svc
        .email_service
        .list_conversations(account_id, "INBOX", 1, 50, false)
        .await
        .unwrap();
    assert_eq!(
        after.conversations[0].unread, 0,
        "整会话标已读后未读数应归零"
    );

    let dtos = svc
        .email_service
        .conversation_emails(thread_id)
        .await
        .unwrap();
    assert!(
        dtos.iter().all(|dto| dto.is_read),
        "会话内全部邮件应已读"
    );
}

// ─── toggle_conversation_star 作用于最新一封 ───

#[tokio::test]
async fn toggle_conversation_star_targets_latest_email_only() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;

    // 旧封已星标，最新封未星标：会话级切换只翻最新封
    let (old_id, thread_id) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            31,
            "Budget",
            "alice@x.com",
            "bob@x.com",
            Some("<b1@x.com>"),
            None,
            100,
            true,
            true,
        ),
    )
    .await;
    let (latest_id, _) = seed(
        &svc,
        account_id,
        "INBOX",
        conv_email(
            32,
            "Re: Budget",
            "bob@x.com",
            "alice@x.com",
            Some("<b2@x.com>"),
            Some("<b1@x.com>"),
            300,
            true,
            false,
        ),
    )
    .await;

    let new_state = svc
        .email_service
        .toggle_conversation_star(thread_id)
        .await
        .unwrap();
    assert!(new_state, "最新封由未星标翻为星标");

    let latest = email_repo::get_by_id(&svc.db, latest_id).await.unwrap().unwrap();
    let old = email_repo::get_by_id(&svc.db, old_id).await.unwrap().unwrap();
    assert!(latest.is_starred.unwrap_or(false), "星标应作用于最新一封");
    assert!(
        old.is_starred.unwrap_or(false),
        "旧封星标状态不得被会话级切换改动"
    );
}
