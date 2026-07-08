# Account Delete Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复设置页删除账号“点击无反应、重启后账号仍存在”的问题，并保证账号删除会清理本地关联数据。

**Architecture:** 账号删除继续由 `AccountService::delete` 作为业务入口，但不再依赖 SQLite foreign key cascade。Service 层按依赖顺序调用 repository 的账号级清理函数；前端 store 对 typed error 明确写入 `error`，设置页展示该错误。

**Tech Stack:** Rust 2024, SeaORM 2.0 RC, Tauri v2, Svelte 5 runes, Vitest, Bun, SQLite.

## Global Constraints

- Shell 命令必须加 `rtk` 前缀。
- 本次不重写数据库迁移或 FTS 触发器体系。
- 本次不引入账号软删除或回收站。
- 本次不重设计设置页账号管理 UI，只增加最小错误反馈。
- keyring 删除失败不阻断数据库删除结果，保持当前容忍策略。
- 触及 `.svelte` 或 `.svelte.ts` 文件后必须运行 Svelte autofixer。
- 生产代码改动前必须先写失败测试并确认失败。
- 全仓库 `cargo fmt --manifest-path src-tauri/Cargo.toml --check` 已知存在既有未格式化文件；本次触及的 Rust 文件必须通过 `rustfmt --edition 2024 --check`。

---

## File Structure

- Modify: `src-tauri/tests/account_commands.rs`
  - 添加后端回归测试，证明删除账号会清理邮件、附件、标签、邮件标签关联、同步状态、同步错误和 keyring 密码，同时不影响其他账号。

- Modify: `src-tauri/src/infrastructure/storage/repository/label_repo.rs`
  - 新增 `delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError>`，清理账号范围内的标签和邮件标签关联。

- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 新增 `delete_by_account(db: &DbConn, account_id: i32) -> Result<u64, MailError>`，清理账号范围内的附件和邮件。

- Modify: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
  - 新增 `delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError>`，清理账号范围内的同步状态和同步错误。

- Modify: `src-tauri/src/service/account_service.rs`
  - 在 `AccountService::delete` 中编排完整删除流程：查询账号、清理关联数据、删除账号、清理 keyring。

- Modify: `src/lib/__tests__/stores/account-state.test.ts`
  - 添加前端 store 回归测试，证明 `deleteAccount` 收到 typed error 时写入 `state.error`，不误删本地账号。

- Modify: `src/lib/stores/account.svelte.ts`
  - 在 `deleteAccount` 中清空旧错误、处理 `result.status === "error"`。

- Modify: `src/routes/settings/+page.svelte`
  - 在账号管理区域展示 `accountStore.error`，让用户点击删除失败时有可见反馈。

---

### Task 1: 后端账号级数据清理

