# 邮件操作本地与远端一致性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完善邮件回复以外的邮件操作入口，并保证已读、未读、星标、取消星标、删除、归档、移动等操作先写入远端 IMAP，远端成功后再更新本地数据库和前端状态。

**Architecture:** 新增 `src-tauri/src/service/mail_operation.rs` 作为统一邮件操作层，`EmailService` 继续作为 command facade。`MailOperationService` 读取邮件和账号，调用可注入的 `MailRemoteOperator` 执行 IMAP flag/move，再更新本地 repository；前端 `EmailState` 只在 typed command 返回成功后更新 UI。删除第一阶段只移动到 Trash，不做永久删除和 `EXPUNGE`。

**Tech Stack:** Rust 2024、Tauri 2、async-imap 0.11.2、tokio-rusqlite、async-trait、Svelte 5 runes、Vitest、svelte-check、tauri-specta bindings。

## Global Constraints

- 文档和计划使用中文书写。
- 没有用户明确指令，不提交代码。
- shell 命令使用 `rtk` 前缀。
- 统一模块名使用 `mail_operation`。
- 邮件状态类操作必须同时兼顾远端 IMAP 和本地数据库。
- 采用远端优先、本地跟随；远端失败不更新本地。
- 第一阶段不增强邮件回复、不接入真实标签、不做批量选择 UI、不做离线操作队列。
- 第一阶段不做永久删除，不执行 `EXPUNGE`。
- 后端测试不能访问真实网络，必须使用 fake remote operator。
- 修改 `.svelte` 或 `.svelte.ts` 文件时必须使用 `svelte-code-writer` 技能，并运行 Svelte 相关校验。

---

## File Structure

- Create: `src-tauri/src/service/mail_operation.rs`
  - 定义 `MailRemoteOperator` trait、真实 IMAP operator、`MailOperationService`。所有 read/star/move/archive/delete 远端优先逻辑集中在这里。
- Modify: `src-tauri/src/service/mod.rs`
  - 导出 `mail_operation` 模块。
- Modify: `src-tauri/src/service/email_service.rs`
  - 增加 `mail_operation` 字段和可注入构造函数，将 `mark_as_read`、`toggle_star`、`delete`、`move_to_folder`、`archive` 委托给 `MailOperationService`。
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
  - 为真实远端操作封装 `uid_mv`，添加 `move_uid_to_folder`。
- Modify: `src-tauri/src/command/email.rs`
  - 新增 `archive_email` command，更新删除文档为“移动到 Trash”语义。
- Modify: `src-tauri/src/lib.rs`
  - 注册 `archive_email`，构造 `EmailService` 时自动使用真实 IMAP operator。
- Modify: `src-tauri/tests/common/mod.rs`
  - 支持注入 fake `MailRemoteOperator`。
- Modify: `src-tauri/tests/email_commands.rs`
  - 增加远端成功/失败、本地不越权更新、归档/删除目标文件夹测试。
- Modify: `src/lib/bindings.ts`
  - 通过 `rtk cargo test --manifest-path src-tauri/Cargo.toml export_bindings` 或项目现有绑定生成命令更新，确保导出 `archiveEmail`。
- Modify: `src/lib/stores/email.svelte.ts`
  - 增加 `error`、`operatingIds`、`markAsRead`、`archiveEmail`、`moveEmailToFolder`、`refreshCurrentCategory`，并让 `selectEmail` 通过 `markAsRead` 走统一路径。
- Modify: `src/lib/components/email/EmailDetail.svelte`
  - 接通归档按钮；删除和星标等待 store 操作完成；失败时不静默。
- Modify: `src/lib/components/email/EmailContextMenu.svelte`
  - 接收操作 handler，接通转发、星标、已读/未读、删除；回复入口暂保留禁用或关闭菜单，不纳入本阶段。
- Modify: `src/lib/components/email/EmailList.svelte`
  - 给右键菜单传递 handlers；刷新按钮执行同步并重载当前分类；列表/网格按钮改为明确禁用状态。
- Modify: `src/lib/__tests__/stores/email-state.test.ts`
  - 增加 store 成功/失败更新行为测试。
- Modify: `src/lib/__tests__/stores/email.test.ts`
  - 增加 `archiveEmail` typed command 调用测试。

---

### Task 1: 后端 `mail_operation` 远端优先层

