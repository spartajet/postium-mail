# 收件附件功能完善实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成收件附件的同步、显示、按需下载、缓存、另存为、打开和 CID 图片显示。

**Architecture:** 后端新增独立 `AttachmentService`，负责附件下载、缓存、保存、打开和 CID 解析；`EmailService` 只负责邮件详情并附带真实附件列表。IMAP 层新增按 MIME section 获取单个 part 的能力，前端在 Svelte store 中维护附件操作状态，邮件详情组件渲染真实附件和操作按钮。

**Tech Stack:** Rust 2024、Tauri 2、async-imap 0.11、tokio-rusqlite、mail-parser、base64、quoted_printable、tauri-specta、Svelte 5 runes、Vitest、DOMPurify、lucide-svelte、@tauri-apps/plugin-dialog。

## Global Constraints

- 一期只做收件附件，不实现发信附件。
- 普通附件和 inline/CID 图片都进入附件元数据链路。
- 小于等于 `10MB` 的附件按需下载到应用缓存目录。
- 大于 `10MB` 的附件不写入应用缓存，保存时直接写入用户选择的路径。
- 小于等于 `10MB` 的 CID 图片按需缓存后替换到 HTML 中显示。
- 大于 `10MB` 的 CID 图片不自动加载，作为附件项允许用户手动保存。
- 暂不做自动缓存清理。
- 不新增数据库迁移；复用 `attachments.path` 保存小附件缓存路径。
- 删除邮件或重新加载邮件时只维护数据库元数据，不主动删除历史缓存文件。
- 每个后端命令必须注册到 tauri-specta builder，并更新 `src/lib/bindings.ts`。
- 修改 Svelte 文件后必须运行 Svelte autofixer 或 `npm run check`。

---

## File Structure

- `src-tauri/src/infrastructure/protocols/types.rs`
  - 保持现有 `AttachmentInfo` 字段；不把 transfer encoding 持久化到数据库。
- `src-tauri/src/infrastructure/protocols/imap/parser.rs`
  - 扩展附件识别规则，补齐无文件名 CID 图片。
  - 新增 CID/文件名安全化相关小工具测试。
- `src-tauri/src/infrastructure/protocols/imap/fetch.rs`
  - 新增 `fetch_body_section_with_mime(folder, uid, section_path)`，一次获取 `BODY.PEEK[section.MIME]` 和 `BODY.PEEK[section]`。
  - 从 MIME 头解析 `Content-Transfer-Encoding`，返回 raw body 和 encoding。
- `src-tauri/src/infrastructure/storage/repository/attachment_repo.rs`
  - 增加附件 DTO 所需查询、缓存路径更新、附件所属邮件/账号上下文查询。
- `src-tauri/src/service/attachment_service.rs`
  - 新文件。定义 `AttachmentDto`、`InlineAttachmentDto`、`AttachmentService`、缓存路径、安全文件名、解码、保存、打开和 CID 映射。
- `src-tauri/src/service/email_service.rs`
  - `EmailDetail` 增加 `attachments`。
  - `has_attachments` 改为真实数据库计算。
  - `get` 和 `reload_email` 返回真实附件列表。
- `src-tauri/src/service/mail_operation.rs`
  - `MailRemoteOperator` 增加附件 section 下载接口。
  - `RealMailRemoteOperator` 调用 IMAP section 下载。
- `src-tauri/src/command/email.rs`
  - 新增附件命令：缓存、另存为、打开、解析 inline 图片。
- `src-tauri/src/lib.rs`
  - 注册 `AttachmentService`、dialog 插件和新增命令。
- `src-tauri/src/error/types.rs`
  - 增加附件相关错误变体。
- `src-tauri/Cargo.toml`
  - 增加 `tauri-plugin-dialog = "2"`；确认 `quoted_printable` 已由依赖图提供时仍显式加入，避免直接使用传递依赖。
- `package.json`
  - 增加 `@tauri-apps/plugin-dialog`。
- `src-tauri/capabilities/default.json`
  - 增加 dialog 保存权限。
- `src-tauri/tauri.conf.json`
  - 如验证 `convertFileSrc` 生成 `asset:` URL，则在 CSP `img-src` 增加 `asset:`。
- `src/lib/bindings.ts`
  - 通过 tauri-specta debug export 更新。
- `src/lib/stores/email.svelte.ts`
  - 增加附件操作状态、缓存/打开/另存为/CID 解析方法。
- `src/lib/components/email/EmailDetail.svelte`
  - 真实渲染附件列表、图标、状态和按钮。
- `src/lib/i18n/zh-CN.ts`
  - 增加附件按钮和状态文案。
- `src/lib/i18n/en-US.ts`
  - 增加对应英文文案。
- `src-tauri/tests/email_commands.rs`
  - 增加服务和命令层附件测试。
- `src/lib/__tests__/stores/email-state.test.ts`
  - 增加附件 store 行为测试。
- `src/lib/__tests__/components/EmailDetail.test.ts`
  - 新增或扩展邮件详情附件 UI 测试。

---

### Task 1: 后端附件 DTO、仓储查询和真实附件状态

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/attachment_repo.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Create: `src-tauri/src/service/attachment_service.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes: `attachments::Model`, `emails::Model`, `accounts::Model`, `DbConn`。
- Produces:
  - `AttachmentDto`
  - `AttachmentWithEmailContext`
  - `attachment_repo::list_by_email`
  - `attachment_repo::list_by_email_ids`
  - `attachment_repo::get_with_email_context`
  - `attachment_repo::update_path`
  - `attachment_repo::has_for_email_ids`
  - `EmailDetail.attachments`

- [ ] **Step 1: 写失败测试，验证邮件详情返回真实附件**

在 `src-tauri/tests/email_commands.rs` 追加测试：

```rust
#[tokio::test]
async fn get_email_returns_real_attachments() {
    let svc = TestServices::new().await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(3001, "with attachment")).await;

    svc.db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                rusqlite::params![
                    email_id,
                    "report.pdf",
                    "application/pdf",
                    1234_i64,
                    "2",
                    "attachment",
                    Option::<String>::None,
                    chrono::Utc::now().timestamp()
                ],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();

    assert!(detail.email.has_attachments);
    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(detail.attachments[0].filename, "report.pdf");
    assert_eq!(detail.attachments[0].content_type, "application/pdf");
    assert_eq!(detail.attachments[0].size, 1234);
    assert!(!detail.attachments[0].is_inline);
    assert!(!detail.attachments[0].is_cached);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml get_email_returns_real_attachments
```

Expected: FAIL，原因是 `EmailDetail` 没有 `attachments` 字段或详情未返回附件。

- [ ] **Step 3: 新增附件 DTO 和映射函数**

创建 `src-tauri/src/service/attachment_service.rs`，先放 DTO 和纯映射逻辑：

```rust
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::attachments;
use crate::infrastructure::storage::repository::attachment_repo;
use serde::{Deserialize, Serialize};
use specta::Type;

pub const SMALL_ATTACHMENT_LIMIT_BYTES: i64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct AttachmentDto {
    pub id: i32,
    pub email_id: i32,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub disposition: Option<String>,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub is_cached: bool,
    pub cache_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InlineAttachmentDto {
    pub content_id: String,
    pub url: String,
}

pub fn attachment_model_to_dto(model: attachments::Model) -> AttachmentDto {
    let filename = model.filename.unwrap_or_else(|| fallback_filename(&model));
    let content_type = model
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let is_inline = model
        .disposition
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("inline"))
        || model.content_id.is_some();
    let is_cached = model.path.is_some();

    AttachmentDto {
        id: model.id,
        email_id: model.email_id,
        filename,
        content_type,
        size: model.size,
        disposition: model.disposition,
        content_id: model.content_id,
        is_inline,
        is_cached,
        cache_path: model.path,
    }
}

