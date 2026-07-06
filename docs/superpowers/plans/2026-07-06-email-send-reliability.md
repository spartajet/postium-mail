# 邮件发送可靠性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 因为仓库指令提醒子 agent 可能卡死，本计划默认推荐 inline execution；如果用户明确要求，也可以改用 superpowers:subagent-driven-development。

**Goal:** 补齐邮件发送第一阶段可靠性：后端校验主题/正文/收件人，使用账号手动 SMTP 配置，生成 Message-ID，SMTP 成功后保存本地 Sent，并对远端 Sent APPEND 做失败降级。

**Architecture:** 新增发送专用模块封装请求校验、SMTP 配置解析、消息构建、SMTP 投递 trait 和 Sent 归档 trait，`EmailService::send` 只做业务编排。SMTP 协议层继续基于 `lettre`，IMAP APPEND 基于现有 `ImapClient` 增加方法；测试通过 mock sender/archiver 覆盖成功、失败和降级，不连接真实网络。

**Tech Stack:** Tauri v2、Rust 2024、lettre 0.11、async-imap 0.11、rusqlite、tauri-specta、SvelteKit、Svelte 5 Runes、Vitest、Testing Library Svelte。

## Global Constraints

- 文档使用中文书写。
- 没有用户明确指令，不提交代码；执行本计划时不要运行 `git commit`。
- shell 命令使用 `rtk` 前缀。
- 使用 TDD：每个行为改动先写失败测试，再实现。
- 默认测试不连接真实 SMTP、IMAP、OAuth 或远程邮件服务。
- 主题和正文不允许为空；正文以后端 `body_text.trim()` 为非空判据。
- 本阶段不实现附件、草稿、签名、定时发送、回复线程头和 BCC 输入 UI。
- 发送命令返回结构化 `SendEmailResponse`，不再返回固定 `"ok"`。
- 手动 SMTP 配置优先于 provider 默认配置。
- SMTP 成功但远端 Sent APPEND 失败时，不提示用户重发。
- 涉及 `.svelte` / `.svelte.ts` 文件时，使用 `svelte-code-writer` 要求的 Svelte autofixer 或前端测试验证。

---

## File Structure

- Create: `src-tauri/src/service/mail_send.rs`
  - 发送专用业务单元：`SendEmailResponse`、`BuiltEmail`、`SmtpEmailSender` trait、`SentArchiveWriter` trait、请求校验、SMTP 配置解析、Message-ID 生成、`lettre::Message` 构建。
- Modify: `src-tauri/src/service/mod.rs`
  - 导出 `mail_send` 模块。
- Modify: `src-tauri/src/service/email_service.rs`
  - `SendEmailRequest` 保留，新增/重导出 `SendEmailResponse`。
  - `EmailService` 注入 SMTP sender 和 Sent archiver，`send` 改为编排新模块。
- Modify: `src-tauri/src/infrastructure/protocols/smtp.rs`
  - 增加基于已构建 `lettre::Message` 的发送入口，避免 SMTP 和 IMAP APPEND 重复构建消息。
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
  - 增加 `append_email(folder, raw)`，内部调用 `session.append(folder, Some("(\\Seen)"), None, raw)`。
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 新增本地 Sent 写入函数。
  - 调整完整邮件同步写入路径：相同 account/folder/message_id 时合并本地发送副本并更新真实 UID，避免重复显示。
- Modify: `src-tauri/src/command/email.rs`
  - `send_email` 返回 `SendEmailResponse`，修正文档注释中附件/草稿/线程等未实现声明。
- Modify: `src-tauri/src/lib.rs`
  - 类型导出会自动包含新的 `SendEmailResponse`，确认 command 注册不变。
- Modify: `src/lib/bindings.ts`
  - 重新生成或同步 `sendEmail` 返回类型和 `SendEmailResponse` 类型。
- Modify: `src/lib/components/email/ComposeModal.svelte`
  - 增加空主题、空正文、无收件人的前端校验。
  - 适配结构化发送响应。
  - 发送成功后在当前 Sent 视图刷新。
- Modify: `src/lib/i18n/zh-CN.ts`
  - 增加写信校验错误文案。
- Modify: `src/lib/i18n/en-US.ts`
  - 增加写信校验错误文案。
- Modify: `src/lib/__tests__/components/ComposeModal.test.ts`
  - 覆盖空主题、空正文、无收件人、后端错误保留内容、成功关闭。
- Modify: `src-tauri/tests/common/mod.rs`
  - 增加 mock SMTP sender / mock Sent archiver 注入测试服务。
- Modify: `src-tauri/tests/email_commands.rs`
  - 覆盖 service/command 发送校验、成功、本地 Sent、远端归档失败降级。
- Modify: `src-tauri/tests/email_repository.rs`
  - 覆盖本地 Sent 写入和 Message-ID 合并。

---

### Task 1: 发送专用模块与请求校验

**Files:**
- Create: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Produces:
  - `pub struct SendEmailResponse { pub message_id: String, pub local_email_id: i32, pub remote_archived: bool, pub remote_archive_error: Option<String> }`
  - `pub fn validate_send_request(req: &SendEmailRequest) -> Result<(), MailError>`
  - `pub fn non_empty_recipients(req: &SendEmailRequest) -> Vec<String>`
- Consumes:
  - Existing `SendEmailRequest`
  - Existing `MailError::InvalidParam`

- [ ] **Step 1: Write failing validation tests**

Append to `src-tauri/tests/email_commands.rs`:

```rust
use postium_mail_lib::service::email_service::SendEmailRequest;
use postium_mail_lib::service::mail_send::validate_send_request;

#[test]
fn send_validation_rejects_missing_recipients() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("收件人")));
}

#[test]
fn send_validation_rejects_empty_subject() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "   ".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("主题")));
}

#[test]
fn send_validation_rejects_empty_body_text() {
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p><br></p>".to_string(),
        body_text: " \n\t ".to_string(),
    };

    let result = validate_send_request(&req);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("正文")));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `rtk cargo test --test email_commands send_validation -- --nocapture`

Expected: FAIL because `service::mail_send` and `validate_send_request` do not exist.

- [ ] **Step 3: Add module and DTO**

Create `src-tauri/src/service/mail_send.rs`:

```rust
use crate::error::MailError;
use crate::service::email_service::SendEmailRequest;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SendEmailResponse {
    pub message_id: String,
    pub local_email_id: i32,
    pub remote_archived: bool,
    pub remote_archive_error: Option<String>,
}

