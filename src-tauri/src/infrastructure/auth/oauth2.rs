//! OAuth2 PKCE 授权流程管理器
//!
//! 本模块实现基于 PKCE（Proof Key for Code Exchange）的 OAuth2 授权码流程，
//! 支持 Google、Microsoft 等邮件服务商。完整流程如下：
//!
//! 1. **生成授权 URL**（[`OAuth2Manager::start_auth`]）：生成 PKCE challenge/verifier
//!    与 CSRF state，构造带 `offline_access` / `access_type=offline` 的授权 URL，
//!    返回给前端打开浏览器。
//! 2. **浏览器授权**：用户在浏览器中登录并授权，服务商重定向回本地回调地址。
//! 3. **localhost 回调监听**（[`handle_callback`]）：`start_auth` 时已绑定临时端口
//!    并 spawn 一个后台任务，监听 `http://localhost:{port}/oauth/callback`，
//!    在 5 分钟超时内接收携带 `code` 与 `state` 的回调请求。
//! 4. **code 换 token**（[`do_token_exchange`]）：用 PKCE verifier 和授权 code
//!    向服务商 token 端点交换 access_token / refresh_token。
//! 5. **持久化**：refresh_token 写入系统 Keyring，access_token 缓存到内存，
//!    并通过 `AccountService` 在数据库中创建账号。
//! 6. **轮询完成状态**（[`OAuth2Manager::poll_oauth2`]）：前端定时轮询 state，
//!    直到拿到 [`OAuth2PollResult::Completed`]（仅含 email，不泄露 token）。
//!
//! # Microsoft token 端点特殊处理
//!
//! Microsoft 的 token 端点要求 `client_id` 必须出现在请求体中，不接受仅靠
//! HTTP Basic Auth 传递 `client_id`。因此在 [`OAuth2Manager::refresh_token`] 中
//! 故意不在 oauth2 client 上设置 `client_secret`（否则 oauth2 crate 会改用
//! Basic Auth），而是手动把 `client_id` / `client_secret` 作为额外参数加入请求体。
//! 详见该方法内的注释。

use crate::domain::auth::AuthManager;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::providers::{ProviderPool, SslMode};
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::service::account_service::{AccountService, CreateOAuth2AccountParams};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
    basic::BasicClient,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// static REDIRECT_URI: &str = "http://localhost:{port}/oauth/callback";

/// OAuth2 授权 URL 结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuth2AuthUrl {
    /// 完整的授权 URL，前端打开此 URL 进入服务商登录/授权页
    pub url: String,
    /// CSRF state，前端必须保存并在轮询时回传用于匹配会话
    pub state: String,
    /// 本地回调监听端口（由系统分配的临时端口）
    pub port: u16,
}

/// OAuth2 Token 结果（仅内部使用，不暴露给前端）
#[derive(Debug, Clone)]
pub struct OAuth2TokenResult {
    /// 访问令牌，用于调用 IMAP/SMTP XOAUTH2 认证
    pub access_token: String,
    /// 刷新令牌，用于在 access_token 过期后换取新令牌（部分 provider 可能不返回）
    pub refresh_token: Option<String>,
    /// access_token 有效期（秒），为空时上层按默认值处理
    pub expires_in: Option<i64>,
}

/// OAuth2 授权完成信息（返回给前端的最小信息）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuth2CompletedInfo {
    /// 授权成功的邮箱地址（token 等敏感信息不返回前端）
    pub email: String,
}

/// OAuth2 轮询状态
///
/// 前端通过 [`OAuth2Manager::poll_oauth2`] 周期性轮询某个 `state` 的结果。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum OAuth2PollResult {
    /// 授权仍在进行中（回调尚未到达或 token 交换未完成）
    Pending,
    /// 授权已完成，携带返回给前端的最小信息（邮箱）
    Completed(OAuth2CompletedInfo),
    /// 授权失败，携带错误描述字符串
    Error(String),
}

/// 正在进行中的 OAuth2 会话
///
/// 在 `start_auth` 时创建并按 `state` 存入 `Inner::sessions`，
/// 回调到达后取出并消费（PKCE verifier 仅可使用一次）。
struct PendingSession {
    /// PKCE 验证器，code 换 token 时使用，使用后置空防止重复消费
    pkce_verifier: Option<PkceCodeVerifier>,
    /// 本地回调监听端口
    port: u16,
    /// 提供商标识（如 "gmail"、"outlook"）
    provider_id: String,
    /// 用户邮箱
    email: String,
    /// 可选的显示名称，用于在数据库创建账号时回填
    display_name: Option<String>,
}

