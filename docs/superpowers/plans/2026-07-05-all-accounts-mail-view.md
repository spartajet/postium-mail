# 所有账号邮件视图 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在账号选择器中增加“所有账号”视图，让用户按文件夹分类聚合查看、搜索、刷新和处理多个邮箱账号的邮件。

**Architecture:** 前端引入明确的 `AccountScope` 状态，保留兼容的 `activeAccountId` getter 以降低改动面。后端新增跨账号邮件查询、跨账号统计和同步所有账号命令，由服务层负责逐账号解析 provider 文件夹映射，再由数据库统一排序分页。UI 根据账号范围切换单账号接口或所有账号接口，并在聚合视图显示邮件来源账号和写信发件账号选择器。

**Tech Stack:** Tauri v2、Rust、rusqlite、tauri-specta、SvelteKit、Svelte 5 Runes、Vitest、Testing Library Svelte、WebdriverIO e2e。

## Global Constraints

- 文档使用中文书写。
- 没有用户明确指令，不提交代码。
- shell 命令使用 `rtk` 前缀。
- 使用 TDD：每个行为改动先写失败测试，再实现。
- 默认账号选择：首次启动为“所有账号”；后续恢复上次选择。
- 所有账号视图下同步按钮必须同步全部账号，然后刷新当前文件夹和统计。
- 所有账号邮件列表必须显示每封邮件所属账号。
- 所有账号视图下写邮件必须选择发件账号。
- 回复或转发邮件时，默认发件账号使用原邮件所属账号。
- 所有账号分页必须由后端统一排序后分页，不能前端合并多个单账号分页结果。
- 不引入数据库分类字段或迁移历史邮件分类。
- 第一版不支持所有账号视图下“同步更早邮件”，该入口在聚合视图隐藏或禁用。

---

## File Structure

- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 新增跨账号分类条件查询、跨账号星标查询、带账号展示字段的邮件行映射。
  - 保持现有单账号查询接口可用。
- Modify: `src-tauri/src/service/email_service.rs`
  - 扩展 `EmailDto`，增加 `account_email` 和 `account_display_name`。
  - 新增 `list_by_category_for_all_accounts(category, page, limit, unread_only)`。
  - 复用每个账号的 `FolderRegistry` 解析逻辑。
- Modify: `src-tauri/src/command/email.rs`
  - 新增 Tauri 命令 `list_emails_by_category_for_all_accounts`。
- Modify: `src-tauri/src/service/sync_service.rs`
  - 新增 `get_folder_stats_for_all_accounts()`。
  - 新增 `sync_all_accounts(app_handle)` 和返回 DTO。
- Modify: `src-tauri/src/command/sync.rs`
  - 新增 Tauri 命令 `get_folder_stats_for_all_accounts` 和 `sync_all_accounts`。
- Modify: `src-tauri/src/lib.rs`
  - 注册新增 email/sync commands 到 specta builder。
- Modify: `src/lib/bindings.ts`
  - 重新生成或手工同步新增命令与 DTO 字段。
- Modify: `src/lib/stores/account.svelte.ts`
  - 引入 `AccountScope`、持久化、`setAllAccounts()`、`lastConcreteAccountId`。
- Modify: `src/lib/stores/email.svelte.ts`
  - 增加所有账号加载、分页、刷新、未读筛选方法。
- Modify: `src/lib/stores/sync.svelte.ts`
  - 增加所有账号统计加载和所有账号同步方法。
- Modify: `src/lib/components/layout/Sidebar.svelte`
  - 下拉框增加“所有账号”选项。
  - 文件夹点击、同步按钮、统计加载按账号范围分支。
- Modify: `src/lib/components/email/EmailList.svelte`
  - 所有账号视图下显示账号来源。
  - 搜索传 `accountId = null`。
  - 聚合视图下隐藏或禁用“同步更早邮件”。
- Modify: `src/lib/components/email/ComposeModal.svelte`
  - 所有账号视图下显示发件账号选择器。
  - 回复/转发支持传入原邮件账号 ID。
- Modify: `src/lib/components/email/EmailDetail.svelte`
  - 调用回复/转发时传入当前邮件所属账号 ID。
- Modify: `src/routes/(main)/+layout.svelte`
  - 托盘同步动作按账号范围选择同步单账号或所有账号。
- Modify: `src/lib/__tests__/stores/account-state.test.ts`
  - 覆盖账号范围持久化和删除回退。
- Modify: `src/lib/__tests__/stores/email-state.test.ts`
  - 覆盖所有账号邮件加载、分页、刷新、未读筛选。
- Modify: `src/lib/__tests__/stores/sync-history.test.ts` or create `src/lib/__tests__/stores/sync-all-accounts.test.ts`
  - 覆盖所有账号统计和同步 store 方法。
- Modify: `src/lib/__tests__/components/Sidebar.test.ts`
  - 覆盖“所有账号”下拉选项和文件夹加载分支。
- Modify: `src/lib/__tests__/components/EmailList.test.ts`
  - 覆盖账号来源标识、搜索参数、同步更早入口隐藏。
- Modify: `src/lib/__tests__/components/EmailDetail.test.ts`
  - 覆盖回复/转发传入原邮件账号 ID。
- Create/Modify: `src/lib/__tests__/components/ComposeModal.test.ts`
  - 覆盖所有账号视图下发件账号选择。
- Modify: `src-tauri/tests/email_repository.rs`
  - 覆盖跨账号 repository 查询。
- Modify: `src-tauri/tests/email_commands.rs`
  - 覆盖 service 层跨账号分类查询和 DTO 账号字段。
- Modify: `src-tauri/tests/sync_history.rs` or create `src-tauri/tests/all_accounts_sync.rs`
  - 覆盖所有账号统计和同步结果结构。
- Modify: `e2e/test/specs/account-switching.e2e.js` or create `e2e/test/specs/all-accounts.e2e.js`
  - 覆盖聚合视图核心流程。

---

### Task 1: 后端邮件 DTO 与跨账号 Repository 查询