pub fn non_empty_recipients(req: &SendEmailRequest) -> Vec<String> {
    req.to
        .iter()
        .chain(req.cc.iter())
        .chain(req.bcc.iter())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn validate_send_request(req: &SendEmailRequest) -> Result<(), MailError> {
    if non_empty_recipients(req).is_empty() {
        return Err(MailError::InvalidParam(
            "至少需要一个收件人、抄送或密送地址".to_string(),
        ));
    }
    if req.subject.trim().is_empty() {
        return Err(MailError::InvalidParam("邮件主题不能为空".to_string()));
    }
    if req.body_text.trim().is_empty() {
        return Err(MailError::InvalidParam("邮件正文不能为空".to_string()));
    }
    Ok(())
}
```

Modify `src-tauri/src/service/mod.rs`:

```rust
pub mod mail_send;
```

Add it next to `pub mod mail_operation;`.

Modify imports/re-exports in `src-tauri/src/service/email_service.rs`:

```rust
pub use crate::service::mail_send::SendEmailResponse;
```

- [ ] **Step 4: Run validation tests**

Run: `rtk cargo test --test email_commands send_validation -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Check worktree**

Run: `rtk git status --short`

Expected: modified Rust files and no commits.

---

### Task 2: SMTP 配置解析与消息构建

**Files:**
- Modify: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Produces:
  - `pub fn smtp_config_from_account(account: &accounts::Model) -> Result<SmtpServerConfig, MailError>`
  - `pub struct BuiltEmail { pub message_id: String, pub raw: Vec<u8>, pub message: lettre::Message }`
  - `pub fn build_email(account: &accounts::Model, req: &SendEmailRequest) -> Result<BuiltEmail, MailError>`
- Consumes:
  - `PROVIDER_POOL`
  - `SmtpServerConfig`
  - `SslMode`
  - `lettre::Message::formatted()`

- [ ] **Step 1: Write failing SMTP config tests**

Append to `src-tauri/tests/email_commands.rs`:

```rust
use postium_mail_lib::infrastructure::storage::models::accounts;
use postium_mail_lib::domain::providers::SslMode;
use postium_mail_lib::service::mail_send::{build_email, smtp_config_from_account};

fn account_model_with_smtp(
    provider: &str,
    smtp_host: Option<&str>,
    smtp_port: Option<i32>,
    smtp_ssl_mode: Option<&str>,
) -> accounts::Model {
    accounts::Model {
        id: 1,
        name: "Work".to_string(),
        email: "work@example.com".to_string(),
        display_name: Some("Work User".to_string()),
        provider: provider.to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl: Some(true),
        imap_ssl_mode: None,
        smtp_host: smtp_host.map(ToOwned::to_owned),
        smtp_port,
        smtp_ssl: Some(true),
        smtp_ssl_mode: smtp_ssl_mode.map(ToOwned::to_owned),
        color: None,
        sync_enabled: Some(true),
        last_sync_at: None,
        auth_type: Some("password".to_string()),
        account_type: "personal".to_string(),
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn smtp_config_from_account_prefers_manual_smtp_settings() {
    postium_mail_lib::domain::providers::pool::init_provider_pool();
    let account = account_model_with_smtp(
        "gmail",
        Some("smtp.manual.example.com"),
        Some(2525),
        Some("StartTls"),
    );

    let config = smtp_config_from_account(&account).unwrap();

    assert_eq!(config.host, "smtp.manual.example.com");
    assert_eq!(config.port, 2525);
    assert!(matches!(config.ssl, SslMode::StartTls));
}

#[test]
fn smtp_config_from_account_rejects_invalid_manual_port() {
    postium_mail_lib::domain::providers::pool::init_provider_pool();
    let account = account_model_with_smtp("gmail", Some("smtp.example.com"), Some(70000), None);

    let result = smtp_config_from_account(&account);

    assert!(matches!(result, Err(MailError::InvalidParam(message)) if message.contains("SMTP 端口")));
}

#[test]
fn build_email_generates_message_id_and_raw_rfc822() {
    let account = account_model_with_smtp("gmail", None, None, None);
    let req = SendEmailRequest {
        account_id: 1,
        to: vec!["to@example.com".to_string()],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".to_string(),
        body_html: "<p>Body</p>".to_string(),
        body_text: "Body".to_string(),
    };

    let built = build_email(&account, &req).unwrap();
    let raw = String::from_utf8(built.raw).unwrap();

    assert!(built.message_id.starts_with('<'));
    assert!(built.message_id.ends_with('>'));
    assert!(raw.contains("Message-ID:"));
    assert!(raw.contains("Subject: Hello"));
    assert!(raw.contains("Content-Type: multipart/alternative"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `rtk cargo test --test email_commands smtp_config_from_account build_email_generates -- --nocapture`

Expected: FAIL because functions and types are missing.

- [ ] **Step 3: Implement SMTP config parsing and message build**

Extend `src-tauri/src/service/mail_send.rs`:

```rust
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::providers::{SmtpServerConfig, SslMode};
use crate::infrastructure::storage::models::accounts;
use lettre::message::header::{ContentType, Date, MessageId};
use lettre::message::Mailbox;
use lettre::message::MultiPart;
use lettre::Message;
use uuid::Uuid;

pub struct BuiltEmail {
    pub message_id: String,
    pub raw: Vec<u8>,
    pub message: Message,
}

pub fn smtp_config_from_account(account: &accounts::Model) -> Result<SmtpServerConfig, MailError> {
    if let Some(host) = account.smtp_host.as_deref().filter(|value| !value.trim().is_empty()) {
        let Some(port) = account.smtp_port else {
            return Err(MailError::InvalidParam("SMTP 端口不能为空".to_string()));
        };
        let port = u16::try_from(port)
            .map_err(|_| MailError::InvalidParam(format!("SMTP 端口无效: {port}")))?;
        return Ok(SmtpServerConfig {
            host: host.trim().to_string(),
            port,
            ssl: parse_smtp_ssl_mode(account.smtp_ssl_mode.as_deref().unwrap_or("Tls"))?,
        });
    }

    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
    let provider = provider_pool
        .get(&account.provider)
        .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
    Ok(provider.smtp_config(&account.email))
}

fn parse_smtp_ssl_mode(value: &str) -> Result<SslMode, MailError> {
    match value {
        "Tls" | "TLS" | "Implicit" => Ok(SslMode::Implicit),
        "StartTls" | "STARTTLS" => Ok(SslMode::StartTls),
        "None" => Ok(SslMode::None),
        other => Err(MailError::InvalidParam(format!("SMTP 加密模式无效: {other}"))),
    }
}

pub fn build_email(account: &accounts::Model, req: &SendEmailRequest) -> Result<BuiltEmail, MailError> {
    validate_send_request(req)?;
    let domain = account.email.split('@').next_back().unwrap_or("postium.local");
    let message_id = format!("<{}@{}>", Uuid::new_v4(), domain);
    let from = account
        .display_name
        .as_ref()
        .map(|name| format!("{name} <{}>", account.email))
        .unwrap_or_else(|| account.email.clone());

    let mut builder = Message::builder()
        .from(from.parse::<Mailbox>().map_err(|e| {
            MailError::InvalidParam(format!("发件人地址无效: {e}"))
        })?)
        .header(MessageId::from(message_id.trim_matches(['<', '>']).to_string()))
        .header(Date::now())
        .subject(req.subject.trim());

    for addr in &req.to {
        if !addr.trim().is_empty() {
            builder = builder.to(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("收件人无效 '{}': {e}", addr.trim()))
            })?);
        }
    }
    for addr in &req.cc {
        if !addr.trim().is_empty() {
            builder = builder.cc(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("抄送无效 '{}': {e}", addr.trim()))
            })?);
        }
    }
    for addr in &req.bcc {
        if !addr.trim().is_empty() {
            builder = builder.bcc(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("密送无效 '{}': {e}", addr.trim()))
            })?);
        }
    }

    let multipart = MultiPart::alternative()
        .singlepart(
            lettre::message::SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(req.body_text.clone()),
        )
        .singlepart(
            lettre::message::SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(req.body_html.clone()),
        );
    let message = builder
        .multipart(multipart)
        .map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {e}")))?;
    let raw = message.formatted();

    Ok(BuiltEmail {
        message_id,
        raw,
        message,
    })
}
```

- [ ] **Step 4: Run tests**

Run: `rtk cargo test --test email_commands smtp_config_from_account build_email_generates -- --nocapture`

Expected: PASS.

---

### Task 3: SMTP Sender Trait 和协议层发送已构建邮件

**Files:**
- Modify: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/src/infrastructure/protocols/smtp.rs`
- Modify: `src-tauri/tests/common/mod.rs`