/// 内部共享状态
///
/// 被 [`OAuth2Manager`] 以 `Arc` 形式持有，并克隆给后台回调任务使用。
/// 所有可变状态都通过 `Mutex` 保护，可跨异步任务共享。
struct Inner {
    /// 进行中的会话表：state → 会话（回调到达后移除）
    sessions: Mutex<HashMap<String, PendingSession>>,
    /// 完成结果表：state → 轮询结果（供 `poll_oauth2` 读取）
    completed: Mutex<HashMap<String, OAuth2PollResult>>,
    /// 提供商池，用于读取 OAuth2/IMAP/SMTP 配置
    provider_pool: Arc<ProviderPool>,
    /// 共享的 reqwest 客户端（关闭重定向，避免跟随 token 端点的 302）
    http_client: reqwest::Client,
    /// 认证管理器，用于 Keyring 存取和 access_token 缓存
    auth_manager: Arc<AuthManager>,
    /// 数据库连接，用于创建 OAuth2 账号
    db: DbConn,
}

/// OAuth2 PKCE 流程管理器
///
/// 负责协调一次完整的 OAuth2 授权码 + PKCE 流程：生成授权 URL、监听本地回调、
/// code 换 token、将 token 持久化到 Keyring / 内存缓存 / 数据库，并暴露轮询接口
/// 供前端查询授权结果。
///
/// 内部状态封装在 [`Inner`] 中并以 `Arc` 共享，因此 [`OAuth2Manager`] 本身可廉价克隆，
/// 也能被后台回调任务持有。实例在应用启动时由 [`AuthManager::set_oauth2_manager`]
/// 注入，全局唯一。
///
/// [`AuthManager::set_oauth2_manager`]: crate::domain::auth::AuthManager::set_oauth2_manager
pub struct OAuth2Manager {
    inner: Arc<Inner>,
}