fn fallback_filename(model: &attachments::Model) -> String {
    if let Some(content_id) = model.content_id.as_deref() {
        let trimmed = content_id.trim_matches(['<', '>']);
        if !trimmed.is_empty() {
            return format!("inline-{trimmed}");
        }
    }
    format!("attachment-{}", model.id)
}

pub async fn list_dtos_by_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<AttachmentDto>, MailError> {
    let attachments = attachment_repo::list_by_email(db, email_id).await?;
    Ok(attachments.into_iter().map(attachment_model_to_dto).collect())
}
```

修改 `src-tauri/src/service/mod.rs`：

```rust
pub mod attachment_service;
pub use attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto};
```

如果 `AttachmentService` 尚未定义导致编译失败，先只导出已存在类型：

```rust
pub mod attachment_service;
pub use attachment_service::{AttachmentDto, InlineAttachmentDto};
```

后续 Task 4 再补 `AttachmentService` 导出。

- [ ] **Step 4: 扩展仓储方法**

在 `src-tauri/src/infrastructure/storage/repository/attachment_repo.rs` 增加：

```rust
use crate::infrastructure::storage::models::{accounts, emails};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct AttachmentWithEmailContext {
    pub attachment: attachments::Model,
    pub email: emails::Model,
    pub account: accounts::Model,
}

pub async fn list_by_email_ids(
    db: &DbConn,
    email_ids: Vec<i32>,
) -> Result<HashMap<i32, Vec<attachments::Model>>, MailError> {
    if email_ids.is_empty() {
        return Ok(HashMap::new());
    }

    db.call(move |conn| {
        let placeholders = std::iter::repeat_n("?", email_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE email_id IN ({placeholders})
             ORDER BY email_id ASC, id ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(email_ids), map_attachment)?;
        let mut grouped: HashMap<i32, Vec<attachments::Model>> = HashMap::new();
        for row in rows {
            let attachment = row?;
            grouped.entry(attachment.email_id).or_default().push(attachment);
        }
        Ok(grouped)
    })
    .await
}

