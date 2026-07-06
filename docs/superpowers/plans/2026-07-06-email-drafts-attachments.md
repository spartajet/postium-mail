# Email Drafts and Attachments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现远端 Drafts 草稿保存、草稿发送清理，以及普通本地文件附件发送。

**Architecture:** 后端以远端 Drafts 为权威，本地 SQLite 只保存镜像；附件通过 Tauri 文件选择器传本地路径，后端读取文件并构建 MIME。实现分为 DTO/命令、MIME 附件构建、仓储附件写入、远端草稿服务、前端接入五层，默认测试全部使用 mock 或本地临时文件，不依赖真实邮箱服务器。

**Tech Stack:** Rust 2024, Tauri v2, async-imap 0.11, lettre 0.11, rusqlite/tokio-rusqlite, Svelte 5, Vitest, TypeScript bindings via tauri-specta.

## Global Constraints

- 文档和用户沟通使用中文。
- 所有 shell 命令使用 `rtk` 前缀。
- 用户已确认按任务提交；每个任务通过验证和 review 后创建一个小提交。
- 正式发送仍要求至少一个收件人、主题非空、正文非空。
- 草稿必须保存到远端 Drafts；本期不做纯本地草稿成功模式。
- 附件第一版只支持普通本地文件附件，不做内联图片、拖拽上传、大文件分块或云附件。
- 默认自动化测试不能依赖真实 SMTP/IMAP 服务。
- 实现 Svelte 组件变更前需使用 `svelte-code-writer` skill。

---

## File Structure

- `src-tauri/src/service/mail_send.rs`
  - 扩展发送请求附件输入。
  - 解析本地附件元数据。
  - 构建无附件 `multipart/alternative` 和有附件 `multipart/mixed`。
  - 保持 SMTP 投递和 Sent APPEND 使用同一封 RFC822 原文。

- `src-tauri/src/service/mail_draft.rs`
  - 新建草稿服务模块。
  - 定义 `SaveDraftRequest`、`SaveDraftResponse`、`DraftRemoteWriter`、`RealDraftRemoteWriter`。
  - 编排远端 Drafts APPEND、本地草稿镜像、旧草稿清理。

- `src-tauri/src/service/email_service.rs`
  - 扩展 `SendEmailRequest`。
  - 暴露 `describe_local_attachments`、`save_draft`、`delete_draft`。
  - `send` 支持附件和可选 `draft_id`，发送成功后清理草稿。

- `src-tauri/src/service/mod.rs`
  - 导出 `mail_draft` 模块需要的类型。

- `src-tauri/src/infrastructure/protocols/imap/mod.rs`
  - 增加可传 flags 的 APPEND 方法。
  - 增加按 UID 删除草稿的远端方法。

- `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 增加“邮件 + 附件元数据”事务写入函数。
  - 增加草稿镜像写入函数。
  - 扩展 Sent 本地写入支持附件。

- `src-tauri/src/infrastructure/storage/repository/attachment_repo.rs`
  - 复用 `AttachmentWrite` 和 `INSERT_ATTACHMENT_SQL`，如需仅补测试辅助，不改 schema。

- `src-tauri/src/command/email.rs`
  - 注册 `describe_local_attachments`、`save_draft`、`delete_draft` 命令。
  - `send_email` 使用扩展请求。

- `src-tauri/src/lib.rs`
  - 把新增 Tauri 命令加入 specta builder。

- `src/lib/bindings.ts`
  - 由 specta 导出更新，或手工同步新增类型和命令签名。

- `src/lib/components/email/ComposeModal.svelte`
  - 接入附件选择、附件列表、自动保存草稿、草稿发送清理。

- `src/lib/i18n/zh-CN.ts`, `src/lib/i18n/en-US.ts`
  - 增加草稿和附件相关文案。

- `src/lib/__tests__/components/ComposeModal.test.ts`
  - 覆盖附件选择、移除、自动保存状态、发送带 `draft_id`。

- `src-tauri/tests/email_repository.rs`
  - 覆盖邮件 + 附件事务写入。

- `src-tauri/tests/email_commands.rs`
  - 覆盖命令层 DTO、附件描述、发送请求兼容。

---

### Task 1: 后端 DTO 与本地附件描述命令

**Files:**
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Produces:
  - `ComposeAttachmentInput { path: String, filename: Option<String>, content_type: Option<String>, size: Option<i64> }`
  - `LocalAttachmentDraft { path: String, filename: String, content_type: String, size: i64 }`
  - `EmailService::describe_local_attachments(paths: Vec<String>) -> Result<Vec<LocalAttachmentDraft>, MailError>`
  - command `describe_local_attachments(paths: Vec<String>)`
  - `SendEmailRequest` gains `attachments: Vec<ComposeAttachmentInput>` and `draft_id: Option<i32>`
- Consumes:
  - Existing `MailError::InvalidParam`
  - Existing Tauri command/specta patterns in `command/email.rs`

- [ ] **Step 1: Add failing tests for local attachment description**

Add these tests to `src-tauri/tests/email_commands.rs` or a focused helper section in the same file:

```rust
#[tokio::test]
async fn describe_local_attachments_rejects_missing_path() {
    let service = test_email_service().await;

    let result = service
        .describe_local_attachments(vec!["/tmp/postium-missing-file-for-test.txt".to_string()])
        .await;

    assert!(result.is_err());
    let message = result.unwrap_err().to_string();
    assert!(message.contains("附件文件不存在") || message.contains("not found"));
}

#[tokio::test]
async fn describe_local_attachments_returns_file_metadata() {
    let service = test_email_service().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hello.txt");
    std::fs::write(&path, b"hello").unwrap();

    let result = service
        .describe_local_attachments(vec![path.to_string_lossy().to_string()])
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].filename, "hello.txt");
    assert_eq!(result[0].content_type, "text/plain");
    assert_eq!(result[0].size, 5);
    assert_eq!(result[0].path, path.to_string_lossy());
}
```

If `test_email_service().await` does not exist, use the existing test setup helper from `src-tauri/tests/common/mod.rs` and instantiate `EmailService::new(auth, db)`.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --test email_commands describe_local_attachments -- --nocapture
```