impl OAuth2Manager {
    /// 创建 OAuth2 管理器。
    ///
    /// 初始化一个禁用重定向的共享 reqwest 客户端（避免 token 端点 302 干扰），
    /// 并从全局 [`PROVIDER_POOL`] 获取 provider 配置来源。
    ///
    /// # 参数
    ///
    /// - `auth_manager`: 认证管理器（Keyring 存取 + access_token 缓存）
    /// - `db`: 数据库连接（用于创建 OAuth2 账号）
    ///
    /// # 错误
    ///
    /// 当全局 provider pool 未初始化时返回 [`MailError::ProviderNotSupported`]。
    pub fn new(auth_manager: Arc<AuthManager>, db: DbConn) -> Result<Self, MailError> {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build reqwest client for OAuth2");
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".to_string(),
            ))?
            .clone();
        Ok(Self {
            inner: Arc::new(Inner {
                sessions: Mutex::new(HashMap::new()),
                completed: Mutex::new(HashMap::new()),
                provider_pool,
                http_client,
                auth_manager,
                db,
            }),
        })
    }

    /// Step 1: 生成带 PKCE 的授权 URL，启动后台回调监听
    ///
    /// 生成 PKCE challenge/verifier 与 CSRF state，绑定一个临时本地端口，
    /// 构造授权 URL 返回给前端打开浏览器；同时 spawn 一个最长等待 5 分钟的
    /// 后台任务（[`handle_callback`]）监听回调。
    ///
    /// # 参数
    ///
    /// - `provider_id`: 提供商标识（如 "gmail"、"outlook"），必须支持 OAuth2
    /// - `email`: 用户邮箱
    /// - `display_name`: 可选显示名称，用于后续在数据库创建账号时回填
    ///
    /// # 返回
    ///
    /// [`OAuth2AuthUrl`]，包含授权 URL、state（轮询时回传）和本地回调端口。
    ///
    /// # 错误
    ///
    /// - [`MailError::InvalidProvider`]: 未知 provider
    /// - [`MailError::OAuth2Error`]: provider 不支持 OAuth2、端口绑定失败、
    ///   或授权/重定向 URL 非法
    pub async fn start_auth(
        &self,
        provider_id: &str,
        email: &str,
        display_name: Option<&str>,
    ) -> Result<OAuth2AuthUrl, MailError> {
        tracing::info!(provider_id, email, "启动 OAuth2 授权");

        let provider = self
            .inner
            .provider_pool
            .get(provider_id)
            .ok_or_else(|| MailError::InvalidProvider(provider_id.to_string()))?;

        let config = provider.oauth_config().ok_or_else(|| {
            MailError::OAuth2Error(format!("provider {provider_id} 不支持 OAuth2"))
        })?;

        // 绑定临时端口用于回调
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| MailError::OAuth2Error(format!("绑定回调端口失败: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| MailError::OAuth2Error(format!("获取本地地址失败: {e}")))?
            .port();
        tracing::info!("使用本地端口 {port} 作为 OAuth2 回调端口");

        let auth_url = AuthUrl::new(config.auth_url.clone())
            .map_err(|e| MailError::OAuth2Error(format!("无效 auth_url: {e}")))?;
        let token_url = TokenUrl::new(config.token_url.clone())
            .map_err(|e| MailError::OAuth2Error(format!("无效 token_url: {e}")))?;
        let redirect_url = RedirectUrl::new(format!("http://localhost:{port}/oauth/callback"))
            .map_err(|e| MailError::OAuth2Error(format!("无效 redirect_url: {e}")))?;

        let client_id = ClientId::new(config.client_id.clone());
        let mut client_builder = BasicClient::new(client_id);
        if let Some(secret) = &config.client_secret
            && !secret.is_empty()
        {
            client_builder = client_builder.set_client_secret(ClientSecret::new(secret.clone()));
        }

        let client = client_builder
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let state_str = CsrfToken::new_random().secret().clone();

        let scopes: Vec<Scope> = config
            .scopes
            .iter()
            .map(|s| Scope::new(s.clone()))
            .collect();

        let (auth_url, _csrf_token) = client
            .authorize_url(|| CsrfToken::new(state_str.clone()))
            .add_scopes(scopes)
            .add_extra_param("access_type", "offline")
            .set_pkce_challenge(pkce_challenge)
            .url();

        // 存储会话
        {
            let mut sessions = self.inner.sessions.lock().await;
            sessions.insert(
                state_str.clone(),
                PendingSession {
                    pkce_verifier: Some(pkce_verifier),
                    port,
                    provider_id: provider_id.to_string(),
                    email: email.to_string(),
                    display_name: display_name.map(|s| s.to_string()),
                },
            );
        }

        // 启动后台回调监听
        tracing::debug!(port, state = %state_str, "后台回调监听已启动");
        let inner = self.inner.clone();
        let state_for_task = state_str.clone();
        tauri::async_runtime::spawn(async move {
            // 超时 5 分钟
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(300),
                handle_callback(listener, &state_for_task, &inner),
            )
            .await;

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let mut completed = inner.completed.lock().await;
                    completed.insert(
                        state_for_task,
                        OAuth2PollResult::Error(format!("回调处理失败: {e}")),
                    );
                }
                Err(_) => {
                    // 超时
                    let mut sessions = inner.sessions.lock().await;
                    sessions.remove(&state_for_task);
                }
            }
        });

        Ok(OAuth2AuthUrl {
            url: auth_url.to_string(),
            state: state_str,
            port,
        })
    }

    /// 轮询 OAuth2 状态
    ///
    /// 前端在打开授权 URL 后，使用 `start_auth` 返回的 `state` 周期性调用本方法，
    /// 直到拿到 [`OAuth2PollResult::Completed`] 或 [`OAuth2PollResult::Error`]。
    /// 若 `state` 仍在进行中（回调尚未到达 / token 交换未完成），返回
    /// [`OAuth2PollResult::Pending`]。
    ///
    /// # 参数
    ///
    /// - `state`: `start_auth` 返回的 CSRF state
    ///
    /// # 返回
    ///
    /// 当前 [`OAuth2PollResult`]；本方法目前不会返回 `Err`。
    pub async fn poll_oauth2(&self, state: &str) -> Result<OAuth2PollResult, MailError> {
        tracing::trace!(state = %state, "轮询 OAuth2 状态");
        let completed = self.inner.completed.lock().await;
        if let Some(result) = completed.get(state) {
            Ok(result.clone())
        } else {
            Ok(OAuth2PollResult::Pending)
        }
    }

    /// 刷新过期的 access_token
    ///
    /// 使用存储在 Keyring 中的 refresh_token 向服务商 token 端点换取新的
    /// access_token（必要时还会拿到新的 refresh_token）。主要由
    /// [`AuthManager::get_credentials`] 在缓存未命中时调用。
    ///
    /// # 参数
    ///
    /// - `provider_id`: 提供商标识，必须支持 OAuth2
    /// - `refresh_token`: 有效的 refresh_token（通常来自 Keyring）
    ///
    /// # 返回
    ///
    /// [`OAuth2TokenResult`]，包含新的 access_token（以及可能的新 refresh_token）。
    ///
    /// # Microsoft token 端点特殊处理
    ///
    /// Microsoft 要求 `client_id` 必须出现在请求体中，**不接受**仅靠 HTTP Basic Auth
    /// 传递 `client_id`。oauth2 crate 在 client 上设置了 `client_secret` 时会改用
    /// Basic Auth，导致请求被拒。因此这里故意不在 client 上设置 `client_secret`，
    /// 而是把 `client_id` / `client_secret` 作为额外参数加入请求体（见下方注释行）。
    ///
    /// # 错误
    ///
    /// - [`MailError::InvalidProvider`]: 未知 provider
    /// - [`MailError::OAuth2Error`]: provider 不支持 OAuth2、token_url 非法或刷新请求失败
    ///
    /// [`AuthManager::get_credentials`]: crate::domain::auth::AuthManager::get_credentials
    pub async fn refresh_token(
        &self,
        provider_id: &str,
        refresh_token: &str,
    ) -> Result<OAuth2TokenResult, MailError> {
        let provider = self
            .inner
            .provider_pool
            .get(provider_id)
            .ok_or_else(|| MailError::InvalidProvider(provider_id.to_string()))?;

        let config = provider.oauth_config().ok_or_else(|| {
            MailError::OAuth2Error(format!("provider {provider_id} 不支持 OAuth2"))
        })?;

        let token_url = TokenUrl::new(config.token_url.clone())
            .map_err(|e| MailError::OAuth2Error(format!("无效 token_url: {e}")))?;

        let client_id = ClientId::new(config.client_id);
        let client = BasicClient::new(client_id).set_token_uri(token_url);

        // 不在 client 上设置 client_secret，确保 oauth2 crate 将 client_id 放入请求体。
        // ── Microsoft token 端点的硬性要求 ──
        // Microsoft 的 token 端点要求 client_id 必须出现在请求体中，不接受仅靠 Basic Auth
        // 传递 client_id。oauth2 crate 一旦在 client 上设置了 client_secret，就会把
        // client_id/client_secret 放进 HTTP Basic Auth 头而不是请求体，导致刷新被拒。
        // 因此这里保持 client 不带 secret，再通过 add_extra_param 把 client_secret
        // 补进请求体（下方）。如未来更换 oauth2 crate 行为，需重新验证此流程。
        let refresh_token = RefreshToken::new(refresh_token.to_string());
        let mut refresh_req = client.exchange_refresh_token(&refresh_token);

        if let Some(secret) = config.client_secret {
            refresh_req = refresh_req.add_extra_param("client_secret", secret);
        }

        let token_result = refresh_req
            .request_async(&self.inner.http_client)
            .await
            .map_err(|e| MailError::OAuth2Error(format!("刷新 token 失败: {e}")))?;

        Ok(to_token_result(&token_result))
    }
}

