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
    pub url: String,
    pub state: String,
    pub port: u16,
}

/// OAuth2 Token 结果（仅内部使用，不暴露给前端）
#[derive(Debug, Clone)]
pub struct OAuth2TokenResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

/// OAuth2 授权完成信息（返回给前端的最小信息）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuth2CompletedInfo {
    pub email: String,
}

/// OAuth2 轮询状态
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum OAuth2PollResult {
    Pending,
    Completed(OAuth2CompletedInfo),
    Error(String),
}

/// 正在进行中的 OAuth2 会话
struct PendingSession {
    pkce_verifier: Option<PkceCodeVerifier>,
    port: u16,
    provider_id: String,
    email: String,
    display_name: Option<String>,
}

/// 内部共享状态
struct Inner {
    sessions: Mutex<HashMap<String, PendingSession>>,
    completed: Mutex<HashMap<String, OAuth2PollResult>>,
    provider_pool: Arc<ProviderPool>,
    http_client: reqwest::Client,
    auth_manager: Arc<AuthManager>,
    db: DbConn,
}

/// OAuth2 PKCE 流程管理器
pub struct OAuth2Manager {
    inner: Arc<Inner>,
}

impl OAuth2Manager {
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
        // Microsoft 的 token 端点要求 client_id 在请求体中，不接受仅靠 Basic Auth 传递。
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
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    email: String,
    display_name: Option<String>,
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