**Files:**
- Create: `src-tauri/src/service/mail_operation.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/tests/common/mod.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `DbConn`
  - `Arc<AuthManager>`
  - `email_repo::{get_by_id, mark_as_read, toggle_star, move_to_folder, soft_delete}`
  - `account_repo::get_by_id`
  - `PROVIDER_POOL`
  - `account_connection::imap_config_from_account`
- Produces:
  - `pub trait MailRemoteOperator: Send + Sync`
  - `pub struct RealMailRemoteOperator`
  - `pub struct MailOperationService`
  - `MailOperationService::new(db: DbConn, auth: Arc<AuthManager>, remote: Arc<dyn MailRemoteOperator>) -> Self`
  - `MailOperationService::{mark_as_read, toggle_star, move_to_folder, delete, archive}`
  - `EmailService::new_with_mail_remote(auth: Arc<AuthManager>, db: DbConn, remote: Arc<dyn MailRemoteOperator>) -> Self`

- [ ] **Step 1: Write failing remote-first backend tests**

Add imports to `src-tauri/tests/email_commands.rs`:

```rust
use async_trait::async_trait;
use postium_mail_lib::error::MailError;
use postium_mail_lib::infrastructure::storage::models::accounts;
use postium_mail_lib::service::mail_operation::MailRemoteOperator;
use std::sync::{Arc, Mutex};
```

Add fake remote below `create_test_account`:

```rust
#[derive(Default)]
struct FakeMailRemote {
    calls: Mutex<Vec<String>>,
    fail: bool,
}

impl FakeMailRemote {
    fn success() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn failing() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: true,
        })
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn maybe_fail(&self) -> Result<(), MailError> {
        if self.fail {
            return Err(MailError::ImapConnectionFailed("fake remote failure".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl MailRemoteOperator for FakeMailRemote {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError> {
        self.calls.lock().unwrap().push(format!(
            "mark_seen:{}:{folder}:{uid}:{seen}",
            account.email
        ));
        self.maybe_fail()
    }

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError> {
        self.calls.lock().unwrap().push(format!(
            "set_flagged:{}:{folder}:{uid}:{flagged}",
            account.email
        ));
        self.maybe_fail()
    }

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        self.calls.lock().unwrap().push(format!(
            "move_to_folder:{}:{folder}:{uid}:{target_folder}",
            account.email
        ));
        self.maybe_fail()
    }
}
```

Add tests:

```rust
#[tokio::test]
async fn test_mark_as_read_remote_success_updates_local() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(501, "远端已读")).await;

    svc.email_service.mark_as_read(email_id, true).await.unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(detail.email.is_read);
    assert_eq!(
        remote.calls(),
        vec!["mark_seen:test@example.com:INBOX:501:true"]
    );
}

#[tokio::test]
async fn test_mark_as_read_remote_failure_keeps_local_state() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(502, "远端失败")).await;

    let result = svc.email_service.mark_as_read(email_id, true).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(!detail.email.is_read);
    assert_eq!(
        remote.calls(),
        vec!["mark_seen:test@example.com:INBOX:502:true"]
    );
}

#[tokio::test]
async fn test_toggle_star_remote_success_updates_local() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(503, "远端星标")).await;

    let new_state = svc.email_service.toggle_star(email_id).await.unwrap();

    assert!(new_state);
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(detail.email.is_starred);
    assert_eq!(
        remote.calls(),
        vec!["set_flagged:test@example.com:INBOX:503:true"]
    );
}

#[tokio::test]
async fn test_toggle_star_remote_failure_keeps_local_state() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(504, "星标失败")).await;

    let result = svc.email_service.toggle_star(email_id).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert!(!detail.email.is_starred);
    assert_eq!(
        remote.calls(),
        vec!["set_flagged:test@example.com:INBOX:504:true"]
    );
}
```

- [ ] **Step 2: Run tests and verify they fail for missing module**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands test_mark_as_read_remote_success_updates_local test_mark_as_read_remote_failure_keeps_local_state test_toggle_star_remote_success_updates_local test_toggle_star_remote_failure_keeps_local_state
```

Expected: FAIL with unresolved import `postium_mail_lib::service::mail_operation` and missing `TestServices::new_with_mail_remote`.

- [ ] **Step 3: Create `mail_operation` implementation**

Create `src-tauri/src/service/mail_operation.rs`:

```rust
use crate::domain::auth::manager::Credentials;
use crate::domain::auth::AuthManager;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::infrastructure::storage::repository::{account_repo, email_repo};
use crate::infrastructure::storage::DbConn;
use crate::service::account_connection::imap_config_from_account;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait MailRemoteOperator: Send + Sync {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError>;

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError>;

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError>;
}

pub struct RealMailRemoteOperator {
    auth: Arc<AuthManager>,
}

impl RealMailRemoteOperator {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self { auth }
    }

    async fn connect_for_account(&self, account: &accounts::Model) -> Result<ImapClient, MailError> {
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let credentials = self
            .auth
            .get_credentials(
                &account.email,
                &provider.provider_info().auth_type,
                Some(&account.provider),
            )
            .await?;
        let imap_config = imap_config_from_account(account)?;

        match credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &account.email, &password).await
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await
            }
        }
    }
}

#[async_trait]
impl MailRemoteOperator for RealMailRemoteOperator {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if seen {
            client.add_flags(uid, "\\Seen").await?;
        } else {
            client.remove_flags(uid, "\\Seen").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if flagged {
            client.add_flags(uid, "\\Flagged").await?;
        } else {
            client.remove_flags(uid, "\\Flagged").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        client.move_uid_to_folder(uid, target_folder).await?;
        client.logout().await.ok();
        Ok(())
    }
}

pub struct MailOperationService {
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
}

impl MailOperationService {
    pub fn new(db: DbConn, _auth: Arc<AuthManager>, remote: Arc<dyn MailRemoteOperator>) -> Self {
        Self { db, remote }
    }

    async fn get_email(&self, email_id: i32) -> Result<emails::Model, MailError> {
        email_repo::get_by_id(&self.db, email_id)
            .await?
            .ok_or(MailError::EmailNotFound(email_id))
    }

    async fn get_account(&self, account_id: i32) -> Result<accounts::Model, MailError> {
        account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))
    }

    async fn email_and_account(&self, email_id: i32) -> Result<(emails::Model, accounts::Model), MailError> {
        let email = self.get_email(email_id).await?;
        let account = self.get_account(email.account_id).await?;
        Ok((email, account))
    }

    pub async fn mark_as_read(&self, email_id: i32, is_read: bool) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        self.remote
            .mark_seen(&account, &email.folder, email.uid, is_read)
            .await?;
        email_repo::mark_as_read(&self.db, email_id, is_read).await
    }

    pub async fn toggle_star(&self, email_id: i32) -> Result<bool, MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let new_state = !email.is_starred.unwrap_or(false);
        self.remote
            .set_flagged(&account, &email.folder, email.uid, new_state)
            .await?;
        email_repo::toggle_star(&self.db, email_id).await
    }

    pub async fn move_to_folder(&self, email_id: i32, target_folder: &str) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        if email.folder == target_folder {
            return Ok(());
        }
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, target_folder).await
    }

    pub async fn delete(&self, email_ids: Vec<i32>) -> Result<usize, MailError> {
        let mut moved = 0;
        for email_id in email_ids {
            let (email, account) = self.email_and_account(email_id).await?;
            let target_folder = self.resolve_special_folder(&account, "trash").await?;
            self.remote
                .move_to_folder(&account, &email.folder, email.uid, &target_folder)
                .await?;
            email_repo::move_to_folder(&self.db, email_id, &target_folder).await?;
            moved += 1;
        }
        Ok(moved)
    }

    pub async fn archive(&self, email_id: i32) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let target_folder = self.resolve_special_folder(&account, "archive").await?;
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, &target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, &target_folder).await
    }

    async fn resolve_special_folder(
        &self,
        account: &accounts::Model,
        kind: &str,
    ) -> Result<String, MailError> {
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let mapping = provider.folder_mapping();
        let candidates = match kind {
            "trash" => mapping.trash,
            "archive" => mapping.archive,
            _ => Vec::new(),
        };
        candidates
            .into_iter()
            .next()
            .ok_or_else(|| MailError::FolderNotFound(format!("账号 {} 未配置 {kind} 文件夹", account.email)))
    }
}
```

Add to `src-tauri/src/service/mod.rs`:

```rust
pub mod mail_operation;
```

- [ ] **Step 4: Wire `EmailService` constructors and methods**

Modify imports in `src-tauri/src/service/email_service.rs`:

```rust
use crate::service::mail_operation::{MailOperationService, MailRemoteOperator, RealMailRemoteOperator};
```

Change struct and constructors:

```rust
pub struct EmailService {
    auth: Arc<AuthManager>,
    db: DbConn,
    mail_operation: MailOperationService,
}

impl EmailService {
    pub fn new(auth: Arc<AuthManager>, db: DbConn) -> Self {
        let remote = Arc::new(RealMailRemoteOperator::new(auth.clone()));
        Self::new_with_mail_remote(auth, db, remote)
    }

    pub fn new_with_mail_remote(
        auth: Arc<AuthManager>,
        db: DbConn,
        remote: Arc<dyn MailRemoteOperator>,
    ) -> Self {
        let mail_operation = MailOperationService::new(db.clone(), auth.clone(), remote);
        Self {
            auth,
            db,
            mail_operation,
        }
    }
```

Replace existing operation methods:

```rust
    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), MailError> {
        self.mail_operation.mark_as_read(id, is_read).await
    }

    pub async fn toggle_star(&self, id: i32) -> Result<bool, MailError> {
        self.mail_operation.toggle_star(id).await
    }

    pub async fn delete(&self, ids: Vec<i32>) -> Result<usize, MailError> {
        self.mail_operation.delete(ids).await
    }

    pub async fn move_to_folder(&self, id: i32, folder: &str) -> Result<(), MailError> {
        self.mail_operation.move_to_folder(id, folder).await
    }

    pub async fn archive(&self, id: i32) -> Result<(), MailError> {
        self.mail_operation.archive(id).await
    }
```

- [ ] **Step 5: Add test service injection**

Modify `src-tauri/tests/common/mod.rs` imports:

```rust
use postium_mail_lib::service::mail_operation::{MailRemoteOperator, RealMailRemoteOperator};
```

Add constructor:

```rust
pub async fn new_with_mail_remote(mail_remote: Arc<dyn MailRemoteOperator>) -> Self {
    init_provider_pool();
    let db = create_test_db().await;
    let auth = Arc::new(AuthManager::in_memory());

    Self {
        account_service: AccountService::new_with_imap_verifier(
            db.clone(),
            auth.clone(),
            Arc::new(NoopImapConnectionVerifier),
        ),
        email_service: EmailService::new_with_mail_remote(auth.clone(), db.clone(), mail_remote),
        label_service: LabelService::new(db.clone()),
        db,
        auth,
    }
}
```

Update `new_with_imap_verifier` so it still avoids real network for tests:

```rust
let mail_remote = Arc::new(RealMailRemoteOperator::new(auth.clone()));
email_service: EmailService::new_with_mail_remote(auth.clone(), db.clone(), mail_remote),
```

- [ ] **Step 6: Run Task 1 tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands test_mark_as_read_remote_success_updates_local test_mark_as_read_remote_failure_keeps_local_state test_toggle_star_remote_success_updates_local test_toggle_star_remote_failure_keeps_local_state
```

Expected: FAIL only because `ImapClient::move_uid_to_folder` is not implemented yet, or PASS if Task 2 method was already added by the implementer before this run.

---

### Task 2: IMAP 移动、归档、删除远端语义

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `async_imap::Session::uid_mv`
  - `EmailService::archive`
  - `EmailService::delete`
- Produces:
  - `ImapClient::move_uid_to_folder(&mut self, uid: u32, target_folder: &str) -> Result<(), MailError>`
  - `archive_email(service: State<EmailService>, email_id: i32) -> Result<(), MailError>`

- [ ] **Step 1: Write failing tests for move/archive/delete**

Add tests to `src-tauri/tests/email_commands.rs`:

```rust
#[tokio::test]
async fn test_move_to_folder_remote_success_updates_local_folder() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(505, "移动邮件")).await;

    svc.email_service
        .move_to_folder(email_id, "Work")
        .await
        .unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "Work");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:505:Work"]
    );
}

#[tokio::test]
async fn test_move_to_folder_remote_failure_keeps_local_folder() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(506, "移动失败")).await;

    let result = svc.email_service.move_to_folder(email_id, "Work").await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "INBOX");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:506:Work"]
    );
}

#[tokio::test]
async fn test_archive_moves_remote_to_provider_archive_folder() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(507, "归档邮件")).await;

    svc.email_service.archive(email_id).await.unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "[Gmail]/All Mail");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:507:[Gmail]/All Mail"]
    );
}

#[tokio::test]
async fn test_delete_moves_remote_to_provider_trash_folder_without_soft_delete() {
    let remote = FakeMailRemote::success();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(508, "删除邮件")).await;

    let count = svc.email_service.delete(vec![email_id]).await.unwrap();

    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(detail.email.folder, "[Gmail]/Trash");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:508:[Gmail]/Trash"]
    );
}

#[tokio::test]
async fn test_delete_remote_failure_keeps_email_in_original_folder() {
    let remote = FakeMailRemote::failing();
    let svc = TestServices::new_with_mail_remote(remote.clone()).await;
    let account_id = create_test_account(&svc).await;
    let email_id = insert_test_email(&svc, account_id, TestEmail::new(509, "删除失败")).await;

    let result = svc.email_service.delete(vec![email_id]).await;

    assert!(result.is_err());
    let detail = svc.email_service.get(email_id).await.unwrap();
    assert_eq!(detail.email.folder, "INBOX");
    assert_eq!(
        remote.calls(),
        vec!["move_to_folder:test@example.com:INBOX:509:[Gmail]/Trash"]
    );
}
```

- [ ] **Step 2: Run tests and verify failures**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands test_move_to_folder_remote_success_updates_local_folder test_move_to_folder_remote_failure_keeps_local_folder test_archive_moves_remote_to_provider_archive_folder test_delete_moves_remote_to_provider_trash_folder_without_soft_delete test_delete_remote_failure_keeps_email_in_original_folder
```