/// 后台处理 OAuth2 回调：解析 code + state → 交换 token → 存储 + 创建账号
///
/// 由 [`OAuth2Manager::start_auth`] spawn，在 5 分钟超时内等待一次回调连接。
/// 成功路径：解析出 `code`/`state` → 校验 state → code 换 token → 把 refresh_token
/// 写入 Keyring、access_token 缓存到内存、经 `AccountService` 在数据库创建账号，
/// 最后把 [`OAuth2PollResult::Completed`] 写入 `completed` 表供前端轮询读取。
///
/// # 参数
///
/// - `listener`: 已绑定临时端口的 TCP 监听器（来自 `start_auth`）
/// - `expected_state`: 期望的 CSRF state，用于和回调带回的 state 比对
/// - `inner`: 共享内部状态
///
/// # 返回
///
/// 成功返回 `Ok(())`；任意环节失败返回 `Err(String)`，
/// 调用方会把它转成 [`OAuth2PollResult::Error`]。
async fn handle_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
    inner: &Arc<Inner>,
) -> Result<(), String> {
    tracing::info!("等待 OAuth2 回调连接...");

    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("接受回调连接失败: {e}"))?;

    let mut buf = vec![0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .map_err(|e| format!("读取回调数据失败: {e}"))?;

    let request_str = String::from_utf8_lossy(&buf[..n]);

    // 解析 HTTP 请求第一行: GET /oauth/callback?code=xxx&state=yyy HTTP/1.1
    let first_line = request_str.lines().next().unwrap_or("");
    let uri = first_line.split_whitespace().nth(1).unwrap_or("");

    // 提取查询参数
    let query = uri.split('?').nth(1).unwrap_or("");
    let mut code = None;
    let mut state = None;

    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "code" => code = Some(urldecode(value)),
                "state" => state = Some(urldecode(value)),
                "error" => {
                    let error_msg = urldecode(value);
                    let html = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
                         <html><body><h2>授权失败</h2><p>{error_msg}</p>\
                         <p>可以关闭此页面返回应用。</p></body></html>"
                    );
                    let _ = tokio::io::AsyncWriteExt::write(&mut stream, html.as_bytes()).await;
                    return Err(format!("OAuth2 错误: {error_msg}"));
                }
                _ => {}
            }
        }
    }

    let code = code.ok_or("回调缺少 code 参数")?;
    let state = state.ok_or("回调缺少 state 参数")?;

    tracing::debug!(state = %state, code_len = code.len(), "解析回调参数成功");

    // 响应浏览器
    let html = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
                <html><body><h2>授权成功！</h2><p>可以关闭此页面返回应用。</p></body></html>";
    let _ = tokio::io::AsyncWriteExt::write(&mut stream, html.as_bytes()).await;

    // 验证 state
    if state != expected_state {
        tracing::warn!(expected = expected_state, received = %state, "state 不匹配");
        return Err(format!("state 不匹配: 期望 {expected_state}, 收到 {state}"));
    }

    // 从 sessions 中取出会话信息
    let session = {
        let mut sessions = inner.sessions.lock().await;
        sessions
            .remove(&state)
            .ok_or_else(|| format!("未找到 state={state} 的会话"))?
    };

    // 交换 token
    let token_result = do_token_exchange(inner, session, code).await?;

    // ──── 后端自动完成：存储 token + 创建账号 ────

    // 1. 保存 refresh_token 到 Keyring
    let refresh_token = token_result
        .refresh_token
        .ok_or_else(|| "OAuth2 未返回 refresh_token".to_string())?;
    inner
        .auth_manager
        .save_password(&token_result.email, &refresh_token)
        .map_err(|e| format!("保存 refresh_token 到 Keyring 失败: {e}"))?;

    // 2. 缓存 access_token 到内存
    let expires_in = token_result.expires_in.unwrap_or(3600);
    inner.auth_manager.cache_access_token(
        &token_result.email,
        &token_result.access_token,
        expires_in,
    );

    // 3. 从 provider 获取 IMAP/SMTP 配置
    let provider = inner
        .provider_pool
        .get(&token_result.provider_id)
        .ok_or_else(|| format!("未找到 provider: {}", token_result.provider_id))?;
    let imap = provider.imap_config(&token_result.email);
    let smtp = provider.smtp_config(&token_result.email);
    let provider_info = provider.provider_info();

    // 4. 在数据库中创建账号（通过 AccountService）
    let account_service = AccountService::new(inner.db.clone(), inner.auth_manager.clone());
    let params = CreateOAuth2AccountParams {
        email: token_result.email.clone(),
        display_name: token_result.display_name.clone(),
        provider_id: token_result.provider_id.clone(),
        imap_host: imap.host.clone(),
        imap_port: imap.port,
        imap_ssl_mode: ssl_mode_to_string(imap.ssl),
        smtp_host: smtp.host.clone(),
        smtp_port: smtp.port,
        smtp_ssl_mode: ssl_mode_to_string(smtp.ssl),
        color: provider_info.color.clone(),
    };

    account_service
        .create_oauth2_account(params)
        .await
        .map_err(|e| format!("创建账号失败: {e}"))?;

    tracing::info!(email = %token_result.email, "OAuth2 账号自动创建成功");

    // 5. 存储完成结果（前端只拿到 email，不拿 token）
    let mut completed = inner.completed.lock().await;
    completed.insert(
        state.clone(),
        OAuth2PollResult::Completed(OAuth2CompletedInfo {
            email: token_result.email,
        }),
    );

    Ok(())
}

