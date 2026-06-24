# 账号添加校验与首次同步 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 密码账号添加前完成 IMAP 登录校验，手动配置账号同步时使用保存的 IMAP 配置，并在账号添加成功后自动触发首次同步。

**Architecture:** 新增一个窄职责账号连接模块，统一把创建请求和账号模型解析为 IMAP 配置，并通过可注入 verifier 执行登录校验。`AccountService::create` 在写库和保存 Keyring 前调用 verifier；`SyncOrchestrator` 复用同一个配置解析函数。前端 `AddAccountModal` 在创建或 OAuth2 完成后设置新账号为活跃账号并调用现有 `SyncState.syncAccount`。

**Tech Stack:** Rust 2024、Tauri 2、tokio-rusqlite、async-imap、Svelte 5 runes、Vitest、Tauri Specta bindings。

## Global Constraints

- 文档和计划使用中文书写。
- 没有用户明确指令，不提交代码。
- shell 命令使用 `rtk` 前缀。
- 后端测试不能依赖真实邮箱服务器。
- SMTP 不作为账号添加阻断校验。
- 首次同步失败不回滚账号创建。
- 修改 `.svelte` 文件后运行 Svelte autofixer 或说明失败原因。

---

## File Structure

- Create: `src-tauri/src/service/account_connection.rs`
  - 负责 IMAP 配置解析、SSL 模式转换、真实 IMAP verifier 和测试 fake verifier。
- Modify: `src-tauri/src/service/mod.rs`
  - 导出新模块。
- Modify: `src-tauri/src/service/account_service.rs`
  - `AccountService` 增加 verifier 依赖，创建账号前校验 IMAP 登录。
- Modify: `src-tauri/src/domain/sync/folder_sync_dispatcher.rs`
  - 同步时复用账号模型 IMAP 配置解析。
- Modify: `src-tauri/tests/common/mod.rs`
  - 测试服务使用 fake IMAP verifier，避免真实网络。
- Modify: `src-tauri/tests/account_commands.rs`
  - 添加 IMAP 校验失败不持久化、成功才保存密码、手动配置解析优先等测试。
- Modify: `src/lib/components/settings/AddAccountModal.svelte`
  - 添加同步 store、流程状态和同步错误显示；创建成功和 OAuth2 完成后触发首次同步。
- Optional Modify: `src/lib/i18n/zh-CN.ts`, `src/lib/i18n/en-US.ts`
  - 如需要新文案，添加短文案；优先复用当前已有文案和少量组件内文案，避免扩大范围。

---

### Task 1: 后端账号连接配置与创建前 IMAP 校验

**Files:**
- Create: `src-tauri/src/service/account_connection.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/service/account_service.rs`
- Modify: `src-tauri/tests/common/mod.rs`
- Modify: `src-tauri/tests/account_commands.rs`

**Interfaces:**
- Consumes:
  - `CreateAccountRequest`
  - `accounts::Model`
  - `ProviderPool`
  - `ImapClient::connect(&ImapServerConfig, &str, &str) -> Result<ImapClient, MailError>`
- Produces:
  - `pub trait ImapConnectionVerifier: Send + Sync`
  - `pub struct RealImapConnectionVerifier`
  - `pub struct NoopImapConnectionVerifier`
  - `pub fn imap_config_from_create_request(req: &CreateAccountRequest) -> Result<ImapServerConfig, MailError>`
  - `pub fn imap_config_from_account(account: &accounts::Model) -> Result<ImapServerConfig, MailError>`
  - `impl AccountService { pub fn new_with_imap_verifier(db: DbConn, auth: Arc<AuthManager>, imap_verifier: Arc<dyn ImapConnectionVerifier>) -> Self }`

- [ ] **Step 1: Add module skeleton and exports**

Create `src-tauri/src/service/account_connection.rs`:

```rust
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::providers::{ImapServerConfig, SslMode};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use crate::service::account_service::CreateAccountRequest;
use async_trait::async_trait;

#[async_trait]
pub trait ImapConnectionVerifier: Send + Sync {
    async fn verify(&self, config: &ImapServerConfig, email: &str, password: &str)
    -> Result<(), MailError>;
}

pub struct RealImapConnectionVerifier;

#[async_trait]
impl ImapConnectionVerifier for RealImapConnectionVerifier {
    async fn verify(
        &self,
        config: &ImapServerConfig,
        email: &str,
        password: &str,
    ) -> Result<(), MailError> {
        let client = ImapClient::connect(config, email, password).await?;
        if let Err(err) = client.logout().await {
            tracing::debug!(error = %err, "IMAP 校验登录成功但登出失败");
        }
        Ok(())
    }
}

pub struct NoopImapConnectionVerifier;

#[async_trait]
impl ImapConnectionVerifier for NoopImapConnectionVerifier {
    async fn verify(
        &self,
        _config: &ImapServerConfig,
        _email: &str,
        _password: &str,
    ) -> Result<(), MailError> {
        Ok(())
    }
}

pub fn imap_config_from_create_request(
    req: &CreateAccountRequest,
) -> Result<ImapServerConfig, MailError> {
    if let Some(config) = manual_imap_config(
        req.imap_host.as_deref(),
        req.imap_port,
        req.imap_ssl_mode.as_deref(),
    )? {
        return Ok(config);
    }

    provider_imap_config(&req.provider, &req.email)
}

pub fn imap_config_from_account(account: &accounts::Model) -> Result<ImapServerConfig, MailError> {
    if let Some(config) = manual_imap_config(
        account.imap_host.as_deref(),
        account.imap_port,
        account.imap_ssl_mode.as_deref(),
    )? {
        return Ok(config);
    }

    provider_imap_config(&account.provider, &account.email)
}

fn manual_imap_config(
    host: Option<&str>,
    port: Option<i32>,
    ssl_mode: Option<&str>,
) -> Result<Option<ImapServerConfig>, MailError> {
    let Some(host) = host.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let Some(port) = port else {
        return Ok(None);
    };
    let port = u16::try_from(port)
        .map_err(|_| MailError::InvalidParam(format!("IMAP 端口无效: {port}")))?;
    let ssl = parse_ssl_mode(ssl_mode.unwrap_or("Tls"))?;

    Ok(Some(ImapServerConfig {
        host: host.trim().to_string(),
        port,
        ssl,
    }))
}

fn provider_imap_config(provider_id: &str, email: &str) -> Result<ImapServerConfig, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
    let provider = provider_pool
        .get(provider_id)
        .ok_or_else(|| MailError::ProviderNotSupported(provider_id.to_string()))?;

    Ok(provider.imap_config(email))
}

fn parse_ssl_mode(value: &str) -> Result<SslMode, MailError> {
    match value {
        "Tls" | "TLS" | "Implicit" => Ok(SslMode::Implicit),
        "StartTls" | "STARTTLS" => Ok(SslMode::StartTls),
        "None" => Ok(SslMode::None),
        other => Err(MailError::InvalidParam(format!("IMAP 加密模式无效: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::providers::pool::init_provider_pool;

    #[test]
    fn manual_imap_config_should_override_provider_config() {
        let req = CreateAccountRequest {
            name: "Manual".to_string(),
            email: "manual@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password".to_string(),
            imap_host: Some("imap.example.com".to_string()),
            imap_port: Some(143),
            imap_ssl_mode: Some("StartTls".to_string()),
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        };

        let config = imap_config_from_create_request(&req).unwrap();

        assert_eq!(config.host, "imap.example.com");
        assert_eq!(config.port, 143);
        assert!(matches!(config.ssl, SslMode::StartTls));
    }

    #[test]
    fn provider_imap_config_should_be_used_when_manual_config_missing() {
        init_provider_pool();
        let req = CreateAccountRequest {
            name: "Gmail".to_string(),
            email: "user@gmail.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        };

        let config = imap_config_from_create_request(&req).unwrap();

        assert_eq!(config.host, "imap.gmail.com");
        assert_eq!(config.port, 993);
        assert!(matches!(config.ssl, SslMode::Implicit));
    }
}
```

Modify `src-tauri/src/service/mod.rs` to include:

```rust
pub mod account_connection;
```

- [ ] **Step 2: Add async-trait dependency if missing**

Check `src-tauri/Cargo.toml`. If `async-trait` is absent, add under `[dependencies]`:

```toml
async-trait = "0.1"
```

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml account_connection
```

Expected: module tests compile and pass.

- [ ] **Step 3: Inject IMAP verifier into AccountService**

Modify `src-tauri/src/service/account_service.rs` imports:

```rust
use crate::service::account_connection::{
    ImapConnectionVerifier, RealImapConnectionVerifier, imap_config_from_create_request,
};
```

Modify `AccountService`:

```rust
pub struct AccountService {
    db: DbConn,
    auth: Arc<AuthManager>,
    imap_verifier: Arc<dyn ImapConnectionVerifier>,
}
```

Modify constructors:

```rust
pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
    Self::new_with_imap_verifier(db, auth, Arc::new(RealImapConnectionVerifier))
}