Expected: FAIL until archive command/service and IMAP move wrapper exist.

- [ ] **Step 3: Add IMAP UID MOVE wrapper**

Add to `impl ImapClient` in `src-tauri/src/infrastructure/protocols/imap/mod.rs`, after `remove_flags`:

```rust
    /// 使用 UID MOVE 将邮件移动到目标文件夹。
    ///
    /// 该操作依赖服务器支持 RFC 6851 MOVE。第一阶段不做 COPY+Deleted 降级，
    /// 避免在不支持 MOVE 的服务器上产生重复邮件或半移动状态。
    pub async fn move_uid_to_folder(
        &mut self,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let uid_str = uid.to_string();
        self.session
            .uid_mv(&uid_str, target_folder)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("移动邮件失败: {e}")))?;
        Ok(())
    }
```

- [ ] **Step 4: Add archive command**

Add to `src-tauri/src/command/email.rs` near `move_email_to_folder`:

```rust
///
/// 归档邮件
///
/// 将邮件移动到当前服务商配置的归档文件夹。该操作先写入 IMAP 远端，
/// 远端成功后再更新本地邮件文件夹。
#[tauri::command]
#[specta::specta]
pub async fn archive_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<(), MailError> {
    tracing::info!(email_id, "命令: 归档邮件");
    service.archive(email_id).await
}
```

Update the `delete_emails` doc comment in the same file so it says:

```rust
/// 删除邮件
///
/// 第一阶段删除语义为移动到服务商配置的 Trash 文件夹。
/// 不执行永久删除，不执行 EXPUNGE。
```

Register the new command in `src-tauri/src/lib.rs` inside `collect_commands`:

```rust
            command::email::archive_email,
```

- [ ] **Step 5: Run backend operation tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands test_mark_as_read_remote_success_updates_local test_mark_as_read_remote_failure_keeps_local_state test_toggle_star_remote_success_updates_local test_toggle_star_remote_failure_keeps_local_state test_move_to_folder_remote_success_updates_local_folder test_move_to_folder_remote_failure_keeps_local_folder test_archive_moves_remote_to_provider_archive_folder test_delete_moves_remote_to_provider_trash_folder_without_soft_delete test_delete_remote_failure_keeps_email_in_original_folder
```

Expected: PASS.

---

### Task 3: 前端 store 统一操作状态和命令绑定

**Files:**
- Modify: `src/lib/bindings.ts`
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src/lib/__tests__/stores/email-state.test.ts`
- Modify: `src/lib/__tests__/stores/email.test.ts`

**Interfaces:**
- Consumes:
  - `commands.markAsRead(emailId, isRead)`
  - `commands.toggleStar(emailId)`
  - `commands.deleteEmails(emailIds)`
  - `commands.moveEmailToFolder(emailId, folder)`
  - `commands.archiveEmail(emailId)`
  - `commands.syncAccount(accountId)`
- Produces:
  - `EmailState.error: string`
  - `EmailState.operatingIds: Set<number>`
  - `EmailState.markAsRead(emailId: number, isRead: boolean): Promise<boolean>`
  - `EmailState.archiveEmail(emailId: number): Promise<boolean>`
  - `EmailState.moveEmailToFolder(emailId: number, folder: string): Promise<boolean>`
  - `EmailState.refreshCurrentCategory(accountId: number): Promise<void>`

- [ ] **Step 1: Regenerate/update bindings for `archiveEmail`**

Run the project binding generation command:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml export_bindings
```

Expected: PASS and `src/lib/bindings.ts` contains:

```ts
archiveEmail: (emailId: number) => typedError<null, MailError>(__TAURI_INVOKE("archive_email", { emailId })),
```

If the export command is absent, add the exact line manually next to `moveEmailToFolder` in `src/lib/bindings.ts` and then run:

```bash
rtk rg -n "archiveEmail|archive_email" src/lib/bindings.ts
```

Expected: one `archiveEmail` command binding exists.

- [ ] **Step 2: Write failing store tests**

Add to `src/lib/__tests__/stores/email-state.test.ts`:

```ts
it("markAsRead 成功后同步更新列表和选中详情", async () => {
  mockInvoke.mockResolvedValue(null);
  const state = new EmailState();
  state.emails = [{ ...email }];
  state.selectedEmail = { ...emailDetail };

  const ok = await state.markAsRead(1, true);

  expect(ok).toBe(true);
  expect(state.emails[0]!.is_read).toBe(true);
  expect(state.selectedEmail?.is_read).toBe(true);
  expect(mockInvoke).toHaveBeenCalledWith("mark_as_read", {
    emailId: 1,
    isRead: true,
  });
});