**Interfaces:**
- Produces:
  - `#[async_trait] pub trait SmtpEmailSender`
  - `pub struct RealSmtpEmailSender`
  - `SmtpClient::send_built_email(...)`
- Consumes:
  - `BuiltEmail`
  - `crate::domain::auth::Credentials`
  - Existing `SmtpClient::send_email_inner` transport construction logic.

- [ ] **Step 1: Add trait and mock scaffolding test helper**

Modify `src-tauri/tests/common/mod.rs` to prepare injectable mocks:

```rust
use postium_mail_lib::service::mail_send::{
    BuiltEmail, SentArchiveWriter, SentArchiveRequest, SmtpEmailSender,
};
use std::sync::Mutex;

pub struct RecordingSmtpSender {
    pub sent_message_ids: Mutex<Vec<String>>,
    pub fail_with: Mutex<Option<MailError>>,
}

#[async_trait]
impl SmtpEmailSender for RecordingSmtpSender {
    async fn send(
        &self,
        _config: &postium_mail_lib::domain::providers::SmtpServerConfig,
        _account_email: &str,
        _credentials: &postium_mail_lib::domain::auth::Credentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError> {
        if let Some(error) = self.fail_with.lock().unwrap().take() {
            return Err(error);
        }
        self.sent_message_ids
            .lock()
            .unwrap()
            .push(email.message_id.clone());
        Ok(())
    }
}
```

This step will not compile until the production traits exist.

- [ ] **Step 2: Run compile check and verify failure**

Run: `rtk cargo test --test email_commands send_validation -- --nocapture`

Expected: FAIL because `SmtpEmailSender`, `BuiltEmail`, `SentArchiveWriter`, or `SentArchiveRequest` are not defined yet.

- [ ] **Step 3: Add SMTP sender trait**

Extend `src-tauri/src/service/mail_send.rs`:

```rust
use async_trait::async_trait;
use crate::domain::auth::Credentials;

#[async_trait]
pub trait SmtpEmailSender: Send + Sync {
    async fn send(
        &self,
        config: &SmtpServerConfig,
        account_email: &str,
        credentials: &Credentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError>;
}

pub struct RealSmtpEmailSender;

#[async_trait]
impl SmtpEmailSender for RealSmtpEmailSender {
    async fn send(
        &self,
        config: &SmtpServerConfig,
        account_email: &str,
        credentials: &Credentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError> {
        crate::infrastructure::protocols::smtp::SmtpClient::send_built_email(
            config,
            account_email,
            credentials,
            email.message.clone(),
        )
        .await
    }
}
```

- [ ] **Step 4: Add SMTP protocol method**

Modify `src-tauri/src/infrastructure/protocols/smtp.rs`:

```rust
use crate::domain::auth::Credentials as MailCredentials;
```

Add method:

```rust
pub async fn send_built_email(
    config: &SmtpServerConfig,
    account_email: &str,
    credentials: &MailCredentials,
    message: Message,
) -> Result<(), MailError> {
    let (credentials, mechanisms) = match credentials {
        MailCredentials::Password(password) => (
            Credentials::new(account_email.to_string(), password.to_string()),
            None,
        ),
        MailCredentials::OAuth2 { access_token } => (
            Credentials::new(account_email.to_string(), access_token.to_string()),
            Some(vec![Mechanism::Xoauth2]),
        ),
    };
    let transport = Self::build_transport(config, credentials, mechanisms)?;
    transport
        .send(message)
        .await
        .map_err(|e| MailError::SmtpSendFailed(format!("发送失败: {e}")))?;
    Ok(())
}
```

Extract the existing transport construction in `send_email_inner` into:

```rust
fn build_transport(
    config: &SmtpServerConfig,
    credentials: Credentials,
    mechanisms: Option<Vec<Mechanism>>,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError>
```