pub fn new_with_imap_verifier(
    db: DbConn,
    auth: Arc<AuthManager>,
    imap_verifier: Arc<dyn ImapConnectionVerifier>,
) -> Self {
    Self {
        db,
        auth,
        imap_verifier,
    }
}
```

- [ ] **Step 4: Write failing persistence test for failed IMAP verification**

In `src-tauri/tests/common/mod.rs`, change test service construction to use `NoopImapConnectionVerifier` by default:

```rust
use postium_mail_lib::service::account_connection::{
    ImapConnectionVerifier, NoopImapConnectionVerifier,
};
```

Add constructor:

```rust
pub async fn new_with_imap_verifier(imap_verifier: Arc<dyn ImapConnectionVerifier>) -> Self {
    let db = create_test_db().await;
    let auth = Arc::new(AuthManager::in_memory());

    Self {
        account_service: AccountService::new_with_imap_verifier(
            db.clone(),
            auth.clone(),
            imap_verifier,
        ),
        email_service: EmailService::new(auth.clone(), db.clone()),
        label_service: LabelService::new(db.clone()),
        db,
        auth,
    }
}
```

Update `new()` to call it:

```rust
pub async fn new() -> Self {
    Self::new_with_imap_verifier(Arc::new(NoopImapConnectionVerifier)).await
}
```

In `src-tauri/tests/account_commands.rs`, add imports:

```rust
use async_trait::async_trait;
use postium_mail_lib::domain::providers::ImapServerConfig;
use postium_mail_lib::error::MailError;
use postium_mail_lib::service::account_connection::ImapConnectionVerifier;
use std::sync::Arc;
```

Add fake verifier:

```rust
struct FailingImapVerifier;

#[async_trait]
impl ImapConnectionVerifier for FailingImapVerifier {
    async fn verify(
        &self,
        _config: &ImapServerConfig,
        _email: &str,
        _password: &str,
    ) -> Result<(), MailError> {
        Err(MailError::AuthFailed("forced imap failure".to_string()))
    }
}
```

Add test:

```rust
#[tokio::test]
async fn test_create_account_does_not_persist_when_imap_verification_fails() {
    let svc = TestServices::new_with_imap_verifier(Arc::new(FailingImapVerifier)).await;
    let req = CreateAccountRequest {
        name: "Invalid Account".to_string(),
        email: "invalid@gmail.com".to_string(),
        display_name: None,
        provider: "gmail".to_string(),
        auth_type: "Password".to_string(),
        password: "bad_password".to_string(),
        imap_host: None,
        imap_port: None,
        imap_ssl_mode: None,
        smtp_host: None,
        smtp_port: None,
        smtp_ssl_mode: None,
        color: None,
        account_type: None,
    };

    let result = svc.account_service.create(req).await;

    assert!(matches!(result, Err(MailError::AuthFailed(_))));
    let count = svc
        .db
        .call(|conn| conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get::<_, i64>(0)))
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(svc.auth.get_password("invalid@gmail.com").is_err());
}
```

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands test_create_account_does_not_persist_when_imap_verification_fails
```

Expected: FAIL until `AccountService::create` performs verifier call before persistence.

- [ ] **Step 5: Implement verifier call in AccountService::create**

In `AccountService::create`, before constructing `AccountWrite` or before `account_repo::create`, add:

```rust
let imap_config = imap_config_from_create_request(&req)?;
self.imap_verifier
    .verify(&imap_config, &req.email, &req.password)
    .await?;
```

Keep repository create and `self.auth.save_password` after this block.

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands test_create_account_does_not_persist_when_imap_verification_fails
```

Expected: PASS.

- [ ] **Step 6: Run account tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
```

Expected: PASS. Existing create tests should pass because `TestServices::new()` uses `NoopImapConnectionVerifier`.

- [ ] **Step 7: Format touched Rust files**

Run:

```bash
rtk rustfmt --edition 2024 --check src-tauri/src/service/account_connection.rs src-tauri/src/service/account_service.rs src-tauri/src/service/mod.rs src-tauri/tests/common/mod.rs src-tauri/tests/account_commands.rs
```

Expected: PASS.

---

### Task 2: 同步编排器复用账号 IMAP 配置

**Files:**
- Modify: `src-tauri/src/domain/sync/folder_sync_dispatcher.rs`
- Modify: `src-tauri/src/service/account_connection.rs`

**Interfaces:**
- Consumes:
  - `imap_config_from_account(account: &accounts::Model) -> Result<ImapServerConfig, MailError>`
- Produces:
  - `SyncOrchestrator::sync_account` uses account model manual IMAP config when present.

- [ ] **Step 1: Write unit test for account model config parsing**

In `src-tauri/src/service/account_connection.rs` test module, add:

```rust
#[test]
fn account_model_manual_imap_config_should_override_provider_config() {
    let account = accounts::Model {
        id: 1,
        name: "Manual".to_string(),
        email: "manual@example.com".to_string(),
        display_name: None,
        provider: "custom".to_string(),
        imap_host: Some("imap.manual.example.com".to_string()),
        imap_port: Some(143),
        imap_ssl: Some(true),
        imap_ssl_mode: Some("StartTls".to_string()),
        smtp_host: None,
        smtp_port: None,
        smtp_ssl: Some(true),
        smtp_ssl_mode: None,
        color: None,
        sync_enabled: Some(true),
        last_sync_at: None,
        auth_type: Some("Password".to_string()),
        account_type: "personal".to_string(),
        created_at: 1,
        updated_at: 1,
    };

    let config = imap_config_from_account(&account).unwrap();

    assert_eq!(config.host, "imap.manual.example.com");
    assert_eq!(config.port, 143);
    assert!(matches!(config.ssl, SslMode::StartTls));
}
```

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml account_model_manual_imap_config_should_override_provider_config
```

Expected: PASS if Task 1 helper is complete; FAIL if `accounts::Model` fields differ and need adjusting to actual model.

- [ ] **Step 2: Replace provider config lookup in SyncOrchestrator**

Modify imports in `src-tauri/src/domain/sync/folder_sync_dispatcher.rs`:

```rust
use crate::service::account_connection::imap_config_from_account;
```

Replace:

```rust
let imap_config = provider.imap_config(&account.email);
```

with:

```rust
let imap_config = imap_config_from_account(&account)?;
```

Keep provider lookup for auth type and folder mapping.

- [ ] **Step 3: Run sync and account tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
rtk cargo test --manifest-path src-tauri/Cargo.toml account_connection
```

Expected: PASS.

- [ ] **Step 4: Format touched Rust files**

Run:

```bash
rtk rustfmt --edition 2024 --check src-tauri/src/service/account_connection.rs src-tauri/src/domain/sync/folder_sync_dispatcher.rs
```

Expected: PASS.

---

### Task 3: 前端添加成功后触发首次同步

**Files:**
- Modify: `src/lib/components/settings/AddAccountModal.svelte`

**Interfaces:**
- Consumes:
  - `commands.createAccount(request) -> { status: "ok", data: AccountDto } | { status: "error", error: MailError }`
  - `accountStore.loadAccounts() -> Promise<void>`
  - `accountStore.setActive(id: number) -> void`
  - `syncStore.syncAccount(accountId: number) -> Promise<void>`
  - `syncStore.error: string | null`
- Produces:
  - Password add flow triggers sync for created account id.
  - OAuth2 completed flow triggers sync for account matching `email`.
  - Completion state can show sync error without rolling back account.

- [ ] **Step 1: Import sync store and add state**

In `src/lib/components/settings/AddAccountModal.svelte`, add import:

```ts
import { getSyncState } from "$lib/stores/sync.svelte";
```

After account store initialization, add:

```ts
const syncStore = getSyncState();
```

Add state near submit state:

```ts
let phase = $state<"idle" | "validating" | "syncing" | "done">("idle");
let syncError = $state("");
```

- [ ] **Step 2: Add helper for syncing new account**

Inside `<script>`, add:

```ts
async function syncCreatedAccount(accountId: number) {
    phase = "syncing";
    syncError = "";
    accountStore.setActive(accountId);

    await syncStore.syncAccount(accountId);

    if (syncStore.error) {
        syncError = syncStore.error;
    }
}
```

Add helper for OAuth2 account lookup:

```ts
function findAccountIdByEmail(targetEmail: string) {
    return accountStore.accounts.find((account) => account.email === targetEmail)?.id ?? null;
}
```

- [ ] **Step 3: Update password submit flow**

In `handleSubmit`, replace start:

```ts
error = "";
submitting = true;
```

with:

```ts
error = "";
syncError = "";
submitting = true;
phase = "validating";
```

In success branch, replace:

```ts
step = "done";
await accountStore.loadAccounts();
```

with:

```ts
await accountStore.loadAccounts();
await syncCreatedAccount(result.data.id);
phase = "done";
step = "done";
```

In error branch, add:

```ts
phase = "idle";
```

In catch branch, add:

```ts
phase = "idle";
```

At the end, before function exits, keep:

```ts
submitting = false;
```

- [ ] **Step 4: Update OAuth2 completed flow**

In `pollOAuth2Status`, inside Completed branch after `await accountStore.loadAccounts();`, add:

```ts
const accountId = findAccountIdByEmail(email);
if (accountId !== null) {
    await syncCreatedAccount(accountId);
}
phase = "done";
```

Then keep:

```ts
step = "done";
```

If no account id is found, do not block completion; set:

```ts
syncError = "账号已添加，但未能定位新账号执行首次同步";
phase = "done";
```

- [ ] **Step 5: Update reset and close state**

In `close()`, reset:

```ts
phase = "idle";
syncError = "";
```

In “继续添加另一个账号” onclick reset:

```ts
phase = "idle";
syncError = "";
```

- [ ] **Step 6: Update button disabled/text**

For password submit buttons, change disabled condition from:

```svelte
disabled={submitting}
```

to:

```svelte
disabled={submitting || phase === "syncing"}
```

Inside button text, replace static confirm text with:

```svelte
{#if phase === "validating"}
    正在验证...
{:else if phase === "syncing"}
    正在同步...
{:else}
    {t.common.confirm}
{/if}
```

Keep `Loader2` visible when:

```svelte
{#if submitting || phase === "syncing"}
```

- [ ] **Step 7: Update done page copy**

In done page, replace the success hint:

```svelte
<p class="mt-1 text-xs text-muted-foreground">
    {t.account.testSuccess}
</p>
```

with:

```svelte
{#if syncError}
    <p class="mt-1 text-xs text-destructive">
        账号已添加，但首次同步失败：{syncError}
    </p>
{:else}
    <p class="mt-1 text-xs text-muted-foreground">
        账号已添加并完成首次同步
    </p>
{/if}
```

- [ ] **Step 8: Run Svelte autofixer**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/settings/AddAccountModal.svelte
```

Expected: no blocking Svelte syntax errors. If network/dependency access fails, record the failure and continue with project tests.

- [ ] **Step 9: Run frontend checks**

Run:

```bash
rtk bun run test:frontend
rtk bun run check
```

Expected: PASS. If tests expose missing mocks for `syncAccount`, update the relevant test mocks to include `sync_account` results.

---

### Task 4: Final integration verification

**Files:**
- Verify all files touched by Tasks 1-3.

**Interfaces:**
- Consumes:
  - All produced backend helpers and frontend flow changes.
- Produces:
  - Verified implementation ready for user review.

- [ ] **Step 1: Run backend targeted tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
```

Expected: PASS.

- [ ] **Step 2: Run backend full tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run frontend tests and checks**

Run:

```bash
rtk bun run test:frontend
rtk bun run check
```

Expected: PASS.

- [ ] **Step 4: Run targeted rustfmt**

Run:

```bash
rtk rustfmt --edition 2024 --check src-tauri/src/service/account_connection.rs src-tauri/src/service/account_service.rs src-tauri/src/service/mod.rs src-tauri/src/domain/sync/folder_sync_dispatcher.rs src-tauri/tests/common/mod.rs src-tauri/tests/account_commands.rs
```

Expected: PASS.

- [ ] **Step 5: Run Svelte autofixer**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/settings/AddAccountModal.svelte
```

Expected: no blocking Svelte syntax errors.

- [ ] **Step 6: Inspect diff**

Run:

```bash
rtk git diff --stat
rtk git diff -- src-tauri/src/service/account_connection.rs src-tauri/src/service/account_service.rs src-tauri/src/service/mod.rs src-tauri/src/domain/sync/folder_sync_dispatcher.rs src-tauri/tests/common/mod.rs src-tauri/tests/account_commands.rs src/lib/components/settings/AddAccountModal.svelte
```

Expected: diff is limited to IMAP verification, IMAP config resolution, first sync flow, and tests.

---

## Self-Review

Spec coverage:

- 创建前 IMAP 校验：Task 1.
- 校验失败不入库、不保存密码：Task 1 tests and implementation.
- 手动配置优先：Task 1 helper tests and Task 2 sync usage.
- 添加成功后首次同步：Task 3.
- OAuth2 完成后首次同步：Task 3.
- 首次同步失败不回滚账号：Task 3 done page and flow.
- 不做 SMTP 阻断校验：Global Constraints and no task adds SMTP verification.
- 不依赖真实网络测试：Task 1 fake verifier.

Placeholder scan:

- No `TBD`, `TODO`, or “稍后实现” placeholders are used.

Type consistency:

- `ImapConnectionVerifier`, `RealImapConnectionVerifier`, `NoopImapConnectionVerifier`, `imap_config_from_create_request`, and `imap_config_from_account` are defined before later tasks consume them.
- `AccountService::new` remains compatible with existing production call sites.
- `AccountService::new_with_imap_verifier` is available for tests.