pub async fn has_for_email_ids(
    db: &DbConn,
    email_ids: Vec<i32>,
) -> Result<HashSet<i32>, MailError> {
    if email_ids.is_empty() {
        return Ok(HashSet::new());
    }

    db.call(move |conn| {
        let placeholders = std::iter::repeat_n("?", email_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT DISTINCT email_id FROM attachments WHERE email_id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let ids = stmt
            .query_map(rusqlite::params_from_iter(email_ids), |row| row.get::<_, i32>(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(ids)
    })
    .await
}

pub async fn update_path(
    db: &DbConn,
    attachment_id: i32,
    path: Option<String>,
) -> Result<Option<attachments::Model>, MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE attachments SET path = ?1 WHERE id = ?2",
            rusqlite::params![path, attachment_id],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE id = ?1",
        )?;
        match stmt.query_row([attachment_id], map_attachment) {
            Ok(model) => Ok(Some(model)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn get_with_email_context(
    db: &DbConn,
    attachment_id: i32,
) -> Result<Option<AttachmentWithEmailContext>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT
                a.id AS attachment_id, a.email_id, a.filename, a.content_type, a.size,
                a.section_path, a.disposition, a.content_id, a.path, a.created_at AS attachment_created_at,
                e.id AS email_id_value, e.account_id, e.folder, e.uid, e.message_id, e.subject,
                e.sender_name, e.sender_email, e.recipient_emails, e.cc_emails, e.bcc_emails,
                e.preview, e.body_text, e.body_html, e.is_read, e.is_starred, e.is_draft,
                e.is_answered, e.is_deleted, e.sent_at, e.received_at, e.created_at AS email_created_at,
                e.updated_at,
                ac.id AS account_id_value, ac.name, ac.email, ac.display_name, ac.provider,
                ac.auth_type, ac.imap_host, ac.imap_port, ac.imap_ssl_mode, ac.smtp_host,
                ac.smtp_port, ac.smtp_ssl_mode, ac.color, ac.is_active, ac.sync_enabled,
                ac.last_sync_at, ac.created_at AS account_created_at, ac.updated_at AS account_updated_at,
                ac.account_type
             FROM attachments a
             JOIN emails e ON e.id = a.email_id
             JOIN accounts ac ON ac.id = e.account_id
             WHERE a.id = ?1",
        )?;

        match stmt.query_row([attachment_id], |row| {
            Ok(AttachmentWithEmailContext {
                attachment: attachments::Model {
                    id: row.get("attachment_id")?,
                    email_id: row.get("email_id")?,
                    filename: row.get("filename")?,
                    content_type: row.get("content_type")?,
                    size: row.get("size")?,
                    section_path: row.get("section_path")?,
                    disposition: row.get("disposition")?,
                    content_id: row.get("content_id")?,
                    path: row.get("path")?,
                    created_at: row.get("attachment_created_at")?,
                },
                email: emails::Model {
                    id: row.get("email_id_value")?,
                    account_id: row.get("account_id")?,
                    folder: row.get("folder")?,
                    uid: row.get("uid")?,
                    message_id: row.get("message_id")?,
                    subject: row.get("subject")?,
                    sender_name: row.get("sender_name")?,
                    sender_email: row.get("sender_email")?,
                    recipient_emails: row.get("recipient_emails")?,
                    cc_emails: row.get("cc_emails")?,
                    bcc_emails: row.get("bcc_emails")?,
                    preview: row.get("preview")?,
                    body_text: row.get("body_text")?,
                    body_html: row.get("body_html")?,
                    is_read: row.get("is_read")?,
                    is_starred: row.get("is_starred")?,
                    is_draft: row.get("is_draft")?,
                    is_answered: row.get("is_answered")?,
                    is_deleted: row.get("is_deleted")?,
                    sent_at: row.get("sent_at")?,
                    received_at: row.get("received_at")?,
                    created_at: row.get("email_created_at")?,
                    updated_at: row.get("updated_at")?,
                },
                account: accounts::Model {
                    id: row.get("account_id_value")?,
                    name: row.get("name")?,
                    email: row.get("email")?,
                    display_name: row.get("display_name")?,
                    provider: row.get("provider")?,
                    auth_type: row.get("auth_type")?,
                    imap_host: row.get("imap_host")?,
                    imap_port: row.get("imap_port")?,
                    imap_ssl_mode: row.get("imap_ssl_mode")?,
                    smtp_host: row.get("smtp_host")?,
                    smtp_port: row.get("smtp_port")?,
                    smtp_ssl_mode: row.get("smtp_ssl_mode")?,
                    color: row.get("color")?,
                    is_active: row.get("is_active")?,
                    sync_enabled: row.get("sync_enabled")?,
                    last_sync_at: row.get("last_sync_at")?,
                    created_at: row.get("account_created_at")?,
                    updated_at: row.get("account_updated_at")?,
                    account_type: row.get("account_type")?,
                },
            })
        }) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}
```

如果 `accounts::Model` 字段和上面不完全一致，以 `src-tauri/src/infrastructure/storage/models/accounts.rs` 为准调整字段列表，保持查询只返回模型真实字段。

- [ ] **Step 5: 让邮件详情返回附件**

修改 `src-tauri/src/service/email_service.rs`：

```rust
use crate::service::attachment_service::{AttachmentDto, list_dtos_by_email};
```

扩展 `EmailDetail`：

```rust
pub struct EmailDetail {
    #[serde(flatten)]
    pub email: EmailDto,
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<AttachmentDto>,
}
```

把 `email_model_to_detail` 改为接收附件：

```rust
fn email_model_to_detail(email: emails::Model, attachments: Vec<AttachmentDto>) -> EmailDetail {
    let has_attachments = !attachments.is_empty();
    EmailDetail {
        email: EmailDto {
            id: email.id,
            account_id: email.account_id,
            folder: email.folder,
            uid: email.uid,
            subject: email.subject,
            sender_name: email.sender_name,
            sender_email: email.sender_email,
            preview: email.preview,
            is_read: email.is_read.unwrap_or(false),
            is_starred: email.is_starred.unwrap_or(false),
            sent_at: email.sent_at,
            has_attachments,
        },
        recipient_emails: email.recipient_emails,
        cc_emails: email.cc_emails,
        body_text: email.body_text,
        body_html: email.body_html,
        attachments,
    }
}
```

修改 `EmailService::get`：

```rust
let attachments = list_dtos_by_email(&self.db, id).await?;
Ok(email_model_to_detail(email, attachments))
```

修改 `reload_email` 成功分支：

```rust
let attachments = list_dtos_by_email(&self.db, email_id).await?;
Ok(ReloadEmailResult::Reloaded {
    email: email_model_to_detail(updated, attachments),
})
```

- [ ] **Step 6: 让邮件列表真实计算 `has_attachments`**

把 `convert_models(emails)` 改成异步函数：

```rust
async fn convert_models_with_attachments(
    db: &DbConn,
    emails: Vec<emails::Model>,
) -> Result<Vec<EmailDto>, MailError> {
    let ids = emails.iter().map(|email| email.id).collect::<Vec<_>>();
    let with_attachments = attachment_repo::has_for_email_ids(db, ids).await?;
    Ok(emails
        .into_iter()
        .map(|email| email_model_to_dto(email, with_attachments.contains(&email.id)))
        .collect())
}
```

新增或调整：

```rust
fn email_model_to_dto(email: emails::Model, has_attachments: bool) -> EmailDto {
    EmailDto {
        id: email.id,
        account_id: email.account_id,
        folder: email.folder,
        uid: email.uid,
        subject: email.subject,
        sender_name: email.sender_name,
        sender_email: email.sender_email,
        preview: email.preview,
        is_read: email.is_read.unwrap_or(false),
        is_starred: email.is_starred.unwrap_or(false),
        sent_at: email.sent_at,
        has_attachments,
    }
}
```

在 `list`、`list_by_category` 和搜索结果相关转换中使用真实附件状态。搜索结果如果类型不是 `EmailDto`，不强行改。

- [ ] **Step 7: 运行目标测试**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml get_email_returns_real_attachments
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS。

- [ ] **Step 8: 提交**

```bash
rtk git add src-tauri/src/infrastructure/storage/repository/attachment_repo.rs src-tauri/src/service/attachment_service.rs src-tauri/src/service/mod.rs src-tauri/src/service/email_service.rs src-tauri/tests/email_commands.rs
rtk git commit -m "feat: return real email attachments"
```

---

### Task 2: BODYSTRUCTURE 解析补齐 CID 图片

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/parser.rs`
- Test: `src-tauri/src/infrastructure/protocols/imap/parser.rs`

**Interfaces:**
- Consumes: `async_imap::imap_proto::BodyStructure`。
- Produces: `is_attachment(common, other)` 支持 CID 图片判断。

- [ ] **Step 1: 写失败测试**

在 `parser.rs` 的 test module 中新增：

```rust
#[test]
fn detects_inline_cid_image_without_filename_as_attachment() {
    use async_imap::imap_proto::{
        BodyContentCommon, BodyContentSinglePart, BodyParams, BodyStructure, ContentEncoding,
        ContentType,
    };
    use std::borrow::Cow;

    let body = BodyStructure::Basic {
        common: BodyContentCommon {
            ty: ContentType {
                ty: Cow::Borrowed("image"),
                subtype: Cow::Borrowed("png"),
                params: BodyParams::default(),
            },
            disposition: None,
            language: None,
            location: None,
        },
        other: BodyContentSinglePart {
            id: Some(Cow::Borrowed("<logo@example.com>")),
            md5: None,
            description: None,
            transfer_encoding: ContentEncoding::Base64,
            octets: 128,
        },
        extension: None,
    };

    let attachments = extract_attachments(&body, "2");

    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].content_type, "image/png");
    assert_eq!(attachments[0].content_id.as_deref(), Some("<logo@example.com>"));
    assert_eq!(attachments[0].section_path, "2");
}
```

如果 `BodyParams::default()` 不存在，用源码中的真实 enum/struct 构造空参数。先通过 `rtk sed -n '530,760p' ~/.cargo/registry/src/.../imap-proto-0.16.7/src/types.rs` 确认。

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml detects_inline_cid_image_without_filename_as_attachment
```

Expected: FAIL，当前 `is_attachment` 没有读取 `other.id`。

- [ ] **Step 3: 修改判定签名和调用点**

把 `is_attachment` 改为：

```rust
pub fn is_attachment(
    common: &imap_proto::BodyContentCommon<'_>,
    other: &imap_proto::BodyContentSinglePart<'_>,
) -> bool {
    if let Some(ref disposition) = common.disposition {
        if disposition.ty.eq_ignore_ascii_case("attachment") {
            return true;
        }
        if disposition.params.as_ref().is_some_and(|params| {
            params
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("filename"))
        }) {
            return true;
        }
    }

    if common
        .ty
        .params
        .as_ref()
        .is_some_and(|params| params.iter().any(|(k, _)| k.eq_ignore_ascii_case("name")))
    {
        return true;
    }

    let is_image = common.ty.ty.eq_ignore_ascii_case("image");
    is_image && other.id.is_some()
}
```

把 `extract_attachments` 中三处调用改为：

```rust
if is_attachment(common, other) {
    attachments.push(build_attachment_info(common, other, section));
}
```

- [ ] **Step 4: 运行解析测试**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml detects_inline_cid_image_without_filename_as_attachment
rtk cargo test --manifest-path src-tauri/Cargo.toml parser
```

Expected: PASS。

- [ ] **Step 5: 提交**

```bash
rtk git add src-tauri/src/infrastructure/protocols/imap/parser.rs
rtk git commit -m "feat: detect cid inline image attachments"
```

---

### Task 3: IMAP 单附件 section 下载和解码基础

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/fetch.rs`
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
- Modify: `src-tauri/src/error/types.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces:
  - `FetchedBodySection { body: Vec<u8>, transfer_encoding: Option<String> }`
  - `ImapClient::fetch_body_section_with_mime(folder, uid, section_path)`
  - `MailError::AttachmentNotFound(i32)`
  - `MailError::AttachmentDownloadFailed(String)`
  - `MailError::AttachmentDecodeFailed(String)`
  - `MailError::FileSystemError(String)`

- [ ] **Step 1: 增加错误类型**

在 `src-tauri/src/error/types.rs` 增加：

```rust
#[error("附件不存在: {0}")]
AttachmentNotFound(i32),

#[error("附件不可下载: {0}")]
AttachmentUnavailable(String),

#[error("附件下载失败: {0}")]
AttachmentDownloadFailed(String),

#[error("附件解码失败: {0}")]
AttachmentDecodeFailed(String),

#[error("文件系统错误: {0}")]
FileSystemError(String),
```

- [ ] **Step 2: 显式加入 quoted_printable 依赖**

在 `src-tauri/Cargo.toml` dependencies 中加入：

```toml
quoted_printable = "0.5"
```

- [ ] **Step 3: 新增 section 返回结构**

在 `src-tauri/src/infrastructure/protocols/imap/mod.rs` 或 `types.rs` 中增加：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedBodySection {
    pub body: Vec<u8>,
    pub transfer_encoding: Option<String>,
}
```

如果放在 `types.rs`，使用路径 `crate::infrastructure::protocols::types::FetchedBodySection`。

- [ ] **Step 4: 实现 MIME 头解析工具**

在 `fetch.rs` 中增加私有函数：

```rust
fn parse_transfer_encoding(mime_header: &[u8]) -> Option<String> {
    let header = String::from_utf8_lossy(mime_header);
    header.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("Content-Transfer-Encoding") {
            Some(value.trim().to_ascii_lowercase())
        } else {
            None
        }
    })
}
```

- [ ] **Step 5: 实现 IMAP section 下载**

在 `impl ImapClient` 增加：

```rust
pub async fn fetch_body_section_with_mime(
    &mut self,
    folder: &str,
    uid: u32,
    section_path: &str,
) -> Result<Option<FetchedBodySection>, MailError> {
    if section_path.trim().is_empty() {
        return Err(MailError::AttachmentUnavailable("附件缺少 MIME section path".to_string()));
    }

    self.session
        .select(folder)
        .await
        .map_err(|e| MailError::ImapError(e.to_string()))?;

    let items = format!(
        "(BODY.PEEK[{section_path}.MIME] BODY.PEEK[{section_path}] UID)"
    );
    let mut fetches = self
        .session
        .uid_fetch(uid.to_string(), items)
        .await
        .map_err(|e| MailError::AttachmentDownloadFailed(e.to_string()))?;

    let Some(fetch_result) = fetches.next().await else {
        return Ok(None);
    };
    let fetch = fetch_result.map_err(|e| MailError::AttachmentDownloadFailed(e.to_string()))?;
    if fetch.uid != Some(uid) {
        return Ok(None);
    }

    let mime_key = format!("BODY[{}.MIME]", section_path).to_ascii_uppercase();
    let body_key = format!("BODY[{}]", section_path).to_ascii_uppercase();
    let mut mime_header: Option<&[u8]> = None;
    let mut body: Option<&[u8]> = None;

    for section in fetch.body_sections() {
        let key = String::from_utf8_lossy(section.0).to_ascii_uppercase();
        if key == mime_key {
            mime_header = Some(section.1);
        } else if key == body_key {
            body = Some(section.1);
        }
    }

    let body = body
        .ok_or_else(|| MailError::AttachmentDownloadFailed("附件 section 内容为空".to_string()))?;
    Ok(Some(FetchedBodySection {
        body: body.to_vec(),
        transfer_encoding: mime_header.and_then(parse_transfer_encoding),
    }))
}
```

如果 `fetch.body_sections()` 的实际 API 不匹配，使用 `async-imap` `Fetch` 的真实方法调整，但保留同样语义：同时取 MIME 头和 body，返回 body 与 transfer encoding。

- [ ] **Step 6: 编译检查**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS。

- [ ] **Step 7: 提交**

```bash
rtk git add src-tauri/src/infrastructure/protocols/imap/fetch.rs src-tauri/src/infrastructure/protocols/imap/mod.rs src-tauri/src/infrastructure/protocols/types.rs src-tauri/src/error/types.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "feat: fetch attachment body sections"
```

---

### Task 4: AttachmentService 缓存、保存和打开

**Files:**
- Modify: `src-tauri/src/service/attachment_service.rs`
- Modify: `src-tauri/src/service/mail_operation.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes: `MailRemoteOperator::fetch_attachment_section`。
- Produces:
  - `AttachmentService::new(db, auth, remote)`
  - `AttachmentService::ensure_cached(attachment_id)`
  - `AttachmentService::save_as(attachment_id, target_path)`
  - `AttachmentService::open(attachment_id)`
  - `AttachmentService::resolve_inline_images(email_id)`

