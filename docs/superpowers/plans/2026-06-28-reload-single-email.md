# Single Email Reload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a right-click “重新加载” action that reloads one email from IMAP by `folder + uid`, overwrites local email and attachment metadata when present, and removes the local record when the remote UID no longer exists.

**Architecture:** The feature adds a narrow single-email reload path instead of reusing full folder sync. `EmailState.reloadEmail` calls a new Tauri command, `EmailService` coordinates local lookup and remote fetch through `MailRemoteOperator`, and `email_repo` performs transactional local replacement or removal. The IMAP protocol layer exposes a single-UID full-message fetch that returns `Option<WholeEmailDto>`.

**Tech Stack:** Rust, Tauri v2 commands, Specta types, async-imap, rusqlite-backed `DbConn`, Svelte 5 runes, Vitest, TypeScript.

## Global Constraints

- Do not trigger full folder sync.
- Do not update folder sync state such as `uidnext`, `uidvalidity`, or `last_sync_uid`.
- Do not implement batch reload.
- Do not delete downloaded attachment files from disk; only replace attachment database metadata.
- Do not automatically reload when opening email detail.
- Remote UID missing returns a successful `Removed` result and deletes the local email plus attachment metadata.
- IMAP, auth, parse, and database errors must leave local email state unchanged unless the remote UID is definitively missing.

---

## File Structure

- `src-tauri/src/infrastructure/protocols/imap/fetch.rs`
  - Add `fetch_email_by_uid(folder, uid)` for a single complete RFC822 fetch.
  - Reuse the parsing shape from `batch_fetch_emails`.
- `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - Add transactional `replace_email_with_attachments`.
  - Add or expose local hard-delete helper for one email and its attachments.
- `src-tauri/src/service/mail_operation.rs`
  - Extend `MailRemoteOperator` with `reload_email`.
  - Implement real IMAP reload using `fetch_email_by_uid`.
- `src-tauri/src/service/email_service.rs`
  - Add `ReloadEmailResult`.
  - Add `EmailService::reload_email`.
- `src-tauri/src/command/email.rs`
  - Add `reload_email` Tauri command.
- `src-tauri/src/lib.rs`
  - Register the new command.
- `src-tauri/tests/common/mod.rs`
  - Extend fake mail remote support for reload tests.
- `src-tauri/tests/email_commands.rs`
  - Add backend reload service/command tests.
- `src/lib/bindings.ts`
  - Regenerate through the existing Tauri Specta debug export path so it includes `ReloadEmailResult` and `commands.reloadEmail`.
- `src/lib/stores/email.svelte.ts`
  - Add `reloadEmail(emailId)`.
- `src/lib/components/email/EmailContextMenu.svelte`
  - Add menu item and `onReload`.
- `src/lib/components/email/EmailList.svelte`
  - Wire context menu reload handler.
- `src/lib/__tests__/stores/email-state.test.ts`
  - Add store behavior tests for `Reloaded`, `Removed`, and failure.
- `src/lib/__tests__/components/EmailContextMenu.test.ts`
  - Add a component test that clicking “重新加载” invokes `onReload(emailId)`.

---

### Task 1: IMAP Single UID Full Fetch

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/fetch.rs`

**Interfaces:**
- Consumes: existing `ImapClient`, `WholeEmailDto`, `MessageParser`, `parser::extract_headers_from_message`, `parser::extract_attachments`.
- Produces: `ImapClient::fetch_email_by_uid(&mut self, folder: &str, uid: u32) -> Result<Option<WholeEmailDto>, MailError>`.

- [ ] **Step 1: Add the single UID fetch method**

Append this method inside the existing `impl ImapClient` in `src-tauri/src/infrastructure/protocols/imap/fetch.rs`, near `batch_fetch_emails`:

```rust
    /// 按单个 UID 获取完整邮件。
    ///
    /// 返回 `Ok(None)` 表示服务器在当前文件夹中没有该 UID。
    pub async fn fetch_email_by_uid(
        &mut self,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        let query = uid.to_string();
        let mut fetches = self
            .session
            .uid_fetch(
                &query,
                "(FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[] BODYSTRUCTURE UID)",
            )
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        let Some(fetch_result) = fetches.next().await else {
            return Ok(None);
        };

        let fetch = fetch_result.map_err(|e| MailError::ImapError(e.to_string()))?;
        let Some(fetch_uid) = fetch.uid else {
            return Ok(None);
        };
        if fetch_uid != uid {
            return Ok(None);
        }

        let raw_body = fetch
            .body()
            .ok_or_else(|| MailError::ImapError("邮件体为空".to_string()))?;
        let message = MessageParser::default()
            .parse(raw_body)
            .ok_or_else(|| MailError::ImapError("解析邮件失败".to_string()))?;
        let headers = extract_headers_from_message(&message, chrono::Utc::now().timestamp());

        let seen = fetch.flags().any(|f| f == async_imap::types::Flag::Seen);
        let flagged = fetch.flags().any(|f| f == async_imap::types::Flag::Flagged);
        let answered = fetch
            .flags()
            .any(|f| f == async_imap::types::Flag::Answered);
        let deleted = fetch.flags().any(|f| f == async_imap::types::Flag::Deleted);
        let draft = fetch.flags().any(|f| f == async_imap::types::Flag::Draft);

        let body_text = message.body_text(0).map(|text| text.to_string());
        let body_html = message.body_html(0).map(|html| html.to_string());
        let preview = body_text.as_ref().map(|text| text.chars().take(200).collect());
        let attachments = fetch
            .bodystructure()
            .map(|bs| parser::extract_attachments(bs, ""))
            .unwrap_or_default();
        let received_at = fetch
            .internal_date()
            .map(|date| date.timestamp())
            .unwrap_or(headers.sent_at);

        Ok(Some(WholeEmailDto {
            id: 0,
            account_id: 0,
            folder: folder.to_string(),
            uid,
            message_id: headers.message_id,
            subject: headers.subject,
            sender_name: headers.sender_name,
            sender_email: headers.sender_email,
            recipient_emails: headers.recipient_emails,
            cc_emails: headers.cc_emails,
            bcc_emails: headers.bcc_emails,
            preview,
            body_text,
            body_html,
            attachments,
            is_read: seen,
            is_starred: flagged,
            is_draft: draft,
            is_answered: answered,
            is_deleted: deleted,
            sent_at: headers.sent_at,
            received_at,
            created_at: 0,
        }))
    }
```

- [ ] **Step 2: Run backend compile check**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk git add src-tauri/src/infrastructure/protocols/imap/fetch.rs
rtk git commit -m "feat: fetch single email by uid"
```

---

### Task 2: Repository Replacement and Local Removal

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`

**Interfaces:**
- Consumes: `WholeEmailDto`, existing `EmailWrite`, `AttachmentWrite`, `execute_email_insert`, `execute_attachment_insert`, `map_email`.
- Produces:
  - `replace_email_with_attachments(db, email_id, account_id, folder, email) -> Result<emails::Model, MailError>`
  - `delete_one_with_attachments(db, email_id) -> Result<bool, MailError>`

- [ ] **Step 1: Add a failing repository test for replacement**

Add this test inside the `#[cfg(test)] mod tests` block in `src-tauri/src/infrastructure/storage/repository/email_repo.rs`:

```rust
    #[tokio::test]
    async fn replace_email_with_attachments_overwrites_email_and_attachments() {
        use crate::infrastructure::protocols::types::{AttachmentInfo, WholeEmailDto};

        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let account_id = create_account(&db).await;
        let email_id = insert_email(&db, account_id, "INBOX", 100, "old body").await;
        insert_attachment(&db, email_id, "old.pdf").await;

        let replacement = WholeEmailDto {
            id: 0,
            account_id: 0,
            folder: "INBOX".to_string(),
            uid: 100,
            message_id: Some("<new@example.com>".to_string()),
            subject: Some("新主题".to_string()),
            sender_name: Some("New Sender".to_string()),
            sender_email: "new@example.com".to_string(),
            recipient_emails: "to@example.com".to_string(),
            cc_emails: Some("cc@example.com".to_string()),
            bcc_emails: None,
            preview: Some("新正文".to_string()),
            body_text: Some("新正文".to_string()),
            body_html: Some("<p>新正文</p>".to_string()),
            attachments: vec![AttachmentInfo {
                filename: Some("new.pdf".to_string()),
                content_type: "application/pdf".to_string(),
                size: 42,
                section_path: "2".to_string(),
                disposition: Some("attachment".to_string()),
                content_id: None,
            }],
            is_read: true,
            is_starred: true,
            is_draft: false,
            is_answered: true,
            is_deleted: false,
            sent_at: 1_800_000_100,
            received_at: 1_800_000_101,
            created_at: 0,
        };

        let updated =
            replace_email_with_attachments(&db, email_id, account_id, "INBOX", replacement)
                .await
                .unwrap();

        assert_eq!(updated.id, email_id);
        assert_eq!(updated.subject.as_deref(), Some("新主题"));
        assert_eq!(updated.body_text.as_deref(), Some("新正文"));

        let attachment_names = attachment_filenames(&db, email_id).await;
        assert_eq!(attachment_names, vec!["new.pdf".to_string()]);
    }
```

Add these helper functions in the same test module before the new test:

```rust
    async fn insert_attachment(db: &DbConn, email_id: i32, filename: &str) {
        let filename = filename.to_string();
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path,
                    disposition, content_id, path, created_at
                ) VALUES (?1, ?2, 'application/pdf', 10, '2', 'attachment', NULL, NULL, ?3)",
                rusqlite::params![email_id, filename, chrono::Utc::now().timestamp()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    }

    async fn attachment_filenames(db: &DbConn, email_id: i32) -> Vec<String> {
        db.call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT filename FROM attachments WHERE email_id = ?1 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([email_id], |row| row.get::<_, Option<String>>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map(|items| items.into_iter().flatten().collect())
        })
        .await
        .unwrap()
    }
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml replace_email_with_attachments_overwrites_email_and_attachments -- --nocapture
```

Expected: FAIL because `replace_email_with_attachments` is not defined.

- [ ] **Step 3: Implement repository functions**

Add these public functions near `update_body` in `email_repo.rs`:

```rust
pub async fn replace_email_with_attachments(
    db: &DbConn,
    email_id: i32,
    account_id: i32,
    folder: &str,
    email: WholeEmailDto,
) -> Result<emails::Model, MailError> {
    let now = chrono::Utc::now().timestamp();
    let folder = folder.to_string();
    let write = EmailWrite {
        account_id,
        folder: folder.clone(),
        uid: email.uid,
        message_id: email.message_id,
        subject: email.subject,
        sender_name: email.sender_name,
        sender_email: email.sender_email,
        recipient_emails: email.recipient_emails,
        cc_emails: email.cc_emails,
        bcc_emails: email.bcc_emails,
        preview: email.preview,
        body_text: email.body_text,
        body_html: email.body_html,
        is_read: Some(email.is_read),
        is_starred: Some(email.is_starred),
        is_draft: Some(email.is_draft),
        is_answered: Some(email.is_answered),
        is_deleted: Some(email.is_deleted),
        sent_at: email.sent_at,
        received_at: email.received_at,
        created_at: now,
        updated_at: now,
    };
    let attachments: Vec<AttachmentWrite> = email
        .attachments
        .into_iter()
        .map(|att| AttachmentWrite {
            email_id,
            filename: att.filename,
            content_type: Some(att.content_type),
            size: i64::from(att.size),
            section_path: att.section_path,
            disposition: att.disposition,
            content_id: att.content_id,
            path: None,
            created_at: now,
        })
        .collect();

    db.transaction(move |tx| {
        tx.execute(
            "UPDATE emails
             SET account_id = ?1, folder = ?2, uid = ?3, message_id = ?4, subject = ?5,
                 sender_name = ?6, sender_email = ?7, recipient_emails = ?8,
                 cc_emails = ?9, bcc_emails = ?10, preview = ?11, body_text = ?12,
                 body_html = ?13, is_read = ?14, is_starred = ?15, is_draft = ?16,
                 is_answered = ?17, is_deleted = ?18, sent_at = ?19, received_at = ?20,
                 created_at = ?21, updated_at = ?22
             WHERE id = ?23",
            rusqlite::params![
                write.account_id,
                write.folder,
                i64::from(write.uid),
                write.message_id,
                write.subject,
                write.sender_name,
                write.sender_email,
                write.recipient_emails,
                write.cc_emails,
                write.bcc_emails,
                write.preview,
                write.body_text,
                write.body_html,
                opt_bool_to_int(write.is_read),
                opt_bool_to_int(write.is_starred),
                opt_bool_to_int(write.is_draft),
                opt_bool_to_int(write.is_answered),
                opt_bool_to_int(write.is_deleted),
                write.sent_at,
                write.received_at,
                write.created_at,
                write.updated_at,
                email_id,
            ],
        )?;
        tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        let mut attachment_stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        for attachment in &attachments {
            execute_attachment_insert(&mut attachment_stmt, attachment)?;
        }
        let mut stmt = tx.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        stmt.query_row([email_id], map_email)
    })
    .await
}

pub async fn delete_one_with_attachments(db: &DbConn, email_id: i32) -> Result<bool, MailError> {
    db.transaction(move |tx| {
        tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        let deleted = tx.execute("DELETE FROM emails WHERE id = ?1", [email_id])?;
        Ok(deleted > 0)
    })
    .await
}
```