**Files:**
- Modify: `src-tauri/tests/account_commands.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/label_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Modify: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
- Modify: `src-tauri/src/service/account_service.rs`

**Interfaces:**
- Consumes:
  - `common::insert_test_email(svc: &TestServices, account_id: i32, email: TestEmail) -> i32`
  - `AccountService::delete(&self, id: i32) -> Result<(), MailError>`
- Produces:
  - `label_repo::delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError>`
  - `email_repo::delete_by_account(db: &DbConn, account_id: i32) -> Result<u64, MailError>`
  - `sync_repo::delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError>`
  - `AccountService::delete` deletes account-local data without relying on SQLite foreign keys.

- [ ] **Step 1: Write the failing backend test**

Edit `src-tauri/tests/account_commands.rs`.

Change the top imports from:

```rust
use common::TestServices;
use postium_mail_lib::service::account_service::CreateAccountRequest;
```

to:

```rust
use common::{TestEmail, TestServices, insert_test_email};
use postium_mail_lib::infrastructure::storage::entities::{
    attachments, email_labels, emails, labels, sync_errors, sync_state,
};
use postium_mail_lib::service::account_service::CreateAccountRequest;
use postium_mail_lib::service::label_service::CreateLabelRequest;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
```

Append this test after `test_delete_account`:

```rust
#[tokio::test]
async fn test_delete_account_removes_local_account_data_without_foreign_keys() {
    let svc = TestServices::new().await;

    let account_a = svc
        .account_service
        .create(CreateAccountRequest {
            name: "Delete A".to_string(),
            email: "delete-a@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password-a".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        })
        .await
        .unwrap();
    let account_b = svc
        .account_service
        .create(CreateAccountRequest {
            name: "Keep B".to_string(),
            email: "keep-b@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password-b".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        })
        .await
        .unwrap();

    let email_a = insert_test_email(&svc, account_a.id, TestEmail::new(901, "delete me")).await;
    let email_b = insert_test_email(&svc, account_b.id, TestEmail::new(902, "keep me")).await;

    let label_a = svc
        .label_service
        .create_label(CreateLabelRequest {
            account_id: account_a.id,
            name: "delete label".to_string(),
            color: "#ff0000".to_string(),
        })
        .await
        .unwrap();
    let label_b = svc
        .label_service
        .create_label(CreateLabelRequest {
            account_id: account_b.id,
            name: "keep label".to_string(),
            color: "#00ff00".to_string(),
        })
        .await
        .unwrap();

    svc.label_service
        .add_label_to_email(email_a, label_a.id)
        .await
        .unwrap();
    svc.label_service
        .add_label_to_email(email_b, label_b.id)
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    attachments::Entity::insert(attachments::ActiveModel {
        email_id: Set(email_a),
        filename: Set(Some("delete.txt".to_string())),
        content_type: Set(Some("text/plain".to_string())),
        size: Set(12),
        section_path: Set("2".to_string()),
        disposition: Set(Some("attachment".to_string())),
        content_id: Set(None),
        path: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    attachments::Entity::insert(attachments::ActiveModel {
        email_id: Set(email_b),
        filename: Set(Some("keep.txt".to_string())),
        content_type: Set(Some("text/plain".to_string())),
        size: Set(34),
        section_path: Set("2".to_string()),
        disposition: Set(Some("attachment".to_string())),
        content_id: Set(None),
        path: Set(None),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();

    sync_state::Entity::insert(sync_state::ActiveModel {
        account_id: Set(account_a.id),
        folder: Set("INBOX".to_string()),
        folder_nick_name: Set(None),
        uidvalidity: Set(Some(1)),
        uidnext: Set(Some(2)),
        synced_at: Set(Some(now)),
        last_sync_uid: Set(Some(1)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();
    sync_errors::Entity::insert(sync_errors::ActiveModel {
        account_id: Set(account_a.id),
        folder: Set(Some("INBOX".to_string())),
        error_type: Set("test".to_string()),
        error_message: Set("delete error row".to_string()),
        uid: Set(Some(1)),
        stack_trace: Set(None),
        resolved: Set(Some(false)),
        created_at: Set(now),
        ..Default::default()
    })
    .exec(&svc.db)
    .await
    .unwrap();

    svc.account_service.delete(account_a.id).await.unwrap();

    assert!(svc.account_service.get(account_a.id).await.is_err());
    assert!(svc.auth.get_password(&account_a.email).is_err());

    assert_eq!(
        emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        attachments::Entity::find()
            .filter(attachments::Column::EmailId.eq(email_a))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        labels::Entity::find()
            .filter(labels::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        email_labels::Entity::find()
            .filter(email_labels::Column::EmailId.eq(email_a))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        email_labels::Entity::find()
            .filter(email_labels::Column::LabelId.eq(label_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sync_state::Entity::find()
            .filter(sync_state::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sync_errors::Entity::find()
            .filter(sync_errors::Column::AccountId.eq(account_a.id))
            .count(&svc.db)
            .await
            .unwrap(),
        0
    );

    assert!(svc.account_service.get(account_b.id).await.is_ok());
    assert_eq!(
        emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_b.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        attachments::Entity::find()
            .filter(attachments::Column::EmailId.eq(email_b))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        labels::Entity::find()
            .filter(labels::Column::AccountId.eq(account_b.id))
            .count(&svc.db)
            .await
            .unwrap(),
        1
    );
}
```

- [ ] **Step 2: Run backend test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_removes_local_account_data_without_foreign_keys --test account_commands -- --nocapture
```

Expected: FAIL. The expected failing assertion is one of the `count == 0` checks for account A data, because current `AccountService::delete` only deletes the account row and keyring password.

- [ ] **Step 3: Add label account cleanup repository function**

Edit `src-tauri/src/infrastructure/storage/repository/label_repo.rs`.

Change the entity import from:

```rust
use crate::infrastructure::storage::entities::{email_labels, labels};
```

to:

```rust
use crate::infrastructure::storage::entities::{email_labels, emails, labels};
```

Append this function after `delete`:

```rust
pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    let email_ids = emails::Entity::find()
        .select_only()
        .filter(emails::Column::AccountId.eq(account_id))
        .column(emails::Column::Id)
        .into_tuple::<i32>()
        .all(db)
        .await?;

    if !email_ids.is_empty() {
        email_labels::Entity::delete_many()
            .filter(email_labels::Column::EmailId.is_in(email_ids))
            .exec(db)
            .await?;
    }

    let label_ids = labels::Entity::find()
        .select_only()
        .filter(labels::Column::AccountId.eq(account_id))
        .column(labels::Column::Id)
        .into_tuple::<i32>()
        .all(db)
        .await?;

    if !label_ids.is_empty() {
        email_labels::Entity::delete_many()
            .filter(email_labels::Column::LabelId.is_in(label_ids))
            .exec(db)
            .await?;
    }

    labels::Entity::delete_many()
        .filter(labels::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(())
}
```

- [ ] **Step 4: Add email account cleanup repository function**

Edit `src-tauri/src/infrastructure/storage/repository/email_repo.rs`.

Append this function after `delete_by_folder`:

```rust
pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<u64, MailError> {
    let email_ids = emails::Entity::find()
        .select_only()
        .filter(emails::Column::AccountId.eq(account_id))
        .column(emails::Column::Id)
        .into_tuple::<i32>()
        .all(db)
        .await?;

    if !email_ids.is_empty() {
        attachments::Entity::delete_many()
            .filter(attachments::Column::EmailId.is_in(email_ids))
            .exec(db)
            .await?;
    }

    let delete_result = emails::Entity::delete_many()
        .filter(emails::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(delete_result.rows_affected)
}
```

- [ ] **Step 5: Add sync account cleanup repository function**

Edit `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`.

Append this function after `list_unresolved_errors`:

```rust
pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    sync_errors::Entity::delete_many()
        .filter(sync_errors::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    sync_state::Entity::delete_many()
        .filter(sync_state::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(())
}
```

- [ ] **Step 6: Wire cleanup into AccountService**

Edit `src-tauri/src/service/account_service.rs`.

Change:

```rust
use crate::infrastructure::storage::repository::account_repo;
```

to:

```rust
use crate::infrastructure::storage::repository::{
    account_repo, email_repo, label_repo, sync_repo,
};
```

Replace the body of `pub async fn delete(&self, id: i32) -> Result<(), MailError>` with:

```rust
pub async fn delete(&self, id: i32) -> Result<(), MailError> {
    tracing::info!(id, "删除账号");

    let account = account_repo::get_by_id(&self.db, id)
        .await?
        .ok_or(MailError::AccountNotFound(id))?;

    label_repo::delete_by_account(&self.db, id).await?;
    email_repo::delete_by_account(&self.db, id).await?;
    sync_repo::delete_by_account(&self.db, id).await?;
    account_repo::delete(&self.db, id).await?;

    let _ = self.auth.delete_password(&account.email);

    Ok(())
}
```

- [ ] **Step 7: Run backend test to verify it passes**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_removes_local_account_data_without_foreign_keys --test account_commands -- --nocapture
```

Expected: PASS with `test_delete_account_removes_local_account_data_without_foreign_keys ... ok`.

- [ ] **Step 8: Format and run account backend tests**

Run:

```bash
rtk rustfmt --edition 2024 src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs
rtk rustfmt --edition 2024 --check src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
```

Expected:
- rustfmt check exits 0.
- `account_commands` test binary exits 0.

- [ ] **Step 9: Commit backend fix**

Run:

```bash
rtk git add src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs
rtk git commit -m "fix: delete account local data"
```

Expected: commit succeeds.

---

### Task 2: 前端删除失败反馈

**Files:**
- Modify: `src/lib/__tests__/stores/account-state.test.ts`
- Modify: `src/lib/stores/account.svelte.ts`
- Modify: `src/routes/settings/+page.svelte`

**Interfaces:**
- Consumes:
  - `commands.deleteAccount(id) -> Promise<{ status: "ok"; data: null } | { status: "error"; error: MailError }>`
  - `formatError(e: unknown) -> string`
- Produces:
  - `AccountState.deleteAccount(id: number) -> Promise<void>` clears stale errors, preserves local account list on typed error, writes `error`.
  - Settings page renders account deletion errors at `data-testid="account-delete-error"`.

- [ ] **Step 1: Write failing frontend store test**

Edit `src/lib/__tests__/stores/account-state.test.ts`.

Append this test inside `describe("AccountState 状态行为", () => { ... })`, after the existing delete tests:

```ts
  it("deleteAccount 收到后端错误时保留账号并记录错误", async () => {
    mockInvoke.mockRejectedValue({
      type: "DatabaseError",
      message: "删除账号失败",
    });
    const state = new AccountState();
    state.accounts = [...accounts];
    state.activeAccountId = 1;

    await state.deleteAccount(1);

    expect(state.accounts.map((account) => account.id)).toEqual([1, 2]);
    expect(state.activeAccountId).toBe(1);
    expect(state.error).toBe("删除账号失败");
    expect(mockInvoke).toHaveBeenCalledWith("delete_account", { id: 1 });
  });
```

- [ ] **Step 2: Run frontend test to verify it fails**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/account-state.test.ts
```

Expected: FAIL. The expected failure is `state.error` being `null` or `"未知错误"` instead of `"删除账号失败"`.

- [ ] **Step 3: Implement AccountState typed error handling**

Edit `src/lib/stores/account.svelte.ts`.

Replace `async deleteAccount(id: number) { ... }` with:

```ts
  async deleteAccount(id: number) {
    this.error = null;

    try {
      // 调用后端命令删除账号
      const result = await commands.deleteAccount(id);

      if (result.status === "error") {
        this.error = String(result.error.message);
        return;
      }

      // 从本地列表中移除已删除的账号
      this.accounts = this.accounts.filter((a) => a.id !== id);

      // 如果删除的是当前活跃账号，需要切换到其他账号
      if (this.activeAccountId === id) {
        // 如果还有其他账号，选中第一个；否则设为 null
        this.activeAccountId =
          this.accounts.length > 0 ? this.accounts[0]!.id : null;
      }
    } catch (e: unknown) {
      // 捕获异常并格式化错误消息
      this.error = formatError(e);
    }
  }
```

- [ ] **Step 4: Show account delete error on settings page**

Edit `src/routes/settings/+page.svelte`.

Inside the account management section, after this header block:

```svelte
            <div class="mb-4 flex items-center justify-between">
                <div class="flex items-center gap-2">
                    <User size={18} class="text-primary" />
                    <h2 class="text-sm font-semibold text-foreground">
                        {t.settings.accounts}
                    </h2>
                </div>
            </div>
```

insert:

```svelte
            {#if accountStore.error}
                <div
                    data-testid="account-delete-error"
                    class="mb-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                >
                    {accountStore.error}
                </div>
            {/if}
```

- [ ] **Step 5: Run frontend test to verify it passes**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/account-state.test.ts
```

Expected: PASS. The output should show `account-state.test.ts` passed.

- [ ] **Step 6: Run Svelte validation for touched files**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/stores/account.svelte.ts
rtk npx @sveltejs/mcp svelte-autofixer ./src/routes/settings/+page.svelte
rtk bun run check
```

Expected:
- `account.svelte.ts` autofixer reports no issues.
- `settings/+page.svelte` autofixer reports no issues or only pre-existing stylistic suggestions that are unrelated to the inserted error block.
- `bun run check` exits 0.

- [ ] **Step 7: Run frontend test suite**

Run:

```bash
rtk bun run test:frontend
```

Expected: PASS with all frontend test files passing.

- [ ] **Step 8: Commit frontend fix**

Run:

```bash
rtk git add src/lib/__tests__/stores/account-state.test.ts src/lib/stores/account.svelte.ts src/routes/settings/+page.svelte
rtk git commit -m "fix: show account delete errors"
```

Expected: commit succeeds.

---

### Task 3: 最终验证与整理

**Files:**
- Verify only; no planned source edits.

**Interfaces:**
- Consumes:
  - Backend behavior from Task 1.
  - Frontend behavior from Task 2.
- Produces:
  - Verified branch with clean worktree.

- [ ] **Step 1: Run backend full test suite**

Run:

```bash
rtk bun run test:rust
```

Expected: PASS. All Rust unit, integration, and doc test compilation checks complete successfully.

- [ ] **Step 2: Run frontend full test suite**

Run:

```bash
rtk bun run test:frontend
```

Expected: PASS. All frontend Vitest files pass.

- [ ] **Step 3: Run Svelte check**

Run:

```bash
rtk bun run check
```

Expected: PASS with `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 4: Run touched Rust formatting check**

Run:

```bash
rtk rustfmt --edition 2024 --check src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs
```

Expected: exits 0.

- [ ] **Step 5: Check for conflict markers and whitespace errors**

Run:

```bash
rtk git grep -n -E '^(<<<<<<< .+|=======|>>>>>>> .+)$' -- src src-tauri e2e docs
rtk git diff --check
```

Expected:
- `git grep` exits 1 with no output.
- `git diff --check` exits 0.

- [ ] **Step 6: Inspect final status**

Run:

```bash
rtk git status --short --branch
rtk git log --oneline --decorate -5
```

Expected:
- Worktree is clean.
- Latest commits include `fix: delete account local data` and `fix: show account delete errors`.

- [ ] **Step 7: Report completion**

Report:

- Backend account deletion now explicitly clears account-local data before deleting the account row.
- Frontend delete failures are no longer silent.
- List the verification commands and pass/fail results.
- Mention that all commits are local and not pushed unless a push was explicitly requested.
