//! OAuth HTTP 回调服务器
//!
//! 使用 hyper 监听 http://localhost:PORT/callback 处理 OAuth 回调

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::auth::http::callback_handler::{OAuthCallbackHandler, url_decode};
use crate::command::{AuthManagerState, DatabaseState, KeyringState, OAuthSessionManagerState};
use crate::storage;

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

/// OAuth HTTP 服务器
pub struct OAuthHttpServer {
    /// 服务器端口
    port: u16,
    /// 是否正在运行
    running: Arc<Mutex<bool>>,
    /// Tauri AppHandle
    app_handle: AppHandle,
}

impl OAuthHttpServer {
    /// 创建新的 OAuth HTTP 服务器
    pub fn new(port: u16, app_handle: AppHandle) -> Self {
        Self {
            port,
            running: Arc::new(Mutex::new(false)),
            app_handle,
        }
    }

    /// 启动服务器
    pub async fn start(&self) -> Result<(), anyhow::Error> {
        let mut running = self.running.lock().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        // 绑定地址 (仅本地)
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("OAuth HTTP 服务器启动: http://{}", addr);

        let app_handle = self.app_handle.clone();
        let running_flag = Arc::clone(&self.running);

        // 启动服务器任务
        tokio::spawn(async move {
            // 接受连接
            loop {
                // 检查是否应该停止
                {
                    let flag = running_flag.lock().await;
                    if !*flag {
                        tracing::info!("OAuth HTTP 服务器停止");
                        break;
                    }
                }

                // 接受新连接
                match listener.accept().await {
                    Ok((stream, _remote_addr)) => {
                        let app_handle = app_handle.clone();

                        // 为每个连接启动处理任务
                        tokio::spawn(async move {
                            let io = TokioIo::new(stream);

                            // 处理 HTTP 请求
                            if let Err(err) = http1::Builder::new()
                                .serve_connection(
                                    io,
                                    service_fn(|req| handle_request(req, app_handle.clone())),
                                )
                                .await
                            {
                                tracing::debug!("HTTP 连接错误: {}", err);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("接受连接失败: {}", e);
                        // 检查是否应该停止
                        let flag = running_flag.lock().await;
                        if !*flag {
                            break;
                        }
                    }
                }
            }

            let mut flag = running_flag.lock().await;
            *flag = false;
        });

        Ok(())
    }

    /// 停止服务器
    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
        // 连接到服务器以解除 accept() 阻塞
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", self.port))
            .await
            .is_err()
        {
            // 忽略连接错误
        }
    }

    /// 检查服务器是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// 获取回调 URL
    pub fn get_callback_url(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }
}

/// 处理 HTTP 请求
async fn handle_request(
    req: Request<Incoming>,
    app_handle: tauri::AppHandle,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    tracing::debug!("收到 HTTP 请求: {} {}", method, path);

    // 只处理 GET /callback
    if method != Method::GET || path != "/callback" {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::new()))
            .unwrap());
    }

    // 解析查询参数并进行 URL 解码
    let query_params: std::collections::HashMap<String, String> = uri
        .query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            let key = parts.next()?.to_string();
            let value = parts.next().unwrap_or("");
            // URL 解码
            let decoded_value = url_decode(value);
            Some((key, decoded_value))
        })
        .collect();

    let code = query_params.get("code").cloned().unwrap_or_default();
    let state = query_params.get("state").cloned().unwrap_or_default();
    let error = query_params.get("error").cloned();
    let error_description = query_params.get("error_description").cloned();

    // 打印详细的回调参数（用于调试）
    tracing::info!("========== OAuth HTTP 回调 ==========");
    tracing::info!(
        "Code (前20字符): {}",
        &code.chars().take(20).collect::<String>()
    );
    tracing::info!("Code 长度: {}", code.len());
    tracing::info!("State: {}", state);
    tracing::info!("Error: {:?}", error);
    tracing::info!("Error Description: {:?}", error_description);
    tracing::info!("=====================================");

    handle_oauth_http_callback(
        &app_handle,
        &code,
        &state,
        error.as_deref(),
        error_description.as_deref(),
    );

    // 返回 HTML 响应
    let html = if error.is_some() {
        // 错误页面
        HTML_ERROR_PAGE.replace(
            "{error}",
            error_description.as_deref().unwrap_or("未知错误"),
        )
    } else {
        // 成功页面
        HTML_SUCCESS_PAGE.to_string()
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(html)))
        .unwrap())
}