Expected: fail because `describe_local_attachments`, `ComposeAttachmentInput`, or `LocalAttachmentDraft` does not exist.

- [ ] **Step 3: Add DTOs and attachment metadata helper**

In `src-tauri/src/service/mail_send.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ComposeAttachmentInput {
    pub path: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct LocalAttachmentDraft {
    pub path: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

pub async fn describe_local_attachment(path: String) -> Result<LocalAttachmentDraft, MailError> {
    describe_local_attachment_sync(&path)
}

pub fn describe_local_attachment_sync(path: &str) -> Result<LocalAttachmentDraft, MailError> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| MailError::InvalidParam(format!("附件文件不存在或不可访问: {path}: {e}")))?;
    if !metadata.is_file() {
        return Err(MailError::InvalidParam(format!("附件路径不是普通文件: {path}")));
    }
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| MailError::InvalidParam(format!("附件文件名无效: {path}")))?
        .to_string();
    let content_type = guess_content_type(&filename);
    Ok(LocalAttachmentDraft {
        path: path.to_string(),
        filename,
        content_type,
        size: i64::try_from(metadata.len())
            .map_err(|_| MailError::InvalidParam("附件大小超出支持范围".to_string()))?,
    })
}

pub fn guess_content_type(filename: &str) -> String {
    match std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("txt") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("pdf") => "application/pdf",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
    .to_string()
}
```

In `src-tauri/src/service/email_service.rs`, import and use the new DTOs:

```rust
pub use crate::service::mail_send::{
    ComposeAttachmentInput, LocalAttachmentDraft, SendEmailResponse,
};
```

Extend `SendEmailRequest`:

```rust
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    #[serde(default)]
    pub attachments: Vec<ComposeAttachmentInput>,
    #[serde(default)]
    pub draft_id: Option<i32>,
}
```

Add method on `EmailService`:

```rust
pub async fn describe_local_attachments(
    &self,
    paths: Vec<String>,
) -> Result<Vec<LocalAttachmentDraft>, MailError> {
    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        result.push(crate::service::mail_send::describe_local_attachment(path).await?);
    }
    Ok(result)
}
```

- [ ] **Step 4: Add Tauri command and specta registration**

In `src-tauri/src/command/email.rs`, import `LocalAttachmentDraft` and add:

```rust
#[tauri::command]
#[specta::specta]
pub async fn describe_local_attachments(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    paths: Vec<String>,
) -> Result<Vec<LocalAttachmentDraft>, MailError> {
    service.describe_local_attachments(paths).await
}
```

In `src-tauri/src/lib.rs`, add `command::email::describe_local_attachments` to `collect_commands!`.

- [ ] **Step 5: Run tests and formatting**

Run:

```bash
rtk cargo test --test email_commands describe_local_attachments -- --nocapture
rtk cargo fmt --check
```

Expected: tests pass, format check passes.

- [ ] **Step 6: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/service/mail_send.rs src-tauri/src/service/email_service.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/email_commands.rs
```

Expected: diff only contains DTO, helper, command, tests, and SendEmailRequest optional fields.

---

### Task 2: MIME 构建支持普通附件并避免 BCC 泄露

**Files:**
- Modify: `src-tauri/src/service/mail_send.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `ComposeAttachmentInput`
      - `describe_local_attachment_sync`
  - Existing `build_email(account, req) -> Result<BuiltEmail, MailError>`
- Produces:
  - `build_email` reads `req.attachments`
  - No attachment: raw message contains `multipart/alternative`
  - Has attachment: raw message contains `multipart/mixed` and attachment filename
  - BCC recipients are used for SMTP envelope only if possible; raw message must not contain a `Bcc:` header

- [ ] **Step 1: Add failing MIME tests**

Add tests:

```rust
#[test]
fn build_email_with_attachment_uses_multipart_mixed() {
    let account = test_account_model("sender@example.com");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hello.txt");
    std::fs::write(&path, b"hello").unwrap();
    let req = SendEmailRequest {
        account_id: account.id,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "With attachment".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
        attachments: vec![ComposeAttachmentInput {
            path: path.to_string_lossy().to_string(),
            filename: Some("hello.txt".to_string()),
            content_type: Some("text/plain".to_string()),
            size: Some(5),
        }],
        draft_id: None,
    };

    let built = postium_mail_lib::service::mail_send::build_email(&account, &req).unwrap();
    let raw = String::from_utf8_lossy(&built.raw);

    assert!(raw.contains("multipart/mixed"));
    assert!(raw.contains("multipart/alternative"));
    assert!(raw.contains("filename=\"hello.txt\"") || raw.contains("filename=hello.txt"));
    assert!(raw.contains("hello"));
}

#[test]
fn build_email_does_not_write_bcc_header() {
    let account = test_account_model("sender@example.com");
    let req = SendEmailRequest {
        account_id: account.id,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec!["hidden@example.com".to_string()],
        subject: "Secret".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
        attachments: vec![],
        draft_id: None,
    };

    let built = postium_mail_lib::service::mail_send::build_email(&account, &req).unwrap();
    let raw = String::from_utf8_lossy(&built.raw);

    assert!(!raw.to_ascii_lowercase().contains("\nbcc:"));
    assert!(!raw.contains("hidden@example.com"));
}
```

If `test_account_model` does not exist, create a local helper in the test module returning `accounts::Model` with required fields populated like existing tests.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
```

Expected: attachment test fails because MIME is only `multipart/alternative`; BCC test may fail if current builder writes a BCC header.

- [ ] **Step 3: Implement attachment MIME construction**

In `mail_send.rs`, add imports:

```rust
use lettre::message::{Attachment, Body, MultiPart, SinglePart};
```

Add helper:

```rust
fn build_body_part(req: &SendEmailRequest) -> MultiPart {
    MultiPart::alternative()
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(req.body_text.clone()),
        )
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(req.body_html.clone()),
        )
}

