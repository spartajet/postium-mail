//! OAuth 认证 Commands
//!
//! 提供基于 OAuth 2.0 的邮件账号认证功能，包括：
//! - 生成 OAuth 授权 URL
//! - 交换授权码获取访问令牌
//! - 刷新访问令牌
//! - 验证令牌有效性
//!
//! # 支持的服务商
//!
//! - Google (Gmail)
//! - Microsoft (Outlook, Hotmail, Office 365)
//!
//! # OAuth 认证流程
//!
//! 1. 调用 `get_oauth_auth_url` 获取授权 URL 和 state
//! 2. 在浏览器中打开授权 URL
//! 3. 用户授权后，收到回调（包含 code 和 state）
//! 4. 调用 `exchange_oauth_code` 交换授权码，创建账号
//! 5. 后续可使用 `refresh_oauth_token` 刷新令牌

use super::{AuthManagerState, DatabaseState, KeyringState, OAuthSessionManagerState};
use crate::auth::OAuthSessionStatus;
use crate::storage;
use crate::providers;
use serde::{Deserialize, Serialize};

/// 启动 OAuth 流程响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartOAuthFlowResponse {
    /// 会话 ID
    pub session_id: String,
    /// 授权 URL（已通过系统浏览器打开）
    pub auth_url: String,
}

/// OAuth 流程完成事件
///
/// 后端处理完 Deep Link 回调后发射此事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthFlowResult {
    /// 会话 ID
    pub session_id: String,
    /// 流程状态: "success" | "error"
    pub status: String,
    /// 创建的账号（成功时）
    pub account: Option<storage::AccountDto>,
    /// 错误信息（失败时）
    pub error: Option<String>,
}