Keep existing public `send_email` and `send_email_xoauth2` working by calling `build_transport`.

- [ ] **Step 5: Run Rust tests**

Run: `rtk cargo test --test email_commands send_validation smtp_config_from_account build_email_generates -- --nocapture`

Expected: PASS.

---

### Task 4: 本地 Sent 写入和 Message-ID 合并

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Modify: `src-tauri/tests/email_repository.rs`

**Interfaces:**
- Produces:
  - `pub async fn insert_sent_email(db: &DbConn, write: EmailWrite) -> Result<emails::Model, MailError>`
  - `save_batch_emails` merges by `(account_id, folder, message_id)` before inserting duplicate remote Sent.
- Consumes:
  - Existing `EmailWrite`
  - Existing `replace_email_with_attachments` SQL shape.

- [ ] **Step 1: Write failing repository tests**

Append to `src-tauri/tests/email_repository.rs`:

```rust
#[tokio::test]
async fn insert_sent_email_assigns_next_uid_and_returns_model() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    insert_repo_email(&db, 1, "Sent", 41, "existing sent", 100, true, false).await;

    let now = 1_900_000_000;
    let model = email_repo::insert_sent_email(
        &db,
        email_repo::EmailWrite {
            account_id: 1,
            folder: "Sent".to_string(),
            uid: 0,
            message_id: Some("<local-send@example.com>".to_string()),
            subject: Some("new sent".to_string()),
            sender_name: Some("Me".to_string()),
            sender_email: "me@example.com".to_string(),
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
        },
    )
    .await
    .unwrap();

    assert_eq!(model.uid, 42);
    assert_eq!(model.folder, "Sent");
    assert_eq!(model.message_id.as_deref(), Some("<local-send@example.com>"));
}

#[tokio::test]
async fn save_batch_emails_merges_remote_sent_by_message_id() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    let now = 1_900_000_000;
    let local = email_repo::insert_sent_email(
        &db,
        email_repo::EmailWrite {
            account_id: 1,
            folder: "Sent".to_string(),
            uid: 0,
            message_id: Some("<same@example.com>".to_string()),
            subject: Some("local".to_string()),
            sender_name: Some("Me".to_string()),
            sender_email: "me@example.com".to_string(),
            recipient_emails: "to@example.com".to_string(),
            cc_emails: None,
            bcc_emails: None,
            preview: Some("local body".to_string()),
            body_text: Some("local body".to_string()),
            body_html: Some("<p>local body</p>".to_string()),
            is_read: Some(true),
            is_starred: Some(false),
            is_draft: Some(false),
            is_answered: Some(false),
            is_deleted: Some(false),
            sent_at: now,
            received_at: now,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let remote = postium_mail_lib::infrastructure::protocols::types::WholeEmailDto {
        id: 0,
        account_id: 1,
        folder: "Sent".to_string(),
        uid: 777,
        message_id: Some("<same@example.com>".to_string()),
        sender_name: Some("Me".to_string()),
        sender_email: "me@example.com".to_string(),
        recipient_emails: "to@example.com".to_string(),
        cc_emails: None,
        bcc_emails: None,
        subject: Some("remote".to_string()),
        preview: Some("remote body".to_string()),
        body_text: Some("remote body".to_string()),
        body_html: Some("<p>remote body</p>".to_string()),
        attachments: vec![],
        is_read: true,
        is_starred: false,
        is_draft: false,
        is_answered: false,
        is_deleted: false,
        sent_at: now,
        received_at: now,
        created_at: now,
        updated_at: now,
    };

    email_repo::save_batch_emails(&db, 1, "Sent", &[remote]).await.unwrap();

    let rows = email_repo::list_by_folder(&db, 1, "Sent", 1, 10).await.unwrap().0;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, local.id);
    assert_eq!(rows[0].uid, 777);
    assert_eq!(rows[0].subject.as_deref(), Some("remote"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `rtk cargo test --test email_repository insert_sent_email save_batch_emails_merges -- --nocapture`

Expected: FAIL because `insert_sent_email` does not exist and merge is not implemented.

- [ ] **Step 3: Implement `insert_sent_email`**

Add to `src-tauri/src/infrastructure/storage/repository/email_repo.rs`:

```rust
pub async fn insert_sent_email(db: &DbConn, mut write: EmailWrite) -> Result<emails::Model, MailError> {
    db.transaction(move |tx| {
        let next_uid = tx.query_row(
            "SELECT COALESCE(MAX(uid), 0) + 1 FROM emails WHERE account_id = ?1 AND folder = ?2",
            rusqlite::params![write.account_id, &write.folder],
            |row| row.get::<_, i64>(0),
        )? as u32;
        write.uid = next_uid;

        tx.execute(INSERT_EMAIL_SQL, rusqlite::params![
            write.account_id,
            &write.folder,
            i64::from(write.uid),
            &write.message_id,
            &write.subject,
            &write.sender_name,
            &write.sender_email,
            &write.recipient_emails,
            &write.cc_emails,
            &write.bcc_emails,
            &write.preview,
            &write.body_text,
            &write.body_html,
            opt_bool_to_int(write.is_read),
            opt_bool_to_int(write.is_starred),
            opt_bool_to_int(write.is_draft),
            opt_bool_to_int(write.is_answered),
            opt_bool_to_int(write.is_deleted),
            write.sent_at,
            write.received_at,
            write.created_at,
            write.updated_at,
        ])?;
        let id = tx.last_insert_rowid() as i32;
        let mut stmt = tx.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_email)
    })
    .await
}
```

- [ ] **Step 4: Implement Message-ID merge in `save_batch_emails`**

Inside `save_batch_emails` transaction loop, before `execute_email_insert`, look for existing by message ID:

```rust
let existing_by_message_id = if let Some(message_id) = &email.message_id {
    tx.query_row(
        "SELECT id FROM emails
         WHERE account_id = ?1 AND folder = ?2 AND message_id = ?3
         LIMIT 1",
        rusqlite::params![email.account_id, &email.folder, message_id],
        |row| row.get::<_, i32>(0),
    )
    .optional()?
} else {
    None
};