Ensure `WholeEmailDto` is imported at the top of the file:

```rust
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
```

- [ ] **Step 4: Run repository tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml replace_email_with_attachments_overwrites_email_and_attachments -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src-tauri/src/infrastructure/storage/repository/email_repo.rs
rtk git commit -m "feat: replace single email locally"
```

---

### Task 3: Backend Service and Command

**Files:**
- Modify: `src-tauri/src/service/mail_operation.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/common/mod.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `ImapClient::fetch_email_by_uid(folder, uid)`
  - `email_repo::replace_email_with_attachments`
  - `email_repo::delete_one_with_attachments`
- Produces:
  - `ReloadEmailResult`
  - `MailRemoteOperator::reload_email`
  - `EmailService::reload_email`
  - Tauri command `reload_email`.

- [ ] **Step 1: Extend the remote trait**

In `src-tauri/src/service/mail_operation.rs`, import `WholeEmailDto`:

```rust
use crate::infrastructure::protocols::types::WholeEmailDto;
```

Add this method to `MailRemoteOperator`:

```rust
    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError>;
```

Implement it for `RealMailRemoteOperator`:

```rust
    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        let mut client = self.connect_for_account(account).await?;
        let result = client.fetch_email_by_uid(folder, uid).await;
        client.logout().await.ok();
        result
    }
```

- [ ] **Step 2: Add result type and service method**

In `src-tauri/src/service/email_service.rs`, add imports:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
```

Add this type near `EmailDetail`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReloadEmailResult {
    Reloaded { email: EmailDetail },
    Removed { email_id: i32 },
}
```

Add this method to `impl EmailService`:

```rust
    pub async fn reload_email(&self, email_id: i32) -> Result<ReloadEmailResult, MailError> {
        let email = email_repo::get_by_id(&self.db, email_id)
            .await?
            .ok_or(MailError::EmailNotFound(email_id))?;
        let account = account_repo::get_by_id(&self.db, email.account_id)
            .await?
            .ok_or(MailError::AccountNotFound(email.account_id))?;

        match self
            .mail_operation
            .remote()
            .reload_email(&account, &email.folder, email.uid)
            .await?
        {
            Some(remote_email) => {
                let updated = email_repo::replace_email_with_attachments(
                    &self.db,
                    email_id,
                    account.id,
                    &email.folder,
                    remote_email,
                )
                .await?;
                let detail = self.get(updated.id).await?;
                Ok(ReloadEmailResult::Reloaded { email: detail })
            }
            None => {
                email_repo::delete_one_with_attachments(&self.db, email_id).await?;
                Ok(ReloadEmailResult::Removed { email_id })
            }
        }
    }
```

Add this accessor to `MailOperationService` in `src-tauri/src/service/mail_operation.rs`:

```rust
    pub(crate) fn remote(&self) -> Arc<dyn MailRemoteOperator> {
        self.remote.clone()
    }
```

- [ ] **Step 3: Add command and register it**