/// 执行 OAuth2 code → token 交换
///
/// 用会话中保存的 PKCE verifier 和回调拿到的授权 code，向服务商 token 端点
/// 换取 access_token / refresh_token。verifier 取出后置空，防止被重复使用。
///
/// # 参数
///
/// - `inner`: 共享内部状态（用于读取 provider 配置和 http_client）
/// - `session`: 已从 `sessions` 表移除的待处理会话（消费其中 PKCE verifier）
/// - `code`: 回调拿到的授权码
///
/// # 返回
///
/// 成功返回 [`TokenExchangeResult`]（含 token 与回填的邮箱/provider 信息），
/// 失败返回 `Err(String)`。
async fn do_token_exchange(
    inner: &Inner,
    mut session: PendingSession,
    code: String,
) -> Result<TokenExchangeResult, String> {
    let provider = inner
        .provider_pool
        .get(&session.provider_id)
        .ok_or_else(|| format!("未找到 provider: {}", session.provider_id))?;

    let config = provider
        .oauth_config()
        .ok_or_else(|| format!("provider {} 不支持 OAuth2", session.provider_id))?;

    let auth_url =
        AuthUrl::new(config.auth_url.clone()).map_err(|e| format!("无效 auth_url: {e}"))?;
    let token_url =
        TokenUrl::new(config.token_url.clone()).map_err(|e| format!("无效 token_url: {e}"))?;
    let redirect_url =
        RedirectUrl::new(format!("http://localhost:{}/oauth/callback", session.port))
            .map_err(|e| format!("无效 redirect_url: {e}"))?;

    let client_id = ClientId::new(config.client_id.clone());
    let mut client_builder = BasicClient::new(client_id);
    if let Some(secret) = &config.client_secret
        && !secret.is_empty()
    {
        client_builder = client_builder.set_client_secret(ClientSecret::new(secret.clone()));
    }

    let client = client_builder
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    let pkce_verifier = session
        .pkce_verifier
        .take()
        .ok_or_else(|| "PKCE verifier already consumed".to_string())?;

    tracing::info!(provider_id = %session.provider_id, "开始 Token 交换");

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&inner.http_client)
        .await
        .map_err(|e| {
            tracing::error!(provider_id = %session.provider_id, "Token 交换失败: {e}");
            format!("Token 交换失败: {e}")
        })?;

    tracing::info!(provider_id = %session.provider_id, "Token 交换成功");

    Ok(TokenExchangeResult {
        access_token: token_response.access_token().secret().clone(),
        refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
        expires_in: token_response.expires_in().map(|d| d.as_secs() as i64),
        email: session.email,
        display_name: session.display_name,
        provider_id: session.provider_id,
    })
}

