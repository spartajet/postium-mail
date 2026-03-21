//! 统一认证命令
//!
//! 提供统一的认证接口，支持 OAuth 和密码认证

use super::{AuthManagerState, DatabaseState, KeyringState};
use crate::auth::{AuthInfo, AuthResponse};
use crate::storage;

/// 统一认证命令
///
/// 根据认证信息类型，自动选择合适的认证方式：
/// - OAuth: 启动授权流程，返回会话 ID
/// - 密码: 验证凭证，创建账号，返回账号信息
///
/// # 参数
///
/// * `auth_manager_state` - AuthManager 状态
/// * `db_state` - 数据库状态
/// * `keyring_state` - Keyring 状态
/// * `auth_info` - 认证信息
///
/// # 返回
///
/// - OAuth: 返回 Pending 状态（包含 session_id 和 auth_url）
/// - 密码: 返回 Success 状态（包含创建的账号信息）
/// - 失败: 返回 Error 状态（包含错误信息）
///
/// # 示例
///
/// ```javascript
/// // OAuth 认证
/// const authInfo = { type: "OauthConfig", email: "user@example.com" };
/// const response = await invoke("start_auth_command", { authInfo });
/// if (response.status === "Pending") {
///     // 等待 OAuth 回调
/// }
///
/// // 密码认证
/// const authInfo = {
///     type: "ImapSmtpConfig",
///     email: "user@example.com",
///     password: "password123"
/// };
/// const response = await invoke("start_auth_command", { authInfo });
/// if (response.status === "Success") {
///     // 账号创建成功
/// }
/// ```
#[tauri::command]
pub async fn start_auth_command(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    auth_info: AuthInfo,
) -> std::result::Result<AuthResponse, String> {
    let auth_manager = auth_manager_state.clone_manager();
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    // 对于密码认证，需要提取创建账号所需的信息
    let (name, color) = match &auth_info {
        AuthInfo::ImapSmtpConfig { name, color, .. } => (name.clone(), color.clone()),
        AuthInfo::OauthConfig { .. } => (None, None),
    };

    // 统一调用 start_auth，不区分认证类型
    let response = auth_manager
        .start_auth(&auth_info)
        .await
        .map_err(|e| e.to_string())?;

    // 根据响应决定下一步操作
    match response {
        AuthResponse::Pending { .. } => {
            // OAuth 等待授权，直接返回
            Ok(response)
        }
        AuthResponse::PasswordAuthSuccess {
            email,
            password,
            display_name,
            provider,
            imap_config,
            smtp_config,
        } => {
            // 密码验证成功，创建账号
            let account_req = storage::CreateAccountRequest {
                name: name.unwrap_or_else(|| {
                    display_name
                        .clone()
                        .unwrap_or_else(|| email.split('@').next().unwrap_or("用户").to_string())
                }),
                email: email.clone(),
                provider,
                password,
                imap_host: Some(imap_config.host.clone()),
                imap_port: Some(imap_config.port as i32),
                imap_ssl: Some(imap_config.ssl),
                smtp_host: Some(smtp_config.host.clone()),
                smtp_port: Some(smtp_config.port as i32),
                smtp_ssl: Some(smtp_config.ssl),
                color: color.or(Some("#0078D4".to_string())),
                auth_type: Some("password".to_string()),
                ..Default::default()
            };

            let account = storage::AccountRepository::create(&db, app_handle, account_req)
                .await
                .map_err(|e| format!("创建账号失败: {}", e))?;

            tracing::info!(
                "密码认证并创建账号成功: email={}, account_id={}",
                email,
                account.id
            );

            Ok(AuthResponse::Success {
                account: account.into(),
            })
        }
        AuthResponse::Success { .. } => {
            // 已成功（OAuth 回调后），直接返回
            Ok(response)
        }
        AuthResponse::Error { .. } => {
            // 认证失败，直接返回
            Ok(response)
        }
    }
}