- [ ] **Step 1: 扩展远端操作 trait**

在 `src-tauri/src/service/mail_operation.rs` 引入 `FetchedBodySection`，并扩展 trait：

```rust
async fn fetch_attachment_section(
    &self,
    account: &accounts::Model,
    folder: &str,
    uid: u32,
    section_path: &str,
) -> Result<Option<FetchedBodySection>, MailError>;
```

`RealMailRemoteOperator` 实现：

```rust
async fn fetch_attachment_section(
    &self,
    account: &accounts::Model,
    folder: &str,
    uid: u32,
    section_path: &str,
) -> Result<Option<FetchedBodySection>, MailError> {
    let mut client = self.connect_for_account(account).await?;
    let result = client
        .fetch_body_section_with_mime(folder, uid, section_path)
        .await;
    client.logout().await.ok();
    result
}
```

同步修改测试里的 `FakeMailRemote` 和 `NoopMailRemoteOperator`，让它们实现新方法。

- [ ] **Step 2: 写失败测试，小附件缓存写 path**

在 `src-tauri/tests/email_commands.rs` 增加 fake remote 的 section 响应字段，并新增测试：

```rust
#[tokio::test]
async fn ensure_cached_downloads_small_attachment_and_updates_path() {
    let remote = FakeMailRemote::with_attachment_section(
        b"hello attachment".to_vec(),
        Some("7bit".to_string()),
    );
    let svc = TestServices::new_with_mail_remote(remote).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(3101, "small attachment")).await;
    let attachment_id = insert_test_attachment(&svc, email_id, "hello.txt", 16, "2").await;

    let service = postium_mail_lib::service::AttachmentService::new_for_test(
        svc.db.clone(),
        svc.auth.clone(),
        svc.email_service.remote_for_test(),
        tempfile::tempdir().unwrap().path().to_path_buf(),
    );

    let dto = service.ensure_cached(attachment_id).await.unwrap();

    assert!(dto.is_cached);
    let cache_path = dto.cache_path.unwrap();
    assert_eq!(std::fs::read(&cache_path).unwrap(), b"hello attachment");
}
```

如果当前服务不暴露 `remote_for_test()`，不要为了测试暴露 EmailService 内部；改成 `AttachmentService::new_for_test(db, auth, remote, cache_root)` 并直接传 `remote.clone()`。

- [ ] **Step 3: 实现服务结构和构造**

在 `attachment_service.rs` 增加：

```rust
use crate::domain::auth::AuthManager;
use crate::service::mail_operation::{MailRemoteOperator, RealMailRemoteOperator};
use base64::Engine;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct AttachmentService {
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
    cache_root: PathBuf,
}

impl AttachmentService {
    pub fn new(db: DbConn, auth: Arc<AuthManager>, data_dir: PathBuf) -> Self {
        let remote = Arc::new(RealMailRemoteOperator::new(auth));
        Self::new_with_remote(db, remote, data_dir.join("attachments-cache"))
    }

    pub fn new_with_remote(
        db: DbConn,
        remote: Arc<dyn MailRemoteOperator>,
        cache_root: PathBuf,
    ) -> Self {
        Self { db, remote, cache_root }
    }
}
```

在 `service/mod.rs` 导出：

```rust
pub use attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto};
```

- [ ] **Step 4: 实现解码工具**

在 `attachment_service.rs` 增加：

```rust
fn decode_body(body: &[u8], transfer_encoding: Option<&str>) -> Result<Vec<u8>, MailError> {
    match transfer_encoding.unwrap_or("7bit").trim().to_ascii_lowercase().as_str() {
        "base64" => base64::engine::general_purpose::STANDARD
            .decode(body)
            .map_err(|e| MailError::AttachmentDecodeFailed(e.to_string())),
        "quoted-printable" => quoted_printable::decode(
            body,
            quoted_printable::ParseMode::Robust,
        )
        .map_err(|e| MailError::AttachmentDecodeFailed(e.to_string())),
        "7bit" | "8bit" | "binary" => Ok(body.to_vec()),
        other => Err(MailError::AttachmentDecodeFailed(format!(
            "不支持的 Content-Transfer-Encoding: {other}"
        ))),
    }
}
```