/// 验证 OAuth 令牌的有效性
///
/// 通过调用服务商的用户信息 API 验证访问令牌是否仍然有效。
///
/// # 参数
/// * `provider` - OAuth 提供商标识（"google" 或 "microsoft"）
/// * `token` - OAuth 访问令牌
///
/// # 返回
/// - `Ok(true)`: 令牌有效
/// - `Ok(false)`: 令牌无效或已过期
/// - `Err(String)`: 验证过程出错（如网络错误、不支持的服务商）
///
/// # 验证方式
///
/// 根据不同的服务商使用不同的 API：
/// - **Google**: 调用 `https://www.googleapis.com/oauth2/v3/userinfo`
/// - **Microsoft**: 调用 `https://graph.microsoft.com/v1.0/me`
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::validate_oauth_token;
///
/// let is_valid = validate_oauth_token("google".to_string(), "ya29.a0Af...".to_string()).await?;
/// if is_valid {
///     println!("令牌有效");
/// } else {
///     println!("令牌无效或已过期");
/// }
/// ```
#[tauri::command]
pub async fn validate_oauth_token(
    provider: String,
    token: String,
) -> Result<bool, String> {
    match provider.as_str() {
        "google" => {
            let client = reqwest::Client::new();
            let response = client
                .get("https://www.googleapis.com/oauth2/v3/userinfo")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;
            Ok(response.status().is_success())
        }
        "microsoft" => {
            let client = reqwest::Client::new();
            let response = client
                .get("https://graph.microsoft.com/v1.0/me")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;
            Ok(response.status().is_success())
        }
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

/// 获取 OAuth 授权 URL
///
/// 为指定的邮箱地址生成 OAuth 授权 URL 和用于防止 CSRF 攻击的 state 参数。
///
/// # 参数
/// * `auth_manager_state` - AuthManager 状态，用于生成授权 URL
/// * `email` - 用户邮箱地址，用于自动检测邮件服务商
///
/// # 返回
/// 成功时返回元组 `(auth_url, state)`：
/// - `auth_url`: OAuth 授权 URL，用户需要在浏览器中打开
/// - `state`: 用于防止 CSRF 攻击的随机字符串，回调时需要验证
///
/// 失败时返回错误信息字符串
///
/// # 授权 URL 使用流程
///
/// 1. 调用此命令获取授权 URL 和 state
/// 2. 在浏览器中打开 `auth_url`
/// 3. 用户登录并授权应用访问邮件
/// 4. 服务商重定向到回调 URL，附带 `code` 和 `state`
/// 5. 使用收到的 `code` 和 `state` 调用 `exchange_oauth_code`
///
/// # 自动检测服务商
///
/// 根据 `email` 的域名自动检测服务商：
/// - `@gmail.com` → Google
/// - `@outlook.com`, `@hotmail.com` → Microsoft
/// - 其他域名 → 尝试通过 MX 记录检测
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::get_oauth_auth_url;
///
/// let (auth_url, state) = get_oauth_auth_url(auth_manager_state, "user@gmail.com".to_string()).await?;
/// // 在浏览器中打开 auth_url
/// open::that(auth_url)?;
/// // 保存 state，用于后续回调验证
/// ```
#[tauri::command]
pub async fn get_oauth_auth_url(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
) -> Result<(String, String), String> {
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 生成 OAuth 授权 URL
    let context = auth_manager
        .get_oauth_url(&email)
        .await
        .map_err(|e| e.to_string())?;

    Ok((context.auth_url, context.state))
}

/// 交换 OAuth 授权码并创建账号
///
/// 使用 OAuth 授权码交换访问令牌，并自动创建邮件账号。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `keyring_state` - 密钥链状态，用于存储令牌
/// * `auth_manager_state` - AuthManager 状态，用于处理 OAuth 认证
/// * `email` - 用户邮箱地址
/// * `code` - OAuth 授权码（从回调中获取）
/// * `state` - OAuth state 参数（从回调中获取，用于验证）
///
/// # 返回
/// 成功时返回创建的账号对象（AccountDto），包含：
/// - 账号基本信息（ID、名称、邮箱等）
/// - 服务器配置
/// - OAuth 令牌信息
///
/// 失败时返回错误信息字符串
///
/// # 处理流程
///
/// 1. 使用 `code` 和 `state` 通过 AuthManager 进行 OAuth 认证
/// 2. 检测邮件服务商并获取默认服务器配置
/// 3. 构建账号创建请求
/// 4. 在数据库中创建账号
/// 5. 将 OAuth 令牌从临时账户迁移到实际账户
///
/// # 自动配置
///
/// - 名称：优先使用 OAuth 返回的显示名称，否则使用邮箱用户名部分
/// - 服务器配置：根据服务商自动填充 IMAP/SMTP 配置
/// - 颜色：默认使用蓝色 (#0078D4)
/// - 认证类型：设置为 OAuth2
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::exchange_oauth_code;
///
/// // 收到 OAuth 回调后
/// let account = exchange_oauth_code(
///     db_state,
///     keyring_state,
///     auth_manager_state,
///     "user@gmail.com".to_string(),
///     "4/0Aa...".to_string(),  // 授权码
///     "abc123...".to_string(), // state
/// ).await?;
///
/// println!("成功创建账号: {}", account.email);
/// ```
#[tauri::command]
pub async fn exchange_oauth_code(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
    code: String,
    state: String,
) -> Result<storage::AccountDto, String> {
    let db = db_state.clone_conn();
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 进行 OAuth 认证
    let auth_result = auth_manager
        .authenticate_oauth(&email, &code, &state)
        .await
        .map_err(|e| e.to_string())?;

    // 获取服务商配置
    let provider_pool = auth_manager.provider_pool();
    let provider = provider_pool
        .detect_provider(&email)
        .await
        .map_err(|e| e.to_string())?;

    let imap_config = provider.imap_config(&email);
    let smtp_config = provider.smtp_config(&email);
    let provider_info = provider.provider_info();

    // 构建账号创建请求
    let account_req = storage::CreateAccountRequest {
        name: auth_result.display_name.unwrap_or_else(|| {
            email.split('@')
                .next()
                .unwrap_or("用户")
                .to_string()
        }),
        email: auth_result.email.clone(),
        provider: provider_info.id.clone(),
        password: String::new(),
        imap_host: Some(imap_config.host),
        imap_port: Some(imap_config.port as i32),
        imap_ssl: Some(matches!(imap_config.ssl, providers::SslMode::Implicit | providers::SslMode::StartTls)),
        smtp_host: Some(smtp_config.host),
        smtp_port: Some(smtp_config.port as i32),
        smtp_ssl: Some(matches!(smtp_config.ssl, providers::SslMode::StartTls)),
        color: Some("#0078D4".to_string()), // 默认颜色
        auth_type: Some("oauth2".to_string()),
        oauth_provider: Some(provider_info.id.clone()),
        oauth_token: auth_result.id_token,
        oauth_refresh_token: Some(
            // 注意：TokenManager 已经存储了 refresh_token，这里只是为了兼容
            // 实际使用时应该从 TokenManager 读取
            String::new()
        ),
        oauth_expires_at: auth_result.expires_at,
    };

    // 创建账号
    let account = storage::AccountRepository::create(
        &db,
        &keyring_state.app_handle,
        account_req,
    )
    .await
    .map_err(|e| e.to_string())?;

    // 迁移 Token 从临时 account_id (0) 到实际的 account_id
    let token_manager = auth_manager.token_manager();
    token_manager
        .migrate_token_account(0, account.id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("成功创建 OAuth 账号: {}", email);

    Ok(account.into())
}

/// 刷新 OAuth 访问令牌
///
/// 使用刷新令牌获取新的访问令牌。
///
/// # 参数
/// * `auth_manager_state` - AuthManager 状态
/// * `email` - 用户邮箱地址（用于识别服务商）
/// * `account_id` - 账号 ID（用于查找存储的刷新令牌）
///
/// # 返回
/// 成功时返回新的 OAuth 令牌信息（OAuthToken），包含：
/// - `access_token`: 新的访问令牌
/// - `expires_in`: 过期时间（秒）
/// - `refresh_token`: 刷新令牌（可能已更新）
/// - `id_token`: ID 令牌（可选）
///
/// 失败时返回错误信息字符串
///
/// # 刷新流程
///
/// 1. 从 TokenManager 中获取存储的刷新令牌
/// 2. 向服务商的令牌端点发送刷新请求
/// 3. 保存新获取的令牌
/// 4. 返回新的令牌信息
///
/// # 何时需要刷新
///
/// - 当 API 调用返回 401 未授权错误时
/// - 在访问令牌即将过期前主动刷新（建议提前 5 分钟）
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::refresh_oauth_token;
///
/// match refresh_oauth_token(auth_manager_state, "user@gmail.com".to_string(), 1).await {
///     Ok(token) => println!("令牌已刷新: {}", token.access_token),
///     Err(e) => println!("刷新失败: {}", e),
/// }
/// ```
#[tauri::command]
pub async fn refresh_oauth_token(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
    account_id: i32,
) -> Result<crate::crypto::OAuthToken, String> {
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 刷新 token
    auth_manager
        .refresh_token(account_id, &email)
        .await
        .map_err(|e| e.to_string())?;

    // 获取新的 token 信息
    let token_manager = auth_manager.token_manager();
    let token = token_manager
        .get_oauth_token(account_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("成功刷新 OAuth token");

    Ok(token)
}

/// 启动 OAuth 流程
///
/// 创建 OAuth 会话、生成授权 URL，并在系统浏览器中打开授权页面。
///
/// # 参数
/// * `auth_manager_state` - AuthManager 状态
/// * `session_manager_state` - OAuthSessionManager 状态
/// * `email` - 用户邮箱地址，用于自动检测服务商
///
/// # 返回
/// 成功时返回会话信息和授权 URL
///
/// # 流程
/// 1. 检测服务商并生成授权 URL
/// 2. 创建 OAuth 会话（记录 state 和过期时间）
/// 3. 在系统浏览器中打开授权页面
/// 4. 用户授权后，Deep Link 回调将触发后续流程
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::start_oauth_flow;
///
/// let result = start_oauth_flow(
///     auth_manager_state,
///     session_manager_state,
///     "user@gmail.com".to_string(),
/// ).await?;
///
/// println!("会话 ID: {}", result.session_id);
/// // 浏览器已自动打开授权页面
/// ```
#[tauri::command]
pub async fn start_oauth_flow(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    session_manager_state: tauri::State<'_, OAuthSessionManagerState>,
    email: String,
) -> Result<StartOAuthFlowResponse, String> {
    let auth_manager = auth_manager_state.clone_manager();
    let session_manager = &session_manager_state.0;

    // 1. 生成授权 URL 和 state
    let context = auth_manager
        .get_oauth_url(&email)
        .await
        .map_err(|e| e.to_string())?;

    // 2. 创建会话
    let provider = auth_manager
        .provider_pool()
        .detect_provider(&email)
        .await
        .map_err(|e| e.to_string())?;

    let provider_info = provider.provider_info();
    let session_id = session_manager
        .create_session(&provider_info.id, &email, &context.state)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 在浏览器打开 URL
    tauri_plugin_opener::open_url(&context.auth_url, None::<&str>)
        .map_err(|e| format!("无法打开浏览器: {}", e))?;

    tracing::info!(
        "启动 OAuth 流程: session_id={}, email={}, provider={}",
        session_id,
        email,
        provider_info.id
    );

    Ok(StartOAuthFlowResponse {
        session_id,
        auth_url: context.auth_url,
    })
}

/// 取消 OAuth 流程
///
/// 标记指定的 OAuth 会话为失败状态。
///
/// # 参数
/// * `session_manager_state` - OAuthSessionManager 状态
/// * `session_id` - 会话 ID
///
/// # 示例
/// ```rust,no_run
/// use crate::command::oauth::cancel_oauth_flow;
///
/// cancel_oauth_flow(session_manager_state, "uuid-xxx".to_string()).await?;
/// ```
#[tauri::command]
pub async fn cancel_oauth_flow(
    session_manager_state: tauri::State<'_, OAuthSessionManagerState>,
    session_id: String,
) -> Result<(), String> {
    let session_manager = &session_manager_state.0;

    session_manager
        .update_session_status(&session_id, OAuthSessionStatus::Failed)
        .await
        .map_err(|e| e.to_string())?;

    session_manager
        .set_session_error(&session_id, "用户取消授权".to_string())
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("取消 OAuth 流程: session_id={}", session_id);

    Ok(())
}