In `src-tauri/src/command/email.rs`, import `ReloadEmailResult` if needed and add:

```rust
#[tauri::command]
#[specta::specta]
pub async fn reload_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<crate::service::email_service::ReloadEmailResult, MailError> {
    tracing::info!(email_id, "命令: 重新加载邮件");
    service.reload_email(email_id).await
}
```

In `src-tauri/src/lib.rs`, add `command::email::reload_email` to the command handler list.

- [ ] **Step 4: Extend test fake remote**

In `src-tauri/tests/common/mod.rs`, import `WholeEmailDto` and add `reload_email` to `NoopMailRemoteOperator`:

```rust
    async fn reload_email(
        &self,
        _account: &accounts::Model,
        _folder: &str,
        _uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        Ok(None)
    }
```

In `src-tauri/tests/email_commands.rs`, update the existing `FakeMailRemote`:

```rust
reload_result: Mutex<Result<Option<WholeEmailDto>, MailError>>,
```

Update constructors:

```rust
    fn success() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: false,
            reload_result: Mutex::new(Ok(None)),
        })
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: true,
            reload_result: Mutex::new(Err(MailError::ImapConnectionFailed(
                "fake remote failure".to_string(),
            ))),
        })
    }

    fn with_reload(result: Result<Option<WholeEmailDto>, MailError>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: false,
            reload_result: Mutex::new(result),
        })
    }
```

Implement trait method:

```rust
    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("reload:{}:{folder}:{uid}", account.email));
        self.reload_result.lock().unwrap().clone()
    }
```

- [ ] **Step 5: Add backend tests**

In `src-tauri/tests/email_commands.rs`, import the needed types:

```rust
use postium_mail_lib::infrastructure::protocols::types::{AttachmentInfo, WholeEmailDto};
use postium_mail_lib::service::email_service::ReloadEmailResult;
```

Add this helper near `create_remote_test_account`:

```rust
fn remote_email(uid: u32, subject: &str, body: &str) -> WholeEmailDto {
    WholeEmailDto {
        id: 0,
        account_id: 0,
        folder: "INBOX".to_string(),
        uid,
        message_id: Some(format!("<reload-{uid}@example.com>")),
        subject: Some(subject.to_string()),
        sender_name: Some("Reload Sender".to_string()),
        sender_email: "reload@example.com".to_string(),
        recipient_emails: "recipient@example.com".to_string(),
        cc_emails: Some("copy@example.com".to_string()),
        bcc_emails: None,
        preview: Some(body.chars().take(200).collect()),
        body_text: Some(body.to_string()),
        body_html: Some(format!("<p>{body}</p>")),
        attachments: vec![AttachmentInfo {
            filename: Some("reload.pdf".to_string()),
            content_type: "application/pdf".to_string(),
            size: 42,
            section_path: "2".to_string(),
            disposition: Some("attachment".to_string()),
            content_id: None,
        }],
        is_read: true,
        is_starred: true,
        is_draft: false,
        is_answered: true,
        is_deleted: false,
        sent_at: 1_800_000_100,
        received_at: 1_800_000_101,
        created_at: 0,
    }
}
```

Add these tests:

```rust
#[tokio::test]
async fn test_reload_email_replaces_local_email_when_remote_exists() {
    let remote = FakeMailRemote::with_reload(Ok(Some(remote_email(514, "远端新主题", "远端新正文"))));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(514, "本地旧主题").body_text("本地旧正文")).await;

    let result = svc.email_service.reload_email(email_id).await.unwrap();

    assert!(matches!(result, ReloadEmailResult::Reloaded { .. }));
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.subject.as_deref(), Some("远端新主题"));
    assert_eq!(detail.body_text.as_deref(), Some("远端新正文"));
    assert!(detail.email.is_read);
    assert!(detail.email.is_starred);
    assert!(detail.email.is_answered);
    assert_eq!(detail.email.sender_email, "reload@example.com");
    assert_eq!(
        remote.calls(),
        vec!["reload:test@example.com:INBOX:514"]
    );
}

#[tokio::test]
async fn test_reload_email_removes_local_email_when_remote_uid_missing() {
    let remote = FakeMailRemote::with_reload(Ok(None));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(515, "远端不存在")).await;

    let result = svc.email_service.reload_email(email_id).await.unwrap();

    assert!(matches!(
        result,
        ReloadEmailResult::Removed { email_id: removed_id } if removed_id == email_id
    ));
    assert!(matches!(
        svc.email_service.get(email_id).await,
        Err(MailError::EmailNotFound(id)) if id == email_id
    ));
    assert_eq!(
        remote.calls(),
        vec!["reload:test@example.com:INBOX:515"]
    );
}

#[tokio::test]
async fn test_reload_email_remote_error_keeps_local_email() {
    let remote = FakeMailRemote::with_reload(Err(MailError::ImapError("remote failed".to_string())));
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_remote_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(516, "保留主题").body_text("保留正文")).await;

    let result = svc.email_service.reload_email(email_id).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.subject.as_deref(), Some("保留主题"));
    assert_eq!(detail.body_text.as_deref(), Some("保留正文"));
    assert_eq!(
        remote.calls(),
        vec!["reload:test@example.com:INBOX:516"]
    );
}
```

- [ ] **Step 6: Run backend tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml reload_email -- --nocapture
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands reload -- --nocapture
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
rtk git add src-tauri/src/service/mail_operation.rs src-tauri/src/service/email_service.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/common/mod.rs src-tauri/tests/email_commands.rs
rtk git commit -m "feat: add backend email reload"
```

---

### Task 4: Frontend Bindings and Store

**Files:**
- Modify: `src/lib/bindings.ts` (generated by Tauri Specta; do not hand-edit except to keep generated output)
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src/lib/__tests__/stores/email-state.test.ts`

**Interfaces:**
- Consumes: backend command `reload_email(email_id)` returning `ReloadEmailResult`.
- Produces: `commands.reloadEmail(emailId)` and `EmailState.reloadEmail(emailId)`.

- [ ] **Step 1: Regenerate binding type and command**

After Task 3 registers `command::email::reload_email` in `create_specta_builder`, regenerate `src/lib/bindings.ts` through the existing debug export path:

```bash
rtk timeout 10s cargo run --manifest-path src-tauri/Cargo.toml --bin postium-mail
```

This command may time out or stop after trying to launch the desktop app; the expected side effect is that `src/lib/bindings.ts` is rewritten before app startup. Verify the generated file contains the new command and type:

```bash
rtk rg -n "ReloadEmailResult|reloadEmail" src/lib/bindings.ts
```

Expected generated shape:

```ts
export type ReloadEmailResult =
  | { status: "reloaded"; email: EmailDetail }
  | { status: "removed"; email_id: number };
```

The generated `commands` object should also contain:

```ts
reloadEmail: (emailId: number) =>
  typedError<ReloadEmailResult, MailError>(
    __TAURI_INVOKE("reload_email", { emailId }),
  ),
```

- [ ] **Step 2: Write failing store tests**

In `src/lib/__tests__/stores/email-state.test.ts`, add:

```ts
it("reloadEmail 收到 reloaded 时更新列表和当前详情", async () => {
  const state = new EmailState();
  state.emails = [{ ...email, id: 1, subject: "旧主题", preview: "旧预览" }];
  state.selectedEmailId = 1;
  state.selectedEmail = { ...emailDetail, subject: "旧主题", body_text: "旧正文" };
  const reloadedEmail = { ...email, id: 1, subject: "新主题", preview: "新预览" };
  mockInvoke.mockResolvedValueOnce({
    status: "reloaded",
    email: { ...emailDetail, ...reloadedEmail, body_text: "新正文" },
  });

  const ok = await state.reloadEmail(1);

  expect(ok).toBe(true);
  expect(state.emails[0]?.subject).toBe("新主题");
  expect(state.selectedEmail?.body_text).toBe("新正文");
  expect(mockInvoke).toHaveBeenCalledWith("reload_email", { emailId: 1 });
});

it("reloadEmail 收到 removed 时移除列表并取消选择", async () => {
  const state = new EmailState();
  state.emails = [{ ...email, id: 1 }, { ...email, id: 2 }];
  state.total = 2;
  state.selectedEmailId = 1;
  state.selectedEmail = { ...emailDetail, id: 1 };
  mockInvoke.mockResolvedValueOnce({
    status: "removed",
    email_id: 1,
  });

  const ok = await state.reloadEmail(1);

  expect(ok).toBe(true);
  expect(state.emails.map((email) => email.id)).toEqual([2]);
  expect(state.selectedEmailId).toBeNull();
  expect(state.selectedEmail).toBeNull();
  expect(state.total).toBe(1);
});

it("reloadEmail 后端失败时保留本地状态", async () => {
  const state = new EmailState();
  state.emails = [{ ...email, id: 1, subject: "旧主题" }];
  mockInvoke.mockRejectedValueOnce(new Error("remote failed"));

  const ok = await state.reloadEmail(1);

  expect(ok).toBe(false);
  expect(state.emails[0]?.subject).toBe("旧主题");
  expect(state.error).toContain("remote failed");
});
```

