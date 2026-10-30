//! SyncEngine 执行层集成测试：四档同步策略在 Mock 会话 + 真实 SQLite 上的行为。
//!
//! 覆盖三条主路径：
//! 1. UidIncremental 只补抓 last_seen_uid 之后的新 UID
//! 2. CondstoreResync 落 flag 变更 + 清理 vanished
//! 3. FullResync 清空本地后全量重建

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use postium_core::domain::sync::SyncProgressEvent;
use postium_core::engine::sync::{execute_plan, FetchNewEmails, LocalSyncState, SyncPlan};
use postium_core::engine::ImapSession;
use postium_core::error::MailError;
use postium_core::infrastructure::protocols::imap::MailboxInfo;
use postium_core::infrastructure::protocols::types::{FetchedBodySection, WholeEmailDto};
use postium_core::infrastructure::storage::database::DbConn;
use postium_core::infrastructure::storage::repository::{email_repo, sync_repo};

const ACCOUNT_ID: i32 = 1;
const FOLDER: &str = "INBOX";

// ─────────────────────────────────────────────────────────────────────
// 夹具
// ─────────────────────────────────────────────────────────────────────

async fn seed_account(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, provider, auth_type, account_type, created_at, updated_at
            ) VALUES (1, 'Test', 'engine-sync@example.com', 'custom', 'password', 'personal', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

/// 预置一封本地邮件（message_id 置 NULL，避免与抓取路径的 message_id 查重冲突）。
async fn insert_email(db: &DbConn, uid: u32, is_read: bool) {
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO emails (
                account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, preview, body_text,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, NULL, ?4, 'Sender', 'sender@example.com',
                'to@example.com', '', ?5, ?6, 0, 0, 0, 0, ?7, ?7, ?7, ?7)",
            rusqlite::params![
                ACCOUNT_ID,
                FOLDER,
                i64::from(uid),
                format!("subject-{uid}"),
                format!("body {uid}"),
                is_read as i64,
                now,
            ],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

/// 构造入库断言用的完整邮件 DTO（uid 即 IMAP UID，由 save_batch_emails 落库）。
fn whole_email(uid: u32) -> WholeEmailDto {
    WholeEmailDto {
        id: 0,
        account_id: 0,
        folder: FOLDER.to_string(),
        uid,
        message_id: Some(format!("<engine-sync-{uid}@example.com>")),
        sender_name: Some("Sender".to_string()),
        sender_email: "sender@example.com".to_string(),
        recipient_emails: "to@example.com".to_string(),
        cc_emails: None,
        bcc_emails: None,
        subject: Some(format!("subject-{uid}")),
        preview: None,
        body_text: Some(format!("body {uid}")),
        body_html: None,
        attachments: vec![],
        is_read: false,
        is_starred: false,
        is_draft: false,
        is_answered: false,
        is_deleted: false,
        sent_at: 1_800_000_000 + i64::from(uid),
        received_at: 1_800_000_000 + i64::from(uid),
        created_at: 0,
    }
}

fn mailbox_info(uidvalidity: u32, highest_modseq: Option<u64>, exists: u32) -> MailboxInfo {
    MailboxInfo {
        exists,
        recent: 0,
        unseen: None,
        uid_validity: Some(uidvalidity),
        uid_next: Some(exists + 1),
        highest_modseq,
    }
}

/// 可控 Mock 会话：UID 搜索 / flag 变更 / 邮箱 meta 均为预设值，其余方法空实现。
struct MockSyncSession {
    mailbox: MailboxInfo,
    search_uids: Vec<u32>,
    flag_changes: Vec<(u32, Vec<String>)>,
}

#[async_trait]
impl ImapSession for MockSyncSession {
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        Ok(vec![])
    }

    async fn enable(&mut self, _caps: &[&str]) -> Result<(), MailError> {
        Ok(())
    }

    async fn select_folder(&mut self, _folder: &str) -> Result<MailboxInfo, MailError> {
        Ok(self.mailbox.clone())
    }

    async fn examine_folder(&mut self, _folder: &str) -> Result<MailboxInfo, MailError> {
        Ok(self.mailbox.clone())
    }

    async fn select_qresync(
        &mut self,
        _folder: &str,
        _uidvalidity: u32,
        _modseq: u64,
    ) -> Result<(), MailError> {
        Ok(())
    }

    async fn fetch_flags_changed_since(
        &mut self,
        _folder: &str,
        _uid_set: &str,
        _modseq: u64,
    ) -> Result<Vec<(u32, Vec<String>)>, MailError> {
        Ok(self.flag_changes.clone())
    }

    async fn uid_search_all(&mut self, _folder: &str) -> Result<Vec<u32>, MailError> {
        Ok(self.search_uids.clone())
    }

    async fn fetch_body(&mut self, _folder: &str, _uid: u32) -> Result<String, MailError> {
        Ok(String::new())
    }

    async fn move_uid(&mut self, _folder: &str, _uid: u32, _to_folder: &str) -> Result<(), MailError> {
        Ok(())
    }

    async fn set_flags(
        &mut self,
        _folder: &str,
        _uid: u32,
        _flags: Vec<String>,
    ) -> Result<(), MailError> {
        Ok(())
    }

    async fn add_flags(&mut self, _folder: &str, _uid: u32, _flag: &str) -> Result<(), MailError> {
        Ok(())
    }

    async fn remove_flags(&mut self, _folder: &str, _uid: u32, _flag: &str) -> Result<(), MailError> {
        Ok(())
    }

    async fn append(
        &mut self,
        _folder: &str,
        _email: &[u8],
        _flags: &[String],
        _message_id: Option<&str>,
    ) -> Result<u32, MailError> {
        Ok(0)
    }

    async fn delete(&mut self, _folder: &str, _uid: u32) -> Result<(), MailError> {
        Ok(())
    }

    async fn fetch_email_by_uid(
        &mut self,
        _folder: &str,
        _uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        Ok(None)
    }

    async fn fetch_body_section(
        &mut self,
        _folder: &str,
        _uid: u32,
        _section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError> {
        Ok(None)
    }

    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        Ok(())
    }
}

/// 构造 `fetch_new_emails` 回调：直接按请求的 UID 构造 DTO 返回（入库由执行函数做），
/// 同时记录每次请求的 UID 批次供断言"只抓新 UID"。测试进程内泄漏，无需回收。
fn make_fetch(
    requests: Arc<Mutex<Vec<Vec<u32>>>>,
) -> FetchNewEmails<'static> {
    let closure = move |_session: &mut dyn ImapSession,
                        _folder: &str,
                        uids: &[u32]|
        -> Pin<Box<dyn Future<Output = Result<Vec<WholeEmailDto>, MailError>> + Send>> {
        requests.lock().unwrap().push(uids.to_vec());
        let dtos: Vec<WholeEmailDto> = uids.iter().copied().map(whole_email).collect();
        Box::pin(async move { Ok(dtos) })
    };
    Box::leak(Box::new(closure)) as FetchNewEmails<'static>
}

