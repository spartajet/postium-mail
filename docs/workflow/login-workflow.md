# 账号登录与添加流程文档

本文档详细记录了用户添加邮件账号、账号登录的完整流程，包括前后端函数调用栈。

---

## 目录

1. [概述](#概述)
2. [密码认证流程](#密码认证流程)
3. [OAuth 认证流程](#oauth-认证流程)
4. [OAuth Token 刷新机制](#oauth-token-刷新机制)
5. [后端组件详解](#后端组件详解)
6. [前端组件详解](#前端组件详解)
7. [同步流程](#同步流程)
8. [调试指南](#调试指南)

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
| **前端** | |
| 添加账号弹窗 | `src/components/common/AddAccountModal.vue` |
| 账号 Store | `src/stores/account.ts` |
| OAuth 辅助工具 | `src/utils/oauthHelper.ts` |
| **后端命令** | |
| 账号命令 | `src-tauri/src/command/account.rs` |
| 认证命令 | `src-tauri/src/command/auth.rs` ⭐ |
| OAuth 命令 | `src-tauri/src/command/oauth.rs` |
| 连接测试 | `src-tauri/src/command/connection.rs` |
| **认证模块** | |
| 认证管理器 | `src-tauri/src/auth/auth_manager.rs` ⭐ |
| OAuth 处理器 | `src-tauri/src/auth/oauth_handler.rs` |
| Token 管理器 | `src-tauri/src/auth/token_manager.rs` |
| 密码认证 | `src-tauri/src/auth/password_auth.rs` |
| **OAuth 会话** | |
| 会话管理器 | `src-tauri/src/auth/oauth_session.rs` ⭐ |
| HTTP 回调服务器 | `src-tauri/src/auth/oauth_http_server.rs` ⭐ |
| **配置** | |
| OAuth 配置加载 | `src-tauri/src/config.rs` ⭐ |
| 配置模板 | `.env.example` ⭐ |
| **存储** | |
| 同步管理器 | `src-tauri/src/sync/sync_manager.rs` |
| 邮件处理器 | `src-tauri/src/sync/mail_processor.rs` |
| 账号仓储 | `src-tauri/src/storage/service/account.rs` |

⭐ 标记表示本次重构新增或重大修改的文件

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
   ├─ 选择服务商 (Gmail / Outlook / Google Workspace / Microsoft 365)
   ├─ 选择认证方式 (OAuth 2.0)
   │
   ▼
2. 点击"使用 XXX 账号授权"
   │
   ├─ AddAccountModal.vue::handleOAuthLogin()
   │  └─ OAuthHelper.startLogin(provider)
   │
   ▼
3. 启动 OAuth 会话
   │
   ├─ invoke('start_oauth_session', { provider })
   │  └─ auth/start_oauth_session()
   │     └─ auth_manager.start_oauth_session(email, provider)
   │        ├─ 创建 OAuthSession (生成 session_id)
   │        ├─ 启动 HTTP 回调服务器 (localhost:36279)
   │        ├─ 生成授权 URL (带 state 和 redirect_uri)
   │        └─ 返回 { auth_url, session_id }
   │
   ▼
4. 打开授权页面
   │
   ├─ 前端打开浏览器到 auth_url
   │  └─ 用户在浏览器中完成授权
   │
   ▼
5. 回调处理
   │
   ├─ 用户授权后，OAuth 提供商重定向到
   │  http://localhost:36279/callback?code=xxx&state=xxx
   │
   ├─ oauth_http_server.rs 接收回调
   │  ├─ 解析查询参数 (code, state, error)
   │  └─ 通过 Tauri 事件发送回调
   │     └─ app_handle.emit("oauth-http-callback", { code, state, error })
   │
   ▼
6. 处理 OAuth 回调
   │
   ├─ auth/process_oauth_callback()
   │  └─ auth_manager.process_oauth_callback(session_id, code, state)
   │     ├─ 验证 session 和 state
   │     ├─ 交换授权码获取 tokens
   │     │  └─ oauth_handler.exchange_code(provider, code, redirect_uri)
   │     │     ├─ POST 请求到 token endpoint
   │     │     ├─ 获取 access_token, refresh_token, expires_in, id_token
   │     │     └─ 解析 id_token 获取用户信息 (email, name)
   │     ├─ 创建账号记录
   │     │  └─ AccountRepository::create()
   │     ├─ 存储 OAuth tokens
   │     │  └─ token_manager.store_token(account_id, token_response)
   │     ├─ 清理 OAuthSession
   │     └─ 发送完成事件
   │        └─ emit("oauth-flow-complete", { session_id, status, account })
   │
   ▼
7. 前端接收完成事件
   │
   ├─ oauthHelper.ts::listenCallback()
   │  └─ listen('oauth-flow-complete', (event) => { ... })
   │     └─ 返回 Account 或抛出错误
   │
   ▼
8. 完成添加
   │
   ├─ 返回 AccountDto
   ├─ accountStore.accounts.value.push(account)
   ├─ accountStore.selectAccount(account)
   └─ 关闭对话框
```

### 配置说明

**OAuth 配置文件**: `.env.example`

```bash
# Google Gmail（需要 client_secret）
GOOGLE_CLIENT_ID=your-google-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-google-client-secret
GOOGLE_REDIRECT_URI=http://localhost:36279/callback

# Microsoft Outlook（公共客户端，不需要 client_secret）
MICROSOFT_CLIENT_ID=your-microsoft-client-id
MICROSOFT_CLIENT_SECRET=
MICROSOFT_TENANT=common
MICROSOFT_REDIRECT_URI=http://localhost:36279/callback
```

**配置加载**:
- 从 `src-tauri/.env` 文件加载（如果存在）
- 支持 Google/Microsoft 各自的 `client_secret` 配置
- Microsoft 作为公共客户端不需要 `client_secret`
- Google 作为桌面应用必须提供 `client_secret`

### 函数调用栈

#### 前端（Vue）

```typescript
// 1. 启动 OAuth 登录
OAuthHelper.startLogin(provider, timeout)
  → invoke<string>('get_oauth_auth_url', { provider })
  → const authUrl = await result
  → const accountPromise = this.listenCallback(timeout)
  → openInBrowser(authUrl)  // 使用默认浏览器打开
  → await accountPromise  // 等待流程完成

// 2. 监听 OAuth 回调
OAuthHelper.listenCallback(timeout)
  → listen<OAuthFlowResultPayload>('oauth-flow-complete', (event) => {
      const payload = event.payload
      if (payload.status === 'success') {
          resolve(payload.account)
      } else {
          reject(new Error(payload.error))
      }
  })

// 3. 回调事件结构
interface OAuthFlowResultPayload {
  session_id: string
  status: 'success' | 'error'
  account?: Account
  error?: string
}

// 4. 刷新 Token
OAuthHelper.refreshTokens(accountId)
  → invoke('refresh_oauth_token', { accountId })
  → accountStore.fetchAccounts()

// 5. 验证 Token
OAuthHelper.validateToken(accountId)
  → invoke<boolean>('validate_oauth_token', { accountId })
```

#### 后端（Rust）

```rust
// ==================== 命令层 ====================

// 1. 启动 OAuth 会话
command/auth.rs::start_oauth_session()
  → auth_manager.start_oauth_session(email, provider)
     → oauth_session_manager.create_session(email, provider)
        → 生成唯一 session_id
        → 生成 state 参数
        → 生成 code_verifier (PKCE)
        → OAuthSession { session_id, email, provider, state, code_verifier, ... }
     → oauth_http_server.start()
        → 绑定 127.0.0.1:36279
        → 监听 /callback 路径
     → 获取 OAuth 配置
        → load_oauth_config_for_provider(provider)
        → OAuthConfig { client_id, client_secret, redirect_uri, scopes, ... }
     → 构建授权 URL
        → auth_url = format!("{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            auth_url, client_id, redirect_uri, scope, state, code_challenge)
     → 返回 { auth_url, session_id }

// 2. 处理 OAuth 回调
command/auth.rs::process_oauth_callback()
  → auth_manager.process_oauth_callback(session_id, code, state)
     → oauth_session_manager.get_session(session_id)
     → 验证 state 参数
     → 获取 OAuth 配置
        → load_oauth_config_for_provider(session.provider)
     → 交换授权码
        → oauth_handler.exchange_code(session.provider, code, session.code_verifier, config)
           → POST { token_url }
             → form = [
                   ("code", code),
                   ("grant_type", "authorization_code"),
                   ("redirect_uri", redirect_uri),
                   ("client_id", client_id),
                   ("client_secret", client_secret),  // Google 需要，Microsoft 为 None
                   ("code_verifier", code_verifier),  // PKCE
               ]
           → TokenResponse { access_token, refresh_token, expires_in, id_token, ... }
     → 解析用户信息
        → jwt_handler.decode_id_token(id_token)
        → Userinfo { email, name, picture, ... }
     → 检测服务商
        → provider_pool.detect_provider(&email)
     → 创建账号
        → AccountRepository::create(&db, app_handle, account_req)
     → 存储 Token
        → token_manager.store_token(account_id, &token_response)
     → 删除会话
        → oauth_session_manager.remove_session(session_id)
     → 停止 HTTP 服务器
        → oauth_http_server.stop()
     → 发送完成事件
        → emit("oauth-flow-complete", { session_id, status: "success", account })

// ==================== HTTP 回调服务器 ====================

// 3. HTTP 回调处理
oauth_http_server.rs::handle_request()
  → if path == "/callback" && method == GET:
     → 解析查询参数
        → code = query_params.get("code")
        → state = query_params.get("state")
        → error = query_params.get("error")
     → 发送事件到主线程
        → app_handle.emit("oauth-http-callback", { code, state, error })
     → 返回 HTML 页面（成功/失败提示）

// ==================== 辅助函数 ====================

// 4. 配置加载
config.rs::load_oauth_config_for_provider(provider)
  → match provider {
        "gmail" => load_google_oauth_config()
        "googleworkspace" => load_google_workspace_oauth_config()
        "outlook" => load_microsoft_oauth_config()
        "microsoft365" => load_microsoft365_oauth_config()
    }
  → 从环境变量加载 (.env 文件)
  → OAuthConfig { client_id, client_secret, redirect_uri, scopes, ... }

// 5. Token 刷新
command/auth.rs::refresh_oauth_token()
  → token_manager.get_refresh_token(account_id)
  → POST { token_url }
    → form = [
          ("refresh_token", refresh_token),
          ("client_id", client_id),
          ("client_secret", client_secret),
          ("grant_type", "refresh_token"),
      ]
  → 更新存储的 token
```

---

## OAuth Token 刷新机制

### 概述

OAuth Token 刷新由 **TokenManager** 和 **AuthManager** 协同完成：

```
┌─────────────────────────────────────────────────────────────────┐
│                    Token 刷新架构                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────────┐      ┌─────────────────┐                 │
│   │   AuthManager   │─────▶│   TokenManager  │                 │
│   │  (刷新执行者)    │      │  (状态管理器)    │                 │
│   └────────┬────────┘      └────────┬────────┘                 │
│            │                        │                           │
│            │ 刷新 token             │ 读取/缓存 token           │
│            ▼                        ▼                           │
│   ┌─────────────────┐      ┌─────────────────┐                 │
│   │  OAuthHandler   │      │    Keyring      │                 │
│   │ (OAuth API 调用) │      │  (安全存储)      │                 │
│   └─────────────────┘      └─────────────────┘                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### TokenManager (Token 状态管理器)

位置: `src-tauri/src/auth/token_manager.rs`

```rust
pub struct TokenManager {
    keyring: Keyring,                           // 安全存储
    token_metadata: HashMap<i32, TokenMetadata>, // 内存元数据缓存
    access_token_cache: HashMap<i32, CachedToken>, // access_token 缓存
}

/// Token 元数据（内存缓存）
pub struct TokenMetadata {
    account_id: i32,
    provider: String,
    expires_at: i64,        // 过期时间戳
    refresh_count: i32,     // 刷新次数
}

/// 缓存的 access_token
pub struct CachedToken {
    token: String,
    cached_at: i64,         // 缓存时间
}
```

**关键常量:**

| 常量 | 值 | 说明 |
|------|-----|------|
| `DEFAULT_EXPIRY_THRESHOLD` | 300s | 提前 5 分钟判定即将过期 |
| `ACCESS_TOKEN_CACHE_TTL` | 300s | access_token 缓存有效期 5 分钟 |

**关键方法:**

| 方法 | 说明 |
|------|------|
| `store_token(account_id, token_response)` | 存储 OAuth tokens 到 Keyring |
| `get_refresh_token(account_id)` | 获取 refresh_token |
| `get_access_token(account_id, refresh_fn)` | 获取 access_token（带自动刷新） |
| `is_token_expiring_soon(account_id)` | 检查 token 是否即将过期 |
| `get_expiring_accounts(within_seconds)` | 获取即将过期的账号列表 |

### AuthManager (刷新执行器)

位置: `src-tauri/src/auth/auth_manager.rs`

```rust
impl AuthManager {
    /// 刷新单个账号的 token
    pub async fn refresh_token(&self, account_id: i32, email: &str) -> Result<()>

    /// 批量刷新即将过期的 tokens
    pub async fn refresh_expiring_tokens(
        &self,
        accounts: Vec<(i32, String)>
    ) -> Result<Vec<i32>>

    /// 获取 IMAP 认证信息（按需刷新）
    pub async fn get_imap_auth(
        &self,
        account_id: i32,
        email: &str,
        auth_type: &AuthType
    ) -> Result<ImapAuthInfo>
}
```

### 按需刷新流程

当 IMAP/SMTP 连接需要认证时，触发按需刷新：

```
SyncManager.sync_account(account_id)
   │
   ├─ auth_manager.get_imap_auth(account_id, email, auth_type)
   │     │
   │     ├─ if OAuth 账号:
   │     │     │
   │     │     ├─ token_manager.get_access_token(account_id, refresh_fn)
   │     │     │     │
   │     │     │     ├─ 检查 access_token_cache
   │     │     │     │     └─ 如果缓存有效 (< 5分钟): 直接返回
   │     │     │     │
   │     │     │     ├─ 检查 token_metadata
   │     │     │     │     └─ 如果即将过期 (< 5分钟): 调用 refresh_fn
   │     │     │     │
   │     │     │     ├─ 从 Keyring 读取 refresh_token
   │     │     │     │
   │     │     │     ├─ oauth_handler.refresh_access_token(provider, refresh_token)
   │     │     │     │     └─ POST 到 token endpoint
   │     │     │     │
   │     │     │     ├─ 更新 Keyring 存储
   │     │     │     ├─ 更新 token_metadata
   │     │     │     ├─ 缓存 access_token (5分钟)
   │     │     │     └─ 返回 access_token
   │     │     │
   │     │     └─ 返回 ImapAuthInfo::OAuth { access_token }
   │     │
   │     └─ if 密码账号:
   │           └─ 从 Keyring 读取密码
   │           └─ 返回 ImapAuthInfo::Password { password }
   │
   └─ 使用 ImapAuthInfo 连接 IMAP 服务器
```

### access_token 缓存机制

为减少不必要的刷新请求，TokenManager 实现了 access_token 缓存：

```rust
/// access_token 缓存配置
const ACCESS_TOKEN_CACHE_TTL: i64 = 300; // 5 分钟

impl TokenManager {
    pub async fn get_access_token<F, Fut>(
        &self,
        account_id: i32,
        refresh_fn: F,
    ) -> Result<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<TokenResponse>>,
    {
        // 1. 检查缓存
        if let Some(cached) = self.access_token_cache.get(&account_id) {
            let now = Utc::now().timestamp();
            if now - cached.cached_at < ACCESS_TOKEN_CACHE_TTL {
                return Ok(cached.token.clone());
            }
        }

        // 2. 缓存失效，执行刷新
        let token_response = refresh_fn().await?;

        // 3. 更新缓存
        self.access_token_cache.insert(account_id, CachedToken {
            token: token_response.access_token.clone(),
            cached_at: Utc::now().timestamp(),
        });

        Ok(token_response.access_token)
    }
}
```

**缓存效果:**
- 短时间内多次连接 IMAP（如同步多个文件夹）只触发一次刷新
- 减少 OAuth provider 的 API 调用
- 降低因频繁刷新导致的限流风险

### 批量刷新机制

对于后台定时任务或应用启动时，可批量刷新多个账号：

```rust
impl AuthManager {
    /// 批量刷新即将过期的 tokens
    pub async fn refresh_expiring_tokens(
        &self,
        accounts: Vec<(i32, String)>,  // (account_id, email)
    ) -> Result<Vec<i32>>              // 返回成功刷新的账号 ID
    {
        let mut refreshed = Vec::new();

        for (account_id, email) in accounts {
            // 检查是否即将过期
            if self.token_manager.is_token_expiring_soon(account_id).await? {
                match self.refresh_token(account_id, &email).await {
                    Ok(()) => {
                        refreshed.push(account_id);
                        tracing::info!("Token 刷新成功: account_id={}", account_id);
                    }
                    Err(e) => {
                        tracing::error!("Token 刷新失败: account_id={}, error={}", account_id, e);
                    }
                }
            }
        }

        Ok(refreshed)
    }
}
```

**使用场景:**

```rust
// 应用启动时预刷新
async fn on_app_ready(auth_manager: Arc<AuthManager>, account_repo: Arc<AccountRepository>) {
    let accounts = account_repo.get_all_oauth_accounts().await?;
    let refreshed = auth_manager.refresh_expiring_tokens(accounts).await?;
    tracing::info!("启动时预刷新了 {} 个账号的 token", refreshed.len());
}

// 后台定时刷新（每 4 小时）
async fn start_background_refresh(auth_manager: Arc<AuthManager>) {
    let mut interval = tokio::time::interval(Duration::from_secs(4 * 60 * 60));

    loop {
        interval.tick().await;
        // 获取即将过期（未来 1 小时内）的账号
        let expiring = auth_manager.token_manager
            .get_expiring_accounts(3600).await?;

        if !expiring.is_empty() {
            auth_manager.refresh_expiring_tokens(expiring).await?;
        }
    }
}
```

### Token 过期检测

```rust
impl TokenManager {
    /// 检查 token 是否即将过期
    pub async fn is_token_expiring_soon(&self, account_id: i32) -> Result<bool> {
        let metadata = self.token_metadata.get(&account_id)
            .ok_or_else(|| MailError::NotFound("Token 元数据不存在".into()))?;

        let now = Utc::now().timestamp();
        let threshold = DEFAULT_EXPIRY_THRESHOLD; // 5 分钟

        Ok(metadata.expires_at - now < threshold)
    }

    /// 获取指定时间内即将过期的账号
    pub async fn get_expiring_accounts(&self, within_seconds: i64) -> Result<Vec<(i32, String)>> {
        let now = Utc::now().timestamp();
        let mut expiring = Vec::new();

        for (account_id, metadata) in &self.token_metadata {
            if metadata.expires_at - now < within_seconds {
                expiring.push((*account_id, metadata.provider.clone()));
            }
        }

        Ok(expiring)
    }
}
```

### 刷新失败处理

当 token 刷新失败时，系统会：

1. **记录错误日志** - 便于排查问题
2. **标记账号状态** - 提示用户重新授权
3. **前端通知** - 显示 token 过期提醒

```rust
// 前端监听 token 过期事件
listen('oauth-token-expired', (event) => {
    const { account_id, email } = event.payload
    // 显示重新授权提示
    showReauthDialog(account_id, email)
})
```

### 总结

| 组件 | 职责 |
|------|------|
| **TokenManager** | Token 存储、内存缓存、过期检测、access_token 缓存 |
| **AuthManager** | 执行刷新、批量刷新协调、获取认证信息 |
| **OAuthHandler** | OAuth API 调用、token 交换 |
| **Keyring** | 安全存储 refresh_token |

**刷新策略:**
- **按需刷新**: 连接时检查并刷新（通过 `get_imap_auth()`）
- **预刷新**: 启动时/定时批量刷新即将过期的 token
- **缓存优化**: access_token 缓存 5 分钟，减少重复刷新

---

## 后端组件详解

### AuthManager (认证管理器)

位置: `src-tauri/src/auth/auth_manager.rs`

```rust
pub struct AuthManager {
    oauth_handler: Arc<OAuthHandler>,         // OAuth 处理
    token_manager: Arc<RwLock<TokenManager>>, // Token 管理
    password_auth: Arc<PasswordAuth>,         // 密码认证
    enterprise_auth: Arc<EnterpriseAuth>,     // 企业认证
    oauth_session_manager: Arc<OAuthSessionManager>, // OAuth 会话管理
    provider_pool: Arc<ProviderPool>,         // 服务商池
}
```

**关键方法:**

| 方法 | 说明 |
|------|------|
| `start_oauth_session(email, provider)` | 启动 OAuth 会话，创建 HTTP 回调服务器 |
| `process_oauth_callback(session_id, code, state)` | 处理 OAuth 回调，交换授权码 |
| `refresh_token(account_id, email)` | 刷新 OAuth Token |
| `validate_token(account_id)` | 验证 Token 是否有效 |
| `get_imap_auth(account_id, email, auth_type)` | 获取 IMAP 认证信息 |
| `get_smtp_auth(account_id, email, auth_type)` | 获取 SMTP 认证信息 |

**统一认证流程:**

```
AuthManager 作为统一入口
  ├─ 密码认证 → password_auth.authenticate()
  ├─ OAuth 认证 → start_oauth_session() + process_oauth_callback()
  └─ Token 刷新 → token_manager.refresh_token()
```

### OAuthSessionManager (会话管理器)

位置: `src-tauri/src/auth/oauth_session.rs`

```rust
pub struct OAuthSessionManager {
    sessions: Arc<RwLock<HashMap<String, OAuthSession>>>,
}

pub struct OAuthSession {
    session_id: String,      // 会话 ID
    email: String,           // 用户邮箱
    provider: String,        // 服务商
    state: String,           // OAuth state 参数
    code_verifier: String,   // PKCE code verifier
    created_at: i64,         // 创建时间
    expires_at: i64,         // 过期时间（5分钟）
}
```

**职责:**
- 管理活跃的 OAuth 会话
- 生成和验证 state 参数
- 生成 PKCE code_verifier
- 清理过期会话

### OAuthHttpServer (HTTP 回调服务器)

位置: `src-tauri/src/auth/oauth_http_server.rs`

```rust
pub struct OAuthHttpServer {
    port: u16,                    // 监听端口 (36279)
    running: Arc<Mutex<bool>>,   // 运行状态
    app_handle: AppHandle,       // Tauri 句柄
}
```

**功能:**
- 监听 `http://localhost:36279/callback`
- 解析 OAuth 回调参数 (code, state, error)
- 通过 Tauri 事件发送回调到主线程
- 返回用户友好的 HTML 页面（成功/失败提示）

**配置端口:**
```bash
# 默认端口
POSTIUM_OAUTH_PORT=36279

# 或在代码中配置
config.rs::get_oauth_callback_port() -> 36279
```

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
| OAuth 回调超时 | HTTP 回调服务器未启动 | 检查端口 36279 是否被占用 |
| Google Token 交换失败 | client_secret 未配置 | 检查 `.env` 文件是否配置 `GOOGLE_CLIENT_SECRET` |
| Microsoft Token 失败 | client_secret 不应为空 | 确保 `.env` 中 `MICROSOFT_CLIENT_SECRET` 为空 |
| 回调页面显示失败 | 授权被用户拒绝 | 重新进行授权流程 |
| 同步无邮件 | 文件夹未选择 | 检查 folder 配置 |
| 密码错误 | Keyring 读取失败 | 检查密钥链权限 |
| Token 过期 | 超过有效期 | 自动刷新或重新授权 |

**OAuth 配置检查清单:**

```bash
# 1. 检查 .env 文件是否存在
ls -la src-tauri/.env

# 2. 检查配置是否正确加载
# 启动应用时查看日志
========== Google OAuth 配置加载 ==========
  client_id: 123456789-xxx.apps.googleusercontent.com
  client_secret: *** (已设置)
  redirect_uri: http://localhost:36279/callback
==========================================

# 3. 检查端口是否被占用
# Windows
netstat -ano | findstr :36279

# Linux/macOS
lsof -i :36279

# 4. 手动测试回调服务器
# 浏览器访问
http://localhost:36279/callback?code=test&state=test
```

**日志调试:**

```bash
# 启用详细日志
RUST_LOG=postium_mail::auth=debug,postium_mail::oauth=debug npm run tauri dev

# 关键日志点
[oauth_session]    - OAuth 会话创建/清理
[oauth_http_server] - HTTP 回调服务器
[auth_manager]     - 认证流程
[token_manager]     - Token 存储/刷新
```

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
/// 标准文件夹映射
///
/// 注意：starred（星标邮件）字段已移除，
/// 星标邮件应通过邮件的 \Flagged 标志识别
pub struct StandardFolder {
    pub inbox: Vec<String>,      // 收件箱 IMAP 名称列表
    pub sent: Vec<String>,       // 已发送 IMAP 名称列表
    pub drafts: Vec<String>,     // 草稿箱 IMAP 名称列表
    pub spam: Vec<String>,       // 垃圾邮件 IMAP 名称列表
    pub trash: Vec<String>,      // 已删除 IMAP 名称列表
    pub archive: Vec<String>,    // 归档 IMAP 名称列表
}
```

**示例服务商映射**：
- **Gmail**: `archive` → `["[Gmail]/All Mail"]`
- **Outlook**: `inbox` → `["收件箱", "INBOX"]`
- **163/QQ**: `sent` → `["已发送邮件", "Sent"]`

---

## OAuth 架构变更说明

### v2.0 重构 (2026-03-22)

**重大架构变化:**

1. **从 Deep Link 改为 HTTP localhost 回调**
   - 旧方案: 使用自定义协议 `postium-mail://oauth/callback`
   - 新方案: 使用 HTTP 服务器 `http://localhost:36279/callback`
   - 优势: 避免协议注册问题，更标准的 OAuth 流程

2. **引入 OAuthSession 管理**
   - 集中管理 OAuth 流程状态
   - 支持并发 OAuth 流程
   - 自动清理过期会话

3. **client_secret 可配置**
   - 支持 Google 和 Microsoft 不同的 client_secret 需求
   - 通过 `.env` 文件配置，避免硬编码
   - Microsoft 公共客户端不需要 client_secret
   - Google 桌面应用必须提供 client_secret

4. **统一认证入口**
   - 所有认证相关命令移至 `command/auth.rs`
   - AuthManager 作为统一认证入口点
   - 简化前端调用逻辑

**向后兼容性:**

- 旧的 `exchange_oauth_code` 命令仍保留（已废弃）
- 建议使用新的 `start_oauth_session` + `process_oauth_callback` 流程
- 前端已更新为使用 `OAuthHelper.startLogin()`

**迁移指南:**

```typescript
// 旧方式（已废弃）
const authUrl = await invoke('get_oauth_auth_url', { provider })
const account = await invoke('exchange_oauth_code', { code, state })

// 新方式（推荐）
const account = await OAuthHelper.startLogin(provider)
```

---

*最后更新: 2026-03-22*
*更新内容: OAuth 架构重构（HTTP localhost 回调）、Token 刷新机制文档*