**Files:**
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/tests/email_repository.rs`

**Interfaces:**
- Produces:
  - `pub struct AccountFolderFilter { pub account_id: i32, pub folders: Vec<String> }`
  - `pub async fn list_starred_all_accounts(db: &DbConn, page: usize, limit: usize, unread_only: bool) -> Result<(Vec<emails::Model>, u64), MailError>`
  - `pub async fn list_by_account_folder_filters(db: &DbConn, filters: Vec<AccountFolderFilter>, page: usize, limit: usize, unread_only: bool) -> Result<(Vec<emails::Model>, u64), MailError>`
  - `EmailDto.account_email: Option<String>`
  - `EmailDto.account_display_name: Option<String>`
- Consumes:
  - Existing `email_repo::list_starred`
  - Existing `email_repo::list_by_folders`
  - Existing `EmailDto`

- [ ] **Step 1: Write failing repository tests for global sorting and unread filtering**

Append to `src-tauri/tests/email_repository.rs`:

```rust
async fn seed_two_accounts(db: &DbConn) {
    db.call(|conn| {
        conn.execute(
            "INSERT INTO accounts (
                id, name, email, display_name, provider, auth_type, account_type, created_at, updated_at
            ) VALUES
                (1, 'Work', 'work@example.com', 'Work Mail', 'gmail', 'password', 'work', 1, 1),
                (2, 'Personal', 'personal@example.com', 'Personal Mail', 'outlook', 'password', 'personal', 2, 2)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn insert_repo_email(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: u32,
    subject: &str,
    sent_at: i64,
    is_read: bool,
    is_starred: bool,
) {
    let folder = folder.to_string();
    let subject = subject.to_string();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO emails (
                account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'Sender', 'sender@example.com',
                'to@example.com', '', '', '', ?6, ?7, 0, 0, 0, ?8, ?8, ?8, ?8)",
            rusqlite::params![
                account_id,
                folder,
                uid,
                format!("<repo-{account_id}-{uid}@example.com>"),
                subject,
                if is_read { 1 } else { 0 },
                if is_starred { 1 } else { 0 },
                sent_at,
            ],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn list_by_account_folder_filters_should_merge_sort_and_page_across_accounts() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(&db, 1, "INBOX", 101, "older work inbox", 100, false, false).await;
    insert_repo_email(&db, 2, "Inbox", 201, "newer personal inbox", 300, false, false).await;
    insert_repo_email(&db, 1, "Sent", 102, "work sent ignored", 400, false, false).await;

    let (items, total) = email_repo::list_by_account_folder_filters(
        &db,
        vec![
            email_repo::AccountFolderFilter {
                account_id: 1,
                folders: vec!["INBOX".to_string()],
            },
            email_repo::AccountFolderFilter {
                account_id: 2,
                folders: vec!["Inbox".to_string()],
            },
        ],
        1,
        1,
        false,
    )
    .await
    .unwrap();

    assert_eq!(total, 2);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("newer personal inbox"));
    assert_eq!(items[0].account_id, 2);
}

#[tokio::test]
async fn list_starred_all_accounts_should_apply_unread_filter() {
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_two_accounts(&db).await;
    insert_repo_email(&db, 1, "INBOX", 101, "read starred", 100, true, true).await;
    insert_repo_email(&db, 2, "Archive", 201, "unread starred", 200, false, true).await;

    let (items, total) = email_repo::list_starred_all_accounts(&db, 1, 50, true)
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].subject.as_deref(), Some("unread starred"));
    assert_eq!(items[0].account_id, 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --test email_repository list_by_account_folder_filters_should_merge_sort_and_page_across_accounts
rtk cargo test --test email_repository list_starred_all_accounts_should_apply_unread_filter
```

Expected:

- FAIL because `AccountFolderFilter`, `list_by_account_folder_filters`, and `list_starred_all_accounts` do not exist.

- [ ] **Step 3: Implement repository filter type and cross-account queries**

In `src-tauri/src/infrastructure/storage/repository/email_repo.rs`, add near existing query helpers:

```rust
#[derive(Clone, Debug)]
pub struct AccountFolderFilter {
    pub account_id: i32,
    pub folders: Vec<String>,
}
```

Add after `list_starred`:

```rust
pub async fn list_starred_all_accounts(
    db: &DbConn,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    db.call(move |conn| {
        let unread_value = if unread_only { 1 } else { 0 };
        let total = conn.query_row(
            "SELECT COUNT(*)
             FROM emails
             WHERE is_starred = 1 AND is_deleted = 0
               AND (?1 = 0 OR is_read = 0 OR is_read IS NULL)",
            [unread_value],
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE is_starred = 1 AND is_deleted = 0
               AND (?1 = 0 OR is_read = 0 OR is_read IS NULL)
             ORDER BY sent_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![unread_value, limit as i64, offset],
                map_email,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
}

pub async fn list_by_account_folder_filters(
    db: &DbConn,
    filters: Vec<AccountFolderFilter>,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    let filters = filters
        .into_iter()
        .filter(|filter| !filter.folders.is_empty())
        .collect::<Vec<_>>();

    if filters.is_empty() {
        return Ok((Vec::new(), 0));
    }

    db.call(move |conn| {
        let mut values = Vec::<Value>::new();
        let mut groups = Vec::<String>::new();

        for filter in &filters {
            let in_clause = placeholders(filter.folders.len());
            groups.push(format!("(account_id = ? AND folder IN ({in_clause}))"));
            values.push(Value::from(filter.account_id));
            values.extend(filter.folders.iter().cloned().map(Value::from));
        }

        let where_groups = groups.join(" OR ");
        let unread_value = if unread_only { 1 } else { 0 };
        let count_sql = format!(
            "SELECT COUNT(*)
             FROM emails
             WHERE ({where_groups}) AND is_deleted = 0
               AND (? = 0 OR is_read = 0 OR is_read IS NULL)"
        );
        let mut count_values = values.clone();
        count_values.push(Value::from(unread_value));
        let total = conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(count_values.iter()),
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let select_sql = format!(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE ({where_groups}) AND is_deleted = 0
               AND (? = 0 OR is_read = 0 OR is_read IS NULL)
             ORDER BY sent_at DESC
             LIMIT ? OFFSET ?"
        );
        let mut select_values = count_values;
        select_values.push(Value::from(limit as i64));
        select_values.push(Value::from(offset));
        let mut stmt = conn.prepare(&select_sql)?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(select_values.iter()), map_email)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok((items, total))
    })
    .await
}
```

- [ ] **Step 4: Extend `EmailDto` with account display fields**

In `src-tauri/src/service/email_service.rs`, add fields to `EmailDto`:

```rust
pub account_email: Option<String>,
pub account_display_name: Option<String>,
```

Update `email_model_to_dto`:

```rust
account_email: None,
account_display_name: None,
```

Add helper:

```rust
fn attach_account_display(
    mut dto: EmailDto,
    account: Option<&crate::infrastructure::storage::models::accounts::Model>,
) -> EmailDto {
    if let Some(account) = account {
        dto.account_email = Some(account.email.clone());
        dto.account_display_name = account.display_name.clone();
    }
    dto
}
```

Keep existing conversion behavior otherwise unchanged.

- [ ] **Step 5: Run repository tests**

Run:

```bash
rtk cargo test --test email_repository list_by_account_folder_filters_should_merge_sort_and_page_across_accounts
rtk cargo test --test email_repository list_starred_all_accounts_should_apply_unread_filter
```

Expected:

- PASS.

---

### Task 2: 后端所有账号邮件服务与命令

**Files:**
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/bindings.ts`
- Modify: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Consumes:
  - `email_repo::AccountFolderFilter`
  - `email_repo::list_starred_all_accounts`
  - `email_repo::list_by_account_folder_filters`
- Produces:
  - `EmailService::list_by_category_for_all_accounts(category: EmailCategory, page: usize, limit: usize, unread_only: bool) -> Result<EmailListResponse, MailError>`
  - Tauri command `list_emails_by_category_for_all_accounts`
  - Binding `commands.listEmailsByCategoryForAllAccounts(category, page, limit, unreadOnly)`

- [ ] **Step 1: Write failing service test for all-account inbox query**

Append to `src-tauri/tests/email_commands.rs`:

```rust
#[tokio::test]
async fn list_by_category_for_all_accounts_should_merge_inbox_across_accounts() {
    let svc = TestServices::new().await;
    let work_id = create_test_account(&svc).await;
    let personal_id = create_test_account(&svc).await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(100, "work inbox").folder("INBOX").sent_at(100),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(200, "personal inbox").folder("INBOX").sent_at(300),
    )
    .await;
    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(101, "work sent").folder("[Gmail]/Sent Mail").sent_at(400),
    )
    .await;

    let response = svc
        .email_service
        .list_by_category_for_all_accounts(EmailCategory::Inbox, 1, 50, false)
        .await
        .unwrap();

    assert_eq!(response.total, 2);
    assert_eq!(response.emails.len(), 2);
    assert_eq!(response.emails[0].subject.as_deref(), Some("personal inbox"));
    assert_eq!(response.emails[0].account_id, personal_id);
    assert_eq!(
        response.emails[0].account_email.as_deref(),
        Some(&format!("test-{}@gmail.com", personal_id))
    );
    assert_eq!(response.emails[1].subject.as_deref(), Some("work inbox"));
}

#[tokio::test]
async fn list_by_category_for_all_accounts_should_query_starred_across_folders() {
    let svc = TestServices::new().await;
    let work_id = create_test_account(&svc).await;
    let personal_id = create_test_account(&svc).await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(300, "work starred")
            .folder("INBOX")
            .starred(true)
            .sent_at(100),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(400, "personal starred")
            .folder("Archive")
            .starred(true)
            .sent_at(200),
    )
    .await;

    let response = svc
        .email_service
        .list_by_category_for_all_accounts(EmailCategory::Starred, 1, 50, false)
        .await
        .unwrap();

    assert_eq!(response.total, 2);
    assert_eq!(response.emails[0].subject.as_deref(), Some("personal starred"));
    assert_eq!(response.emails[1].subject.as_deref(), Some("work starred"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --test email_commands list_by_category_for_all_accounts_should_merge_inbox_across_accounts
rtk cargo test --test email_commands list_by_category_for_all_accounts_should_query_starred_across_folders
```

Expected:

- FAIL because `EmailService::list_by_category_for_all_accounts` does not exist.

- [ ] **Step 3: Add helper to resolve category folders for one account**

In `src-tauri/src/service/email_service.rs`, extract the folder resolution logic from `list_by_category` into a private async helper:

```rust
async fn resolve_category_folders_for_account(
    db: &DbConn,
    account: &crate::infrastructure::storage::models::accounts::Model,
    category: &EmailCategory,
) -> Result<Vec<String>, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported(
            "未找到provider pool".into(),
        ))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let (local_names, known_categories) = local_folder_registry_inputs(db, account.id).await?;
    let registry = FolderRegistry::builder()
        .remote_folders(local_names)
        .known_categories(known_categories)
        .provider_mapping(provider.folder_mapping())
        .build();
    let cat = match FolderCategory::from_email_category(category) {
        Some(cat) => cat,
        None => return Ok(Vec::new()),
    };
    Ok(registry.resolve(cat))
}
```

Update existing `list_by_category` to call this helper for non-starred categories.

- [ ] **Step 4: Implement all-account service method**

In `impl EmailService`, add:

```rust
pub async fn list_by_category_for_all_accounts(
    &self,
    category: EmailCategory,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<EmailListResponse, MailError> {
    if category == EmailCategory::Starred {
        let (emails, total) =
            email_repo::list_starred_all_accounts(&self.db, page, limit, unread_only).await?;
        return Ok(EmailListResponse {
            emails: convert_models_with_account_display(&self.db, emails).await?,
            total,
            page,
            limit,
        });
    }

    let accounts = account_repo::list(&self.db).await?;
    let mut filters = Vec::new();
    let mut failures = Vec::new();

    for account in accounts {
        match resolve_category_folders_for_account(&self.db, &account, &category).await {
            Ok(folders) if !folders.is_empty() => {
                filters.push(email_repo::AccountFolderFilter {
                    account_id: account.id,
                    folders,
                });
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    account_id = account.id,
                    error = %err,
                    "所有账号分类查询跳过异常账号"
                );
                failures.push(err);
            }
        }
    }

    if filters.is_empty() {
        if let Some(err) = failures.into_iter().next() {
            return Err(err);
        }
        return Ok(EmailListResponse {
            emails: Vec::new(),
            total: 0,
            page,
            limit,
        });
    }

    let (emails, total) =
        email_repo::list_by_account_folder_filters(&self.db, filters, page, limit, unread_only)
            .await?;
    Ok(EmailListResponse {
        emails: convert_models_with_account_display(&self.db, emails).await?,
        total,
        page,
        limit,
    })
}
```

Add conversion helper near `convert_models_with_attachments`:

```rust
async fn convert_models_with_account_display(
    db: &DbConn,
    emails: Vec<emails::Model>,
) -> Result<Vec<EmailDto>, MailError> {
    let mut dtos = convert_models_with_attachments(db, emails).await?;
    let accounts = account_repo::list(db).await?;

    for dto in &mut dtos {
        if let Some(account) = accounts.iter().find(|account| account.id == dto.account_id) {
            dto.account_email = Some(account.email.clone());
            dto.account_display_name = account.display_name.clone();
        }
    }

    Ok(dtos)
}
```

- [ ] **Step 5: Add command and specta registration**

In `src-tauri/src/command/email.rs`, add:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_emails_by_category_for_all_accounts(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    category: EmailCategory,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<EmailListResponse, MailError> {
    tracing::debug!(category = ?category, page, limit, unread_only, "命令: 按分类列出所有账号邮件");
    service
        .list_by_category_for_all_accounts(category, page, limit, unread_only)
        .await
}
```

In `src-tauri/src/lib.rs`, add to `collect_commands!`:

```rust
command::email::list_emails_by_category_for_all_accounts,
```

- [ ] **Step 6: Update bindings**

Run binding generation if the project supports it during check/build. If not generated automatically, update `src/lib/bindings.ts`:

```ts
listEmailsByCategoryForAllAccounts: (
    category: EmailCategory,
    page: number,
    limit: number,
    unreadOnly: boolean,
) =>
    typedError<EmailListResponse, MailError>(
        __TAURI_INVOKE("list_emails_by_category_for_all_accounts", {
            category,
            page,
            limit,
            unreadOnly,
        }),
    ),
```

Add `account_email?: string | null` and `account_display_name?: string | null` to generated `EmailDto` type using the style already present in the file.

- [ ] **Step 7: Run tests**

Run:

```bash
rtk cargo test --test email_commands list_by_category_for_all_accounts_should_merge_inbox_across_accounts
rtk cargo test --test email_commands list_by_category_for_all_accounts_should_query_starred_across_folders
```

Expected:

- PASS.

---

### Task 3: 后端所有账号统计与同步命令

**Files:**
- Modify: `src-tauri/src/service/sync_service.rs`
- Modify: `src-tauri/src/command/sync.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/bindings.ts`
- Create: `src-tauri/tests/all_accounts_sync.rs`

**Interfaces:**
- Produces:
  - `pub struct SyncAllAccountsResult`
  - `pub struct SyncAccountFailure`
  - `SyncService::get_folder_stats_for_all_accounts(&self) -> Result<Vec<FolderStat>, MailError>`
  - `SyncService::sync_all_accounts_with_progress(&self, app_handle: tauri::AppHandle) -> Result<SyncAllAccountsResult, MailError>`
  - Commands `get_folder_stats_for_all_accounts`, `sync_all_accounts`

- [ ] **Step 1: Write failing stats service test**

Create `src-tauri/tests/all_accounts_sync.rs`:

```rust
mod common;

use common::{TestEmail, TestServices, insert_test_email};
use postium_mail_lib::service::account_service::CreateAccountRequest;

static ACCOUNT_EMAILS: &[&str] = &["all-work@gmail.com", "all-personal@gmail.com"];

async fn create_account(svc: &TestServices, index: usize) -> i32 {
    let req = CreateAccountRequest {
        name: format!("All Account {}", index),
        email: ACCOUNT_EMAILS[index].to_string(),
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

#[tokio::test]
async fn get_folder_stats_for_all_accounts_should_sum_sidebar_categories() {
    let svc = TestServices::new().await;
    let work_id = create_account(&svc, 0).await;
    let personal_id = create_account(&svc, 1).await;

    insert_test_email(
        &svc,
        work_id,
        TestEmail::new(10, "work unread").folder("INBOX").read(false),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(20, "personal read").folder("INBOX").read(true),
    )
    .await;
    insert_test_email(
        &svc,
        personal_id,
        TestEmail::new(21, "personal starred").folder("INBOX").starred(true).read(false),
    )
    .await;

    let stats = svc.sync_service.get_folder_stats_for_all_accounts().await.unwrap();
    let inbox = stats.iter().find(|stat| stat.folder == "inbox").unwrap();
    let starred = stats.iter().find(|stat| stat.folder == "starred").unwrap();

    assert_eq!(inbox.total, 3);
    assert_eq!(inbox.unread, 2);
    assert_eq!(starred.total, 1);
    assert_eq!(starred.unread, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --test all_accounts_sync get_folder_stats_for_all_accounts_should_sum_sidebar_categories
```

Expected:

- FAIL because `sync_service` field may need exposure in `TestServices`, and `get_folder_stats_for_all_accounts` does not exist.

- [ ] **Step 3: Expose `SyncService` in test helper**

Modify `src-tauri/tests/common/mod.rs`:

```rust
use postium_mail_lib::service::{AccountService, LabelService, SyncService};
```

Add field:

```rust
pub sync_service: SyncService,
```

In `TestServices::new_with_imap_verifier` and `new_with_mail_remote`, initialize:

```rust
sync_service: SyncService::new(db.clone(), auth.clone()),
```

Use existing constructor signature from `src-tauri/src/service/sync_service.rs`.

- [ ] **Step 4: Add DTOs and all-account stats service**

In `src-tauri/src/service/sync_service.rs`, add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SyncAccountFailure {
    pub account_id: i32,
    pub email: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SyncAllAccountsResult {
    pub total: usize,
    pub succeeded: Vec<i32>,
    pub failed: Vec<SyncAccountFailure>,
}
```

Add method in `impl SyncService`:

```rust
pub async fn get_folder_stats_for_all_accounts(&self) -> Result<Vec<FolderStat>, MailError> {
    let accounts = account_repo::list(&self.db).await?;
    let mut grouped = HashMap::<String, FolderStat>::new();
    let mut failures = Vec::new();

    for account in accounts {
        match self.get_folder_stats(account.id).await {
            Ok(stats) => {
                for stat in stats {
                    let entry = grouped.entry(stat.folder.clone()).or_insert(FolderStat {
                        folder: stat.folder,
                        total: 0,
                        unread: 0,
                    });
                    entry.total += stat.total;
                    entry.unread += stat.unread;
                }
            }
            Err(err) => {
                tracing::warn!(
                    account_id = account.id,
                    error = %err,
                    "所有账号统计跳过异常账号"
                );
                failures.push(err);
            }
        }
    }

    if grouped.is_empty() {
        if let Some(err) = failures.into_iter().next() {
            return Err(err);
        }
    }

    let mut stats = grouped.into_values().collect::<Vec<_>>();
    stats.sort_by(|a, b| a.folder.cmp(&b.folder));
    Ok(stats)
}
```

Ensure `HashMap` and `account_repo` are imported in `sync_service.rs`; they likely already are, otherwise add:

```rust
use std::collections::HashMap;
use crate::infrastructure::storage::repository::account_repo;
```

- [ ] **Step 5: Add sync-all service method**

In `impl SyncService`, add:

```rust
pub async fn sync_all_accounts_with_progress(
    &self,
    app_handle: tauri::AppHandle,
) -> Result<SyncAllAccountsResult, MailError> {
    let accounts = account_repo::list(&self.db).await?;
    let total = accounts.len();
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for account in accounts {
        match self
            .sync_account_with_progress(app_handle.clone(), account.id)
            .await
        {
            Ok(()) => succeeded.push(account.id),
            Err(err) => {
                failed.push(SyncAccountFailure {
                    account_id: account.id,
                    email: Some(account.email.clone()),
                    message: err.to_string(),
                });
            }
        }
    }

    if total > 0 && succeeded.is_empty() && !failed.is_empty() {
        return Err(MailError::SyncFailed(format!(
            "所有账号同步失败: {}",
            failed
                .iter()
                .map(|failure| failure.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    Ok(SyncAllAccountsResult {
        total,
        succeeded,
        failed,
    })
}
```

- [ ] **Step 6: Add commands and register them**

In `src-tauri/src/command/sync.rs`, import DTO:

```rust
use crate::service::sync_service::{HistorySyncState, OlderSyncResult, SyncAllAccountsResult};
```

Add:

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_folder_stats_for_all_accounts(
    service: tauri::State<'_, crate::service::SyncService>,
) -> Result<Vec<FolderStat>, MailError> {
    tracing::debug!("命令: 获取所有账号文件夹统计");
    service.get_folder_stats_for_all_accounts().await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_all_accounts(
    service: tauri::State<'_, crate::service::SyncService>,
    app_handle: tauri::AppHandle,
) -> Result<SyncAllAccountsResult, MailError> {
    tracing::info!("命令: 同步所有账号");
    service.sync_all_accounts_with_progress(app_handle).await
}
```

In `src-tauri/src/lib.rs`, add:

```rust
command::sync::get_folder_stats_for_all_accounts,
command::sync::sync_all_accounts,
```

- [ ] **Step 7: Update bindings**

In `src/lib/bindings.ts`, add command bindings:

```ts
getFolderStatsForAllAccounts: () =>
    typedError<FolderStat[], MailError>(
        __TAURI_INVOKE("get_folder_stats_for_all_accounts"),
    ),
syncAllAccounts: () =>
    typedError<SyncAllAccountsResult, MailError>(
        __TAURI_INVOKE("sync_all_accounts"),
    ),
```

Add generated types:

```ts
export type SyncAccountFailure = {
    account_id: number;
    email: string | null;
    message: string;
};

export type SyncAllAccountsResult = {
    total: number;
    succeeded: number[];
    failed: SyncAccountFailure[];
};
```

- [ ] **Step 8: Run tests**

Run:

```bash
rtk cargo test --test all_accounts_sync get_folder_stats_for_all_accounts_should_sum_sidebar_categories
rtk cargo test --test email_commands list_by_category_for_all_accounts_should_merge_inbox_across_accounts
```

Expected:

- PASS.

---

### Task 4: AccountState 账号范围与持久化

**Files:**
- Modify: `src/lib/stores/account.svelte.ts`
- Modify: `src/lib/__tests__/stores/account-state.test.ts`

**Interfaces:**
- Produces:
  - `export type AccountScope = { kind: "all" } | { kind: "account"; accountId: number }`
  - `accountScope`
  - `isAllAccounts`
  - `selectedAccountId`
  - `lastConcreteAccountId`
  - `setAllAccounts(): void`
  - `setActive(id: number): void` updated to set `{ kind: "account" }`
  - Compatibility getter/setter `activeAccountId`

- [ ] **Step 1: Write failing account state tests**

Modify `src/lib/__tests__/stores/account-state.test.ts`.

In `beforeEach`, add:

```ts
localStorage.clear();
```

Replace test `"loadAccounts 成功后写入账号并自动选中第一个账号"` with:

```ts
it("loadAccounts 首次加载后默认选择所有账号", async () => {
  mockInvoke.mockResolvedValue(accounts);
  const state = new AccountState();

  await state.loadAccounts();

  expect(state.loading).toBe(false);
  expect(state.error).toBeNull();
  expect(state.accounts).toHaveLength(2);
  expect(state.isAllAccounts).toBe(true);
  expect(state.activeAccountId).toBeNull();
  expect(state.activeAccount).toBeNull();
  expect(mockInvoke).toHaveBeenCalledWith("list_accounts");
});
```

Add tests:

```ts
it("setActive 选择具体账号并持久化", async () => {
  mockInvoke.mockResolvedValue(accounts);
  const state = new AccountState();
  await state.loadAccounts();

  state.setActive(2);

  expect(state.isAllAccounts).toBe(false);
  expect(state.activeAccountId).toBe(2);
  expect(state.activeAccount?.email).toBe("personal@example.com");
  expect(state.lastConcreteAccountId).toBe(2);
  expect(localStorage.getItem("postium-account-scope")).toBe(
    JSON.stringify({ kind: "account", accountId: 2 }),
  );
});

it("setAllAccounts 选择所有账号并持久化", async () => {
  mockInvoke.mockResolvedValue(accounts);
  const state = new AccountState();
  await state.loadAccounts();
  state.setActive(1);

  state.setAllAccounts();

  expect(state.isAllAccounts).toBe(true);
  expect(state.activeAccountId).toBeNull();
  expect(localStorage.getItem("postium-account-scope")).toBe(
    JSON.stringify({ kind: "all" }),
  );
});

it("loadAccounts 恢复已持久化的具体账号", async () => {
  localStorage.setItem(
    "postium-account-scope",
    JSON.stringify({ kind: "account", accountId: 2 }),
  );
  mockInvoke.mockResolvedValue(accounts);
  const state = new AccountState();

  await state.loadAccounts();

  expect(state.isAllAccounts).toBe(false);
  expect(state.activeAccountId).toBe(2);
  expect(state.activeAccount?.email).toBe("personal@example.com");
});

it("loadAccounts 遇到不存在的持久化账号时回退到所有账号", async () => {
  localStorage.setItem(
    "postium-account-scope",
    JSON.stringify({ kind: "account", accountId: 999 }),
  );
  mockInvoke.mockResolvedValue(accounts);
  const state = new AccountState();

  await state.loadAccounts();

  expect(state.isAllAccounts).toBe(true);
  expect(state.activeAccountId).toBeNull();
});
```

Update delete tests:

```ts
expect(state.isAllAccounts).toBe(true);
expect(state.activeAccountId).toBeNull();
```

for deleting current account.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/account-state.test.ts
```

Expected:

- FAIL because `isAllAccounts`, `setAllAccounts`, `lastConcreteAccountId` do not exist and default behavior still selects first account.

- [ ] **Step 3: Implement AccountScope**

In `src/lib/stores/account.svelte.ts`, add near imports:

```ts
export type AccountScope =
  | { kind: "all" }
  | { kind: "account"; accountId: number };

const ACCOUNT_SCOPE_STORAGE_KEY = "postium-account-scope";
```

Add helpers:

```ts
function readStoredAccountScope(): AccountScope {
  if (typeof localStorage === "undefined") return { kind: "all" };
  try {
    const raw = localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY);
    if (!raw) return { kind: "all" };
    const parsed = JSON.parse(raw) as Partial<AccountScope>;
    if (parsed.kind === "account" && typeof parsed.accountId === "number") {
      return { kind: "account", accountId: parsed.accountId };
    }
    return { kind: "all" };
  } catch {
    return { kind: "all" };
  }
}

function storeAccountScope(scope: AccountScope) {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(ACCOUNT_SCOPE_STORAGE_KEY, JSON.stringify(scope));
}
```

In class:

```ts
accountScope = $state<AccountScope>(readStoredAccountScope());
lastConcreteAccountId = $state<number | null>(
  this.accountScope.kind === "account" ? this.accountScope.accountId : null,
);

isAllAccounts = $derived(this.accountScope.kind === "all");
selectedAccountId = $derived(
  this.accountScope.kind === "account" ? this.accountScope.accountId : null,
);
```

Replace `activeAccountId = $state<number | null>(null);` with getter/setter:

```ts
get activeAccountId() {
  return this.selectedAccountId;
}

set activeAccountId(id: number | null) {
  if (id === null) {
    this.setAllAccounts();
  } else {
    this.setActive(id);
  }
}
```

Update `activeAccount` derived:

```ts
activeAccount = $derived(
  this.selectedAccountId === null
    ? null
    : this.accounts.find((a) => a.id === this.selectedAccountId) ?? null,
);
```

- [ ] **Step 4: Update load/delete/set behavior**

In `loadAccounts`, after setting `this.accounts = result.data`, replace first-account selection with:

```ts
this.reconcileAccountScope();
```

Add methods:

```ts
private reconcileAccountScope() {
  if (this.accountScope.kind === "account") {
    const exists = this.accounts.some(
      (account) => account.id === this.accountScope.accountId,
    );
    if (!exists) {
      this.accountScope = { kind: "all" };
      storeAccountScope(this.accountScope);
    }
  }

  if (
    this.lastConcreteAccountId !== null &&
    !this.accounts.some((account) => account.id === this.lastConcreteAccountId)
  ) {
    this.lastConcreteAccountId = this.accounts[0]?.id ?? null;
  }
}

setAllAccounts() {
  this.accountScope = { kind: "all" };
  storeAccountScope(this.accountScope);
}
```

Update `setActive`:

```ts
setActive(id: number) {
  this.accountScope = { kind: "account", accountId: id };
  this.lastConcreteAccountId = id;
  storeAccountScope(this.accountScope);
}
```

Update `deleteAccount` after removing account:

```ts
if (this.accountScope.kind === "account" && this.accountScope.accountId === id) {
  this.setAllAccounts();
}
if (this.lastConcreteAccountId === id) {
  this.lastConcreteAccountId = this.accounts[0]?.id ?? null;
}
```

- [ ] **Step 5: Run account tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/account-state.test.ts
```

Expected:

- PASS.

---

### Task 5: EmailState 与 SyncState 增加所有账号方法

**Files:**
- Modify: `src/lib/stores/email.svelte.ts`
- Modify: `src/lib/stores/sync.svelte.ts`
- Modify: `src/lib/__tests__/stores/email-state.test.ts`
- Create: `src/lib/__tests__/stores/sync-all-accounts.test.ts`

**Interfaces:**
- Produces:
  - `EmailState.loadEmailsByCategoryForAllAccounts(category, page?)`
  - `EmailState.loadNextPageForAllAccounts()`
  - `EmailState.refreshLoadedEmailsByCategoryForAllAccounts(minimumLimit?)`
  - `EmailState.setUnreadOnlyForAllAccounts(unreadOnly)`
  - `SyncState.loadFolderStatsForAllAccounts()`
  - `SyncState.syncAllAccounts()`

- [ ] **Step 1: Write failing EmailState tests**

Add to `src/lib/__tests__/stores/email-state.test.ts`:

```ts
it("loadEmailsByCategoryForAllAccounts 调用所有账号分类接口", async () => {
  mockInvoke.mockResolvedValue({ emails: [email], total: 1, page: 1, limit: 50 });
  const state = new EmailState();

  await state.loadEmailsByCategoryForAllAccounts("inbox", 1);

  expect(state.loading).toBe(false);
  expect(state.currentFolder).toBe("inbox");
  expect(state.emails).toEqual([email]);
  expect(mockInvoke).toHaveBeenCalledWith(
    "list_emails_by_category_for_all_accounts",
    {
      category: "inbox",
      page: 1,
      limit: 50,
      unreadOnly: false,
    },
  );
});

it("setUnreadOnlyForAllAccounts 切换未读筛选并重新加载", async () => {
  mockInvoke.mockResolvedValue({ emails: [], total: 0, page: 1, limit: 50 });
  const state = new EmailState();
  state.currentFolder = "starred";

  await state.setUnreadOnlyForAllAccounts(true);

  expect(state.unreadOnly).toBe(true);
  expect(mockInvoke).toHaveBeenCalledWith(
    "list_emails_by_category_for_all_accounts",
    {
      category: "starred",
      page: 1,
      limit: 50,
      unreadOnly: true,
    },
  );
});
```

- [ ] **Step 2: Write failing SyncState tests**

Create `src/lib/__tests__/stores/sync-all-accounts.test.ts`:

```ts
import { beforeEach, describe, expect, it } from "vitest";
import { SyncState } from "$lib/stores/sync.svelte";
import { mockInvoke } from "../mocks/tauri";

describe("SyncState 所有账号行为", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue({ status: "ok", data: null });
  });

  it("loadFolderStatsForAllAccounts 调用所有账号统计接口", async () => {
    mockInvoke.mockResolvedValueOnce({
      status: "ok",
      data: [{ folder: "inbox", total: 3, unread: 2 }],
    });
    const state = new SyncState();

    await state.loadFolderStatsForAllAccounts();

    expect(state.folderStats).toEqual([{ folder: "inbox", total: 3, unread: 2 }]);
    expect(mockInvoke).toHaveBeenCalledWith("get_folder_stats_for_all_accounts");
  });

  it("syncAllAccounts 调用所有账号同步接口", async () => {
    mockInvoke.mockResolvedValueOnce({
      status: "ok",
      data: { total: 2, succeeded: [1], failed: [] },
    });
    const state = new SyncState();

    const result = await state.syncAllAccounts();

    expect(result).toEqual({ total: 2, succeeded: [1], failed: [] });
    expect(mockInvoke).toHaveBeenCalledWith("sync_all_accounts");
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/stores/sync-all-accounts.test.ts
```

Expected:

- FAIL because new methods do not exist.

- [ ] **Step 4: Implement EmailState all-account methods**

In `src/lib/stores/email.svelte.ts`, add methods mirroring single-account methods:

```ts
async loadEmailsByCategoryForAllAccounts(category: EmailCategory, page = 1) {
  this.loading = true;
  this.currentFolder = category;

  try {
    const result = await commands.listEmailsByCategoryForAllAccounts(
      category,
      page,
      this.limit,
      this.unreadOnly,
    );

    if (result.status === "ok") {
      this.emails = result.data.emails;
      this.total = result.data.total;
      this.page = result.data.page;
    }
  } catch (e: unknown) {
    console.error("Failed to load all-account emails:", e);
  } finally {
    this.loading = false;
  }
}

async loadNextPageForAllAccounts() {
  if (this.loading || this.loadingNextPage || this.emails.length >= this.total)
    return;

  const nextPage = this.page + 1;
  this.loadingNextPage = true;

  try {
    const result = await commands.listEmailsByCategoryForAllAccounts(
      this.currentFolder,
      nextPage,
      this.limit,
      this.unreadOnly,
    );

    if (result.status === "ok") {
      this.emails = [...this.emails, ...result.data.emails];
      this.total = result.data.total;
      this.page = result.data.page;
    }
  } catch (e: unknown) {
    this.setError(e, "Failed to load next all-account email page");
  } finally {
    this.loadingNextPage = false;
  }
}

async refreshLoadedEmailsByCategoryForAllAccounts(minimumLimit = 0) {
  const loadedCount = Math.max(this.emails.length, minimumLimit, this.limit);

  try {
    const result = await commands.listEmailsByCategoryForAllAccounts(
      this.currentFolder,
      1,
      loadedCount,
      this.unreadOnly,
    );

    if (result.status === "ok") {
      this.emails = result.data.emails;
      this.total = result.data.total;
      this.page = Math.max(1, Math.ceil(result.data.emails.length / this.limit));
    }
  } catch (e: unknown) {
    this.setError(e, "Failed to refresh loaded all-account emails");
  }
}

async setUnreadOnlyForAllAccounts(unreadOnly: boolean) {
  if (this.unreadOnly === unreadOnly && this.page === 1) return;
  this.unreadOnly = unreadOnly;
  await this.loadEmailsByCategoryForAllAccounts(this.currentFolder, 1);
}
```

- [ ] **Step 5: Implement SyncState all-account methods**

In `src/lib/stores/sync.svelte.ts`, import type `SyncAllAccountsResult` from bindings.

Add:

```ts
async syncAllAccounts(): Promise<SyncAllAccountsResult | null> {
  this.syncing = true;
  this.error = null;

  try {
    const result = await commands.syncAllAccounts();
    if (result.status === "error") {
      this.error = String(result.error.message);
      return null;
    }
    return result.data;
  } catch (e: unknown) {
    this.error = formatError(e);
    return null;
  } finally {
    this.syncing = false;
  }
}

async loadFolderStatsForAllAccounts() {
  try {
    const result = await commands.getFolderStatsForAllAccounts();
    if (result.status === "ok") {
      this.folderStats = result.data;
    } else {
      this.error = String(result.error.message);
    }
  } catch (e: unknown) {
    this.error = formatError(e);
  }
}
```

- [ ] **Step 6: Run store tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/stores/sync-all-accounts.test.ts
```

Expected:

- PASS.

---

### Task 6: Sidebar 聚合账号选择、文件夹加载与同步

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte`
- Modify: `src/routes/(main)/+layout.svelte`
- Modify: `src/lib/__tests__/components/Sidebar.test.ts`

**Interfaces:**
- Consumes:
  - `accountStore.isAllAccounts`
  - `accountStore.setAllAccounts()`
  - `emailStore.loadEmailsByCategoryForAllAccounts`
  - `syncStore.loadFolderStatsForAllAccounts`
  - `syncStore.syncAllAccounts`
- Produces UI:
  - `data-testid="account-option-all"`

- [ ] **Step 1: Write failing Sidebar tests**

In `src/lib/__tests__/components/Sidebar.test.ts`, extend the mocked account store with:

```ts
let isAllAccounts = false;
const setAllAccounts = vi.fn(() => {
    activeAccountId = null;
    isAllAccounts = true;
});
```

Expose:

```ts
get isAllAccounts() {
    return isAllAccounts;
},
setAllAccounts,
```

Add mocked methods:

```ts
const loadEmailsByCategoryForAllAccounts = vi.fn();
const loadFolderStatsForAllAccounts = vi.fn();
const syncAllAccounts = vi.fn();
```

Add tests:

```ts
it("账号下拉显示所有账号选项并可切换", async () => {
    render(Sidebar);

    await fireEvent.click(screen.getByTestId("account-switcher"));
    await fireEvent.click(screen.getByTestId("account-option-all"));

    expect(setAllAccounts).toHaveBeenCalled();
    expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith("inbox");
    expect(loadFolderStatsForAllAccounts).toHaveBeenCalled();
});

it("所有账号视图下点击文件夹加载所有账号邮件", async () => {
    activeAccountId = null;
    isAllAccounts = true;
    render(Sidebar);

    await fireEvent.click(screen.getByTestId("folder-sent"));

    expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith("sent");
});

it("所有账号视图下同步按钮同步全部账号", async () => {
    activeAccountId = null;
    isAllAccounts = true;
    syncAllAccounts.mockResolvedValue({ total: 2, succeeded: [1, 2], failed: [] });
    render(Sidebar);

    await fireEvent.click(screen.getByTitle("同步"));

    expect(syncAllAccounts).toHaveBeenCalled();
    expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith("inbox");
    expect(loadFolderStatsForAllAccounts).toHaveBeenCalled();
});
```

Use the actual localized title string in the existing test mock if it is not `"同步"`.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/Sidebar.test.ts
```

Expected:

- FAIL because UI and branch methods are missing.

- [ ] **Step 3: Add all-account option to dropdown**

In `Sidebar.svelte`, add handler:

```ts
function handleAllAccountsSwitch() {
    accountStore.setAllAccounts();
    emailStore.deselectEmail();
    emailStore.loadEmailsByCategoryForAllAccounts(emailStore.currentFolder);
    syncStore.loadFolderStatsForAllAccounts();
    showAccountDropdown = false;
}
```

In dropdown before account loop:

```svelte
<button
    data-testid="account-option-all"
    class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-glass-hover {accountStore.isAllAccounts
        ? 'bg-primary/10 text-primary'
        : 'text-foreground'}"
    onclick={handleAllAccountsSwitch}
>
    <div
        class="flex h-5 w-5 items-center justify-center rounded-full bg-primary/15 text-[9px] font-bold text-primary"
    >
        全
    </div>
    <span class="truncate">{t.sidebar.allAccounts}</span>
</button>
```

Update trigger avatar/label:

```svelte
{accountStore.isAllAccounts
    ? "全"
    : accountStore.activeAccount?.email?.charAt(0)?.toUpperCase() || "?"}
```

```svelte
{accountStore.isAllAccounts
    ? t.sidebar.allAccounts
    : accountStore.activeAccount?.email || t.sidebar.allAccounts}
```

- [ ] **Step 4: Branch folder stats and folder selection**

Update `selectFolder`:

```ts
function selectFolder(folderId: EmailCategory) {
    activeFolder = folderId;
    emailStore.currentFolder = folderId;
    if (accountStore.isAllAccounts) {
        emailStore.loadEmailsByCategoryForAllAccounts(folderId);
    } else if (accountStore.activeAccountId) {
        emailStore.loadEmailsByCategory(accountStore.activeAccountId, folderId);
    }
    goto("/");
}
```

Update stats effect:

```ts
$effect(() => {
    const accountId = accountStore.activeAccountId;
    const isAll = accountStore.isAllAccounts;
    if (isAll) {
        syncStore.loadFolderStatsForAllAccounts();
    } else if (accountId) {
        loadFolderStatsForAccount(accountId);
    }
});
```

Update `handleAccountSwitch` to ensure single-account branch:

```ts
function handleAccountSwitch(accountId: number) {
    accountStore.setActive(accountId);
    emailStore.deselectEmail();
    emailStore.loadEmailsByCategory(accountId, emailStore.currentFolder);
    loadFolderStatsForAccount(accountId, true);
    showAccountDropdown = false;
}
```

- [ ] **Step 5: Branch sync**

Update `handleSync`:

```ts
async function handleSync() {
    if (accountStore.isAllAccounts) {
        await syncStore.syncAllAccounts();
        await emailStore.loadEmailsByCategoryForAllAccounts(emailStore.currentFolder);
        await syncStore.loadFolderStatsForAllAccounts();
    } else if (accountStore.activeAccountId) {
        await syncStore.syncAccount(accountStore.activeAccountId);
        await emailStore.loadEmailsByCategory(
            accountStore.activeAccountId,
            emailStore.currentFolder,
        );
        await syncStore.loadFolderStats(accountStore.activeAccountId);
    }
}
```

In `src/routes/(main)/+layout.svelte`, update tray sync branch:

```ts
} else if (event.payload === "sync") {
    if (account.isAllAccounts) {
        sync.syncAllAccounts();
    } else if (account.activeAccountId) {
        sync.syncAccount(account.activeAccountId);
    }
}
```

- [ ] **Step 6: Run Sidebar tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/Sidebar.test.ts
```

Expected:

- PASS.

---

### Task 7: EmailList 聚合视图渲染、搜索、刷新与分页

**Files:**
- Modify: `src/lib/components/email/EmailList.svelte`
- Modify: `src/lib/__tests__/components/EmailList.test.ts`

**Interfaces:**
- Consumes:
  - `accountStore.isAllAccounts`
  - `emailState.loadEmailsByCategoryForAllAccounts`
  - `emailState.loadNextPageForAllAccounts`
  - `emailState.refreshLoadedEmailsByCategoryForAllAccounts`
  - `emailState.setUnreadOnlyForAllAccounts`
  - `syncStore.syncAllAccounts`
  - `syncStore.loadFolderStatsForAllAccounts`

- [ ] **Step 1: Write failing EmailList tests**

In `src/lib/__tests__/components/EmailList.test.ts`, add mock state:

```ts
let isAllAccounts = false;
const loadEmailsByCategoryForAllAccounts = vi.fn();
const loadNextPageForAllAccounts = vi.fn();
const refreshLoadedEmailsByCategoryForAllAccounts = vi.fn();
const setUnreadOnlyForAllAccounts = vi.fn();
const syncAllAccounts = vi.fn();
const loadFolderStatsForAllAccounts = vi.fn();
```

Expose these in mocked stores.

Add test email with account display:

```ts
const allAccountEmail = {
    ...email,
    account_email: "work@example.com",
    account_display_name: "Work",
};
```

Add tests:

```ts
it("所有账号视图下显示邮件来源账号", () => {
    isAllAccounts = true;
    emailStateMock.emails = [allAccountEmail];

    render(EmailList);

    expect(screen.getByText("Work")).toBeInTheDocument();
});

it("所有账号视图下刷新会同步全部账号并重新加载当前分类", async () => {
    isAllAccounts = true;
    syncAllAccounts.mockResolvedValue({ total: 2, succeeded: [1, 2], failed: [] });

    render(EmailList);
    await fireEvent.click(screen.getByTestId("email-refresh-button"));

    expect(syncAllAccounts).toHaveBeenCalled();
    expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith("inbox", 1);
    expect(loadFolderStatsForAllAccounts).toHaveBeenCalled();
});

it("所有账号视图下搜索传空账号参数", async () => {
    isAllAccounts = true;
    vi.useFakeTimers();
    render(EmailList);

    await fireEvent.input(screen.getByTestId("email-search-input"), {
        target: { value: "invoice" },
    });
    await vi.runAllTimersAsync();

    expect(commands.searchEmails).toHaveBeenCalledWith("invoice", null, 50);
    vi.useRealTimers();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/EmailList.test.ts
```

Expected:

- FAIL because all-account branches and account source rendering are missing.

- [ ] **Step 3: Branch auto-load and history state**

Update first load `$effect`:

```ts
$effect(() => {
    const accountId = accountStore.activeAccountId;
    const isAll = accountStore.isAllAccounts;
    const folder = emailState.currentFolder;

    if (isAll) {
        emailState.loadEmailsByCategoryForAllAccounts(folder);
    } else if (accountId) {
        emailState.loadEmailsByCategory(accountId, folder);
    }
});
```

Update history-state derived values to disable all-account older sync:

```ts
let supportsOlderSync = $derived(
    searchResults === null &&
        emailState.currentFolder !== "starred" &&
        !accountStore.isAllAccounts,
);
```

Keep existing history state calls only when `accountId && !accountStore.isAllAccounts`.

- [ ] **Step 4: Branch refresh, unread filter, pagination**

Update `refreshFolderStats`:

```ts
function refreshFolderStats() {
    if (accountStore.isAllAccounts) {
        void syncStore.loadFolderStatsForAllAccounts();
        return;
    }
    const accountId = accountStore.activeAccountId;
    if (accountId) {
        void syncStore.loadFolderStats(accountId);
    }
}
```

Update `handleRefresh`:

```ts
async function handleRefresh() {
    if (accountStore.isAllAccounts) {
        await syncStore.syncAllAccounts();
        await emailState.loadEmailsByCategoryForAllAccounts(
            emailState.currentFolder,
            emailState.page,
        );
        await syncStore.loadFolderStatsForAllAccounts();
    } else if (accountStore.activeAccountId) {
        await syncStore.syncAccount(accountStore.activeAccountId);
        await emailState.loadEmailsByCategory(
            accountStore.activeAccountId,
            emailState.currentFolder,
            emailState.page,
        );
        await syncStore.loadFolderStats(accountStore.activeAccountId);
    }
}
```

Update `toggleUnreadOnly`:

```ts
async function toggleUnreadOnly() {
    if (accountStore.isAllAccounts) {
        await emailState.setUnreadOnlyForAllAccounts(!emailState.unreadOnly);
        return;
    }
    if (!accountStore.activeAccountId) return;
    await emailState.setUnreadOnly(
        accountStore.activeAccountId,
        !emailState.unreadOnly,
    );
}
```

Update `handleLoadMore`:

```ts
async function handleLoadMore() {
    if (accountStore.isAllAccounts) {
        await emailState.loadNextPageForAllAccounts();
    } else if (accountStore.activeAccountId) {
        await emailState.loadNextPage(accountStore.activeAccountId);
    }
}
```

- [ ] **Step 5: Branch search account parameter**

In search effect:

```ts
const accountId = accountStore.isAllAccounts
    ? null
    : accountStore.activeAccountId;
const result = await commands.searchEmails(q, accountId, 50);
```

- [ ] **Step 6: Render account source in all-account view**

Add helper:

```ts
function accountLabel(email: { account_display_name?: string | null; account_email?: string | null }) {
    return email.account_display_name || email.account_email || "";
}
```

In normal email list first row, between sender and time:

```svelte
{#if accountStore.isAllAccounts && accountLabel(email)}
    <span
        data-testid="email-account-source"
        class="max-w-24 shrink-0 truncate rounded bg-glass-active px-1.5 py-0.5 text-[11px] text-muted-foreground"
    >
        {accountLabel(email)}
    </span>
{/if}
```

Use same pattern in search result items if `SearchResult` contains or is extended with account fields. If the generated `SearchResult` does not contain account fields yet, keep search source rendering for Task 10 after backend search DTO is inspected.

- [ ] **Step 7: Disable buttons based on all-account availability**

Update refresh/unread disabled conditions:

```svelte
disabled={(!accountStore.isAllAccounts && !accountStore.activeAccountId) || emailState.loading}
```

Do not show “同步更早邮件” when `accountStore.isAllAccounts` because `supportsOlderSync` is false.

- [ ] **Step 8: Run EmailList tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/EmailList.test.ts
```

Expected:

- PASS.

---

### Task 8: ComposeModal 发件账号选择与回复/转发默认账号

**Files:**
- Modify: `src/lib/components/email/ComposeModal.svelte`
- Modify: `src/lib/components/email/EmailDetail.svelte`
- Create/Modify: `src/lib/__tests__/components/ComposeModal.test.ts`
- Modify: `src/lib/__tests__/components/EmailDetail.test.ts`

**Interfaces:**
- Produces:
  - `ComposeModal.show(options?: { accountId?: number }): void`
  - `ComposeModal.showReply(replyTo, replySubject, replyBody, accountId?: number): void`
  - `ComposeModal.showForward(fwdSubject, fwdBody, accountId?: number): void`
  - UI `data-testid="compose-account-select"`

- [ ] **Step 1: Write failing ComposeModal tests**

Create or extend `src/lib/__tests__/components/ComposeModal.test.ts`:

```ts
import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ComposeModal from "$lib/components/email/ComposeModal.svelte";
import { mockInvoke } from "../mocks/tauri";

const accounts = [
    { id: 1, email: "work@example.com", display_name: "Work" },
    { id: 2, email: "personal@example.com", display_name: "Personal" },
];

let isAllAccounts = true;
let activeAccountId: number | null = null;
let lastConcreteAccountId: number | null = 2;

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts,
        get isAllAccounts() {
            return isAllAccounts;
        },
        get activeAccountId() {
            return activeAccountId;
        },
        get lastConcreteAccountId() {
            return lastConcreteAccountId;
        },
    }),
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            sidebar: { compose: "写邮件" },
            email: {
                to: "收件人",
                cc: "抄送",
                subject: "主题",
                send: "发送",
                loading: "发送中",
            },
            common: { cancel: "取消" },
        },
    }),
}));

describe("ComposeModal 所有账号发件账号", () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue({ status: "ok", data: "message-id" });
        isAllAccounts = true;
        activeAccountId = null;
        lastConcreteAccountId = 2;
    });

    it("所有账号视图下显示发件账号选择器并默认上次具体账号", async () => {
        let modal: ComposeModal;
        render(ComposeModal, {
            bind: (component) => {
                modal = component;
            },
        });

        modal!.show();

        const select = await screen.findByTestId("compose-account-select");
        expect(select).toHaveValue("2");
    });

    it("所有账号视图下发送使用选择的发件账号", async () => {
        let modal: ComposeModal;
        render(ComposeModal, {
            bind: (component) => {
                modal = component;
            },
        });

        modal!.show();
        await fireEvent.change(await screen.findByTestId("compose-account-select"), {
            target: { value: "1" },
        });
        await fireEvent.input(screen.getByTestId("compose-to-input"), {
            target: { value: "to@example.com" },
        });
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ account_id: 1 }),
        });
    });
});
```

If the project’s component test binding syntax differs, adapt to the existing `bind:this` test pattern used in this codebase.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/ComposeModal.test.ts
```

Expected:

- FAIL because selector and account selection state do not exist.

- [ ] **Step 3: Implement selected send account state**

In `ComposeModal.svelte`, add:

```ts
let selectedAccountId = $state<number | null>(null);

let shouldShowAccountSelect = $derived(accountStore.isAllAccounts);

function defaultSendAccountId(preferredAccountId?: number) {
    if (
        preferredAccountId &&
        accountStore.accounts.some((account) => account.id === preferredAccountId)
    ) {
        return preferredAccountId;
    }
    if (
        accountStore.lastConcreteAccountId &&
        accountStore.accounts.some(
            (account) => account.id === accountStore.lastConcreteAccountId,
        )
    ) {
        return accountStore.lastConcreteAccountId;
    }
    return accountStore.activeAccountId ?? accountStore.accounts[0]?.id ?? null;
}

function openWithAccount(preferredAccountId?: number) {
    selectedAccountId = defaultSendAccountId(preferredAccountId);
    open = true;
}
```

Update `handleSend`:

```ts
const sendAccountId = accountStore.isAllAccounts
    ? selectedAccountId
    : accountStore.activeAccountId;
if (!sendAccountId) return;
```

Use:

```ts
account_id: sendAccountId,
```

Update `close`:

```ts
selectedAccountId = null;
```

- [ ] **Step 4: Update public methods**

Replace:

```ts
export function show() {
    open = true;
}
```

with:

```ts
export function show(options: { accountId?: number } = {}) {
    openWithAccount(options.accountId);
}
```

Update:

```ts
export function showReply(
    replyTo: string,
    replySubject: string,
    replyBody: string,
    accountId?: number,
) {
    openWithAccount(accountId);
    to = replyTo;
    subject = `Re: ${replySubject.replace(/^(Re|Fwd):\s*/i, "")}`;
    richEditor?.setContent(`<p>${replyBody}</p>`);
}

export function showForward(fwdSubject: string, fwdBody: string, accountId?: number) {
    openWithAccount(accountId);
    subject = `Fwd: ${fwdSubject.replace(/^(Re|Fwd):\s*/i, "")}`;
    richEditor?.setContent(`<p>${fwdBody}</p>`);
}
```

- [ ] **Step 5: Render account select**

In fields area before To row:

```svelte
{#if shouldShowAccountSelect}
    <div class="flex items-center border-b border-border px-4">
        <span class="w-14 shrink-0 text-sm text-muted-foreground">发件</span>
        <select
            data-testid="compose-account-select"
            bind:value={selectedAccountId}
            class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
        >
            {#each accountStore.accounts as account}
                <option value={account.id}>
                    {account.display_name || account.email}
                </option>
            {/each}
        </select>
    </div>
{/if}
```

Update send button disabled:

```svelte
disabled={sending || !to || !subject || (accountStore.isAllAccounts && !selectedAccountId)}
```

- [ ] **Step 6: Pass original email account from detail**

In `EmailDetail.svelte`, find reply/forward calls. Update:

```ts
modal.showReply(
    email.sender_email,
    email.subject || "",
    email.body_text || email.body_html || "",
    email.account_id,
);
```

and:

```ts
modal.showForward(
    email.subject || "",
    email.body_text || email.body_html || "",
    email.account_id,
);
```

Update `EmailDetail.test.ts` mock expectations accordingly.

- [ ] **Step 7: Run component tests**

Run:

```bash
rtk bun run test:frontend -- src/lib/__tests__/components/ComposeModal.test.ts src/lib/__tests__/components/EmailDetail.test.ts
```

Expected:

- PASS.

---

### Task 9: Binding, TypeScript, Rust 全量回归修正

**Files:**
- Modify as needed:
  - `src/lib/bindings.ts`
  - Frontend mocks under `src/lib/__tests__`
  - Rust tests under `src-tauri/tests`

**Interfaces:**
- Consumes all prior tasks.
- Produces passing typecheck and core test suite.

- [ ] **Step 1: Run Rust formatting check**

Run:

```bash
rtk cargo fmt --check
```

Expected:

- PASS. If it fails, run `rtk cargo fmt`, then rerun `rtk cargo fmt --check`.

- [ ] **Step 2: Run Rust tests**

Run:

```bash
rtk cargo test
```

Expected:

- PASS.

- [ ] **Step 3: Run Svelte/TypeScript check**

Run:

```bash
rtk bun run check
```

Expected:

- PASS.

Common fixes:

- Add `account_email` and `account_display_name` to test mock email objects.
- Update mocked account stores to include `isAllAccounts`, `setAllAccounts`, and `lastConcreteAccountId`.
- Update command mock expectations for generated camelCase binding names.

- [ ] **Step 4: Run frontend tests**

Run:

```bash
rtk bun run test:frontend
```

Expected:

- PASS.

---

### Task 10: E2E 覆盖所有账号核心流程

**Files:**
- Create: `e2e/test/specs/all-accounts.e2e.js`
- Modify: `e2e/pageobjects/sidebar.page.js`
- Modify: `e2e/pageobjects/email.page.js`
- Modify: `e2e/helpers/selectors.js`

**Interfaces:**
- Consumes UI test IDs:
  - `account-switcher`
  - `account-option-all`
  - `folder-inbox`
  - `email-item`
  - `email-account-source`
  - `compose-account-select`

- [ ] **Step 1: Add pageobject selectors**

In `e2e/helpers/selectors.js`, add:

```js
accountOptionAll: '[data-testid="account-option-all"]',
emailAccountSource: '[data-testid="email-account-source"]',
composeAccountSelect: '[data-testid="compose-account-select"]',
```

In `e2e/pageobjects/sidebar.page.js`, add:

```js
async selectAllAccounts() {
  await this.accountSwitcher.click();
  await $(selectors.accountOptionAll).click();
}
```

In `e2e/pageobjects/email.page.js`, add:

```js
get accountSources() {
  return $$(selectors.emailAccountSource);
}
```

- [ ] **Step 2: Write E2E spec**

Create `e2e/test/specs/all-accounts.e2e.js`:

```js
const SidebarPage = require("../pageobjects/sidebar.page");
const EmailPage = require("../pageobjects/email.page");

describe("All accounts mail view", () => {
  it("shows inbox mail from multiple accounts and account source labels", async () => {
    await SidebarPage.selectAllAccounts();
    await SidebarPage.openFolder("inbox");

    const items = await EmailPage.emailItems;
    await expect(items.length).toBeGreaterThanOrEqual(2);

    const sources = await EmailPage.accountSources;
    await expect(sources.length).toBeGreaterThanOrEqual(1);
  });

  it("returns to single-account filtering after selecting a concrete account", async () => {
    await SidebarPage.selectAllAccounts();
    await SidebarPage.selectPrimaryAccount();
    await SidebarPage.openFolder("inbox");

    const sources = await EmailPage.accountSources;
    await expect(sources.length).toBe(0);
  });

  it("shows sender account selector when composing in all-account view", async () => {
    await SidebarPage.selectAllAccounts();
    await SidebarPage.composeButton.click();

    await expect($(selectors.composeAccountSelect)).toBeDisplayed();
  });
});
```

Use existing helper names in `sidebar.page.js`; if `openFolder`, `selectPrimaryAccount`, or `composeButton` are named differently, adapt to the actual pageobject.

- [ ] **Step 3: Run targeted e2e**

Run:

```bash
rtk bun run test:e2e -- --spec e2e/test/specs/all-accounts.e2e.js
```

Expected:

- PASS in the configured e2e environment.

If local e2e requires real account fixtures not available in the current environment, document the failure output and run the full frontend/Rust test suite from Task 9.

---

## Final Verification

- [ ] Run all required verification commands:

```bash
rtk cargo fmt --check
rtk cargo test
rtk bun run check
rtk bun run test:frontend
```

- [ ] Run e2e when environment is available:

```bash
rtk bun run test:e2e
```

- [ ] Confirm `rtk git status --short` only shows intended files.

- [ ] Do not commit unless the user explicitly asks.

## Self-Review Notes

- Spec coverage:
  - Account dropdown and persisted default all-account state: Task 4 and Task 6.
  - Cross-account folder query, pagination and unread filtering: Task 1 and Task 2.
  - Folder stats aggregation: Task 3.
  - Sync all accounts: Task 3, Task 6, Task 7.
  - Account source label in list: Task 2 and Task 7.
  - Compose sender account selection and reply/forward default: Task 8.
  - Search account scope: Task 7.
  - Sync older unsupported in all-account view: Task 7.
  - Verification and e2e: Task 9 and Task 10.
- Placeholder scan:
  - No unfinished placeholder markers remain.
- Type consistency:
  - Frontend method names use generated camelCase command bindings.
  - Rust command names use snake_case Tauri command names.
  - `activeAccountId` remains as compatibility getter returning `number | null`.