/// 处理 OAuth HTTP 回调
fn handle_oauth_http_callback(
    app_handle: &AppHandle,
    code: &str,
    state: &str,
    error: Option<&str>,
    error_description: Option<&str>,
) {
    // 显示主窗口并聚焦
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.unminimize();
        let _ = window.show();
    }

    // 获取状态并克隆 Arc（确保 async block 中拥有所有权）
    let session_manager: Arc<crate::auth::session::OAuthSessionManager> =
        Arc::clone(&app_handle.state::<OAuthSessionManagerState>().0);
    let auth_manager = app_handle.state::<AuthManagerState>().clone_manager();
    let callback_handler = Arc::new(OAuthCallbackHandler::new(auth_manager));

    // 从 State<'_, DatabaseState> 中提取并克隆内部的 Arc
    let db_arc = Arc::clone(&app_handle.state::<DatabaseState>().0);
    let db_state = DatabaseState(db_arc);

    // KeyringState 包含 AppHandle，可以直接克隆
    let keyring_state = KeyringState {
        app_handle: app_handle.clone(),
    };

    // 获取 HTTP Server 用于处理完成后关闭
    let http_server = callback_handler.auth_manager().get_http_server().clone();
    let app_handle_clone = app_handle.clone();
    let code_clone = code.to_string();
    let error_description_clone = error_description.map(|s| s.to_string());
    let error_clone = error.map(|s| s.to_string());
    let session_manager_for_shutdown = Arc::clone(&session_manager);
    let state_clone = state.to_string();

    // 在异步运行时中处理
    tauri::async_runtime::spawn(async move {
        // 验证会话
        let session = match session_manager_for_shutdown
            .verify_and_get_session(&state_clone)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("验证 OAuth 会话失败: {}", e);
                // 发射错误事件
                let _ = app_handle_clone.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id: String::new(),
                        status: "error".to_string(),
                        account: None,
                        error: Some(format!("无效的会话: {}", e)),
                    },
                );

                // 关闭 HTTP Server
                http_server.stop().await;
                tracing::info!("OAuth HTTP 服务器已关闭（会话验证失败）");
                return;
            }
        };

        let session_id = session.session_id.clone();

        // 如果有错误，标记会话失败
        if let Some(err) = error_clone {
            tracing::warn!("OAuth 授权失败: {}", err);
            let error_msg = error_description_clone.as_deref().unwrap_or(&err);
            let _ = session_manager
                .set_session_error(&session_id, error_msg.to_string())
                .await;

            let _ = app_handle_clone.emit(
                "oauth-flow-complete",
                OAuthFlowResult {
                    session_id: session_id.clone(),
                    status: "error".to_string(),
                    account: None,
                    error: error_description_clone.map(|s| s.to_string()),
                },
            );
            return;
        }

        // 交换 token 并创建账号
        let email = session.email.clone();

        match callback_handler
            .handle_callback(
                db_state,
                keyring_state,
                &email,
                &code_clone,
                &state_clone,
            )
            .await
        {
            Ok(account) => {
                tracing::info!("OAuth 流程成功创建账号: {}", account.email);

                // 更新会话状态
                let _ = session_manager
                    .set_session_account_id(&session_id, account.id)
                    .await;

                // 发射成功事件
                let _ = app_handle_clone.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id,
                        status: "success".to_string(),
                        account: Some(account),
                        error: None,
                    },
                );
            }
            Err(e) => {
                tracing::error!("OAuth 流程失败: {}", e);

                // 更新会话状态
                let _ = session_manager
                    .set_session_error(&session_id, e.clone())
                    .await;

                // 发射错误事件
                let _ = app_handle_clone.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id,
                        status: "error".to_string(),
                        account: None,
                        error: Some(e),
                    },
                );
            }
        }
    });
}

/// OAuth 授权成功页面
const HTML_SUCCESS_PAGE: &str = r##"
<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>认证成功</title>
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100vh;
    margin: 0;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
}
.container {
    text-align: center;
    padding: 50px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 24px;
    backdrop-filter: blur(20px);
    box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37);
    max-width: 500px;
    margin: 20px;
}
.icon {
    font-size: 64px;
    margin-bottom: 20px;
    animation: checkmark 0.6s ease-in-out;
}
@keyframes checkmark {
    0% { transform: scale(0); opacity: 0; }
    50% { transform: scale(1.2); }
    100% { transform: scale(1); opacity: 1; }
}
h1 {
    margin: 0 0 16px 0;
    font-size: 28px;
    font-weight: 600;
}
.message {
    margin: 0 0 12px 0;
    font-size: 16px;
    opacity: 0.95;
    line-height: 1.5;
}
.sub-message {
    margin: 0 0 32px 0;
    font-size: 14px;
    opacity: 0.85;
}
.info {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 16px;
    margin: 24px 0;
    font-size: 13px;
    line-height: 1.6;
    opacity: 0.9;
}
.countdown {
    font-size: 12px;
    opacity: 0.7;
    margin-top: 20px;
}
</style>
</head>
<body>
<div class="container">
<div class="icon">✓</div>
<h1>认证成功</h1>
<p class="message">您的账号已成功添加</p>
<p class="sub-message">现在可以返回应用继续操作</p>

<div class="info">
    💡 提示：此窗口可以关闭了<br>
    应用已经收到您的账号信息
</div>

<p class="countdown" id="countdown"></p>
</div>
<script>
// 尝试自动关闭窗口（对通过 window.open 打开的窗口有效）
let countdown = 5;
const countdownEl = document.getElementById('countdown');

function updateCountdown() {
    if (countdown > 0) {
        countdownEl.textContent = '窗口将在 ' + countdown + ' 秒后自动关闭';
        countdown--;
        setTimeout(updateCountdown, 1000);
    } else {
        try {
            window.close();
            // 如果无法关闭，显示手动关闭提示
            countdownEl.textContent = '请点击上方按钮手动关闭窗口';
        } catch (e) {
            console.log('无法自动关闭窗口:', e);
        }
    }
}

// 开始倒计时
setTimeout(updateCountdown, 1000);
</script>
</body>
</html>
    "##;

/// OAuth 授权失败页面
const HTML_ERROR_PAGE: &str = r##"
<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>OAuth 认证失败</title>
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100vh;
    margin: 0;
    background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);
    color: white;
}
.container {
    text-align: center;
    padding: 40px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    backdrop-filter: blur(10px);
}
h1 { margin-bottom: 20px; }
p { opacity: 0.9; }
</style>
</head>
<body>
<div class="container">
    <div style="font-size: 64px; margin-bottom: 20px;">✕</div>
    <h1>认证失败</h1>
    <p>{error}</p>
    <p style="font-size: 14px; margin-top: 20px; opacity: 0.8;">
        请关闭此窗口返回应用重试
    </p>
</div>
</body>
</html>
    "##;