- [ ] **Step 5: 实现安全文件名和缓存路径**

```rust
fn safe_filename(filename: &str) -> String {
    let cleaned = filename
        .chars()
        .map(|ch| {
            if ch.is_control() || ch == '/' || ch == '\\' || ch == ':' {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if cleaned.is_empty() {
        "attachment".to_string()
    } else {
        cleaned
    }
}

fn cache_path_for(
    cache_root: &Path,
    account_id: i32,
    email_id: i32,
    attachment_id: i32,
    filename: &str,
) -> PathBuf {
    cache_root
        .join(account_id.to_string())
        .join(email_id.to_string())
        .join(format!("{}-{}", attachment_id, safe_filename(filename)))
}
```

- [ ] **Step 6: 实现 `ensure_cached`**

```rust
pub async fn ensure_cached(&self, attachment_id: i32) -> Result<AttachmentDto, MailError> {
    let context = attachment_repo::get_with_email_context(&self.db, attachment_id)
        .await?
        .ok_or(MailError::AttachmentNotFound(attachment_id))?;
    if context.attachment.size > SMALL_ATTACHMENT_LIMIT_BYTES {
        return Err(MailError::AttachmentUnavailable(
            "大于 10MB 的附件不进入应用缓存".to_string(),
        ));
    }

    if let Some(path) = context.attachment.path.as_deref() {
        if tokio::fs::metadata(path).await.is_ok() {
            return Ok(attachment_model_to_dto(context.attachment));
        }
    }

    let section = self
        .remote
        .fetch_attachment_section(
            &context.account,
            &context.email.folder,
            context.email.uid,
            &context.attachment.section_path,
        )
        .await?
        .ok_or_else(|| MailError::AttachmentUnavailable("远端附件 section 不存在".to_string()))?;
    let bytes = decode_body(&section.body, section.transfer_encoding.as_deref())?;
    let dto = attachment_model_to_dto(context.attachment.clone());
    let path = cache_path_for(
        &self.cache_root,
        context.account.id,
        context.email.id,
        context.attachment.id,
        &dto.filename,
    );
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| MailError::FileSystemError(e.to_string()))?;
    }
    let tmp_path = path.with_extension("tmp");
    tokio::fs::write(&tmp_path, bytes)
        .await
        .map_err(|e| MailError::FileSystemError(e.to_string()))?;
    tokio::fs::rename(&tmp_path, &path)
        .await
        .map_err(|e| MailError::FileSystemError(e.to_string()))?;

    let updated = attachment_repo::update_path(
        &self.db,
        attachment_id,
        Some(path.to_string_lossy().to_string()),
    )
    .await?
    .ok_or(MailError::AttachmentNotFound(attachment_id))?;
    Ok(attachment_model_to_dto(updated))
}
```

- [ ] **Step 7: 实现 `save_as`**

```rust
pub async fn save_as(&self, attachment_id: i32, target_path: String) -> Result<(), MailError> {
    let target = PathBuf::from(target_path);
    let context = attachment_repo::get_with_email_context(&self.db, attachment_id)
        .await?
        .ok_or(MailError::AttachmentNotFound(attachment_id))?;

    if context.attachment.size <= SMALL_ATTACHMENT_LIMIT_BYTES {
        let cached = self.ensure_cached(attachment_id).await?;
        if let Some(cache_path) = cached.cache_path {
            tokio::fs::copy(cache_path, &target)
                .await
                .map_err(|e| MailError::FileSystemError(e.to_string()))?;
            return Ok(());
        }
    }

    let section = self
        .remote
        .fetch_attachment_section(
            &context.account,
            &context.email.folder,
            context.email.uid,
            &context.attachment.section_path,
        )
        .await?
        .ok_or_else(|| MailError::AttachmentUnavailable("远端附件 section 不存在".to_string()))?;
    let bytes = decode_body(&section.body, section.transfer_encoding.as_deref())?;
    tokio::fs::write(&target, bytes)
        .await
        .map_err(|e| MailError::FileSystemError(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 8: 实现 `open` 和 inline 初版**

`open`：

```rust
pub async fn open(&self, attachment_id: i32) -> Result<(), MailError> {
    let dto = self.ensure_cached(attachment_id).await?;
    let path = dto
        .cache_path
        .ok_or_else(|| MailError::AttachmentUnavailable("附件尚未缓存".to_string()))?;
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|e| MailError::FileSystemError(e.to_string()))
}
```

`resolve_inline_images` 先返回缓存路径，Task 8 再接入前端本地 URL：

```rust
pub async fn resolve_inline_images(
    &self,
    email_id: i32,
) -> Result<Vec<InlineAttachmentDto>, MailError> {
    let attachments = attachment_repo::list_by_email(&self.db, email_id).await?;
    let mut resolved = Vec::new();
    for attachment in attachments {
        let Some(content_id) = attachment.content_id.clone() else {
            continue;
        };
        let is_image = attachment
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.starts_with("image/"));
        if !is_image || attachment.size > SMALL_ATTACHMENT_LIMIT_BYTES {
            continue;
        }
        let dto = self.ensure_cached(attachment.id).await?;
        if let Some(path) = dto.cache_path {
            resolved.push(InlineAttachmentDto {
                content_id,
                url: path,
            });
        }
    }
    Ok(resolved)
}
```

- [ ] **Step 9: 在 lib.rs 管理 AttachmentService**

在 `run()` 中已有 `data_dir` 局部变量只在 block 内可见。调整为在 block 外先算：

```rust
let data_dir = resolve_data_dir();
let db = tauri::async_runtime::block_on(async {
    std::fs::create_dir_all(&data_dir).expect("无法创建数据目录");
    let db = infrastructure::storage::database::init_database(&data_dir).await.expect("数据库初始化失败");
    ...
    db
});
let attachment_service =
    service::AttachmentService::new(db.clone(), auth.clone(), data_dir.clone());
```

并在 builder 上增加：

```rust
.manage(attachment_service)
```

- [ ] **Step 10: 运行测试**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml ensure_cached_downloads_small_attachment_and_updates_path
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS。

- [ ] **Step 11: 提交**

```bash
rtk git add src-tauri/src/service/attachment_service.rs src-tauri/src/service/mail_operation.rs src-tauri/src/service/mod.rs src-tauri/src/lib.rs src-tauri/tests/email_commands.rs
rtk git commit -m "feat: add attachment cache service"
```

---

### Task 5: Tauri 附件命令、dialog 插件和 bindings

**Files:**
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json`
- Modify: `src/lib/bindings.ts`

**Interfaces:**
- Produces frontend commands:
  - `ensureAttachmentCached(attachmentId: number): Promise<AttachmentDto>`
  - `saveAttachmentAs(attachmentId: number, targetPath: string): Promise<null>`
  - `openAttachment(attachmentId: number): Promise<null>`
  - `resolveInlineAttachments(emailId: number): Promise<InlineAttachmentDto[]>`

- [ ] **Step 1: 增加依赖**

`src-tauri/Cargo.toml`：

```toml
tauri-plugin-dialog = "2"
```

`package.json` dependencies：

```json
"@tauri-apps/plugin-dialog": "^2"
```

如果 lockfile 没有对应包，运行 `rtk bun install`。若网络受限导致失败，记录原因并改为让用户本地安装后继续。

- [ ] **Step 2: 注册 dialog 插件和权限**