it("markAsRead 后端失败时不更新本地状态", async () => {
  mockInvoke.mockRejectedValue(new Error("remote failed"));
  const state = new EmailState();
  state.emails = [{ ...email, is_read: false }];
  state.selectedEmail = { ...emailDetail, is_read: false };

  const ok = await state.markAsRead(1, true);

  expect(ok).toBe(false);
  expect(state.emails[0]!.is_read).toBe(false);
  expect(state.selectedEmail?.is_read).toBe(false);
  expect(state.error).toContain("remote failed");
});

it("archiveEmail 成功后从当前列表移除并取消选择", async () => {
  mockInvoke.mockResolvedValue(null);
  const state = new EmailState();
  state.emails = [{ ...email }];
  state.total = 1;
  state.selectedEmailId = 1;
  state.selectedEmail = { ...emailDetail };

  const ok = await state.archiveEmail(1);

  expect(ok).toBe(true);
  expect(state.emails).toEqual([]);
  expect(state.total).toBe(0);
  expect(state.selectedEmailId).toBeNull();
  expect(mockInvoke).toHaveBeenCalledWith("archive_email", { emailId: 1 });
});

it("moveEmailToFolder 成功后从当前列表移除", async () => {
  mockInvoke.mockResolvedValue(null);
  const state = new EmailState();
  state.emails = [{ ...email }];
  state.total = 1;

  const ok = await state.moveEmailToFolder(1, "Work");

  expect(ok).toBe(true);
  expect(state.emails).toEqual([]);
  expect(state.total).toBe(0);
  expect(mockInvoke).toHaveBeenCalledWith("move_email_to_folder", {
    emailId: 1,
    folder: "Work",
  });
});
```

Add to `src/lib/__tests__/stores/email.test.ts`:

```ts
it("archiveEmail 调用正确", async () => {
  mockInvoke.mockResolvedValue(null);

  const result = await commands.archiveEmail(123);

  expect(result.status).toBe("ok");
  expect(mockInvoke).toHaveBeenCalledWith("archive_email", { emailId: 123 });
});
```

- [ ] **Step 3: Implement `EmailState` operation helpers**

Modify `src/lib/stores/email.svelte.ts`, add state fields after `loading`:

```ts
  /** 最近一次邮件操作错误，空字符串表示无错误 */
  error = $state("");

  /** 正在执行远端操作的邮件 ID 集合 */
  operatingIds = $state<Set<number>>(new Set());
```

Add helper methods inside `EmailState`:

```ts
  private beginOperation(emailId: number) {
    this.error = "";
    this.operatingIds = new Set([...this.operatingIds, emailId]);
  }

  private endOperation(emailId: number) {
    const next = new Set(this.operatingIds);
    next.delete(emailId);
    this.operatingIds = next;
  }

  private setError(e: unknown, fallback: string) {
    this.error = e instanceof Error ? e.message : fallback;
    console.error(fallback, e);
  }

  private removeFromCurrentList(emailId: number) {
    const before = this.emails.length;
    this.emails = this.emails.filter((email) => email.id !== emailId);
    if (this.total > 0 && this.emails.length < before) {
      this.total -= 1;
    }
    if (this.selectedEmailId === emailId) {
      this.deselectEmail();
    }
  }
```

Replace the auto mark read block in `selectEmail` with:

```ts
        if (!this.selectedEmail.is_read) {
          await this.markAsRead(id, true);
        }
```

Add public methods before `toggleStar`:

```ts
  async markAsRead(emailId: number, isRead: boolean): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.markAsRead(emailId, isRead);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }

      const email = this.emails.find((item) => item.id === emailId);
      if (email) email.is_read = isRead;

      if (this.selectedEmail?.id === emailId) {
        this.selectedEmail.is_read = isRead;
      }
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to mark email read state");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  async archiveEmail(emailId: number): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.archiveEmail(emailId);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }
      this.removeFromCurrentList(emailId);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to archive email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  async moveEmailToFolder(emailId: number, folder: string): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.moveEmailToFolder(emailId, folder);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }
      this.removeFromCurrentList(emailId);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to move email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }
```

Update `toggleStar` catch and typed error branch:

```ts
      if (result.status === "error") {
        this.error = formatError(result.error);
        return;
      }