fn progress_channel() -> smol::channel::Sender<SyncProgressEvent> {
    let (tx, _rx) = smol::channel::unbounded::<SyncProgressEvent>();
    tx
}

// ─────────────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn uid_incremental_fetches_only_new_uids() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    for uid in 1..=3 {
        insert_email(&db, uid, false).await;
    }

    let requests = Arc::new(Mutex::new(Vec::new()));
    let fetch = make_fetch(Arc::clone(&requests));
    let mut session = MockSyncSession {
        mailbox: mailbox_info(1, Some(9), 5),
        search_uids: vec![2, 3, 4, 5],
        flag_changes: vec![],
    };
    let events = progress_channel();

    let local = LocalSyncState {
        uidvalidity: Some(1),
        highest_modseq: None,
        last_seen_uid: Some(3),
    };

    let outcome = execute_plan(
        &mut session,
        db.clone(),
        ACCOUNT_ID,
        FOLDER,
        &SyncPlan::UidIncremental,
        &local,
        fetch,
        &events,
    )
    .await
    .unwrap();

    // 只补抓 uid > 3 的两封，且单批到位
    assert_eq!(requests.lock().unwrap().as_slice(), [vec![4u32, 5]].as_slice());
    assert_eq!(outcome.new_emails, 2);
    assert_eq!(outcome.flag_updates, 0);
    assert_eq!(outcome.deleted_emails, 0);

    // uid 4,5 入库且带正文
    for uid in [4u32, 5] {
        let email = email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, uid)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("uid {uid} 应已入库"));
        assert_eq!(email.body_text.as_deref(), Some(&format!("body {uid}")[..]));
    }

    // 状态回写：last_seen_uid=5，uidvalidity/modseq 传 None → COALESCE 保留
    let state = sync_repo::get_sync_state(&db, ACCOUNT_ID, FOLDER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.last_sync_uid, Some(5));
    assert_eq!(state.uidvalidity, None);
    assert_eq!(state.highest_modseq, None);
}