`src-tauri/src/lib.rs` builder 增加：

```rust
.plugin(tauri_plugin_dialog::init())
```

`src-tauri/capabilities/default.json` permissions 增加：

```json
"dialog:default"
```

- [ ] **Step 3: 新增命令函数**

在 `src-tauri/src/command/email.rs` 增加 import：

```rust
use crate::service::attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto};
```

新增命令：

```rust
#[tauri::command]
#[specta::specta]
pub async fn ensure_attachment_cached(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
) -> Result<AttachmentDto, MailError> {
    service.ensure_cached(attachment_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_attachment_as(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
    target_path: String,
) -> Result<(), MailError> {
    service.save_as(attachment_id, target_path).await
}

#[tauri::command]
#[specta::specta]
pub async fn open_attachment(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
) -> Result<(), MailError> {
    service.open(attachment_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_inline_attachments(
    service: tauri::State<'_, AttachmentService>,
    email_id: i32,
) -> Result<Vec<InlineAttachmentDto>, MailError> {
    service.resolve_inline_images(email_id).await
}
```

- [ ] **Step 4: 注册到 specta builder**

在 `src-tauri/src/lib.rs` 的 `collect_commands!` 中加入：

```rust
command::email::ensure_attachment_cached,
command::email::save_attachment_as,
command::email::open_attachment,
command::email::resolve_inline_attachments,
```

- [ ] **Step 5: 生成 bindings**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
rtk bun run tauri dev
```

`tauri dev` 可能在启动桌面窗口后持续运行；确认 `src/lib/bindings.ts` 已生成后停止进程。验证：

```bash
rtk rg -n "AttachmentDto|InlineAttachmentDto|ensureAttachmentCached|saveAttachmentAs|openAttachment|resolveInlineAttachments" src/lib/bindings.ts
```

Expected: 能找到所有类型和命令。

- [ ] **Step 6: 提交**

```bash
rtk git add src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities/default.json package.json bun.lock src/lib/bindings.ts
rtk git commit -m "feat: expose attachment commands"
```

如果仓库使用 `bun.lockb` 而不是 `bun.lock`，提交实际变更的 lockfile。

---

### Task 6: 前端 EmailState 附件操作

**Files:**
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src/lib/__tests__/stores/email-state.test.ts`

**Interfaces:**
- Consumes: `commands.ensureAttachmentCached`、`commands.saveAttachmentAs`、`commands.openAttachment`、`commands.resolveInlineAttachments`。
- Produces:
  - `attachmentOperatingIds`
  - `attachmentErrors`
  - `resolvedBodyHtml`
  - `downloadAttachment`
  - `saveAttachmentAs`
  - `openAttachment`
  - `resolveInlineAttachmentsForSelectedEmail`

- [ ] **Step 1: 写 store 失败测试**

在 `src/lib/__tests__/stores/email-state.test.ts` 增加：

```ts
it("updates selected email attachment after cache download", async () => {
  const state = new EmailState();
  state.selectedEmail = {
    id: 1,
    account_id: 1,
    folder: "INBOX",
    uid: 10,
    subject: "hello",
    sender_name: null,
    sender_email: "sender@example.com",
    preview: null,
    is_read: false,
    is_starred: false,
    sent_at: 1,
    has_attachments: true,
    recipient_emails: "to@example.com",
    cc_emails: null,
    body_text: null,
    body_html: "<p>Hello</p>",
    attachments: [
      {
        id: 7,
        email_id: 1,
        filename: "a.txt",
        content_type: "text/plain",
        size: 12,
        disposition: "attachment",
        content_id: null,
        is_inline: false,
        is_cached: false,
        cache_path: null,
      },
    ],
  };

  mockCommands.ensureAttachmentCached.mockResolvedValue({
    status: "ok",
    data: {
      ...state.selectedEmail.attachments[0],
      is_cached: true,
      cache_path: "/tmp/a.txt",
    },
  });

  await state.downloadAttachment(7);

  expect(state.selectedEmail.attachments[0].is_cached).toBe(true);
  expect(state.selectedEmail.attachments[0].cache_path).toBe("/tmp/a.txt");
});
```

按当前 mock 结构调整 `mockCommands` 名称，以 `src/lib/__tests__/mocks/tauri.ts` 为准。

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1
```

Expected: FAIL，`downloadAttachment` 不存在。

- [ ] **Step 3: 增加状态和帮助函数**

在 `EmailState` 中增加：

```ts
attachmentOperatingIds = $state<Set<number>>(new Set());
attachmentErrors = $state<Record<number, string>>({});
resolvedBodyHtml = $state<string | null>(null);

private beginAttachmentOperation(attachmentId: number) {
  this.attachmentErrors = { ...this.attachmentErrors, [attachmentId]: "" };
  this.attachmentOperatingIds = new Set([
    ...this.attachmentOperatingIds,
    attachmentId,
  ]);
}

private endAttachmentOperation(attachmentId: number) {
  const next = new Set(this.attachmentOperatingIds);
  next.delete(attachmentId);
  this.attachmentOperatingIds = next;
}

private replaceSelectedAttachment(updated: AttachmentDto) {
  if (!this.selectedEmail) return;
  this.selectedEmail = {
    ...this.selectedEmail,
    attachments: this.selectedEmail.attachments.map((attachment) =>
      attachment.id === updated.id ? updated : attachment,
    ),
  };
}

private setAttachmentError(attachmentId: number, error: unknown, fallback: string) {
  this.attachmentErrors = {
    ...this.attachmentErrors,
    [attachmentId]: formatError(error, fallback),
  };
}
```

确保 import 包含：

```ts
import type { AttachmentDto, InlineAttachmentDto } from "$lib/bindings";
```

- [ ] **Step 4: 实现附件操作方法**

```ts
async downloadAttachment(attachmentId: number) {
  this.beginAttachmentOperation(attachmentId);
  try {
    const result = await commands.ensureAttachmentCached(attachmentId);
    if (result.status === "ok") {
      this.replaceSelectedAttachment(result.data);
    } else {
      this.setAttachmentError(attachmentId, result.error, "附件下载失败");
    }
  } catch (error) {
    this.setAttachmentError(attachmentId, error, "附件下载失败");
  } finally {
    this.endAttachmentOperation(attachmentId);
  }
}

async saveAttachmentAs(attachmentId: number, targetPath: string) {
  this.beginAttachmentOperation(attachmentId);
  try {
    const result = await commands.saveAttachmentAs(attachmentId, targetPath);
    if (result.status !== "ok") {
      this.setAttachmentError(attachmentId, result.error, "附件保存失败");
    }
  } catch (error) {
    this.setAttachmentError(attachmentId, error, "附件保存失败");
  } finally {
    this.endAttachmentOperation(attachmentId);
  }
}

async openAttachment(attachmentId: number) {
  this.beginAttachmentOperation(attachmentId);
  try {
    const result = await commands.openAttachment(attachmentId);
    if (result.status !== "ok") {
      this.setAttachmentError(attachmentId, result.error, "附件打开失败");
    }
  } catch (error) {
    this.setAttachmentError(attachmentId, error, "附件打开失败");
  } finally {
    this.endAttachmentOperation(attachmentId);
  }
}
```

- [ ] **Step 5: 实现 CID 解析状态**

```ts
async resolveInlineAttachmentsForSelectedEmail() {
  const email = this.selectedEmail;
  if (!email?.body_html) {
    this.resolvedBodyHtml = email?.body_html ?? null;
    return;
  }

  const smallInlineImages = email.attachments.filter(
    (attachment) =>
      attachment.content_id &&
      attachment.content_type.startsWith("image/") &&
      attachment.size <= 10 * 1024 * 1024,
  );
  if (smallInlineImages.length === 0) {
    this.resolvedBodyHtml = email.body_html;
    return;
  }

  const result = await commands.resolveInlineAttachments(email.id);
  if (result.status !== "ok") {
    this.resolvedBodyHtml = email.body_html;
    return;
  }

  this.resolvedBodyHtml = replaceCidReferences(email.body_html, result.data);
}
```

在同文件底部新增纯函数并导出便于测试：

```ts
export function normalizeContentId(contentId: string): string {
  return contentId.trim().replace(/^<|>$/g, "");
}