if let Some(email_id) = existing_by_message_id {
    update_existing_email_by_id(tx, email_id, email)?;
    tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
    for attachment in attachments {
        let mut attachment = attachment.clone();
        attachment.email_id = email_id;
        execute_attachment_insert(&mut attachment_stmt, &attachment)?;
    }
    affected_rows += 1;
    continue;
}
```

Add helper near existing private helpers:

```rust
fn update_existing_email_by_id(
    tx: &rusqlite::Transaction<'_>,
    email_id: i32,
    write: &EmailWrite,
) -> rusqlite::Result<usize> {
    tx.execute(
        "UPDATE emails
         SET account_id = ?1, folder = ?2, uid = ?3, message_id = ?4, subject = ?5,
             sender_name = ?6, sender_email = ?7, recipient_emails = ?8,
             cc_emails = ?9, bcc_emails = ?10, preview = ?11, body_text = ?12,
             body_html = ?13, is_read = ?14, is_starred = ?15, is_draft = ?16,
             is_answered = ?17, is_deleted = ?18, sent_at = ?19, received_at = ?20,
             updated_at = ?21
         WHERE id = ?22",
        rusqlite::params![
            write.account_id,
            &write.folder,
            i64::from(write.uid),
            &write.message_id,
            &write.subject,
            &write.sender_name,
            &write.sender_email,
            &write.recipient_emails,
            &write.cc_emails,
            &write.bcc_emails,
            &write.preview,
            &write.body_text,
            &write.body_html,
            opt_bool_to_int(write.is_read),
            opt_bool_to_int(write.is_starred),
            opt_bool_to_int(write.is_draft),
            opt_bool_to_int(write.is_answered),
            opt_bool_to_int(write.is_deleted),
            write.sent_at,
            write.received_at,
            write.updated_at,
            email_id,
        ],
    )
}
```

Import:

```rust
use rusqlite::OptionalExtension;
```

- [ ] **Step 5: Run repository tests**

Run: `rtk cargo test --test email_repository insert_sent_email save_batch_emails_merges -- --nocapture`

Expected: PASS.

---

### Task 5: Sent 归档 Trait、IMAP APPEND 和 EmailService 编排

**Files:**
- Modify: `src-tauri/src/service/mail_send.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
- Modify: `src-tauri/tests/common/mod.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Produces:
  - `pub struct SentArchiveRequest`
  - `#[async_trait] pub trait SentArchiveWriter`
  - `pub struct RealSentArchiveWriter`
  - `TestServices::new_with_send_dependencies(...)`
  - `ImapClient::append_email(folder, raw)`
- Consumes:
  - `SmtpEmailSender`
  - `BuiltEmail.raw`
  - `email_repo::insert_sent_email`
  - `AuthManager::get_credentials`

- [ ] **Step 1: Write failing service tests**

Append to `src-tauri/tests/email_commands.rs`:

```rust
#[tokio::test]
async fn send_email_writes_local_sent_and_returns_message_id() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(None),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(None),
    });
    let svc = TestServices::new_with_send_dependencies(smtp.clone(), archiver).await;
    let account = create_test_account(&svc, "sender@example.com", "gmail").await;
    svc.auth.save_password(&account.email, "password").unwrap();

    let response = svc.email_service
        .send(SendEmailRequest {
            account_id: account.id,
            to: vec!["to@example.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Hello".to_string(),
            body_html: "<p>Body</p>".to_string(),
            body_text: "Body".to_string(),
        })
        .await
        .unwrap();

    assert!(response.message_id.starts_with('<'));
    assert!(response.local_email_id > 0);
    assert!(response.remote_archived);
    assert_eq!(smtp.sent_message_ids.lock().unwrap().len(), 1);

    let detail = svc.email_service.get(response.local_email_id).await.unwrap();
    assert_eq!(detail.subject.as_deref(), Some("Hello"));
    assert_eq!(detail.sender_email, "sender@example.com");
    assert_eq!(detail.recipient_emails, "to@example.com");
}

#[tokio::test]
async fn send_email_does_not_write_sent_when_smtp_fails() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(Some(MailError::SmtpSendFailed("boom".to_string()))),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(None),
    });
    let svc = TestServices::new_with_send_dependencies(smtp, archiver).await;
    let account = create_test_account(&svc, "sender@example.com", "gmail").await;
    svc.auth.save_password(&account.email, "password").unwrap();

    let result = svc.email_service
        .send(SendEmailRequest {
            account_id: account.id,
            to: vec!["to@example.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Hello".to_string(),
            body_html: "<p>Body</p>".to_string(),
            body_text: "Body".to_string(),
        })
        .await;

    assert!(matches!(result, Err(MailError::SmtpSendFailed(_))));
    let list = svc.email_service.list(account.id, "Sent", 1, 10).await.unwrap();
    assert_eq!(list.total, 0);
}

#[tokio::test]
async fn send_email_keeps_local_sent_when_remote_archive_fails() {
    let smtp = Arc::new(RecordingSmtpSender {
        sent_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(None),
    });
    let archiver = Arc::new(RecordingSentArchiveWriter {
        archived_message_ids: Mutex::new(vec![]),
        fail_with: Mutex::new(Some(MailError::ImapConnectionFailed("append failed".to_string()))),
    });
    let svc = TestServices::new_with_send_dependencies(smtp, archiver).await;
    let account = create_test_account(&svc, "sender@example.com", "gmail").await;
    svc.auth.save_password(&account.email, "password").unwrap();

    let response = svc.email_service
        .send(SendEmailRequest {
            account_id: account.id,
            to: vec!["to@example.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Hello".to_string(),
            body_html: "<p>Body</p>".to_string(),
            body_text: "Body".to_string(),
        })
        .await
        .unwrap();

    assert!(!response.remote_archived);
    assert!(response.remote_archive_error.unwrap().contains("append failed"));
    assert!(svc.email_service.get(response.local_email_id).await.is_ok());
}
```

If `create_test_account` does not exist in the test file, add local helper:

```rust
async fn create_test_account(svc: &TestServices, email: &str, provider: &str) -> accounts::Model {
    let now = chrono::Utc::now().timestamp();
    svc.db
        .call({
            let email = email.to_string();
            let provider = provider.to_string();
            move |conn| {
                conn.execute(
                    "INSERT INTO accounts (
                        name, email, display_name, provider, smtp_host, smtp_port, smtp_ssl_mode,
                        auth_type, account_type, created_at, updated_at
                    ) VALUES ('Test', ?1, 'Sender', ?2, 'smtp.test.local', 2525, 'StartTls',
                        'password', 'personal', ?3, ?3)",
                    rusqlite::params![email, provider, now],
                )?;
                let id = conn.last_insert_rowid() as i32;
                let mut stmt = conn.prepare(
                    "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                            imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                            sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
                     FROM accounts WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok(accounts::Model {
                        id: row.get("id")?,
                        name: row.get("name")?,
                        email: row.get("email")?,
                        display_name: row.get("display_name")?,
                        provider: row.get("provider")?,
                        imap_host: row.get("imap_host")?,
                        imap_port: row.get("imap_port")?,
                        imap_ssl: postium_mail_lib::infrastructure::storage::row::opt_int_to_bool(row.get("imap_ssl")?),
                        imap_ssl_mode: row.get("imap_ssl_mode")?,
                        smtp_host: row.get("smtp_host")?,
                        smtp_port: row.get("smtp_port")?,
                        smtp_ssl: postium_mail_lib::infrastructure::storage::row::opt_int_to_bool(row.get("smtp_ssl")?),
                        smtp_ssl_mode: row.get("smtp_ssl_mode")?,
                        color: row.get("color")?,
                        sync_enabled: postium_mail_lib::infrastructure::storage::row::opt_int_to_bool(row.get("sync_enabled")?),
                        last_sync_at: row.get("last_sync_at")?,
                        auth_type: row.get("auth_type")?,
                        account_type: row.get("account_type")?,
                        created_at: row.get("created_at")?,
                        updated_at: row.get("updated_at")?,
                    })
                })
            }
        })
        .await
        .unwrap()
}
```

- [ ] **Step 2: Run tests and verify failure**

Run: `rtk cargo test --test email_commands send_email_writes_local_sent send_email_does_not_write_sent send_email_keeps_local_sent -- --nocapture`

Expected: FAIL because injection and archiver interfaces are missing.

- [ ] **Step 3: Implement archiver interfaces**

Extend `src-tauri/src/service/mail_send.rs`:

```rust
#[derive(Clone)]
pub struct SentArchiveRequest {
    pub account: accounts::Model,
    pub folder: String,
    pub raw: Vec<u8>,
    pub credentials: Credentials,
}

#[async_trait]
pub trait SentArchiveWriter: Send + Sync {
    async fn append_to_sent(&self, request: SentArchiveRequest) -> Result<(), MailError>;
}

pub struct RealSentArchiveWriter;

#[async_trait]
impl SentArchiveWriter for RealSentArchiveWriter {
    async fn append_to_sent(&self, request: SentArchiveRequest) -> Result<(), MailError> {
        let imap_config = crate::service::account_connection::imap_config_from_account(&request.account)?;
        let mut client = match request.credentials {
            Credentials::Password(password) => {
                crate::infrastructure::protocols::imap::ImapClient::connect(
                    &imap_config,
                    &request.account.email,
                    &password,
                )
                .await?
            }
            Credentials::OAuth2 { access_token } => {
                crate::infrastructure::protocols::imap::ImapClient::connect_xoauth2(
                    &imap_config,
                    &request.account.email,
                    &access_token,
                )
                .await?
            }
        };
        let result = client.append_email(&request.folder, &request.raw).await;
        let _ = client.logout().await;
        result
    }
}
```

- [ ] **Step 4: Add IMAP append method**

Modify `src-tauri/src/infrastructure/protocols/imap/mod.rs`:

```rust
pub async fn append_email(&mut self, folder: &str, raw: &[u8]) -> Result<(), MailError> {
    self.session
        .append(folder, Some("(\\Seen)"), None, raw)
        .await
        .map_err(|e| MailError::ImapConnectionFailed(format!("追加邮件到已发送失败: {e}")))?;
    Ok(())
}
```

Place it near other folder/message operations before `logout`.

- [ ] **Step 5: Add injectable dependencies to EmailService**

Modify `src-tauri/src/service/email_service.rs`:

```rust
use crate::service::mail_send::{
    build_email, smtp_config_from_account, validate_send_request, RealSentArchiveWriter,
    RealSmtpEmailSender, SentArchiveRequest, SentArchiveWriter, SmtpEmailSender,
};
```

Add fields:

```rust
smtp_sender: Arc<dyn SmtpEmailSender>,
sent_archiver: Arc<dyn SentArchiveWriter>,
```

Update constructors:

```rust
pub fn new(auth: Arc<AuthManager>, db: DbConn) -> Self {
    let remote = Arc::new(RealMailRemoteOperator::new(auth.clone()));
    Self::new_with_dependencies(
        auth,
        db,
        remote,
        Arc::new(RealSmtpEmailSender),
        Arc::new(RealSentArchiveWriter),
    )
}

pub fn new_with_mail_remote(
    auth: Arc<AuthManager>,
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
) -> Self {
    Self::new_with_dependencies(
        auth,
        db,
        remote,
        Arc::new(RealSmtpEmailSender),
        Arc::new(RealSentArchiveWriter),
    )
}

pub fn new_with_dependencies(
    auth: Arc<AuthManager>,
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
    smtp_sender: Arc<dyn SmtpEmailSender>,
    sent_archiver: Arc<dyn SentArchiveWriter>,
) -> Self {
    let mail_operation = MailOperationService::new(db.clone(), auth.clone(), remote);
    Self {
        auth,
        db,
        mail_operation,
        smtp_sender,
        sent_archiver,
    }
}
```

- [ ] **Step 6: Implement EmailService::send orchestration**

Replace `EmailService::send` body with this flow:

```rust
pub async fn send(&self, req: SendEmailRequest) -> Result<SendEmailResponse, MailError> {
    validate_send_request(&req)?;
    let account = account_repo::get_by_id(&self.db, req.account_id)
        .await?
        .ok_or(MailError::AccountNotFound(req.account_id))?;
    let smtp_config = smtp_config_from_account(&account)?;
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported("未找到provider pool".to_string()))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let mail_auth_type = &provider.as_ref().provider_info().auth_type;
    let credentials = self
        .auth
        .get_credentials(&account.email, mail_auth_type, Some(&account.provider))
        .await?;
    let built = build_email(&account, &req)?;

    self.smtp_sender
        .send(&smtp_config, &account.email, &credentials, &built)
        .await?;

    let sent_folder = provider
        .folder_mapping()
        .sent
        .first()
        .cloned()
        .unwrap_or_else(|| "Sent".to_string());
    let now = chrono::Utc::now().timestamp();
    let sent_model = email_repo::insert_sent_email(
        &self.db,
        email_repo::EmailWrite {
            account_id: account.id,
            folder: sent_folder.clone(),
            uid: 0,
            message_id: Some(built.message_id.clone()),
            subject: Some(req.subject.trim().to_string()),
            sender_name: account.display_name.clone(),
            sender_email: account.email.clone(),
            recipient_emails: req.to.join(","),
            cc_emails: if req.cc.is_empty() { None } else { Some(req.cc.join(",")) },
            bcc_emails: if req.bcc.is_empty() { None } else { Some(req.bcc.join(",")) },
            preview: Some(req.body_text.chars().take(200).collect()),
            body_text: Some(req.body_text.clone()),
            body_html: Some(req.body_html.clone()),
            is_read: Some(true),
            is_starred: Some(false),
            is_draft: Some(false),
            is_answered: Some(false),
            is_deleted: Some(false),
            sent_at: now,
            received_at: now,
            created_at: now,
            updated_at: now,
        },
    )
    .await?;

    let archive_result = self
        .sent_archiver
        .append_to_sent(SentArchiveRequest {
            account,
            folder: sent_folder,
            raw: built.raw,
            credentials,
        })
        .await;

    let (remote_archived, remote_archive_error) = match archive_result {
        Ok(()) => (true, None),
        Err(error) => {
            tracing::warn!(error = %error, message_id = %built.message_id, "SMTP 已成功但远端 Sent 归档失败");
            (false, Some(error.to_string()))
        }
    };

    Ok(SendEmailResponse {
        message_id: built.message_id,
        local_email_id: sent_model.id,
        remote_archived,
        remote_archive_error,
    })
}
```

- [ ] **Step 7: Add test helpers**

Modify `src-tauri/tests/common/mod.rs` to add:

```rust
pub struct RecordingSentArchiveWriter {
    pub archived_message_ids: Mutex<Vec<String>>,
    pub fail_with: Mutex<Option<MailError>>,
}

#[async_trait]
impl SentArchiveWriter for RecordingSentArchiveWriter {
    async fn append_to_sent(&self, request: SentArchiveRequest) -> Result<(), MailError> {
        if let Some(error) = self.fail_with.lock().unwrap().take() {
            return Err(error);
        }
        let raw = String::from_utf8_lossy(&request.raw);
        if let Some(line) = raw.lines().find(|line| line.starts_with("Message-ID:")) {
            self.archived_message_ids
                .lock()
                .unwrap()
                .push(line.to_string());
        }
        Ok(())
    }
}

pub async fn new_with_send_dependencies(
    smtp_sender: Arc<dyn SmtpEmailSender>,
    sent_archiver: Arc<dyn SentArchiveWriter>,
) -> Self {
    init_provider_pool();
    let db = create_test_db().await;
    let auth = Arc::new(AuthManager::in_memory());
    let mail_remote = Arc::new(NoopMailRemoteOperator);
    Self {
        account_service: AccountService::new_with_imap_verifier(
            db.clone(),
            auth.clone(),
            Arc::new(NoopImapConnectionVerifier),
        ),
        email_service: EmailService::new_with_dependencies(
            auth.clone(),
            db.clone(),
            mail_remote,
            smtp_sender,
            sent_archiver,
        ),
        label_service: LabelService::new(db.clone()),
        sync_service: SyncService::new(db.clone(), auth.clone()),
        db,
        auth,
    }
}
```

Place the function inside `impl TestServices`.

- [ ] **Step 8: Run service tests**

Run: `rtk cargo test --test email_commands send_email_writes_local_sent send_email_does_not_write_sent send_email_keeps_local_sent -- --nocapture`

Expected: PASS.

---

### Task 6: Command 返回类型、绑定和前端校验

**Files:**
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src/lib/bindings.ts`
- Modify: `src/lib/components/email/ComposeModal.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Modify: `src/lib/__tests__/components/ComposeModal.test.ts`

**Interfaces:**
- Produces:
  - `commands.sendEmail(request)` returns `TypedResult<SendEmailResponse, MailError>`
  - Compose modal blocks empty recipients, subject, and body text.
- Consumes:
  - `SendEmailResponse`
  - Existing `RichTextEditor.getText()`

- [ ] **Step 1: Update command signature test by compiling**

Modify `src-tauri/src/command/email.rs`:

```rust
use crate::service::email_service::{
    EmailCategory, EmailDetail, EmailListResponse, ReloadEmailResult, SendEmailRequest,
    SendEmailResponse,
};
```

Change:

```rust
) -> Result<SendEmailResponse, MailError> {
```

Run: `rtk cargo test --test email_commands send_email_writes_local_sent -- --nocapture`

Expected: PASS.

- [ ] **Step 2: Regenerate or synchronize bindings**

Preferred when a GUI session is acceptable:

Run: `rtk bun run tauri dev`

Expected: app starts and `src/lib/bindings.ts` is regenerated; stop the dev process after the binding export log appears.

If running Tauri dev is not practical, update `src/lib/bindings.ts` consistently:

```ts
sendEmail: (request: SendEmailRequest) => typedError<SendEmailResponse, MailError>(__TAURI_INVOKE("send_email", { request })),
```

Add type:

```ts
export type SendEmailResponse = {
    message_id: string,
    local_email_id: number,
    remote_archived: boolean,
    remote_archive_error: string | null,
}
```

- [ ] **Step 3: Write failing frontend tests**

Append to `src/lib/__tests__/components/ComposeModal.test.ts`:

```ts
it("空主题时不调用发送命令", async () => {
    const { component } = render(ComposeModal);
    component.show();
    await fireEvent.input(await screen.findByTestId("compose-to-input"), {
        target: { value: "to@example.com" },
    });
    await fireEvent.click(screen.getByTestId("compose-send-button"));
    expect(mockInvoke).not.toHaveBeenCalled();
});

it("后端发送错误时保留窗口和输入内容", async () => {
    mockInvoke.mockResolvedValueOnce({
        status: "error",
        error: { type: "SmtpSendFailed", message: "SMTP failed" },
    });
    const { component } = render(ComposeModal);
    component.show();
    await fireEvent.input(await screen.findByTestId("compose-to-input"), {
        target: { value: "to@example.com" },
    });
    await fireEvent.input(screen.getByTestId("compose-subject-input"), {
        target: { value: "Hello" },
    });
    await fireEvent.click(screen.getByTestId("compose-send-button"));

    expect(screen.getByTestId("compose-modal")).toBeTruthy();
    expect((screen.getByTestId("compose-subject-input") as HTMLInputElement).value).toBe("Hello");
});
```