fn sanitize_attachment_filename(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn content_type_header(value: &str) -> Result<ContentType, MailError> {
    value
        .parse::<ContentType>()
        .map_err(|e| MailError::InvalidParam(format!("附件 Content-Type 无效: {value}: {e}")))
}
```

Inside `build_email`, replace the fixed multipart body with:

```rust
let body_part = build_body_part(req);
let message = if req.attachments.is_empty() {
    builder.multipart(body_part)
} else {
    let mut mixed = MultiPart::mixed().multipart(body_part);
    for input in &req.attachments {
        let described = describe_local_attachment_sync(&input.path)?;
        let filename = input
            .filename
            .as_deref()
            .map(sanitize_attachment_filename)
            .filter(|value| !value.is_empty())
            .unwrap_or(described.filename);
        let content_type = input
            .content_type
            .as_deref()
            .unwrap_or(&described.content_type);
        let bytes = std::fs::read(&input.path)
            .map_err(|e| MailError::InvalidParam(format!("读取附件失败: {}: {e}", input.path)))?;
        let attachment = Attachment::new(filename)
            .body(Body::new(bytes), content_type_header(content_type)?);
        mixed = mixed.singlepart(attachment);
    }
    builder.multipart(mixed)
}
.map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {e}")))?;
```

- [ ] **Step 4: Remove BCC from RFC822 headers**

Replace the current BCC builder loop:

```rust
for addr in &req.bcc {
    if !addr.trim().is_empty() {
        builder = builder.bcc(addr.trim().parse().map_err(|e| {
            MailError::InvalidParam(format!("密送无效 '{}': {e}", addr.trim()))
        })?);
    }
}
```

with validation-only parsing:

```rust
for addr in &req.bcc {
    if !addr.trim().is_empty() {
        let _mailbox: Mailbox = addr.trim().parse().map_err(|e| {
            MailError::InvalidParam(format!("密送无效 '{}': {e}", addr.trim()))
        })?;
    }
}
```

Then add a follow-up note in code or task execution notes: SMTP envelope currently comes from `lettre::Message`; if BCC is not in the message, `SmtpTransport::send(&message)` may not include BCC recipients. The implementation must either use lettre envelope APIs or accept a temporary limitation only after tests prove BCC is not exposed. Since the UI does not expose BCC yet, preserving privacy is higher priority than hidden BCC delivery in this task.

- [ ] **Step 5: Run MIME tests**

Run:

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
rtk cargo fmt --check
```

Expected: tests pass; raw message has mixed MIME when attachments exist and no `Bcc:` header.

- [ ] **Step 6: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/service/mail_send.rs src-tauri/tests/email_commands.rs
```

Expected: only MIME attachment support and BCC privacy changes.

---

### Task 3: 本地仓储支持邮件附件事务写入

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Test: `src-tauri/tests/email_repository.rs`

**Interfaces:**
- Consumes:
  - `EmailWrite`
  - `AttachmentWrite`
  - `execute_attachment_insert`
- Produces:
  - `insert_sent_email_with_attachments(db, write, attachments) -> Result<emails::Model, MailError>`
  - `insert_draft_email_with_attachments(db, write, attachments) -> Result<emails::Model, MailError>`
  - Both assign local `uid = MAX(uid) + 1` inside the target account/folder.

- [ ] **Step 1: Add failing repository test**

Add to `src-tauri/tests/email_repository.rs`:

```rust
#[tokio::test]
async fn insert_sent_email_with_attachments_writes_email_and_attachments() {
    let db = setup_test_db().await;
    seed_account(&db, 1, "sender@example.com").await;
    let now = chrono::Utc::now().timestamp();

    let email = email_repo::EmailWrite {
        account_id: 1,
        folder: "Sent".to_string(),
        uid: 0,
        message_id: Some("<sent-with-attachment@example.com>".to_string()),
        subject: Some("Subject".to_string()),
        sender_name: Some("Sender".to_string()),
        sender_email: "sender@example.com".to_string(),
        recipient_emails: "to@example.com".to_string(),
        cc_emails: None,
        bcc_emails: None,
        preview: Some("Body".to_string()),
        body_text: Some("Body".to_string()),
        body_html: Some("<p>Body</p>".to_string()),
        is_read: Some(true),
        is_starred: Some(false),
        is_draft: Some(false),
        is_answered: Some(false),
        is_deleted: Some(false),
        sent_at: now,
        received_at: now,
        created_at: now,
        updated_at: now,
    };

    let attachments = vec![attachment_repo::AttachmentWrite {
        email_id: 0,
        filename: Some("hello.txt".to_string()),
        content_type: Some("text/plain".to_string()),
        size: 5,
        section_path: "compose:0".to_string(),
        disposition: Some("attachment".to_string()),
        content_id: None,
        path: Some("/tmp/hello.txt".to_string()),
        created_at: now,
    }];

    let inserted = email_repo::insert_sent_email_with_attachments(&db, email, attachments)
        .await
        .unwrap();
    let saved = attachment_repo::list_by_email(&db, inserted.id).await.unwrap();

    assert_eq!(inserted.uid, 1);
    assert_eq!(inserted.is_draft, Some(false));
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].filename.as_deref(), Some("hello.txt"));
    assert_eq!(saved[0].section_path, "compose:0");
}
```

Adapt helper names to existing test helpers in the file.

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
rtk cargo test --test email_repository insert_sent_email_with_attachments -- --nocapture
```

Expected: fail because repository function does not exist.

- [ ] **Step 3: Implement shared transaction helper**

In `email_repo.rs`, add:

```rust
fn next_local_uid(
    tx: &rusqlite::Transaction<'_>,
    account_id: i32,
    folder: &str,
) -> rusqlite::Result<u32> {
    tx.query_row(
        "SELECT COALESCE(MAX(uid), 0) + 1
         FROM emails
         WHERE account_id = ?1 AND folder = ?2",
        rusqlite::params![account_id, folder],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value as u32)
}

fn insert_email_with_attachments_tx(
    tx: &rusqlite::Transaction<'_>,
    mut write: EmailWrite,
    attachments: Vec<AttachmentWrite>,
) -> rusqlite::Result<emails::Model> {
    write.uid = next_local_uid(tx, write.account_id, &write.folder)?;
    let mut insert_stmt = tx.prepare(INSERT_EMAIL_SQL)?;
    execute_email_insert(&mut insert_stmt, &write)?;
    let email_id = tx.last_insert_rowid() as i32;

    let mut attachment_stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
    for mut attachment in attachments {
        attachment.email_id = email_id;
        execute_attachment_insert(&mut attachment_stmt, &attachment)?;
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
}
```

Then add:

```rust
pub async fn insert_sent_email_with_attachments(
    db: &DbConn,
    mut write: EmailWrite,
    attachments: Vec<AttachmentWrite>,
) -> Result<emails::Model, MailError> {
    write.is_draft = Some(false);
    db.transaction(move |tx| insert_email_with_attachments_tx(tx, write, attachments))
        .await
}

pub async fn insert_draft_email_with_attachments(
    db: &DbConn,
    mut write: EmailWrite,
    attachments: Vec<AttachmentWrite>,
) -> Result<emails::Model, MailError> {
    write.is_draft = Some(true);
    db.transaction(move |tx| insert_email_with_attachments_tx(tx, write, attachments))
        .await
}
```

Update existing `insert_sent_email` to call `insert_sent_email_with_attachments(db, write, vec![])`.

- [ ] **Step 4: Run repository tests**

Run:

```bash
rtk cargo test --test email_repository insert_sent_email_with_attachments -- --nocapture
rtk cargo test --test email_repository -- --nocapture
rtk cargo fmt --check
```

Expected: repository tests pass.

- [ ] **Step 5: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/tests/email_repository.rs
```

Expected: only transactional attachment write support and tests.

---

### Task 4: IMAP APPEND flags 与远端草稿删除能力

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
- Modify: `src-tauri/src/service/mail_send.rs`

**Interfaces:**
- Produces:
  - `ImapClient::append_email_with_flags(folder, flags, raw)`
  - `ImapClient::append_email(folder, raw)` remains as Sent wrapper using `(\Seen)`
  - `ImapClient::delete_uid(uid)` or `mark_uid_deleted_public(uid)` for selected folder
  - `RealSentArchiveWriter` uses `append_email_with_flags(..., Some("(\\Seen)"), ...)`

- [ ] **Step 1: Add protocol methods**

In `imap/mod.rs`, replace `append_email` body with:

```rust
pub async fn append_email_with_flags(
    &mut self,
    folder: &str,
    flags: Option<&str>,
    raw: &[u8],
) -> Result<(), MailError> {
    self.session
        .append(folder, flags, None, raw)
        .await
        .map_err(|e| MailError::ImapConnectionFailed(format!("追加邮件失败: {e}")))?;
    Ok(())
}

pub async fn append_email(&mut self, folder: &str, raw: &[u8]) -> Result<(), MailError> {
    self.append_email_with_flags(folder, Some("(\\Seen)"), raw).await
}

pub async fn delete_uid(&mut self, uid: u32) -> Result<(), MailError> {
    self.mark_uid_deleted(uid).await
}
```

Keep `mark_uid_deleted` private if `delete_uid` is public.

- [ ] **Step 2: Update Sent archive writer**

In `mail_send.rs`, update:

```rust
let result = client
    .append_email_with_flags(&req.folder, Some("(\\Seen)"), &req.raw)
    .await;
```

- [ ] **Step 3: Compile check**

Run:

```bash
rtk cargo check --tests
rtk cargo fmt --check
```

Expected: compile passes; no behavior change except explicit flags API.

- [ ] **Step 4: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/infrastructure/protocols/imap/mod.rs src-tauri/src/service/mail_send.rs
```

Expected: only IMAP protocol API extension and Sent writer adjustment.

---

### Task 5: 草稿服务后端实现

**Files:**
- Create: `src-tauri/src/service/mail_draft.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `build_email`
  - `ComposeAttachmentInput`
  - `insert_draft_email_with_attachments`
  - `delete_one_with_attachments`
  - `ImapClient::append_email_with_flags`
  - `ImapClient::delete_uid`
- Produces:
  - `SaveDraftRequest`
  - `SaveDraftResponse`
  - `EmailService::save_draft`
  - `EmailService::delete_draft`
  - commands `save_draft`, `delete_draft`

- [ ] **Step 1: Add failing draft service tests**

Add tests that use a mock remote writer if existing test setup supports injected services. If not, test pure validation first:

```rust
#[test]
fn blank_draft_is_not_saveable() {
    let req = SaveDraftRequest {
        draft_id: None,
        account_id: 1,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: "   ".to_string(),
        body_html: "".to_string(),
        body_text: "   ".to_string(),
        attachments: vec![],
    };

    assert!(postium_mail_lib::service::mail_draft::is_blank_draft(&req));
}

