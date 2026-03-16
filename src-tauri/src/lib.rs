#![allow(dead_code, ambiguous_glob_reexports)]
mod config;
mod crypto;
mod database;
mod migration;
mod models;
pub mod services;

use anyhow::anyhow;
use sea_orm::DbConn;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_keyring::KeyringExt;
use url::Url;

// 时间处理

// 全局数据库连接（使用 Arc<Mutex<>> 实现共享）
struct DatabaseState(Arc<Mutex<DbConn>>);

impl DatabaseState {
    fn clone_conn(&self) -> DbConn {
        let guard = self.0.lock().unwrap_or_else(|e| {
            tracing::error!("数据库 Mutex 已被污染: {}", e);
            e.into_inner()
        });
        (*guard).clone()
    }
}

// Keyring 密钥环状态（存储 AppHandle 以便使用 keyring() 方法）
struct KeyringState {
    app_handle: tauri::AppHandle,
}

// OAuth 服务状态
struct OAuthState(services::oauth_service::OAuthService);

// ============================================================
// 账号管理 Commands
// ============================================================

#[tauri::command]
async fn add_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::create(&db, app_handle, account)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_accounts(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    services::account_service::get_all(&db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_account(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<Option<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    match services::account_service::get_by_id(&db, id).await {
        Ok(Some(account)) => Ok(Some(account.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn update_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::update(&db, app_handle, id, account)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
) -> Result<(), String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::delete(&db, app_handle, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_account_connection(
    _keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<services::imap::ConnectionTestResult, String> {
    let password = account.password.clone();

    // 获取服务器配置
    let host = account
        .imap_host
        .clone()
        .unwrap_or_else(|| match account.provider.as_str() {
            "gmail" => "imap.gmail.com".to_string(),
            "outlook" | "hotmail" => "outlook.office365.com".to_string(),
            "icloud" => "imap.mail.me.com".to_string(),
            "yahoo" => "imap.mail.yahoo.com".to_string(),
            _ => "imap.example.com".to_string(),
        });

    let port = account.imap_port.unwrap_or(993);

    let auth = services::imap::ImapAuth::Password(password);

    services::imap::test_connection(&host, port as u16, &account.email, auth)
        .await
        .map_err(|e| e.to_string())
}

/// 测试邮箱连接（用于添加账号前验证）
#[tauri::command]
async fn test_email_connection(
    email: String,
    password: String,
    provider: String,
    imap_host: Option<String>,
    imap_port: Option<u16>,
    _imap_ssl: Option<bool>,
    _smtp_host: Option<String>,
    _smtp_port: Option<u16>,
    _smtp_ssl: Option<bool>,
) -> Result<(), String> {
    // 获取服务器配置
    let host = imap_host.unwrap_or_else(|| match provider.as_str() {
        "gmail" => "imap.gmail.com".to_string(),
        "outlook" | "hotmail" => "outlook.office365.com".to_string(),
        "icloud" => "imap.mail.me.com".to_string(),
        "yahoo" => "imap.mail.yahoo.com".to_string(),
        _ => "imap.example.com".to_string(),
    });

    let port = imap_port.unwrap_or(993);

    let auth = services::imap::ImapAuth::Password(password);

    services::imap::test_connection(&host, port, &email, auth)
        .await
        .map_err(|e| format!("IMAP 连接失败: {}", e))?;

    Ok(())
}

// ============================================================
// OAuth 2.0 Commands
// ============================================================

/// 验证 OAuth Token 是否有效
#[tauri::command]
async fn validate_oauth_token(provider: String, token: String) -> Result<bool, String> {
    match provider.as_str() {
        "google" => {
            // 使用 Google UserInfo API 验证
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
            // 使用 Microsoft Graph API 验证
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

#[tauri::command]
async fn get_oauth_auth_url(
    state: tauri::State<'_, OAuthState>,
    provider: String,
) -> Result<(String, String), String> {
    // 返回 (auth_url, csrf_state)
    let oauth_service = &state.0;

    match provider.as_str() {
        "microsoft" => {
            let auth_context = oauth_service
                .get_microsoft_auth_url()
                .await
                .map_err(|e| e.to_string())?;

            Ok((auth_context.auth_url, auth_context.csrf_token))
        }
        "google" => Err("Google OAuth 暂未实现，请使用 Microsoft OAuth".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
async fn exchange_oauth_code(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    oauth_state: tauri::State<'_, OAuthState>,
    provider: String,
    code: String,
    csrf_state: String,
) -> Result<models::account::AccountDto, String> {
    let oauth_service = &oauth_state.0;

    match provider.as_str() {
        "microsoft" => {
            // 使用OAuth服务交换token
            let token = oauth_service
                .exchange_microsoft_code(&code, &csrf_state)
                .await
                .map_err(|e| e.to_string())?;

            // 从 id_token 中解析用户信息
            // id_token 是 JWT 格式，包含用户的 email 和 name
            // access_token 是不透明token，仅用于 SASL XOAUTH2 认证
            let id_token = token
                .id_token
                .as_ref()
                .ok_or_else(|| "未获取到 id_token，请确保 openid scope 已启用".to_string())?;
            let (email, display_name) =
                get_user_info_from_token(id_token).map_err(|e| e.to_string())?;

            tracing::info!(
                "从 token 解析用户信息: email={}, display_name={}",
                email,
                display_name
            );

            // 创建账号记录
            let db = db_state.clone_conn();
            let account_req = models::account::CreateAccountRequest {
                name: display_name,
                email: email.clone(),
                provider: "outlook".to_string(),
                password: String::new(), // OAuth不需要密码
                imap_host: Some("outlook.office365.com".to_string()),
                imap_port: Some(993),
                imap_ssl: Some(true),
                smtp_host: Some("smtp-mail.outlook.com".to_string()),
                smtp_port: Some(587),
                smtp_ssl: Some(true),
                color: Some("#0078D4".to_string()),
                auth_type: Some("oauth2".to_string()),
                oauth_provider: Some("microsoft".to_string()),
                oauth_token: Some(token.access_token.clone()),
                oauth_refresh_token: Some(token.refresh_token.clone()),
                oauth_expires_at: Some(token.expires_at),
            };

            let account =
                services::account_service::create(&db, &keyring_state.app_handle, account_req)
                    .await
                    .map_err(|e| e.to_string())?;

            tracing::info!("成功创建OAuth账号: {}", email);

            Ok(account.into())
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

/// 从 JWT access token 中解析用户信息
/// Microsoft 的 access token 是 JWT 格式，包含 email 和 name
fn get_user_info_from_token(access_token: &str) -> anyhow::Result<(String, String)> {
    tracing::info!("========== JWT Token 解析 ==========");
    tracing::info!("  token 长度: {}", access_token.len());
    tracing::info!(
        "  token 前100字符: {}",
        &access_token[..100.min(access_token.len())]
    );

    // JWT 格式: header.payload.signature
    let parts: Vec<&str> = access_token.split('.').collect();
    tracing::info!("  分段数量: {}", parts.len());

    if parts.len() != 3 {
        // 打印每段的长度用于调试
        for (i, part) in parts.iter().enumerate() {
            tracing::info!("  段{} 长度: {}", i, part.len());
        }
        return Err(anyhow!(
            "无效的 JWT token 格式，期望3段，实际{}段",
            parts.len()
        ));
    }

    // 解码 payload（第二部分）
    let payload = parts
        .get(1)
        .ok_or_else(|| anyhow!("JWT token 缺少 payload"))?;

    // Base64URL 解码
    let payload_json = base64_url_decode(payload)?;

    // 解析 JSON
    let claims: serde_json::Value =
        serde_json::from_str(&payload_json).map_err(|e| anyhow!("解析 JWT payload 失败: {}", e))?;

    // 提取 email 和 name
    // Microsoft 使用 "upn" (User Principal Name) 或 "email" 或 "unique_name"
    let email = claims
        .get("upn")
        .or_else(|| claims.get("email"))
        .or_else(|| claims.get("unique_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@example.com");

    // 提取显示名称
    let name = claims
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            // 如果没有 name，使用 email 的用户名部分
            email.split('@').next().unwrap_or("用户")
        });

    Ok((email.to_string(), name.to_string()))
}

/// Base64URL 解码（处理 JWT 的编码方式）
fn base64_url_decode(input: &str) -> anyhow::Result<String> {
    use base64::Engine;

    // Base64URL 需要添加 padding
    let input_padded = if input.len() % 4 == 0 {
        input.to_string()
    } else {
        let padding = "=".repeat(4 - (input.len() % 4));
        format!("{}{}", input, padding)
    };

    // 将 Base64URL 字符转换为标准 Base64
    let input_standard = input_padded.replace('-', "+").replace('_', "/");

    // 解码
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&input_standard)
        .map_err(|e| anyhow!("Base64 解码失败: {}", e))?;

    String::from_utf8(bytes).map_err(|e| anyhow!("UTF-8 转换失败: {}", e))
}

#[tauri::command]
async fn refresh_oauth_token(
    oauth_state: tauri::State<'_, OAuthState>,
    provider: String,
    refresh_token: String,
) -> Result<crypto::OAuthToken, String> {
    let oauth_service = &oauth_state.0;

    match provider.as_str() {
        "microsoft" => {
            // 使用OAuth服务刷新token
            let token = oauth_service
                .refresh_microsoft_token(&refresh_token)
                .await
                .map_err(|e| e.to_string())?;

            tracing::info!("成功刷新OAuth token");

            // 只返回 refresh_token 用于存储
            // access_token 由调用者直接使用，不需要存储到 Keyring
            Ok(crypto::OAuthToken {
                refresh_token: token.refresh_token,
                expires_at: token.expires_at,
            })
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

// ============================================================
// 邮件操作 Commands
// ============================================================

#[tauri::command]
async fn list_emails(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<services::email_service::EmailListResponse, String> {
    tracing::info!("📨 [list_emails] ========== 邮件列表请求 ==========");
    tracing::info!(
        "📨 [list_emails] 参数: account_id={}, folder='{}', page={}, limit={}",
        account_id,
        folder,
        page,
        limit
    );

    let db = state.clone_conn();

    let result = services::email_service::list(&db, account_id, &folder, page, limit)
        .await
        .map_err(|e| {
            tracing::error!("📨 [list_emails] ❌ 查询失败: {}", e);
            e.to_string()
        })?;

    tracing::info!(
        "📨 [list_emails] ✅ 查询成功: 返回 {} 封邮件，总计 {} 封",
        result.emails.len(),
        result.total
    );

    tracing::info!("📨 [list_emails] ========== 请求结束 ==========");

    Ok(result)
}

#[tauri::command]
async fn get_email(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<models::email::EmailDetail, String> {
    let db = state.clone_conn();
    services::email_service::get_detail(&db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_emails_fts(
    state: tauri::State<'_, DatabaseState>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<services::search_service::SearchResult>, String> {
    let db = state.clone_conn();
    services::search_service::SearchService::search_emails(&db, account_id, &query, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn mark_as_read(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    is_read: bool,
) -> Result<(), String> {
    let db = state.clone_conn();
    services::email_service::update_read_status(&db, email_id, is_read)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_star(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
) -> Result<bool, String> {
    let db = state.clone_conn();
    services::email_service::toggle_star(&db, email_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_emails(
    state: tauri::State<'_, DatabaseState>,
    email_ids: Vec<i32>,
) -> Result<usize, String> {
    let db = state.clone_conn();
    services::email_service::batch_delete(&db, email_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_email_to_folder(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    folder: String,
) -> Result<(), String> {
    let db = state.clone_conn();
    services::email_service::move_to_folder(&db, email_id, &folder)
        .await
        .map_err(|e| e.to_string())
}

/// 获取账号的文件夹统计数据
#[tauri::command]
async fn get_folder_stats(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<Vec<models::folder::FolderDto>, String> {
    let db = state.clone_conn();
    let folders = services::folder_service::get_by_account(&db, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folders.into_iter().map(|f| f.into()).collect())
}

// ============================================================
// 邮件同步 Commands
// ============================================================

/// 带进度的账号同步命令（使用 SyncManager）
#[tauri::command]
async fn sync_account_with_progress(
    db_state: tauri::State<'_, DatabaseState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), String> {
    let db = std::sync::Arc::new(db_state.clone_conn());

    // 创建 SyncManager
    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle.clone());

    // 执行同步
    let result = sync_manager
        .sync_account(account_id)
        .await
        .map_err(|e| format!("同步失败: {}", e))?;

    tracing::info!(
        "同步完成: 账号 {}, 同步了 {} 封邮件, {} 个文件夹, {} 个错误, 耗时 {}ms",
        account_id,
        result.total_synced,
        result.folders_synced,
        result.errors,
        result.duration_ms
    );

    Ok(())
}

/// 同步账号（简化版，不发送进度事件）
#[tauri::command]
async fn sync_account(
    db_state: tauri::State<'_, DatabaseState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<usize, String> {
    let db = std::sync::Arc::new(db_state.clone_conn());

    // 创建 SyncManager
    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle);

    // 执行同步
    let result = sync_manager
        .sync_account(account_id)
        .await
        .map_err(|e| format!("同步失败: {}", e))?;

    Ok(result.total_synced)
}

#[tauri::command]
async fn send_email(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    let db = db_state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 从 Keyring 获取密码
    let username = crypto::password_username(request.account_id);
    let keyring = keyring_state.app_handle.keyring();
    let password = keyring
        .get_password(crypto::KEYRING_SERVICE, &username)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "密码未找到".to_string())?;

    // 连接 SMTP 并发送
    let mut smtp_service = services::smtp_service::SmtpService::new();

    let host = account.smtp_host.unwrap_or_else(|| {
        match account.provider.as_str() {
            "gmail" => "smtp.gmail.com",
            "outlook" | "hotmail" => "smtp-mail.outlook.com",
            "icloud" => "smtp.mail.me.com",
            "yahoo" => "smtp.mail.yahoo.com",
            _ => "smtp.example.com",
        }
        .to_string()
    });

    let port = account.smtp_port.unwrap_or(587) as u16;

    smtp_service
        .connect(
            &host,
            port,
            &account.email,
            services::smtp_service::SmtpAuth::Password(password),
        )
        .map_err(|e| e.to_string())?;

    let to_addresses: Vec<String> = request.to.iter().map(|a| a.email.clone()).collect();

    let message_id = smtp_service
        .send_email(
            &account.email,
            to_addresses,
            &request.subject,
            &request.body_html,
            request.body_text.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    tracing::info!("邮件已发送: {}", message_id);

    Ok(message_id)
}

// ============================================================
// 旧的测试命令
// ============================================================

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// 处理 OAuth Deep Link 回调
/// 解析 URL 中的 code, state, error 参数并发射事件到前端
fn handle_oauth_deep_link(app: &tauri::AppHandle, url: &str) {
    tracing::info!("收到 Deep Link: {}", url);

    // 解析 URL
    let parsed_url = match Url::parse(url) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("解析 Deep Link URL 失败: {}", e);
            return;
        }
    };

    // 检查是否是 OAuth 回调
    // URL 格式: postium-mail://oauth/callback?code=xxx&state=yyy
    let host = parsed_url.host_str().unwrap_or("");
    let path = parsed_url.path();

    if host != "oauth" || path != "/callback" {
        tracing::warn!("忽略非 OAuth Deep Link: host={}, path={}", host, path);
        return;
    }

    // 提取查询参数
    let query_params: std::collections::HashMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let code = query_params.get("code").cloned().unwrap_or_default();
    let state = query_params.get("state").cloned().unwrap_or_default();
    let error = query_params.get("error").cloned();
    let error_description = query_params.get("error_description").cloned();

    tracing::info!(
        "OAuth Deep Link 参数: code={}, state={}, error={:?}",
        if code.is_empty() { "无" } else { "有" },
        if state.is_empty() { "无" } else { "有" },
        error
    );

    // 将主窗口带到前台
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.unminimize();
        let _ = window.show();
    }

    // 发射事件到前端
    let emit_result = if let Some(err) = error {
        app.emit(
            "oauth-deep-link-callback",
            serde_json::json!({
                "error": err,
                "errorDescription": error_description.unwrap_or_default()
            }),
        )
    } else {
        app.emit(
            "oauth-deep-link-callback",
            serde_json::json!({
                "code": code,
                "state": state
            }),
        )
    };

    if let Err(e) = emit_result {
        tracing::error!("发射 OAuth Deep Link 事件失败: {}", e);
    }
}

// ============================================================
// 应用入口
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        // 单实例插件 - 在 deep-link 之前初始化，以便转发 deep link URL 到主实例
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("单实例检测：收到参数 {:?}", args);

            // 检查参数中是否包含 deep link URL
            for arg in args {
                if arg.starts_with("postium-mail://") {
                    tracing::info!("单实例转发 Deep Link: {}", arg);
                    handle_oauth_deep_link(app, &arg);
                    return;
                }
            }

            // 如果不是 deep link，只是将窗口带到前台
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
                let _ = window.show();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        // Keyring 插件（系统原生密钥链）
        .plugin(tauri_plugin_keyring::init())
        .setup(|app| {
            // ========== 系统托盘初始化 ==========
            // 创建托盘菜单项
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            // 创建托盘菜单
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.unminimize();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击托盘图标显示窗口
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.unminimize();
                        }
                    }
                })
                .build(app)?;

            // ========== 窗口关闭事件处理（隐藏到托盘而非退出） ==========
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // 阻止默认关闭行为
                        api.prevent_close();
                        // 隐藏窗口到托盘
                        let _ = window_clone.hide();
                        tracing::info!("窗口已隐藏到系统托盘");
                    }
                });
            }

            tracing::info!("系统托盘初始化完成");

            // ========== Deep Link 事件处理器 ==========
            // 注册 Deep Link 事件处理器
            let app_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_oauth_deep_link(&app_handle, url.as_str());
                }
            });

            // 在 Windows 上注册协议 scheme（开发模式下需要）
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = app.deep_link().register("postium-mail") {
                    tracing::warn!("注册 Deep Link 协议失败（可能需要管理员权限）: {}", e);
                    tracing::warn!("请尝试以管理员身份运行应用，或者手动注册协议");
                } else {
                    tracing::info!("Deep Link 协议注册成功");
                }
            }

            // 应用启动时初始化数据库和 Keyring
            tauri::async_runtime::block_on(async move {
                // 建立数据库连接
                let db = database::establish_connection()
                    .await
                    .expect("无法连接到数据库");

                // 初始化数据库表
                database::init_database(&db)
                    .await
                    .expect("数据库初始化失败");

                // 将数据库连接存储到应用状态中
                app.manage(DatabaseState(Arc::new(Mutex::new(db))));

                // 初始化 OAuth 服务
                let oauth_config = config::load_oauth_config().expect("无法加载OAuth配置");
                let oauth_service = services::oauth_service::OAuthService::new(oauth_config)
                    .expect("无法初始化OAuth服务");
                app.manage(OAuthState(oauth_service));

                // 存储 AppHandle 到 KeyringState，用于后续访问 keyring
                app.manage(KeyringState {
                    app_handle: app.handle().clone(),
                });

                tracing::info!("Postium Mail 后端初始化完成");
                tracing::info!("密码存储: 操作系统原生密钥链 (Tauri Plugin Keyring)");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号管理
            add_account,
            list_accounts,
            get_account,
            update_account,
            delete_account,
            test_account_connection,
            test_email_connection,
            // OAuth 2.0
            validate_oauth_token,
            get_oauth_auth_url,
            exchange_oauth_code,
            refresh_oauth_token,
            // 邮件操作
            list_emails,
            get_email,
            search_emails_fts,
            mark_as_read,
            toggle_star,
            delete_emails,
            move_email_to_folder,
            get_folder_stats,
            // 邮件同步
            sync_account,
            sync_account_with_progress,
            send_email,
            // 测试
            greet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