For the existing successful send mock, change resolved data to:

```ts
mockInvoke.mockResolvedValue({
    status: "ok",
    data: {
        message_id: "<message-id@example.com>",
        local_email_id: 123,
        remote_archived: true,
        remote_archive_error: null,
    },
});
```

- [ ] **Step 4: Run frontend tests and verify failure**

Run: `rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts`

Expected: FAIL because ComposeModal still allows empty subject/body and expects string response.

- [ ] **Step 5: Implement ComposeModal validation**

Modify `src/lib/components/email/ComposeModal.svelte`:

```ts
function recipientList(value: string) {
    return value
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
}

function validateBeforeSend() {
    if (recipientList(to).length === 0 && recipientList(cc).length === 0) {
        return "请填写至少一个收件人";
    }
    if (!subject.trim()) {
        return "请填写邮件主题";
    }
    if (!richEditor?.getText().trim()) {
        return "请填写邮件正文";
    }
    return "";
}
```

At the start of `handleSend()` after account check:

```ts
const validationError = validateBeforeSend();
if (validationError) {
    error = validationError;
    return;
}
```

Use parsed recipients:

```ts
const toRecipients = recipientList(to);
const ccRecipients = recipientList(cc);
const bodyText = richEditor?.getText() || "";
```

Pass `subject: subject.trim()` and `body_text: bodyText`.

After successful result:

```ts
if (!result.data.remote_archived) {
    console.warn("Sent locally but remote Sent archive failed", result.data.remote_archive_error);
}
close();
```

- [ ] **Step 6: Add i18n keys or keep local strings**

Preferred: add keys under `email` in `src/lib/i18n/zh-CN.ts`:

```ts
missingRecipient: "请填写至少一个收件人",
missingSubject: "请填写邮件主题",
missingBody: "请填写邮件正文",
remoteArchiveFailed: "邮件已发送并保存在本地，但远端已发送归档失败",
```

And in `src/lib/i18n/en-US.ts`:

```ts
missingRecipient: "Add at least one recipient",
missingSubject: "Add a subject",
missingBody: "Write a message body",
remoteArchiveFailed: "Message sent and saved locally, but remote Sent archiving failed",
```

Then use `t.email.missingRecipient`, `t.email.missingSubject`, `t.email.missingBody`.

- [ ] **Step 7: Validate Svelte and frontend tests**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/email/ComposeModal.svelte --svelte-version 5
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts
```

Expected: autofixer reports no blocking issue; ComposeModal tests PASS.

---

### Task 7: Full Verification and Documentation Cleanup

**Files:**
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/infrastructure/protocols/smtp.rs`
- Modify: `src/lib/bindings.ts`
- Modify: any touched tests from prior tasks.

**Interfaces:**
- Consumes all previous task outputs.
- Produces a verified working tree with no commits.

- [ ] **Step 1: Remove stale sending documentation**

In `src-tauri/src/command/email.rs`, remove or rewrite claims that first stage supports:

```text
附件上传
自动保存到"已发送"文件夹
in_reply_to
数字签名
返回邮件的唯一标识（Message-ID） as String
```

Replace with accurate text:

```text
发送新邮件，要求收件人、主题、正文非空。SMTP 投递成功后保存本地 Sent，并尝试远端 Sent APPEND。附件、草稿和回复线程头不在第一阶段支持范围。
```

- [ ] **Step 2: Run targeted Rust tests**

Run:

```bash
rtk cargo test --test email_commands send_validation smtp_config_from_account build_email_generates send_email_writes_local_sent send_email_does_not_write_sent send_email_keeps_local_sent -- --nocapture
rtk cargo test --test email_repository insert_sent_email save_batch_emails_merges -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run broader Rust test suite**

Run: `rtk cargo test`

Expected: PASS.

- [ ] **Step 4: Run frontend tests**

Run: `rtk bun run test:frontend`

Expected: PASS.

- [ ] **Step 5: Run type/check command**

Run: `rtk bun run check`

Expected: PASS.

- [ ] **Step 6: Inspect git status**

Run: `rtk git status --short`

Expected: implementation files and docs are modified/untracked; no commit exists unless the user explicitly requested one later.

---

## Self-Review

### Spec Coverage

- 请求校验：Task 1、Task 6。
- 手动 SMTP 优先：Task 2。
- Message-ID 和 RFC822 原文：Task 2、Task 3。
- SMTP 成功后本地 Sent：Task 4、Task 5。
- 远端 Sent APPEND 降级：Task 5。
- 结构化响应：Task 1、Task 6。
- 前端失败保留内容、成功关闭：Task 6。
- 附件/草稿/线程排除：Global Constraints、Task 7 文档清理。
- 默认测试不连真实网络：Task 3、Task 5 mock trait。

### Placeholder Scan

本计划不使用 TBD/TODO/稍后实现等占位语句。每个任务都有明确文件、接口、测试命令和预期结果。

### Type Consistency

- `SendEmailResponse` 在 `mail_send.rs` 定义，经 `email_service.rs` re-export，被 command 和 bindings 使用。
- `BuiltEmail` 同时包含 `lettre::Message` 和 `raw: Vec<u8>`，满足 SMTP 投递和 IMAP APPEND 同源消息要求。
- `SmtpEmailSender` 与 `SentArchiveWriter` 都用 `async_trait`，与现有测试 mock 模式一致。
- 本地 Sent repository 使用现有 `EmailWrite`，避免新增数据库表。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-06-email-send-reliability.md`. Two execution options:

1. **Inline Execution (recommended for this repo)** - Execute tasks in this session using `superpowers:executing-plans`, with checkpoints after each task. This avoids subagent hangs noted in the repo instructions.
2. **Subagent-Driven** - Dispatch a fresh subagent per task and review between tasks. Faster in theory, but riskier here because repo instructions warn about subagent stalls.

Which approach?
