# 账号登录与添加流程文档

本文档详细记录了用户添加邮件账号、账号登录的完整流程，包括前后端函数调用栈。

---

## 目录

1. [概述](#概述)
2. [密码认证流程](#密码认证流程)
3. [OAuth 认证流程](#oauth-认证流程)
4. [后端组件详解](#后端组件详解)
5. [前端组件详解](#前端组件详解)
6. [同步流程](#同步流程)
7. [调试指南](#调试指南)

---

## 概述

### 系统架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         前端 (Vue 3)                            │
│  ┌──────────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ AddAccountModal  │  │ AccountStore │  │  EmailStore      │  │
│  └────────┬─────────┘  └──────┬───────┘  └──────────────────┘  │
│           │                    │                                  │
└───────────┼────────────────────┼──────────────────────────────────┘
            │ Tauri IPC          │
            ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                      后端 (Rust/Tauri)                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    Command Layer                          │  │
│  │  account.rs  │  oauth.rs  │  connection.rs  │  sync.rs    │  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                  │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │                    Service Layer                          │  │
│  │  AuthManager  │  SyncManager  │  ProviderPool  │ Token... │  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                  │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │                    Protocol Layer                         │  │
│  │     IMAP Client    │    SMTP Client    │    OAuth         │  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                  │
│  ┌───────────────────────────▼───────────────────────────────┐  │
│  │                    Storage Layer                          │  │
│  │     Database (SQLite)    │    Keyring (Credentials)       │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 关键文件位置

| 组件 | 文件路径 |
|------|----------|
| 前端添加账号弹窗 | `src/components/common/AddAccountModal.vue` |
| 前端账号 Store | `src/stores/account.ts` |
| 后端账号命令 | `src-tauri/src/command/account.rs` |
| 后端 OAuth 命令 | `src-tauri/src/command/oauth.rs` |
| 后端连接测试 | `src-tauri/src/command/connection.rs` |
| 认证管理器 | `src-tauri/src/auth/auth_manager.rs` |
| 同步管理器 | `src-tauri/src/sync/sync_manager.rs` |
| 邮件处理器 | `src-tauri/src/sync/mail_processor.rs` |
| 账号仓储 | `src-tauri/src/storage/accounts.rs` |

---

## 密码认证流程

### 流程图

```
用户操作
   │
   ▼
1. 打开添加账号对话框
   │
   ├─ AddAccountModal.vue::show = true
   │
   ▼
2. 填写表单
   │
   ├─ 输入账号名称、邮箱地址
   ├─ 选择服务商（自动检测）
   ├─ 输入密码
   │
   ▼
3. 点击"添加"按钮
   │
   ▼
4. 前端验证
   │
   ├─ AddAccountModal.vue::handleSubmit()
   │  └─ 验证账号名称、邮箱格式
   │
   ▼
5. 测试连接
   │
   ├─ invoke('test_email_connection', { email, password, provider, ... })
   │  └─ command/connection.rs::test_email_connection()
   │     └─ protocols/imap::test_connection(host, port, email, auth)
   │
   ▼
6. 创建账号
   │
   ├─ invoke('add_account', { account })
   │  └─ command/account.rs::add_account()
   │     └─ storage::AccountRepository::create()
   │        ├─ 保存账号信息到数据库
   │        └─ 存储密码到 Keyring
   │
   ▼
7. 刷新账号列表
   │
   ├─ accountStore.fetchAccounts()
   │  └─ invoke('list_accounts')
   │     └─ storage::AccountRepository::get_all()
   │
   ▼
8. 切换到新账号
   │
   ├─ accountStore.selectAccountById(accountId)
   │
   ▼
9. 关闭对话框
   │
   └─ uiStore.closeAddAccountModal()
```

### 函数调用栈

#### 前端（Vue）

```typescript
// 1. 打开对话框
uiStore.modals.addAccount = true

// 2. 提交表单
AddAccountModal.vue::handleSubmit()
  → form.value.authType === AuthType.Password
  → invoke('test_email_connection', {
      email: form.value.email,
      password: form.value.password,
      provider: form.value.provider,
      imapHost: isCustom.value ? form.value.imapHost : null,
      imapPort: isCustom.value ? form.value.imapPort : null,
      imapSsl: isCustom.value ? form.value.imapSsl : null,
      // ...
    })

// 3. 创建账号
AddAccountModal.vue::createAccountAndSync()
  → invoke('add_account', { account: accountData })
  → accountStore.fetchAccounts()
  → accountStore.selectAccountById(accountId)
  → invoke('sync_account_with_progress', { accountId })

// 4. AccountStore
accountStore.addAccount(accountData)
  → invoke('add_account', { account: accountToRequest(accountData) })
  → accounts.value.push(dtoToAccount(dto))

accountStore.fetchAccounts()
  → invoke<AccountDto[]>('list_accounts')
  → accounts.value = dtos.map(dtoToAccount)
```

#### 后端（Rust）

```rust
// 1. 连接测试
command/connection.rs::test_email_connection()
  → // 检测服务器
  → let host = imap_host.unwrap_or_else(|| match provider.as_str() {
        "gmail" => "imap.gmail.com",
        "outlook" | "hotmail" => "outlook.office365.com",
        "icloud" => "imap.mail.me.com",
        "yahoo" => "imap.mail.yahoo.com",
        _ => "imap.example.com",
    })
  → protocols/imap::test_connection(&host, port, &email, ImapAuth::Password(password))
     → AsyncImapClient::connect()
     → AsyncImapClient::login()
     → AsyncImapClient::logout()

// 2. 创建账号
command/account.rs::add_account()
  → let request = storage::CreateAccountRequest::from(account)
  → storage::AccountRepository::create(&db, app_handle, request)
     → // 保存到数据库
     → account::ActiveModel { ... }.insert(db)
     → // 存储密码到 Keyring
     → app_handle.keyring()
         .set_password(KEYRING_SERVICE, &format!("password:{}", account_id), &password)
     → // 返回 AccountDto
     → Ok(account.into())

// 3. 列出账号
command/account.rs::list_accounts()
  → storage::AccountRepository::get_all(&db)
  → account::Entity::find().all(db)
```

---

## OAuth 认证流程

### 流程图

```
用户操作
   │
   ▼
1. 打开添加账号对话框
   │
   ├─ 选择服务商 (Gmail / Outlook)
   ├─ 选择认证方式 (OAuth 2.0)
   │
   ▼
2. 点击"使用 XXX 账号授权"
   │
   ├─ AddAccountModal.vue::startOAuthLogin()
   │  └─ showOAuthModal.value = true
   │
   ▼
3. OAuth 弹窗
   │
   ├─ OAuthLoginModal.vue
   │  ├─ 调用 get_oauth_auth_url
   │  ├─ 在浏览器中打开授权页面
   │  └─ 用户授权后获取 code 和 state
   │
   ▼
4. 回调处理
   │
   ├─ deep-link-handler 接收回调
   │  └─ invoke('exchange_oauth_code', { code, state })
   │
   ▼
5. 交换授权码
   │
   ├─ command/oauth.rs::exchange_oauth_code()
   │  ├─ auth_manager.authenticate_oauth(email, code, state)
   │  │  ├─ oauth_handler.exchange_code(provider, code, state)
   │  │  │  ├─ 向服务商发送 POST 请求
   │  │  │  ├─ 获取 access_token, refresh_token, id_token
   │  │  │  └─ TokenManager::store_token(account_id, token)
   │  │  └─ 解析 id_token 获取用户信息
   │  ├─ 创建账号记录
   │  └─ TokenManager::migrate_token_account(0, account.id)
   │
   ▼
6. 完成添加
   │
   ├─ 返回 AccountDto
   ├─ accountStore.accounts.value.push(account)
   ├─ accountStore.selectAccount(account)
   └─ 关闭对话框
```

### 函数调用栈

#### 前端（Vue）

```typescript
// 1. 获取授权 URL
accountStore.getOAuthAuthUrl(provider)
  → invoke<string>('get_oauth_auth_url', { provider })
  → // 返回 auth_url

// 2. 交换授权码
accountStore.exchangeOAuthCode(code, state)
  → invoke<AccountDto>('exchange_oauth_code', { code, state })
  → const account = dtoToAccount(dto)
  → accounts.value.push(account)
  → currentAccount.value = account

// 3. OAuthLoginModal.vue
OAuthLoginModal.vue::mounted()
  → emit('get-auth-url')
  → accountStore.getOAuthAuthUrl(provider)
  → // 打开浏览器窗口
  → open::that(auth_url)

// 4. 接收回调
deep-link-handler::handleOAuthCallback()
  → const params = new URL(url).searchParams
  → const code = params.get('code')
  → const state = params.get('state')
  → accountStore.exchangeOAuthCode(code, state)
```

#### 后端（Rust）

```rust
// 1. 获取授权 URL
command/oauth.rs::get_oauth_auth_url()
  → auth_manager.get_oauth_url(&email)
     → provider_pool.detect_provider(&email)
     → oauth_handler.get_authorization_url(provider)
        → // 生成 state
        → let state = generate_random_state()
        → // 构建授权 URL
        → let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            client_id, redirect_uri, scope, state
        )
        → AuthorizationContext { auth_url, state }

// 2. 验证 Token
command/oauth.rs::validate_oauth_token()
  → match provider.as_str() {
        "google" => {
            client.get("https://www.googleapis.com/oauth2/v3/userinfo")
                .bearer_auth(&token)
                .send()
        }
        "microsoft" => {
            client.get("https://graph.microsoft.com/v1.0/me")
                .bearer_auth(&token)
                .send()
        }
    }

// 3. 交换授权码
command/oauth.rs::exchange_oauth_code()
  → auth_manager.authenticate_oauth(&email, &code, &state)
     → provider_pool.detect_provider(&email)
     → oauth_handler.exchange_code(provider, code, state)
        → // POST 请求到 token endpoint
        → client.post(token_url)
            .form(&[
                ("code", code),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
        → TokenResponse { access_token, refresh_token, expires_in, id_token, ... }
     → // 存储 Token
     → token_manager.store_token(0, &token_response).await
     → // 解析 id_token
     → AuthResult { email, display_name, id_token, expires_at, ... }
  → // 检测服务商
  → provider_pool.detect_provider(&email)
  → // 创建账号
  → AccountRepository::create(&db, app_handle, account_req)
  → // 迁移 Token
  → token_manager.migrate_token_account(0, account.id)
```

---

## 后端组件详解

### AuthManager (认证管理器)

位置: `src-tauri/src/auth/auth_manager.rs`

```rust
pub struct AuthManager {
    oauth_handler: Arc<OAuthHandler>,      // OAuth 处理
    token_manager: Arc<TokenManager>,      // Token 管理
    password_auth: Arc<PasswordAuth>,      // 密码认证
    enterprise_auth: Arc<EnterpriseAuth>,  // 企业认证
    provider_pool: Arc<ProviderPool>,      // 服务商池
}
```

**关键方法:**

| 方法 | 说明 |
|------|------|
| `get_oauth_url(email)` | 获取 OAuth 授权 URL |
| `authenticate_oauth(email, code, state)` | OAuth 认证，交换授权码 |
| `refresh_token(account_id, email)` | 刷新 OAuth Token |
| `get_imap_auth(account_id, email, auth_type)` | 获取 IMAP 认证信息 |
| `get_smtp_auth(account_id, email, auth_type)` | 获取 SMTP 认证信息 |

### SyncManager (同步管理器)

位置: `src-tauri/src/sync/sync_manager.rs`

```rust
pub struct SyncManager {
    db: Arc<DbConn>,
    app_handle: AppHandle,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    delta_sync: Arc<DeltaSync>,
    folder_manager: Arc<FolderManager>,
    mail_processor: Arc<MailProcessor>,
    change_detector: Arc<ChangeDetector>,
    sync_state_manager: Arc<SyncStateManager>,
}
```

**同步流程:**

```
sync_account(account_id)
  → 1. 获取账号信息
  → 2. 检测服务商，获取 IMAP 配置
  → 3. 连接到 IMAP 服务器
  → 4. 同步文件夹列表
  → 5. 对每个文件夹执行增量同步
     → 5.1 检查 CONDSTORE 支持
     → 5.2 获取服务器 UID 列表
     → 5.3 检测变更
     → 5.4 获取新邮件/修改邮件的内容
     → 5.5 批量处理邮件
     → 5.6 更新同步状态
  → 6. 发送完成事件
```

### MailProcessor (邮件处理器)

位置: `src-tauri/src/sync/mail_processor.rs`

```rust
pub struct MailProcessor {
    db: Arc<DbConn>,
}
```

**处理流程:**

```
process_mails(account_id, folder, mails)
  → for each mail in mails:
      → process_single_mail(account_id, folder, mail)
         → save_mail_to_db(account_id, folder, mail)
            → check_mail_exists(account_id, folder, uid)
            → if exists:
                → update_mail_flags()
            → else:
                → insert new mail
```

### AccountRepository (账号仓储)

位置: `src-tauri/src/storage/accounts.rs`

**Keyring 存储:**

```
密码:
  Service: "com.postium.mail"
  Username: "password:<account_id>"
  Password: "<encrypted_password>"

OAuth Token:
  Service: "com.postium.mail"
  Username: "oauth:<account_id>"
  Password: "<json_token>"
```

---

## 前端组件详解

### AddAccountModal.vue

**状态管理:**

```typescript
interface FormState {
  name: string
  email: string
  provider: EmailProvider
  accountType: AccountType
  authType: AuthType
  password: string
  imapHost: string
  imapPort: number
  imapSsl: boolean
  smtpHost: string
  smtpPort: number
  smtpSsl: boolean
  color: string
  enterpriseTenantId: string
  enterpriseDomain: string
}
```

**监听器:**

1. **邮箱地址变化监听**
   - 自动检测服务商
   - 自动设置认证方式
   - 自动填充服务器配置

2. **服务商变化监听**
   - 自动填充邮箱后缀
   - 自动切换认证方式

3. **SSL 变化监听**
   - 自动切换端口 (143 ↔ 993, 25 ↔ 465)

**提交流程:**

```
handleSubmit()
  → if OAuth2:
      → if 没有 token:
          → startOAuthLogin()
          → return
      → validate_oauth_token()
  → if Password:
      → test_email_connection()
  → createAccountAndSync()
      → invoke('add_account')
      → accountStore.fetchAccounts()
      → accountStore.selectAccountById()
      → invoke('sync_account_with_progress')
      → uiStore.closeAddAccountModal()
```

### AccountStore

**关键方法:**

| 方法 | 说明 |
|------|------|
| `fetchAccounts()` | 获取账号列表 |
| `addAccount(accountData)` | 添加账号 |
| `updateAccount(accountId, updates)` | 更新账号 |
| `removeAccount(accountId)` | 删除账号 |
| `selectAccount(account)` | 选择当前账号 |
| `syncAccount(accountId)` | 同步账号 |
| `getOAuthAuthUrl(provider)` | 获取 OAuth 授权 URL |
| `exchangeOAuthCode(code, state)` | 交换 OAuth 授权码 |

---

## 同步流程

### 同步事件

```
后端发送事件:
  emit("sync://progress/{account_id}/{event_id}", SyncProgress)

前端监听:
  listen(`sync-progress-{account_id}`, (event) => {
    // 更新进度条
  })
```

### SyncProgress 结构

```typescript
interface SyncProgress {
  stage: 'connecting' | 'syncing_folders' | 'syncing_emails' | 'completed' | 'error'
  folder: string | null
  current: number
  total: number
  message: string
}
```

### 同步结果

```typescript
interface SyncResult {
  total_synced: number
  folders_synced: number
  errors: number
  duration_ms: number
}
```

---

## 调试指南

### 前端调试

1. **打开 DevTools**
   ```bash
   npm run tauri dev
   ```

2. **关键日志点**
   - `[AddAccountModal]` - 添加账号流程
   - `[AccountStore]` - 账号 Store 操作
   - `[fetchAccounts]` - 获取账号列表
   - `[sync-account]` - 同步操作

3. **查看 Store 状态**
   ```javascript
   // 在浏览器控制台
   import { useAccountStore } from '@/stores'
   const accountStore = useAccountStore()
   console.log(accountStore.accounts)
   console.log(accountStore.currentAccount)
   ```

### 后端调试

1. **启用日志**
   ```bash
   RUST_LOG=debug npm run tauri dev
   ```

2. **关键日志模块**
   ```
   [account]       - 账号操作
   [oauth]         - OAuth 认证
   [imap]          - IMAP 连接
   [sync]          - 同步操作
   [auth_manager]  - 认证管理
   [token_manager] - Token 管理
   ```

3. **数据库查看**
   ```bash
   # 数据库位置
   ~/.postium-mail/postium-mail.db

   # 使用 SQLite 客户端
   sqlite3 ~/.postium-mail/postium-mail.db
   ```

4. **Keyring 查看**

   **macOS:**
   ```bash
   security find-generic-password -s "com.postium.mail" -a "password:1"
   ```

   **Windows:**
   ```bash
   cmdkey /list | findstr "com.postium.mail"
   ```

   **Linux:**
   ```bash
   secret-tool search service "com.postium.mail"
   ```

### 常见问题排查

| 问题 | 可能原因 | 排查方法 |
|------|----------|----------|
| 添加账号失败 | IMAP 连接失败 | 检查服务器配置、端口、SSL |
| OAuth 授权失败 | Token 无效 | 检查系统时间、重新授权 |
| 同步无邮件 | 文件夹未选择 | 检查 folder 配置 |
| 密码错误 | Keyring 读取失败 | 检查密钥链权限 |
| Token 过期 | 超过有效期 | 自动刷新或重新授权 |

---

## 附录：数据模型

### Account 表

```sql
CREATE TABLE account (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL,
    account_type TEXT DEFAULT 'personal',
    auth_type TEXT DEFAULT 'password',
    imap_host TEXT,
    imap_port INTEGER,
    imap_ssl BOOLEAN,
    smtp_host TEXT,
    smtp_port INTEGER,
    smtp_ssl BOOLEAN,
    color TEXT,
    sync_enabled BOOLEAN DEFAULT 1,
    last_sync_at INTEGER,
    oauth_provider TEXT,
    oauth_token_expiry INTEGER,
    enterprise_tenant_id TEXT,
    enterprise_domain TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### Email 表

```sql
CREATE TABLE email (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    folder TEXT NOT NULL,
    uid INTEGER,
    message_id TEXT,
    subject TEXT,
    sender_name TEXT,
    sender_email TEXT NOT NULL,
    recipient_emails TEXT NOT NULL,
    cc_emails TEXT,
    bcc_emails TEXT,
    body_text TEXT,
    body_html TEXT,
    is_read BOOLEAN DEFAULT 0,
    is_starred BOOLEAN DEFAULT 0,
    is_draft BOOLEAN DEFAULT 0,
    sent_at INTEGER,
    received_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (account_id) REFERENCES account(id) ON DELETE CASCADE
);
```

### FolderSyncState 表

> **注意**: 旧的 `folders` 表已被删除（m010 迁移）
>
> 文件夹配置现在由 `provider.folder_mapping()` 动态提供，不再存储到数据库
>
> `folder_sync_states` 表只存储 IMAP 同步元数据（uidvalidity, uidnext, highest_modseq）

```sql
CREATE TABLE folder_sync_states (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    imap_name TEXT NOT NULL,
    uidvalidity INTEGER,
    uidnext INTEGER,
    highest_modseq INTEGER,
    synced_at INTEGER,
    created_at INTEGER DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    UNIQUE(account_id, imap_name)
);

CREATE INDEX idx_folder_sync_states_account ON folder_sync_states(account_id);
CREATE INDEX idx_folder_sync_states_uidvalidity ON folder_sync_states(uidvalidity);
CREATE INDEX idx_folder_sync_states_synced_at ON folder_sync_states(synced_at);
```

### 标准文件夹映射

各服务商通过 `MailProvider::folder_mapping()` 方法声明其标准文件夹：

```rust
pub struct StandardFolder {
    pub inbox: Vec<String>,      // 收件箱 IMAP 名称列表
    pub sent: Vec<String>,       // 已发送 IMAP 名称列表
    pub drafts: Vec<String>,     // 草稿箱 IMAP 名称列表
    pub spam: Vec<String>,       // 垃圾邮件 IMAP 名称列表
    pub trash: Vec<String>,      // 已删除 IMAP 名称列表
    pub archive: Vec<String>,    // 归档 IMAP 名称列表
    pub starred: Vec<String>,    // 星标邮件 IMAP 名称列表
}
```

**示例服务商映射**：
- **Gmail**: `archive` → `["[Gmail]/All Mail"]`
- **Outlook**: `inbox` → `["收件箱", "INBOX"]`
- **163/QQ**: `sent` → `["已发送", "Sent"]`

---

*最后更新: 2026-03-21*