/// Token 交换内部结果（包含会话信息，仅 handle_callback 使用）
struct TokenExchangeResult {
    /// 访问令牌
    access_token: String,
    /// 刷新令牌（部分 provider 可能不返回）
    refresh_token: Option<String>,
    /// access_token 有效期（秒）
    expires_in: Option<i64>,
    /// 用户邮箱（来自会话）
    email: String,
    /// 可选显示名称（来自会话）
    display_name: Option<String>,
    /// 提供商标识（来自会话）
    provider_id: String,
}

/// SslMode → 数据库字符串
fn ssl_mode_to_string(mode: SslMode) -> String {
    match mode {
        SslMode::Implicit => "Implicit".to_string(),
        SslMode::StartTls => "StartTls".to_string(),
        SslMode::None => "None".to_string(),
    }
}

/// 简易 URL 解码
fn urldecode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let val = hex_val(hi) << 4 | hex_val(lo);
            result.push(val as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

/// 单个十六进制字节 → 数值（非十六进制字符返回 0），供 [`urldecode`] 解析 `%XX` 使用。
fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// 从 oauth2 TokenResponse 提取 DTO
fn to_token_result<TR: TokenResponse>(token_result: &TR) -> OAuth2TokenResult {
    OAuth2TokenResult {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
        expires_in: token_result.expires_in().map(|d| d.as_secs() as i64),
    }
}