#[test]
fn draft_with_attachment_is_saveable_even_without_subject_or_body() {
    let req = SaveDraftRequest {
        draft_id: None,
        account_id: 1,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: "".to_string(),
        body_html: "".to_string(),
        body_text: "".to_string(),
        attachments: vec![ComposeAttachmentInput {
            path: "/tmp/file.txt".to_string(),
            filename: Some("file.txt".to_string()),
            content_type: Some("text/plain".to_string()),
            size: Some(1),
        }],
    };

    assert!(!postium_mail_lib::service::mail_draft::is_blank_draft(&req));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test --test email_commands draft_ -- --nocapture
```

Expected: fail because `mail_draft` module and DTOs do not exist.

- [ ] **Step 3: Create `mail_draft.rs` DTOs and blank detection**

Add:

```rust
use crate::error::MailError;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::service::mail_send::ComposeAttachmentInput;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SaveDraftRequest {
    pub draft_id: Option<i32>,
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    #[serde(default)]
    pub attachments: Vec<ComposeAttachmentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SaveDraftResponse {
    pub draft_id: i32,
    pub message_id: String,
    pub folder: String,
    pub saved_at: i64,
    pub remote_saved: bool,
    pub cleanup_error: Option<String>,
}

pub struct DraftAppendRequest {
    pub account: accounts::Model,
    pub folder: String,
    pub credentials: crate::domain::auth::Credentials,
    pub raw: Vec<u8>,
}

#[async_trait]
pub trait DraftRemoteWriter: Send + Sync {
    async fn append_draft(&self, req: DraftAppendRequest) -> Result<(), MailError>;
    async fn delete_draft(
        &self,
        account: &accounts::Model,
        credentials: &crate::domain::auth::Credentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), MailError>;
}

pub fn is_blank_draft(req: &SaveDraftRequest) -> bool {
    req.to.iter().chain(req.cc.iter()).chain(req.bcc.iter()).all(|v| v.trim().is_empty())
        && req.subject.trim().is_empty()
        && req.body_text.trim().is_empty()
        && req.body_html.trim().is_empty()
        && req.attachments.is_empty()
}
```

Export module in `service/mod.rs`:

```rust
pub mod mail_draft;
```

- [ ] **Step 4: Implement real remote writer**

In `mail_draft.rs`, add `RealDraftRemoteWriter` mirroring `RealSentArchiveWriter`:

```rust
pub struct RealDraftRemoteWriter;

#[async_trait]
impl DraftRemoteWriter for RealDraftRemoteWriter {
    async fn append_draft(&self, req: DraftAppendRequest) -> Result<(), MailError> {
        let imap_config = crate::service::account_connection::imap_config_from_account(&req.account)?;
        let mut client = match &req.credentials {
            crate::domain::auth::Credentials::Password(password) => {
                crate::infrastructure::protocols::imap::ImapClient::connect(
                    &imap_config,
                    &req.account.email,
                    password,
                )
                .await?
            }
            crate::domain::auth::Credentials::OAuth2 { access_token } => {
                crate::infrastructure::protocols::imap::ImapClient::connect_xoauth2(
                    &imap_config,
                    &req.account.email,
                    access_token,
                )
                .await?
            }
        };
        let result = client
            .append_email_with_flags(&req.folder, Some("(\\Draft \\Seen)"), &req.raw)
            .await;
        client.logout().await.ok();
        result
    }

    async fn delete_draft(
        &self,
        account: &accounts::Model,
        credentials: &crate::domain::auth::Credentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), MailError> {
        let imap_config = crate::service::account_connection::imap_config_from_account(account)?;
        let mut client = match credentials {
            crate::domain::auth::Credentials::Password(password) => {
                crate::infrastructure::protocols::imap::ImapClient::connect(
                    &imap_config,
                    &account.email,
                    password,
                )
                .await?
            }
            crate::domain::auth::Credentials::OAuth2 { access_token } => {
                crate::infrastructure::protocols::imap::ImapClient::connect_xoauth2(
                    &imap_config,
                    &account.email,
                    access_token,
                )
                .await?
            }
        };
        client.select_folder(folder).await?;
        let result = client.delete_uid(uid).await;
        client.logout().await.ok();
        result
    }
}
```

- [ ] **Step 5: Add EmailService methods**

In `email_service.rs`, re-export:

```rust
pub use crate::service::mail_draft::{SaveDraftRequest, SaveDraftResponse};
```

Add methods:

```rust
pub async fn save_draft(&self, req: SaveDraftRequest) -> Result<SaveDraftResponse, MailError> {
    crate::service::mail_draft::save_draft(self, req).await
}

pub async fn delete_draft(&self, draft_id: i32) -> Result<(), MailError> {
    crate::service::mail_draft::delete_draft(self, draft_id).await
}
```

If private fields block this helper style, implement the methods directly in `EmailService` and keep `mail_draft.rs` for DTOs, remote writer, and pure helpers.

- [ ] **Step 6: Implement save/delete orchestration**

Implementation must perform:

```rust
if is_blank_draft(&req) {
    return Err(MailError::InvalidParam("空白草稿不需要保存".to_string()));
}
```

Then:

1. Load account by `account_repo::get_by_id`.
2. Resolve credentials with `AuthManager::get_credentials`.
3. Resolve Drafts folder with existing `FolderRegistry`/provider mapping; fallback `"Drafts"`.
4. Convert `SaveDraftRequest` into a `SendEmailRequest`-compatible struct for `build_email`, but skip send validation. If `build_email` still enforces send validation, split MIME builder into a lower-level `build_compose_email(account, fields, require_send_validation)` helper.
5. Append remote draft first.
6. Insert local draft mirror with `email_repo::insert_draft_email_with_attachments`.
7. If `req.draft_id` existed, delete old remote draft and local row after new local row succeeds.
8. Return `SaveDraftResponse`.

- [ ] **Step 7: Add commands and specta registration**

In `command/email.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn save_draft(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    request: SaveDraftRequest,
) -> Result<SaveDraftResponse, MailError> {
    service.save_draft(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_draft(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    draft_id: i32,
) -> Result<(), MailError> {
    service.delete_draft(draft_id).await
}
```

Register both in `lib.rs`.

- [ ] **Step 8: Run backend checks**

Run:

```bash
rtk cargo test --test email_commands draft_ -- --nocapture
rtk cargo check --tests
rtk cargo fmt --check
```

Expected: draft DTO/helper tests pass and test compilation succeeds.

- [ ] **Step 9: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/service/mail_draft.rs src-tauri/src/service/mod.rs src-tauri/src/service/email_service.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/email_commands.rs
```

Expected: only draft service, commands, registration, and tests.

---

### Task 6: 发送链路写入附件并发送成功后清理草稿

**Files:**
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Test: `src-tauri/tests/email_commands.rs`
- Test: `src-tauri/tests/email_repository.rs`

**Interfaces:**
- Consumes:
  - `SendEmailRequest.attachments`
  - `SendEmailRequest.draft_id`
  - `insert_sent_email_with_attachments`
  - `EmailService::delete_draft`
- Produces:
  - Sent local record contains attachment rows.
  - SMTP success with `draft_id` attempts draft cleanup.

- [ ] **Step 1: Add failing tests for SendEmailRequest compatibility**

Update existing send tests to construct `SendEmailRequest` with:

```rust
attachments: vec![],
draft_id: None,
```

Add a targeted repository/service test if available:

```rust
#[tokio::test]
async fn sent_insert_from_send_request_persists_attachment_metadata() {
    let db = setup_test_db().await;
    seed_account(&db, 1, "sender@example.com").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("send-attachment.txt");
    std::fs::write(&path, b"hello").unwrap();
    let now = chrono::Utc::now().timestamp();

    let writes = postium_mail_lib::service::mail_send::attachment_writes_from_inputs(
        &[ComposeAttachmentInput {
            path: path.to_string_lossy().to_string(),
            filename: Some("send-attachment.txt".to_string()),
            content_type: Some("text/plain".to_string()),
            size: Some(5),
        }],
        now,
    )
    .unwrap();

    let email = email_repo::EmailWrite {
        account_id: 1,
        folder: "Sent".to_string(),
        uid: 0,
        message_id: Some("<sent-attachment@example.com>".to_string()),
        subject: Some("Subject".to_string()),
        sender_name: Some("Sender".to_string()),
        sender_email: "sender@example.com".to_string(),
        recipient_emails: "to@example.com".to_string(),
        cc_emails: None,
        bcc_emails: None,
        preview: Some("Body".to_string()),
        body_text: Some("Body".to_string()),
        body_html: Some("<p>Body</p>".to_string()),
        is_read: Some(true),
        is_starred: Some(false),
        is_draft: Some(false),
        is_answered: Some(false),
        is_deleted: Some(false),
        sent_at: now,
        received_at: now,
        created_at: now,
        updated_at: now,
    };

    let inserted = email_repo::insert_sent_email_with_attachments(&db, email, writes)
        .await
        .unwrap();
    let attachments = attachment_repo::list_by_email(&db, inserted.id).await.unwrap();

    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename.as_deref(), Some("send-attachment.txt"));
    assert_eq!(attachments[0].path.as_deref(), Some(path.to_string_lossy().as_ref()));
}
```

- [ ] **Step 2: Build attachment write mapper**

In `mail_send.rs`, add:

```rust
pub fn attachment_writes_from_inputs(
    inputs: &[ComposeAttachmentInput],
    created_at: i64,
) -> Result<Vec<crate::infrastructure::storage::repository::attachment_repo::AttachmentWrite>, MailError> {
    inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let described = describe_local_attachment_sync(&input.path)?;
            Ok(crate::infrastructure::storage::repository::attachment_repo::AttachmentWrite {
                email_id: 0,
                filename: Some(
                    input
                        .filename
                        .clone()
                        .unwrap_or(described.filename),
                ),
                content_type: Some(
                    input
                        .content_type
                        .clone()
                        .unwrap_or(described.content_type),
                ),
                size: input.size.unwrap_or(described.size),
                section_path: format!("compose:{index}"),
                disposition: Some("attachment".to_string()),
                content_id: None,
                path: Some(input.path.clone()),
                created_at,
            })
        })
        .collect()
}
```

- [ ] **Step 3: Update EmailService::send local Sent write**

Find the existing local Sent write in `EmailService::send` and replace `email_repo::insert_sent_email` with:

```rust
let attachment_writes =
    crate::service::mail_send::attachment_writes_from_inputs(&req.attachments, now)?;
let local_email =
    email_repo::insert_sent_email_with_attachments(&self.db, sent_write, attachment_writes).await?;
```

- [ ] **Step 4: Add draft cleanup after successful SMTP/Sent write**

After SMTP success and local Sent write, add:

```rust
if let Some(draft_id) = req.draft_id {
    if let Err(err) = self.delete_draft(draft_id).await {
        tracing::warn!(
            draft_id,
            message_id = %built.message_id,
            error = %err,
            "发送成功后清理草稿失败"
        );
    }
}
```

Do this after SMTP success so send failure never deletes a draft.

- [ ] **Step 5: Run backend tests**

Run:

```bash
rtk cargo test --test email_repository -- --nocapture
rtk cargo test --test email_commands -- --nocapture
rtk cargo fmt --check
```

Expected: tests pass except any known unrelated type fixture failures outside these Rust tests.

- [ ] **Step 6: Checkpoint**

Run:

```bash
rtk git diff -- src-tauri/src/service/email_service.rs src-tauri/src/service/mail_send.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/tests/email_commands.rs src-tauri/tests/email_repository.rs
```

Expected: send path now writes attachment metadata and cleans draft only after send success.

---

### Task 7: TypeScript bindings and i18n

**Files:**
- Modify: `src/lib/bindings.ts`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`

**Interfaces:**
- Consumes:
  - New Rust specta types and commands.
- Produces:
  - Frontend can call `commands.describeLocalAttachments`, `commands.saveDraft`, `commands.deleteDraft`.
  - Frontend has text for attachments and draft save statuses.

- [ ] **Step 1: Generate or sync bindings**

Preferred command:

```bash
rtk cargo check
```

Expected: in debug build, specta export updates `src/lib/bindings.ts`.

If `cargo check` does not export bindings in this environment, update `bindings.ts` manually to include:

```ts
export type ComposeAttachmentInput = {
    path: string;
    filename: string | null;
    content_type: string | null;
    size: number | null;
};

export type LocalAttachmentDraft = {
    path: string;
    filename: string;
    content_type: string;
    size: number;
};

export type SaveDraftRequest = {
    draft_id: number | null;
    account_id: number;
    to: string[];
    cc: string[];
    bcc: string[];
    subject: string;
    body_html: string;
    body_text: string;
    attachments: ComposeAttachmentInput[];
};

export type SaveDraftResponse = {
    draft_id: number;
    message_id: string;
    folder: string;
    saved_at: number;
    remote_saved: boolean;
    cleanup_error: string | null;
};
```

and command wrappers matching existing naming style in `bindings.ts`.

- [ ] **Step 2: Add i18n keys**

In `zh-CN.ts`, add under `email`:

```ts
attach: "添加附件",
removeAttachment: "移除附件",
draftSaving: "草稿保存中",
draftSaved: "草稿已保存",
draftSaveFailed: "草稿保存失败",
attachmentUnavailable: "附件不可用，请重新选择",
```

In `en-US.ts`, add:

```ts
attach: "Add attachment",
removeAttachment: "Remove attachment",
draftSaving: "Saving draft",
draftSaved: "Draft saved",
draftSaveFailed: "Draft save failed",
attachmentUnavailable: "Attachment unavailable. Choose it again.",
```

- [ ] **Step 3: Run checks**

Run:

```bash
rtk bun run test:frontend
```

Expected: existing frontend tests still pass before ComposeModal behavior changes.

- [ ] **Step 4: Checkpoint**

Run:

```bash
rtk git diff -- src/lib/bindings.ts src/lib/i18n/zh-CN.ts src/lib/i18n/en-US.ts
```

Expected: only bindings and i18n additions.

---

### Task 8: ComposeModal 附件选择、草稿自动保存和发送草稿

**Files:**
- Modify: `src/lib/components/email/ComposeModal.svelte`
- Modify: `src/lib/__tests__/components/ComposeModal.test.ts`

**Required sub-skill before implementation:** `svelte-code-writer`

**Interfaces:**
- Consumes:
  - `commands.describeLocalAttachments(paths)`
  - `commands.saveDraft(request)`
  - `commands.sendEmail(request)`
  - Tauri dialog `open`
- Produces:
  - Attachment button and list.
  - Debounced remote draft save.
  - Send request includes `attachments` and `draft_id`.
  - Close does not clear content until explicit close or send success.

- [ ] **Step 1: Load Svelte skill**

Read:

```bash
rtk cat /home/guo/.codex/skills/svelte-code-writer/SKILL.md
```

Follow its instructions for Svelte 5 component changes.

- [ ] **Step 2: Add failing frontend tests**

In `ComposeModal.test.ts`, mock dialog:

```ts
vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(),
}));
```

Add tests:

```ts
it("选择附件后展示附件并发送时包含附件", async () => {
    const dialog = await import("@tauri-apps/plugin-dialog");
    vi.mocked(dialog.open).mockResolvedValue("/tmp/hello.txt");
    mockInvoke.mockImplementation((cmd) => {
        if (cmd === "describe_local_attachments") {
            return Promise.resolve([
                {
                    path: "/tmp/hello.txt",
                    filename: "hello.txt",
                    content_type: "text/plain",
                    size: 5,
                },
            ]);
        }
        return Promise.resolve({
            message_id: "<message-id@example.com>",
            local_email_id: 1,
            remote_archived: true,
            remote_archive_error: null,
        });
    });

    const { component } = render(ComposeModal);
    component.show();
    await fireEvent.click(await screen.findByTestId("compose-attach-button"));

    expect(await screen.findByText("hello.txt")).toBeTruthy();

    await fireEvent.input(screen.getByTestId("compose-to-input"), {
        target: { value: "to@example.com" },
    });
    await fireEvent.input(screen.getByTestId("compose-subject-input"), {
        target: { value: "Hello" },
    });
    await fillBody("Body");
    await fireEvent.click(screen.getByTestId("compose-send-button"));

    expect(mockInvoke).toHaveBeenCalledWith("send_email", {
        request: expect.objectContaining({
            attachments: [
                {
                    path: "/tmp/hello.txt",
                    filename: "hello.txt",
                    content_type: "text/plain",
                    size: 5,
                },
            ],
        }),
    });
});

it("发送已保存草稿时携带 draft_id", async () => {
    vi.useFakeTimers();
    mockInvoke.mockImplementation((cmd) => {
        if (cmd === "save_draft") {
            return Promise.resolve({
                draft_id: 42,
                message_id: "<draft@example.com>",
                folder: "Drafts",
                saved_at: 123,
                remote_saved: true,
                cleanup_error: null,
            });
        }
        return Promise.resolve({
            message_id: "<message-id@example.com>",
            local_email_id: 1,
            remote_archived: true,
            remote_archive_error: null,
        });
    });

    const { component } = render(ComposeModal);
    component.show();
    await fireEvent.input(await screen.findByTestId("compose-to-input"), {
        target: { value: "to@example.com" },
    });
    await fireEvent.input(screen.getByTestId("compose-subject-input"), {
        target: { value: "Hello" },
    });
    await fillBody("Body");

    await vi.advanceTimersByTimeAsync(900);
    await waitFor(() => expect(screen.getByText("草稿已保存")).toBeTruthy());

    await fireEvent.click(screen.getByTestId("compose-send-button"));
    expect(mockInvoke).toHaveBeenCalledWith("send_email", {
        request: expect.objectContaining({ draft_id: 42 }),
    });
    vi.useRealTimers();
});
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts
```

Expected: fail because attachment button and draft autosave do not exist.

- [ ] **Step 4: Implement attachment state and picker**

In `ComposeModal.svelte`, add:

```ts
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Paperclip, Trash2 } from "lucide-svelte";
import type { LocalAttachmentDraft } from "$lib/bindings";