- [ ] **Step 3: Run failing store tests**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1 --reporter dot
```

Expected: FAIL because `reloadEmail` is not defined.

- [ ] **Step 4: Implement store method**

In `src/lib/stores/email.svelte.ts`, add:

```ts
  async reloadEmail(emailId: number): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.reloadEmail(emailId);

      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }

      if (result.data.status === "reloaded") {
        const detail = result.data.email;
        const updatedEmail: EmailDto = {
          id: detail.id,
          account_id: detail.account_id,
          folder: detail.folder,
          uid: detail.uid,
          subject: detail.subject,
          sender_name: detail.sender_name,
          sender_email: detail.sender_email,
          preview: detail.preview,
          is_read: detail.is_read,
          is_starred: detail.is_starred,
          sent_at: detail.sent_at,
          has_attachments: detail.has_attachments,
        };
        this.emails = this.emails.map((email) =>
          email.id === emailId ? updatedEmail : email,
        );
        if (this.selectedEmailId === emailId) {
          this.selectedEmail = detail;
        }
        return true;
      }

      const removedId = result.data.email_id;
      const before = this.emails.length;
      this.emails = this.emails.filter((email) => email.id !== removedId);
      const removed = before - this.emails.length;
      if (this.selectedEmailId === removedId) {
        this.deselectEmail();
      }
      this.total = Math.max(0, this.total - removed);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to reload email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }
```

- [ ] **Step 5: Run store tests**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1 --reporter dot
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/bindings.ts src/lib/stores/email.svelte.ts src/lib/__tests__/stores/email-state.test.ts
rtk git commit -m "feat: add email reload store action"
```

---

### Task 5: Context Menu UI Wiring

**Files:**
- Modify: `src/lib/components/email/EmailContextMenu.svelte`
- Modify: `src/lib/components/email/EmailList.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Add: `src/lib/__tests__/components/EmailContextMenu.test.ts`

**Interfaces:**
- Consumes: `EmailState.reloadEmail(emailId: number)`.
- Produces: visible context menu item “重新加载” that calls `onReload(emailId)`.

- [ ] **Step 1: Update context menu props and action**

Add `reload` under the existing `email` object in `src/lib/i18n/zh-CN.ts`:

```ts
reload: "重新加载",
```

Add the matching key under `email` in `src/lib/i18n/en-US.ts`:

```ts
reload: "Reload",
```

In `EmailContextMenu.svelte`, import `RefreshCw`:

```ts
        RefreshCw, // 重新加载图标
```

Add `onReload` to the `$props()` destructuring and type:

```ts
        onReload,
```

```ts
        onReload: (emailId: number) => Promise<void> | void;
```

Add action branch:

```ts
        } else if (action === "reload") {
            await onReload(emailId);
```

- [ ] **Step 2: Add menu item**

Place this button after the mark-read button and before the divider above delete:

```svelte
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("reload")}
        disabled={disabled}
    >
        <RefreshCw size={15} class="text-muted-foreground" />
        <span>{t.email.reload}</span>
    </button>
```

- [ ] **Step 3: Wire EmailList**

In `EmailList.svelte`, add:

```ts
    async function handleContextReload(emailId: number) {
        await emailState.reloadEmail(emailId);
    }