export function replaceCidReferences(
  html: string,
  inlineAttachments: InlineAttachmentDto[],
): string {
  let next = html;
  for (const attachment of inlineAttachments) {
    const cid = normalizeContentId(attachment.content_id);
    const escaped = cid.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    next = next.replace(
      new RegExp(`cid:${escaped}`, "gi"),
      attachment.url,
    );
  }
  return next;
}
```

Task 8 会把 `url` 转成 Tauri 可显示 URL。

- [ ] **Step 6: 在 select/deselect 中接入**

选择邮件成功后：

```ts
this.selectedEmail = result.data;
this.resolvedBodyHtml = result.data.body_html;
void this.resolveInlineAttachmentsForSelectedEmail();
```

取消选择时：

```ts
this.resolvedBodyHtml = null;
this.attachmentErrors = {};
this.attachmentOperatingIds = new Set();
```

- [ ] **Step 7: 运行 store 测试和类型检查**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1
rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json
```

Expected: PASS。

- [ ] **Step 8: 提交**

```bash
rtk git add src/lib/stores/email.svelte.ts src/lib/__tests__/stores/email-state.test.ts
rtk git commit -m "feat: manage attachment actions in email state"
```

---

### Task 7: 邮件详情附件 UI 和系统保存对话框

**Files:**
- Modify: `src/lib/components/email/EmailDetail.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Test: `src/lib/__tests__/components/EmailDetail.test.ts`

**Interfaces:**
- Consumes: `emailState.selectedEmail.attachments`、`emailState.downloadAttachment`、`emailState.saveAttachmentAs`、`emailState.openAttachment`。
- Produces: 真实附件列表 UI。

- [ ] **Step 1: 写失败组件测试**

创建或扩展 `src/lib/__tests__/components/EmailDetail.test.ts`：

```ts
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import EmailDetail from "$lib/components/email/EmailDetail.svelte";
import { mockEmailState, mockI18nState } from "../mocks/state";

it("renders real attachments and triggers download", async () => {
  const user = userEvent.setup();
  const downloadAttachment = vi.fn();
  mockEmailState({
    selectedEmail: {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 10,
      subject: "subject",
      sender_name: "Alice",
      sender_email: "alice@example.com",
      preview: null,
      is_read: true,
      is_starred: false,
      sent_at: 1,
      has_attachments: true,
      recipient_emails: "bob@example.com",
      cc_emails: null,
      body_text: null,
      body_html: "<p>Hello</p>",
      attachments: [
        {
          id: 7,
          email_id: 1,
          filename: "report.pdf",
          content_type: "application/pdf",
          size: 1024,
          disposition: "attachment",
          content_id: null,
          is_inline: false,
          is_cached: false,
          cache_path: null,
        },
      ],
    },
    resolvedBodyHtml: "<p>Hello</p>",
    attachmentOperatingIds: new Set(),
    attachmentErrors: {},
    downloadAttachment,
  });
  mockI18nState();

  render(EmailDetail);

  expect(screen.getByText("report.pdf")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /下载/ }));
  expect(downloadAttachment).toHaveBeenCalledWith(7);
});
```

如果现有测试 mock 结构不同，以现有 `EmailContextMenu.test.ts` 的 context/mock 方式为准。

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/EmailDetail.test.ts --pool threads --maxWorkers 1
```

Expected: FAIL，附件 UI 仍是占位或没有按钮。

- [ ] **Step 3: 增加 i18n 文案**

`zh-CN.ts` 的 email 区域增加：

```ts
attachmentDownload: "下载",
attachmentOpen: "打开",
attachmentSaveAs: "另存为",
attachmentSave: "保存",
attachmentDownloading: "下载中",
attachmentCached: "已缓存",
attachmentNotDownloaded: "未下载",
attachmentSaveCanceled: "已取消保存",
```

`en-US.ts` 增加对应：

```ts
attachmentDownload: "Download",
attachmentOpen: "Open",
attachmentSaveAs: "Save as",
attachmentSave: "Save",
attachmentDownloading: "Downloading",
attachmentCached: "Cached",
attachmentNotDownloaded: "Not downloaded",
attachmentSaveCanceled: "Save canceled",
```

- [ ] **Step 4: 在组件导入图标和 dialog**

`EmailDetail.svelte` 导入：

```ts
import {
  Archive,
  ChevronDown,
  ChevronUp,
  Download,
  ExternalLink,
  File,
  FileArchive,
  FileAudio,
  FileImage,
  FileText,
  FileVideo,
  FolderDown,
  Forward,
  Layers,
  Mail,
  Paperclip,
  Reply,
  Star,
  Trash2,
} from "lucide-svelte";
import { save } from "@tauri-apps/plugin-dialog";
import type { AttachmentDto } from "$lib/bindings";
```

- [ ] **Step 5: 增加格式化和图标选择**

```ts
function formatFileSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

function isLargeAttachment(attachment: AttachmentDto): boolean {
  return attachment.size > 10 * 1024 * 1024;
}

function attachmentIcon(attachment: AttachmentDto) {
  const type = attachment.content_type;
  if (type.startsWith("image/")) return FileImage;
  if (type.startsWith("audio/")) return FileAudio;
  if (type.startsWith("video/")) return FileVideo;
  if (type.includes("zip") || type.includes("rar") || type.includes("7z")) return FileArchive;
  if (type.startsWith("text/") || type.includes("pdf")) return FileText;
  return File;
}
```

- [ ] **Step 6: 增加操作处理**

```ts
async function handleDownload(attachment: AttachmentDto) {
  if (isLargeAttachment(attachment)) {
    await handleSaveAs(attachment);
    return;
  }
  await emailState.downloadAttachment(attachment.id);
}

async function handleSaveAs(attachment: AttachmentDto) {
  const targetPath = await save({
    defaultPath: attachment.filename,
  });
  if (!targetPath) return;
  await emailState.saveAttachmentAs(attachment.id, targetPath);
}

async function handleOpen(attachment: AttachmentDto) {
  await emailState.openAttachment(attachment.id);
}
```

- [ ] **Step 7: 替换附件占位 UI**

把原占位附件块替换为：