let attachments = $state<LocalAttachmentDraft[]>([]);

async function chooseAttachments() {
    const selected = await openDialog({
        multiple: true,
        directory: false,
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (paths.length === 0) return;
    const result = await commands.describeLocalAttachments(paths);
    if (result.status === "error") {
        error = result.error.message as string;
        return;
    }
    attachments = [...attachments, ...result.data];
}

function removeAttachment(path: string) {
    attachments = attachments.filter((attachment) => attachment.path !== path);
}
```

Add toolbar button near send/cancel:

```svelte
<button
    type="button"
    data-testid="compose-attach-button"
    class="rounded-md p-2 text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
    onclick={chooseAttachments}
    aria-label={t.email.attach}
    title={t.email.attach}
>
    <Paperclip size={16} />
</button>
```

Add attachment list above error area:

```svelte
{#if attachments.length > 0}
    <div class="border-t border-border px-4 py-2">
        {#each attachments as attachment}
            <div class="flex h-8 items-center gap-2 text-sm" data-testid="compose-attachment-row">
                <Paperclip size={14} class="shrink-0 text-muted-foreground" />
                <span class="min-w-0 flex-1 truncate text-foreground">{attachment.filename}</span>
                <span class="shrink-0 text-xs text-muted-foreground">{formatBytes(attachment.size)}</span>
                <button
                    type="button"
                    class="rounded p-1 text-muted-foreground hover:bg-glass-hover hover:text-foreground"
                    onclick={() => removeAttachment(attachment.path)}
                    aria-label={t.email.removeAttachment}
                >
                    <Trash2 size={14} />
                </button>
            </div>
        {/each}
    </div>
{/if}
```

Add `formatBytes`.

- [ ] **Step 5: Implement draft autosave**

Add state:

```ts
let draftId = $state<number | null>(null);
let draftSaveStatus = $state<"idle" | "saving" | "saved" | "failed">("idle");
let draftTimer: ReturnType<typeof setTimeout> | null = null;

function hasDraftContent() {
    return (
        to.trim() ||
        cc.trim() ||
        subject.trim() ||
        (richEditor?.getText() || "").trim() ||
        attachments.length > 0
    );
}

function scheduleDraftSave() {
    if (!open || sending || !hasDraftContent()) return;
    if (draftTimer) clearTimeout(draftTimer);
    draftTimer = setTimeout(() => void saveDraftNow(), 800);
}

async function saveDraftNow() {
    const sendAccountId = accountStore.isAllAccounts
        ? selectedAccountId
        : accountStore.activeAccountId;
    if (!sendAccountId || !hasDraftContent()) return;
    draftSaveStatus = "saving";
    const bodyText = richEditor?.getText() || "";
    const result = await commands.saveDraft({
        draft_id: draftId,
        account_id: sendAccountId,
        to: parseRecipients(to),
        cc: parseRecipients(cc),
        bcc: [],
        subject,
        body_html: richEditor?.getHtml() || `<pre style="white-space:pre-wrap">${bodyText}</pre>`,
        body_text: bodyText,
        attachments,
    });
    if (result.status === "error") {
        draftSaveStatus = "failed";
        error = result.error.message as string;
        return;
    }
    draftId = result.data.draft_id;
    draftSaveStatus = "saved";
}
```

Attach `oninput={scheduleDraftSave}` or Svelte reactive effect to recipient/subject inputs and call `scheduleDraftSave` from editor input if `RichTextEditor` exposes change events. If the editor does not expose change events, add a simple `on:input` dispatch in `RichTextEditor.svelte` as a focused supporting change and test it.

- [ ] **Step 6: Include attachments and draft_id in send request**

Update send request:

```ts
attachments,
draft_id: draftId,
```

In `close()`, clear:

```ts
attachments = [];
draftId = null;
draftSaveStatus = "idle";
if (draftTimer) clearTimeout(draftTimer);
draftTimer = null;
```

- [ ] **Step 7: Add draft status UI**

Near footer or error area:

```svelte
{#if draftSaveStatus !== "idle"}
    <div class="px-4 py-1 text-xs text-muted-foreground">
        {draftSaveStatus === "saving"
            ? t.email.draftSaving
            : draftSaveStatus === "saved"
              ? t.email.draftSaved
              : t.email.draftSaveFailed}
    </div>
{/if}
```

- [ ] **Step 8: Run frontend tests**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts
rtk bun run test:frontend
```

Expected: ComposeModal tests and frontend suite pass.

- [ ] **Step 9: Checkpoint**

Run:

```bash
rtk git diff -- src/lib/components/email/ComposeModal.svelte src/lib/__tests__/components/ComposeModal.test.ts
```

Expected: only attachment UI, draft autosave, and related tests.

---

### Task 9: Full Verification and Known Check Failure Audit

**Files:**
- No source edits expected unless verification reveals a defect in this feature.

**Interfaces:**
- Consumes all earlier tasks.
- Produces final verification evidence.

- [ ] **Step 1: Rust verification**

Run:

```bash
rtk cargo test --test email_commands -- --nocapture
rtk cargo test --test email_repository -- --nocapture
rtk cargo fmt --check
```

Expected: Rust command/repository tests pass and formatting passes.

- [ ] **Step 2: Frontend verification**

Run:

```bash
rtk bun run test:frontend
```

Expected: frontend tests pass.

- [ ] **Step 3: Type/check verification**

Run:

```bash
rtk bun run check
```

Expected: this may still fail on pre-existing test fixture type issues around `EmailDto`/`EmailDetail` mock fields. If it fails, confirm whether new errors mention files touched by this feature. New feature errors must be fixed; pre-existing fixture failures should be reported separately.

- [ ] **Step 4: Final diff audit**

Run:

```bash
rtk git status --short
rtk git diff --stat
```

Expected: changed files match this plan. No unrelated refactors or generated noise beyond `bindings.ts`.

- [ ] **Step 5: Manual smoke path**

If a dev server is run later, use:

```bash
rtk bun tauri dev
```

If it fails with OS file watch limit reached, use the existing known workaround outside this plan: increase inotify watch limit or run targeted tests only. Do not change application code for the watch-limit failure.