```

Update `deleteEmails` to use operation tracking and not decrement below zero:

```ts
  async deleteEmails(ids: number[]) {
    ids.forEach((id) => this.beginOperation(id));
    try {
      const result = await commands.deleteEmails(ids);

      if (result.status === "error") {
        this.error = formatError(result.error);
        return;
      }

      const before = this.emails.length;
      this.emails = this.emails.filter((e) => !ids.includes(e.id));
      const removed = before - this.emails.length;

      if (this.selectedEmailId && ids.includes(this.selectedEmailId)) {
        this.deselectEmail();
      }

      this.total = Math.max(0, this.total - removed);
    } catch (e: unknown) {
      this.setError(e, "Failed to delete emails");
    } finally {
      ids.forEach((id) => this.endOperation(id));
    }
  }
```

Add refresh helper:

```ts
  async refreshCurrentCategory(accountId: number) {
    const result = await commands.syncAccount(accountId);
    if (result.status === "error") {
      this.error = formatError(result.error);
      return;
    }
    await this.loadEmailsByCategory(accountId, this.currentFolder, this.page);
  }
```

- [ ] **Step 4: Run frontend store tests**

Run:

```bash
rtk npm test -- --run src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/stores/email.test.ts
```

Expected: PASS.

---

### Task 4: 前端邮件操作入口接线

**Files:**
- Modify: `src/lib/components/email/EmailDetail.svelte`
- Modify: `src/lib/components/email/EmailContextMenu.svelte`
- Modify: `src/lib/components/email/EmailList.svelte`

**Interfaces:**
- Consumes:
  - `EmailState.markAsRead`
  - `EmailState.toggleStar`
  - `EmailState.deleteEmails`
  - `EmailState.archiveEmail`
  - `EmailState.refreshCurrentCategory`
  - `ComposeModal.showForward`
- Produces:
  - 详情页归档按钮执行真实 archive。
  - 右键菜单星标、已读/未读、删除、转发执行真实操作。
  - 刷新按钮同步当前账号并重载当前分类。

- [ ] **Step 1: Wire detail archive and async handlers**

Modify `src/lib/components/email/EmailDetail.svelte` handlers:

```ts
    async function handleToggleStar() {
        if (emailState.selectedEmail) {
            await emailState.toggleStar(emailState.selectedEmail.id);
        }
    }

    async function handleArchive() {
        if (emailState.selectedEmail) {
            await emailState.archiveEmail(emailState.selectedEmail.id);
        }
    }

    async function handleDelete() {
        if (emailState.selectedEmail) {
            await emailState.deleteEmails([emailState.selectedEmail.id]);
        }
    }
```

Change the archive button around the existing `Archive` icon so it calls `handleArchive`:

```svelte
<button
    class="icon-btn flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
    title={t.email.archive}
    onclick={handleArchive}
    disabled={emailState.selectedEmail
        ? emailState.operatingIds.has(emailState.selectedEmail.id)
        : false}
>
    <Archive size={18} />
</button>
```

Ensure star/delete buttons use `onclick={handleToggleStar}` and `onclick={handleDelete}` and disable while `operatingIds` contains the selected id.

- [ ] **Step 2: Replace context menu placeholder actions with handlers**

Modify props in `src/lib/components/email/EmailContextMenu.svelte`:

```ts
    let {
        x,
        y,
        emailId,
        isRead,
        isStarred,
        disabled = false,
        onToggleStar,
        onToggleRead,
        onDelete,
        onForward,
        onClose,
    }: {
        x: number;
        y: number;
        emailId: number;
        isRead: boolean;
        isStarred: boolean;
        disabled?: boolean;
        onToggleStar: (emailId: number) => Promise<void> | void;
        onToggleRead: (emailId: number, isRead: boolean) => Promise<void> | void;
        onDelete: (emailId: number) => Promise<void> | void;
        onForward: (emailId: number) => Promise<void> | void;
        onClose: () => void;
    } = $props();
```

Replace `handleAction`:

```ts
    async function handleAction(action: string) {
        if (disabled) return;

        if (action === "star") {
            await onToggleStar(emailId);
        } else if (action === "toggleRead") {
            await onToggleRead(emailId, !isRead);
        } else if (action === "delete") {
            await onDelete(emailId);
        } else if (action === "forward") {
            await onForward(emailId);
        }

        onClose();
    }
```

Disable reply/replyAll buttons for this phase:

```svelte
<button
    class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-muted-foreground opacity-50"
    disabled
>
```

Add `disabled={disabled}` to star/read/delete/forward buttons.

- [ ] **Step 3: Wire list context menu and refresh**

Modify imports in `src/lib/components/email/EmailList.svelte`:

```ts
    import { getContext } from "svelte";
    import type ComposeModal from "./ComposeModal.svelte";