```svelte
{#if emailState.selectedEmail.attachments.length > 0}
  <section class="attachments" aria-label={t.email.attachments}>
    <div class="attachments-header">
      <Paperclip size={16} />
      <span>{t.email.attachments}</span>
    </div>
    <div class="attachments-list">
      {#each emailState.selectedEmail.attachments as attachment (attachment.id)}
        {@const Icon = attachmentIcon(attachment)}
        {@const operating = emailState.attachmentOperatingIds.has(attachment.id)}
        {@const large = isLargeAttachment(attachment)}
        <div class="attachment-row">
          <div class="attachment-icon">
            <Icon size={20} />
          </div>
          <div class="attachment-meta">
            <div class="attachment-name">{attachment.filename}</div>
            <div class="attachment-subtitle">
              {formatFileSize(attachment.size)}
              ·
              {attachment.is_cached ? t.email.attachmentCached : t.email.attachmentNotDownloaded}
            </div>
            {#if emailState.attachmentErrors[attachment.id]}
              <div class="attachment-error">{emailState.attachmentErrors[attachment.id]}</div>
            {/if}
          </div>
          <div class="attachment-actions">
            <button
              type="button"
              title={large ? t.email.attachmentSave : t.email.attachmentDownload}
              aria-label={large ? t.email.attachmentSave : t.email.attachmentDownload}
              disabled={operating}
              on:click={() => handleDownload(attachment)}
            >
              {#if large}
                <FolderDown size={16} />
              {:else}
                <Download size={16} />
              {/if}
            </button>
            {#if !large}
              <button
                type="button"
                title={t.email.attachmentOpen}
                aria-label={t.email.attachmentOpen}
                disabled={operating}
                on:click={() => handleOpen(attachment)}
              >
                <ExternalLink size={16} />
              </button>
              <button
                type="button"
                title={t.email.attachmentSaveAs}
                aria-label={t.email.attachmentSaveAs}
                disabled={operating}
                on:click={() => handleSaveAs(attachment)}
              >
                <FolderDown size={16} />
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>
{/if}
```

使用项目现有样式系统补 CSS，保持紧凑、8px 内圆角，不做嵌套卡片。

- [ ] **Step 8: HTML 渲染使用 resolvedBodyHtml**

把正文 HTML 来源改为：

```ts
const sanitizedHtml = $derived(
  emailState.resolvedBodyHtml
    ? DOMPurify.sanitize(emailState.resolvedBodyHtml)
    : "",
);
```

模板使用 `sanitizedHtml`。

- [ ] **Step 9: Svelte 校验和测试**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer src/lib/components/email/EmailDetail.svelte
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/EmailDetail.test.ts --pool threads --maxWorkers 1
rtk npm run check
```

Expected: PASS。

- [ ] **Step 10: 提交**

```bash
rtk git add src/lib/components/email/EmailDetail.svelte src/lib/i18n/zh-CN.ts src/lib/i18n/en-US.ts src/lib/__tests__/components/EmailDetail.test.ts
rtk git commit -m "feat: render real attachment actions"
```

---

### Task 8: CID 图片本地 URL、CSP 和 HTML 替换

**Files:**
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/lib/__tests__/stores/email-state.test.ts`

**Interfaces:**
- Consumes: `InlineAttachmentDto.url` 当前为本地文件路径。
- Produces: 前端 HTML 中 `cid:` 被替换为 Tauri 可加载资源 URL。

- [ ] **Step 1: 写 CID 替换测试**

在 `email-state.test.ts` 增加：

```ts
it("replaces cid references case-insensitively", () => {
  const html = '<p><img src="cid:Logo@Example.Com"></p>';
  const result = replaceCidReferences(html, [
    {
      content_id: "<logo@example.com>",
      url: "asset://localhost/logo.png",
    },
  ]);

  expect(result).toContain('src="asset://localhost/logo.png"');
});
```

- [ ] **Step 2: 使用 Tauri convertFileSrc**

在 `email.svelte.ts` 导入：

```ts
import { convertFileSrc } from "@tauri-apps/api/core";
```

修改 `resolveInlineAttachmentsForSelectedEmail`：

```ts
const mapped = result.data.map((attachment) => ({
  ...attachment,
  url: convertFileSrc(attachment.url),
}));
this.resolvedBodyHtml = replaceCidReferences(email.body_html, mapped);
```

- [ ] **Step 3: 固定 Content-ID 匹配规则**

修改 `replaceCidReferences`，同时支持 `cid:<id>`、`cid:id` 和 URL 编码的 content id：

```ts
export function replaceCidReferences(
  html: string,
  inlineAttachments: InlineAttachmentDto[],
): string {
  let next = html;
  for (const attachment of inlineAttachments) {
    const cid = normalizeContentId(attachment.content_id);
    const candidates = new Set([cid, encodeURIComponent(cid)]);
    for (const candidate of candidates) {
      const escaped = candidate.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      next = next.replace(new RegExp(`cid:${escaped}`, "gi"), attachment.url);
    }
  }
  return next;
}
```

- [ ] **Step 4: 更新 CSP**

运行一次小检查或手动确认 `convertFileSrc("/tmp/a.png")` 在 Tauri v2 中生成的协议。若为 `asset://localhost/...`，修改 `src-tauri/tauri.conf.json`：

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https: asset:; font-src 'self' data:; connect-src 'self' https:; frame-src 'none';"
```

如果实际协议不是 `asset:`，写入实际协议。

- [ ] **Step 5: 运行测试和检查**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1
rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json
rtk npm run check
```

Expected: PASS。

- [ ] **Step 6: 提交**

```bash
rtk git add src/lib/stores/email.svelte.ts src/lib/__tests__/stores/email-state.test.ts src-tauri/tauri.conf.json
rtk git commit -m "feat: resolve cid images from attachment cache"
```

---

### Task 9: 最终验证、文档同步和风险收敛

**Files:**
- Modify only if needed: `docs/superpowers/specs/2026-06-29-receive-attachments-design.md`
- Modify only if needed: `docs/superpowers/plans/2026-06-29-receive-attachments.md`

**Interfaces:**
- Produces: 完整验证记录和可提交分支。

- [ ] **Step 1: 全量 Rust 验证**

Run:

```bash
rtk cargo fmt --check --manifest-path src-tauri/Cargo.toml
rtk cargo test --manifest-path src-tauri/Cargo.toml
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS。

- [ ] **Step 2: 全量前端验证**

Run:

```bash
rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json
rtk node node_modules/vitest/vitest.mjs run --pool threads --maxWorkers 1 --reporter dot
rtk npm run check
rtk npm run build
```

Expected: PASS。

- [ ] **Step 3: 手动验证真实 IMAP**

启动应用：

```bash
rtk bun run tauri dev
```

手动验证四类邮件：

- 小于等于 `10MB` 的普通附件：能下载到缓存、打开、另存为。
- 大于 `10MB` 的普通附件：点击保存弹系统对话框，文件保存到用户选择路径，数据库 `attachments.path` 不更新。
- HTML 正文内嵌 CID 小图片：正文中图片显示，附件列表仍可见。
- 异常附件：断网或远端 section 不存在时，只显示该附件错误，不破坏邮件详情。

- [ ] **Step 4: 检查无意改动**

Run:

```bash
rtk git status --short
rtk git diff -- src-tauri/src src-tauri/Cargo.toml src-tauri/capabilities/default.json src-tauri/tauri.conf.json package.json src/lib docs/superpowers
```

Expected: diff 只包含附件功能、测试、bindings、依赖和文档。

- [ ] **Step 5: 最终提交**

如果 Step 1-4 有补丁：

```bash
rtk git add docs/superpowers/specs/2026-06-29-receive-attachments-design.md docs/superpowers/plans/2026-06-29-receive-attachments.md src-tauri src package.json bun.lock bun.lockb
rtk git commit -m "test: verify receive attachment workflow"
```

如果没有补丁，不创建空提交。

---

## 自审清单

- [ ] Spec 覆盖：收件附件、CID、10MB 阈值、小文件缓存、大文件另存为、无自动清理、真实附件 UI 都有任务。
- [ ] 无发信附件任务。
- [ ] 无自动缓存清理任务。
- [ ] 没有要求新增数据库迁移。
- [ ] 所有新增 Tauri 命令都要求注册到 specta builder。
- [ ] 前端保存对话框只在用户点击保存或另存为时触发。
- [ ] CID 替换只使用后端返回的当前邮件附件映射。
- [ ] 大附件不写入 `attachments.path`。
- [ ] 小附件缓存路径只落在应用缓存目录。
- [ ] 每个任务都有测试或编译检查。