#[tokio::test]
async fn modseq_resync_updates_flags_and_deletes_vanished() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    insert_email(&db, 1, true).await;
    insert_email(&db, 2, false).await;
    insert_email(&db, 3, false).await;

    let requests = Arc::new(Mutex::new(Vec::new()));
    let fetch = make_fetch(requests);
    let mut session = MockSyncSession {
        mailbox: mailbox_info(1, Some(9), 3),
        // uid2 已从服务器消失；uid2/uid3 的 flag 变为 \Seen
        search_uids: vec![1, 3],
        flag_changes: vec![
            (2, vec!["\\Seen".to_string()]),
            (3, vec!["\\Seen".to_string()]),
        ],
    };
    let events = progress_channel();

    let local = LocalSyncState {
        uidvalidity: Some(1),
        highest_modseq: Some(5),
        last_seen_uid: Some(3),
    };

    let outcome = execute_plan(
        &mut session,
        db.clone(),
        ACCOUNT_ID,
        FOLDER,
        &SyncPlan::CondstoreResync,
        &local,
        fetch,
        &events,
    )
    .await
    .unwrap();

    assert!(outcome.flag_updates >= 1);
    assert!(outcome.deleted_emails >= 1);
    assert_eq!(outcome.new_emails, 0);

    // uid2 已消失被删；uid1 仍存在且保持已读
    assert!(
        email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, 2)
            .await
            .unwrap()
            .is_none(),
        "vanished uid2 应被删除"
    );
    let uid1 = email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, 1)
        .await
        .unwrap()
        .expect("uid1 应仍存在");
    assert_eq!(uid1.is_read, Some(true));
    // uid3 被标为已读
    let uid3 = email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, 3)
        .await
        .unwrap()
        .expect("uid3 应仍存在");
    assert_eq!(uid3.is_read, Some(true), "CHANGEDSINCE 应落 uid3 已读");

    // 状态回写：uidvalidity / modseq 用服务器 meta
    let state = sync_repo::get_sync_state(&db, ACCOUNT_ID, FOLDER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.uidvalidity, Some(1));
    assert_eq!(state.highest_modseq, Some(9));
    assert_eq!(state.last_sync_uid, Some(3));
}

#[tokio::test]
async fn full_resync_clears_and_refills() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    insert_email(&db, 1, false).await;
    insert_email(&db, 2, false).await;

    let requests = Arc::new(Mutex::new(Vec::new()));
    let fetch = make_fetch(requests);
    let mut session = MockSyncSession {
        mailbox: mailbox_info(7, Some(12), 2),
        search_uids: vec![10, 11],
        flag_changes: vec![],
    };
    let events = progress_channel();

    let local = LocalSyncState {
        uidvalidity: Some(1),
        highest_modseq: None,
        last_seen_uid: Some(2),
    };

    let outcome = execute_plan(
        &mut session,
        db.clone(),
        ACCOUNT_ID,
        FOLDER,
        &SyncPlan::FullResync,
        &local,
        fetch,
        &events,
    )
    .await
    .unwrap();

    // 旧邮件被清、新邮件全量入库
    assert_eq!(outcome.deleted_emails, 2);
    assert_eq!(outcome.new_emails, 2);
    assert!(
        email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, 1)
            .await
            .unwrap()
            .is_none(),
        "旧 uid1 应被全量清空"
    );
    assert!(
        email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, 2)
            .await
            .unwrap()
            .is_none(),
        "旧 uid2 应被全量清空"
    );
    for uid in [10u32, 11] {
        let email = email_repo::get_by_uid(&db, ACCOUNT_ID, FOLDER, uid)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("uid {uid} 应已入库"));
        assert_eq!(email.body_text.as_deref(), Some(&format!("body {uid}")[..]));
    }

    // 状态回写：uidvalidity + last_seen_uid=最大 + highest_modseq
    let state = sync_repo::get_sync_state(&db, ACCOUNT_ID, FOLDER)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.uidvalidity, Some(7));
    assert_eq!(state.last_sync_uid, Some(11));
    assert_eq!(state.highest_modseq, Some(12));
}