```

Add after store setup:

```ts
    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal");
    const getComposeModal =
        getContext<() => ComposeModal | undefined>(COMPOSE_MODAL_KEY);
```

Add handlers:

```ts
    async function handleRefresh() {
        if (accountStore.activeAccountId) {
            await emailState.refreshCurrentCategory(accountStore.activeAccountId);
        }
    }

    async function handleContextToggleStar(emailId: number) {
        await emailState.toggleStar(emailId);
    }

    async function handleContextToggleRead(emailId: number, isRead: boolean) {
        await emailState.markAsRead(emailId, isRead);
    }

    async function handleContextDelete(emailId: number) {
        await emailState.deleteEmails([emailId]);
    }

    async function handleContextForward(emailId: number) {
        if (emailState.selectedEmailId !== emailId) {
            await emailState.selectEmail(emailId);
        }
        const modal = getComposeModal?.();
        const email = emailState.selectedEmail;
        if (modal && email) {
            modal.showForward(
                email.subject,
                email.body_text || email.body_html || "",
                email.sender_name,
                email.sent_at,
            );
        }
    }
```

Wire refresh button:

```svelte
<button
    data-testid="email-refresh-button"
    class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
    title={t.sidebar.sync}
    onclick={handleRefresh}
    disabled={!accountStore.activeAccountId || emailState.loading}
>
    <RefreshCw size={18} class={emailState.loading ? "animate-spin" : ""} />
</button>
```

Disable view buttons explicitly:

```svelte
<button
    class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground opacity-50"
    disabled
    aria-disabled="true"
>
```

Pass handlers to `EmailContextMenu` at the render site:

```svelte
<EmailContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    emailId={contextMenu.emailId}
    isRead={contextMenu.isRead}
    isStarred={contextMenu.isStarred}
    disabled={emailState.operatingIds.has(contextMenu.emailId)}
    onToggleStar={handleContextToggleStar}
    onToggleRead={handleContextToggleRead}
    onDelete={handleContextDelete}
    onForward={handleContextForward}
    onClose={closeContextMenu}
/>
```

- [ ] **Step 4: Run Svelte checks and autofixer**

Run:

```bash
rtk npm run check
```

Expected: PASS.

If the project has a Svelte autofixer script, run it:

```bash
rtk npm run format
```

Expected: PASS or a documented script-missing failure if `format` does not exist.

---

### Task 5: End-to-end verification and diff review

**Files:**
- Review all modified files.

**Interfaces:**
- Consumes all previous tasks.
- Produces verified working changes ready for user review. No commit unless the user explicitly asks.

- [ ] **Step 1: Run backend tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
rtk npm test -- --run
```

Expected: PASS.

- [ ] **Step 3: Run type/check suite**

Run:

```bash
rtk npm run check
```

Expected: PASS.

- [ ] **Step 4: Inspect working tree**

Run:

```bash
rtk git diff -- src-tauri/src/service/mail_operation.rs src-tauri/src/service/email_service.rs src-tauri/src/infrastructure/protocols/imap/mod.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/common/mod.rs src-tauri/tests/email_commands.rs src/lib/stores/email.svelte.ts src/lib/components/email/EmailDetail.svelte src/lib/components/email/EmailContextMenu.svelte src/lib/components/email/EmailList.svelte src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/stores/email.test.ts src/lib/bindings.ts
```

Expected: diff only covers the requested mail operations, remote-first logic, tests, and bindings.

- [ ] **Step 5: Report result without committing**

Final implementation report must include:

```text
已完成邮件操作本地/远端一致性改造，未提交代码。

验证：
- rtk cargo test --manifest-path src-tauri/Cargo.toml
- rtk npm test -- --run
- rtk npm run check
```

If any command fails, report the exact failing command and first actionable error.

---

## Self-Review

- Spec coverage: 计划覆盖 `mail_operation` 命名、远端优先、本地跟随、已读/未读 IMAP `\Seen`、星标 IMAP `\Flagged`、移动/归档/删除 IMAP UID MOVE、前端详情页/右键菜单/刷新入口、fake remote 测试。
- Placeholder scan: 未使用占位实现作为执行步骤；每个代码修改步骤给出具体代码或具体命令。
- Type consistency: `MailRemoteOperator` 从第一处定义开始就接收 `&accounts::Model`，fake remote、real remote、`MailOperationService` 调用一致；前端新增方法与组件 handler 名称一致。
- Risk note: `async-imap` 的 `uid_mv` 要求服务器支持 RFC 6851 MOVE。本阶段不做 COPY+Deleted 降级，避免不支持 MOVE 的服务器产生重复或半移动状态；如果后续需要兼容不支持 MOVE 的服务商，应单独设计安全降级策略。