```

Pass it to `EmailContextMenu`:

```svelte
            onReload={handleContextReload}
```

- [ ] **Step 4: Add context menu component test**

Create `src/lib/__tests__/components/EmailContextMenu.test.ts`:

```ts
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import EmailContextMenu from "$lib/components/email/EmailContextMenu.svelte";

vi.mock("$lib/stores/i18n.svelte", () => ({
  getI18nState: () => ({
    t: {
      email: {
        reply: "回复",
        replyAll: "全部回复",
        forward: "转发",
        star: "星标",
        unstar: "取消星标",
        markRead: "标记已读",
        markUnread: "标记未读",
        reload: "重新加载",
        delete: "删除",
      },
      common: {
        operations: "更多操作",
      },
    },
  }),
}));

describe("EmailContextMenu", () => {
  it("点击重新加载时调用 onReload", async () => {
    const onReload = vi.fn();
    const onClose = vi.fn();

    render(EmailContextMenu, {
      props: {
        x: 0,
        y: 0,
        emailId: 42,
        isRead: false,
        isStarred: false,
        onToggleStar: vi.fn(),
        onToggleRead: vi.fn(),
        onDelete: vi.fn(),
        onForward: vi.fn(),
        onReload,
        onClose,
      },
    });

    await fireEvent.click(screen.getByRole("button", { name: "重新加载" }));

    expect(onReload).toHaveBeenCalledWith(42);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
```

- [ ] **Step 5: Run frontend tests and Svelte validation**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/components/EmailContextMenu.test.ts --pool threads --maxWorkers 1 --reporter dot
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/email/EmailContextMenu.svelte --svelte-version 5
rtk npm run check
rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json
rtk npm run build
```

Expected: all pass; `svelte-autofixer` reports no required fixes.

- [ ] **Step 6: Commit**

```bash
rtk git add src/lib/components/email/EmailContextMenu.svelte src/lib/components/email/EmailList.svelte src/lib/i18n/zh-CN.ts src/lib/i18n/en-US.ts src/lib/__tests__/components/EmailContextMenu.test.ts
rtk git commit -m "feat: add reload action to email menu"
```

---

### Task 6: Final Verification

**Files:**
- No code changes expected.

**Interfaces:**
- Consumes all previous tasks.
- Produces verified single-email reload feature.

- [ ] **Step 1: Run full backend test suite**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all non-ignored tests pass.

- [ ] **Step 2: Run backend compile check**

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests**

```bash
rtk node node_modules/vitest/vitest.mjs run --pool threads --maxWorkers 1 --reporter dot
```

Expected: all tests pass.

- [ ] **Step 4: Run TypeScript check**

```bash
rtk npm run check
rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json
```

Expected: PASS.

- [ ] **Step 5: Run Svelte component autofixer**

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/email/EmailContextMenu.svelte --svelte-version 5
```

Expected: no required fixes.

- [ ] **Step 6: Run production build**

```bash
rtk npm run build
```

Expected: PASS.

- [ ] **Step 7: Inspect final diff**

```bash
rtk git status --short
rtk git log --oneline -6
```

Expected: working tree clean; recent commits include the task commits.

---

## Self-Review

- Spec coverage:
  - Right-click menu action is covered in Task 5.
  - `reload_email(email_id)` command is covered in Task 3.
  - Full RFC822 single UID fetch is covered in Task 1.
  - Local email plus attachment replacement is covered in Task 2.
  - Remote UID missing local removal is covered in Tasks 2, 3, and 4.
  - No full sync or sync-state update is preserved by using a dedicated service path in Task 3.
  - Frontend `Reloaded` and `Removed` handling is covered in Task 4.
- Placeholder scan:
  - No TBD/TODO placeholders.
  - No intentionally vague implementation steps remain.
- Type consistency:
  - Backend command is `reload_email(email_id: i32)`.
  - Frontend wrapper is `reloadEmail(emailId: number)` and passes `{ emailId }`, matching Tauri camel-case argument mapping used elsewhere.
  - Result discriminant is `status: "reloaded" | "removed"` on frontend, matching `#[serde(tag = "status", rename_all = "snake_case")]`.
