//! 认证管理器
//!
//! 统一管理各种认证方式，协调各认证处理器

use std::sync::Arc;

use tauri::AppHandle;

use crate::auth::credentials::OAuthAuth;
use crate::auth::credentials::PasswordAuth;
use crate::auth::enterprise_auth::EnterpriseAuth;
use crate::auth::http::OAuthHttpServer;
use crate::auth::oauth_handler::{AuthorizationContext, OAuthHandler};
use crate::auth::session::OAuthSessionManager;
use crate::auth::token::TokenManager;
use crate::auth::token::TokenRefresher;
use crate::error::{MailError, Result};
use crate::providers::{AuthType, ProviderPool};

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// 账号 ID（需要外部设置）
    pub account_id: Option<i32>,
    /// 邮箱地址
    pub email: String,
    /// 显示名称
    pub display_name: Option<String>,
    /// 认证类型
    pub auth_type: AuthType,
    /// 服务商 ID
    pub provider: String,
    /// Token 过期时间（仅 OAuth）
    pub expires_at: Option<i64>,
    /// ID Token（仅 OAuth，包含用户信息）
    pub id_token: Option<String>,
}

/// IMAP 认证信息
#[derive(Debug, Clone)]
pub enum ImapAuthInfo {
    /// 密码认证
    Password { username: String, password: String },
    /// OAuth 认证
    OAuth { email: String, access_token: String },
}

/// SMTP 认证信息
#[derive(Debug, Clone)]
pub enum SmtpAuthInfo {
    /// 密码认证
    Password { username: String, password: String },
    /// OAuth 认证
    OAuth { email: String, access_token: String },
}

/// 认证状态
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// 已认证
    Authenticated,
    /// Token 即将过期
    ExpiringSoon,
    /// Token 已过期
    Expired,
    /// 认证失败
    Failed,
    /// 需要重新授权
    ReauthorizationRequired,
}

/// 统一认证信息枚举
///
/// 用于前端向后端传递认证配置，根据类型自动选择合适的认证方式
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AuthInfo {
    /// IMAP/SMTP 密码配置
    ImapSmtpConfig {
        /// 邮箱地址
        email: String,
        /// 密码
        password: String,
        /// IMAP 服务器配置（host 为空时自动检测）
        #[serde(rename = "imapConfig")]
        imap_config: ServerConfig,
        /// SMTP 服务器配置（host 为空时自动检测）
        #[serde(rename = "smtpConfig")]
        smtp_config: ServerConfig,
        /// 账号显示名称（可选）
        name: Option<String>,
        /// 颜色（可选）
        color: Option<String>,
    },
    /// OAuth 配置
    OauthConfig {
        /// 邮箱地址
        email: String,
        /// 账号显示名称（可选）
        name: Option<String>,
        /// 颜色（可选）
        color: Option<String>,
    },
}

/// 服务器配置
///
/// 用于 IMAP/SMTP 服务器设置
/// 当 host 为空字符串时，后端会自动检测提供商配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    /// 服务器主机名（空字符串表示自动检测）
    pub host: String,
    /// 服务器端口
    pub port: u16,
    /// 是否使用 SSL/TLS
    pub ssl: bool,
}

/// 统一认证响应
///
/// 返回给前端的认证结果，根据认证类型返回不同的响应格式
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum AuthResponse {
    /// OAuth 需要等待用户授权
    Pending {
        /// 会话 ID
        session_id: String,
        /// 授权 URL
        auth_url: String,
    },
    /// 密码认证验证成功（包含创建账号所需的信息）
    PasswordAuthSuccess {
        /// 邮箱地址
        email: String,
        /// 密码
        password: String,
        /// 显示名称
        display_name: Option<String>,
        /// 服务商 ID
        provider: String,
        /// IMAP 配置
        imap_config: ServerConfig,
        /// SMTP 配置
        smtp_config: ServerConfig,
    },
    /// 认证成功（账号已创建，用于 OAuth 回调后）
    Success {
        /// 创建的账号信息
        account: crate::storage::AccountDto,
    },
    /// 认证失败
    Error {
        /// 错误信息
        message: String,
    },
}

/// 认证管理器
///
/// 负责：
/// - 统一认证入口
/// - 协调各认证处理器
/// - 路由认证请求到适当的处理器
pub struct AuthManager {
    /// OAuth 处理器
    oauth_handler: Arc<OAuthHandler>,
    /// Token 管理器
    token_manager: Arc<TokenManager>,
    /// OAuth 认证器
    oauth_auth: Arc<OAuthAuth>,
    /// Token 刷新器
    token_refresher: Arc<TokenRefresher>,
    /// 密码认证处理器
    password_auth: Arc<PasswordAuth>,
    /// 企业认证处理器
    enterprise_auth: Arc<EnterpriseAuth>,
    /// 服务商池
    provider_pool: Arc<ProviderPool>,
    /// OAuth 会话管理器
    session_manager: Arc<OAuthSessionManager>,
    /// OAuth HTTP 回调服务器
    http_server: Arc<OAuthHttpServer>,
}

impl AuthManager {
    /// 创建新的认证管理器
    ///
    /// # 参数
    ///
    /// * `app_handle` - Tauri 应用句柄
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let manager = AuthManager::new(&app_handle)?;
    /// ```
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let provider_pool = Arc::new(ProviderPool::default());
        let oauth_handler = Arc::new(OAuthHandler::new());
        let token_manager = Arc::new(TokenManager::new(app_handle)?);
        let password_auth = Arc::new(PasswordAuth::new(app_handle)?);
        let enterprise_auth = Arc::new(EnterpriseAuth::new());

        // 创建 OAuth 认证器
        let oauth_auth = Arc::new(OAuthAuth::new(
            Arc::clone(&oauth_handler),
            Arc::clone(&token_manager),
            Arc::clone(&provider_pool),
        ));

        // 创建 Token 刷新器
        let token_refresher = Arc::new(TokenRefresher::new(
            Arc::clone(&oauth_handler),
            Arc::clone(&token_manager),
            Arc::clone(&provider_pool),
        ));

        // 创建 OAuth 会话管理器（10分钟超时）
        let session_manager = Arc::new(OAuthSessionManager::new(600));

        // 启动后台清理任务
        Arc::clone(&session_manager).spawn_cleanup_task();

        // 初始化 OAuth HTTP 服务器（但不启动）
        // let port = crate::config::get_oauth_callback_port();
        let port = 36279;
        let http_server = Arc::new(OAuthHttpServer::new(port, app_handle.clone()));

        tracing::info!("OAuth HTTP 服务器创建成功，将在异步运行时中启动");

        Ok(Self {
            oauth_handler,
            token_manager,
            oauth_auth,
            token_refresher,
            password_auth,
            enterprise_auth,
            provider_pool,
            session_manager,
            http_server,
        })
    }

    /// 获取 OAuth 授权 URL
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// 返回授权上下文（包含授权 URL 和 state）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let context = manager.get_oauth_url("user@example.com").await?;
    /// // 在浏览器中打开 context.auth_url
    /// ```
    pub async fn get_oauth_url(&self, email: &str) -> Result<AuthorizationContext> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;

        // 2. 获取授权 URL
        let context = self.oauth_handler.get_authorization_url(provider).await?;

        tracing::info!("生成 OAuth 授权 URL: email={}", email);

        Ok(context)
    }

    /// 统一认证入口（仅用于 OAuth）
    ///
    /// 根据认证信息类型，自动路由到合适的认证方式：
    /// - OAuth: 启动授权流程，返回会话 ID 和授权 URL
    /// - 密码: 密码认证请直接使用 `authenticate_password` 方法
    ///
    /// # 参数
    ///
    /// * `auth_info` - 认证信息（仅支持 OAuth）
    ///
    /// # 返回
    ///
    /// - OAuth: 返回会话 ID 和授权 URL
    /// - 密码: 返回错误（提示使用 authenticate_password）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let auth_info = AuthInfo::OauthConfig {
    ///     email: "user@example.com".to_string(),
    ///     name: None,
    ///     color: None,
    /// };
    /// let response = manager.start_auth(auth_info).await?;
    /// ```
    pub async fn start_auth(&self, auth_info: &AuthInfo) -> Result<AuthResponse> {
        match auth_info {
            AuthInfo::OauthConfig { email, .. } => {
                // OAuth 认证流程
                self.start_oauth_auth(email).await
            }
            AuthInfo::ImapSmtpConfig {
                email,
                password,
                imap_config,
                smtp_config,
                ..
            } => {
                // 密码认证流程：确定配置并验证
                self.start_password_auth(email, password, imap_config, smtp_config)
                    .await
            }
        }
    }

    /// 启动密码认证
    ///
    /// 确定最终配置，验证凭证，返回验证结果和配置
    async fn start_password_auth(
        &self,
        email: &str,
        password: &str,
        imap_config: &ServerConfig,
        smtp_config: &ServerConfig,
    ) -> Result<AuthResponse> {
        // 1. 检测提供商
        let provider = self
            .provider_pool
            .detect_provider(email)
            .await
            .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;
        let provider_info = provider.provider_info();
        let provider_id = provider_info.id.clone();

        // 2. 确定最终配置
        let imap_config_final = if !imap_config.host.is_empty() {
            imap_config.clone()
        } else {
            let cfg = provider.imap_config(email);
            ServerConfig {
                host: cfg.host,
                port: cfg.port,
                ssl: matches!(
                    cfg.ssl,
                    crate::providers::SslMode::Implicit | crate::providers::SslMode::StartTls
                ),
            }
        };

        let smtp_config_final = if !smtp_config.host.is_empty() {
            smtp_config.clone()
        } else {
            let cfg = provider.smtp_config(email);
            ServerConfig {
                host: cfg.host,
                port: cfg.port,
                ssl: matches!(cfg.ssl, crate::providers::SslMode::StartTls),
            }
        };

        // 3. 验证凭证
        let auth_result = self
            .authenticate_password(email, password, &imap_config_final, &smtp_config_final)
            .await
            .map_err(|e| MailError::Internal(format!("密码认证失败: {}", e)))?;

        // 4. 返回验证成功信息（不创建账号）
        Ok(AuthResponse::PasswordAuthSuccess {
            email: auth_result.email.clone(),
            password: password.to_string(),
            display_name: auth_result.display_name.clone(),
            provider: provider_id,
            imap_config: imap_config_final,
            smtp_config: smtp_config_final,
        })
    }

    /// 启动 OAuth 认证
    ///
    /// 创建 OAuth 会话，生成授权 URL，并在浏览器中打开授权页面
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// 返回会话 ID 和授权 URL
    async fn start_oauth_auth(&self, email: &str) -> Result<AuthResponse> {
        // 🆕 按需启动 HTTP Server
        let http_server = self.get_http_server();
        if !http_server.is_running().await {
            if let Err(e) = http_server.start().await {
                tracing::error!("OAuth HTTP 服务器启动失败: {}", e);
                return Err(MailError::Internal(e.to_string()));
            } else {
                tracing::info!("OAuth HTTP 服务器已启动（按需启动）");
            }
        } else {
            tracing::info!("OAuth HTTP 服务器已在运行");
        }

        // 1. 生成授权 URL 和 state
        let context = self.get_oauth_url(email).await?;

        // 2. 检测服务商并创建会话
        let provider = self.provider_pool.detect_provider(email).await?;
        let provider_info = provider.provider_info();
        let session_id = self
            .session_manager
            .create_session(&provider_info.id, email, &context.state)
            .await?;

        // 3. 在浏览器中打开授权页面
        tauri_plugin_opener::open_url(&context.auth_url, None::<&str>)
            .map_err(|e| MailError::Internal(format!("无法打开浏览器: {}", e)))?;

        tracing::info!(
            "启动 OAuth 认证: session_id={}, email={}",
            session_id,
            email
        );

        Ok(AuthResponse::Pending {
            session_id,
            auth_url: context.auth_url,
        })
    }

    /// OAuth 认证
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `auth_code` - OAuth 授权码
    /// * `state` - CSRF 防护令牌
    ///
    /// # 返回
    ///
    /// 返回认证结果（包含用户信息和 id_token）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let result = manager.authenticate_oauth("user@example.com", "code", "state").await?;
    /// ```
    pub async fn authenticate_oauth(
        &self,
        email: &str,
        auth_code: &str,
        state: &str,
    ) -> Result<AuthResult> {
        // 委托给 OAuthAuth 处理
        self.oauth_auth.authenticate(email, auth_code, state).await
    }

    /// 密码认证
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `password` - 密码
    ///
    /// # 返回
    ///
    /// 返回认证结果
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let imap_config = ServerConfig {
    ///     host: "imap.example.com".to_string(),
    ///     port: 993,
    ///     ssl: true,
    /// };
    /// let smtp_config = ServerConfig {
    ///     host: "smtp.example.com".to_string(),
    ///     port: 587,
    ///     ssl: true,
    /// };
    /// let result = manager.authenticate_password("user@example.com", "password", &imap_config, &smtp_config).await?;
    /// ```
    pub async fn authenticate_password(
        &self,
        email: &str,
        password: &str,
        imap_config: &ServerConfig,
        smtp_config: &ServerConfig,
    ) -> Result<AuthResult> {
        // 1. 根据配置检测提供商（用于获取 provider_id）
        let provider = self.provider_pool.detect_provider(email).await?;
        let provider_info = provider.provider_info();
        let provider_id = provider_info.id.clone();

        // 2. 使用提供的 IMAP 配置验证密码
        let state = self
            .validate_credentials_for_password(email, password, imap_config)
            .await?;

        // 如果验证失败，返回错误
        if state != AuthState::Authenticated {
            return Err(MailError::Internal(format!("密码认证失败: {:?}", state)));
        }

        // 3. 存储密码
        // 注意：account_id 需要外部设置，这里先使用临时值
        let temp_account_id = 0;
        self.password_auth
            .store_password(temp_account_id, password)
            .await?;

        // 提取显示名称（从 email 的用户名部分）
        let display_name = email.split('@').next().map(|s| s.to_string());

        tracing::info!(
            "密码认证成功: email={}, provider={}, imap_server={}",
            email,
            provider_id,
            imap_config.host
        );

        Ok(AuthResult {
            account_id: None,
            email: email.to_string(),
            display_name,
            auth_type: AuthType::Password,
            provider: provider_id,
            expires_at: None,
            id_token: None,
        })
    }

    /// 验证密码凭据（连接测试）
    ///
    /// 用于添加账号前的连接测试。
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `password` - 密码
    /// * `imap_config` - IMAP 服务器配置
    ///
    /// # 返回
    ///
    /// 返回认证状态
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let state = manager.validate_credentials_for_password(
    ///     "user@example.com",
    ///     "password",
    ///     &ServerConfig { host: "imap.example.com".to_string(), port: 993, ssl: true }
    /// ).await?;
    /// assert_eq!(state, AuthState::Authenticated);
    /// ```
    pub async fn validate_credentials_for_password(
        &self,
        email: &str,
        password: &str,
        imap_config: &ServerConfig,
    ) -> Result<AuthState> {
        // 使用 PasswordAuth 验证
        let valid = self
            .password_auth
            .validate_password(&imap_config.host, imap_config.port, email, password)
            .await?;

        if valid {
            Ok(AuthState::Authenticated)
        } else {
            Ok(AuthState::Failed)
        }
    }

    /// 刷新 Token
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 刷新成功
    /// - `Err(_)` - 刷新失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// manager.refresh_token(1, "user@example.com").await?;
    /// ```
    pub async fn refresh_token(&self, account_id: i32, email: &str) -> Result<()> {
        // 委托给 TokenRefresher 处理
        self.token_refresher.refresh_token(account_id, email).await
    }

    /// 验证凭证状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回认证状态
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let state = manager.validate_credentials(1, AuthType::OAuth2).await?;
    /// ```
    pub async fn validate_credentials(
        &self,
        account_id: i32,
        auth_type: &AuthType,
    ) -> Result<AuthState> {
        // 委托给 TokenRefresher 处理
        self.token_refresher
            .validate_credentials(account_id, auth_type)
            .await
    }

    /// 批量刷新即将过期的 Token
    ///
    /// # 参数
    ///
    /// * `accounts` - 账号 ID 和邮箱地址的映射
    ///
    /// # 返回
    ///
    /// 返回成功刷新的账号 ID 列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let accounts = vec![(1, "user1@example.com"), (2, "user2@example.com")];
    /// let refreshed = manager.refresh_expiring_tokens(accounts).await?;
    /// ```
    pub async fn refresh_expiring_tokens(&self, accounts: Vec<(i32, String)>) -> Result<Vec<i32>> {
        // 委托给 TokenRefresher 处理
        self.token_refresher.refresh_expiring_tokens(accounts).await
    }

    /// 获取 IMAP 认证信息
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回 IMAP 认证信息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let auth_info = manager.get_imap_auth(1, "user@example.com", &AuthType::OAuth2).await?;
    /// ```
    pub async fn get_imap_auth(
        &self,
        account_id: i32,
        email: &str,
        auth_type: &AuthType,
    ) -> Result<ImapAuthInfo> {
        let auth_type = match auth_type {
            AuthType::Auto => &AuthType::OAuth2,
            other => other,
        };

        match auth_type {
            AuthType::OAuth2 => {
                // 1. 获取服务商（用于刷新 token）
                let provider = self
                    .provider_pool
                    .detect_provider(email)
                    .await
                    .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

                // 2. 获取 access_token（带自动刷新）
                let access_token = self
                    .token_manager
                    .get_access_token(account_id, || async {
                        // 缓存未命中或已过期，执行刷新

                        // 2.1 获取 refresh_token
                        let refresh_token = self
                            .token_manager
                            .get_oauth_token(account_id)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("获取 refresh_token 失败: {}", e))
                            })?;

                        // 2.2 刷新 token
                        let token_response = self
                            .oauth_handler
                            .refresh_token(provider, &refresh_token.refresh_token)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("刷新 access_token 失败: {}", e))
                            })?;

                        // 2.3 提取 access_token（注意：不是 Option）
                        Ok(token_response.access_token)
                    })
                    .await?;

                // // 3. 生成 XOAUTH2 字符串
                // let xoauth2 = self.oauth_handler.generate_xoauth2(email, &access_token);

                Ok(ImapAuthInfo::OAuth {
                    email: email.to_string(),
                    access_token,
                })
            }
            AuthType::Password | AuthType::AppPassword => {
                // 获取密码
                let password = self.password_auth.get_password(account_id).await?;

                Ok(ImapAuthInfo::Password {
                    username: email.to_string(),
                    password,
                })
            }
            AuthType::Auto => {
                // unreachable because we replaced Auto with OAuth2 above
                unreachable!()
            }
        }
    }

    /// 获取 SMTP 认证信息
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回 SMTP 认证信息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let auth_info = manager.get_smtp_auth(1, "user@example.com", &AuthType::OAuth2).await?;
    /// ```
    pub async fn get_smtp_auth(
        &self,
        account_id: i32,
        email: &str,
        auth_type: &AuthType,
    ) -> Result<SmtpAuthInfo> {
        let auth_type = match auth_type {
            AuthType::Auto => &AuthType::OAuth2,
            other => other,
        };

        match auth_type {
            AuthType::OAuth2 => {
                // 1. 获取服务商（用于刷新 token）
                let provider = self
                    .provider_pool
                    .detect_provider(email)
                    .await
                    .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

                // 2. 获取 access_token（带自动刷新）
                let access_token = self
                    .token_manager
                    .get_access_token(account_id, || async {
                        // 缓存未命中或已过期，执行刷新

                        // 2.1 获取 refresh_token
                        let refresh_token = self
                            .token_manager
                            .get_oauth_token(account_id)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("获取 refresh_token 失败: {}", e))
                            })?;

                        // 2.2 刷新 token
                        let token_response = self
                            .oauth_handler
                            .refresh_token(provider, &refresh_token.refresh_token)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("刷新 access_token 失败: {}", e))
                            })?;

                        // 2.3 提取 access_token（注意：不是 Option）
                        Ok(token_response.access_token)
                    })
                    .await?;

                Ok(SmtpAuthInfo::OAuth {
                    email: email.to_string(),
                    access_token,
                })
            }
            AuthType::Password | AuthType::AppPassword => {
                // 获取密码
                let password = self.password_auth.get_password(account_id).await?;

                Ok(SmtpAuthInfo::Password {
                    username: email.to_string(),
                    password,
                })
            }
            AuthType::Auto => {
                // unreachable because we replaced Auto with OAuth2 above
                unreachable!()
            }
        }
    }

    /// 更新账号 ID（在创建账号后调用）
    ///
    /// # 参数
    ///
    /// * `temp_id` - 临时账号 ID
    /// * `new_id` - 新的账号 ID
    pub async fn update_account_id(&self, temp_id: i32, new_id: i32) -> Result<()> {
        // TODO: 实现账号 ID 更新逻辑
        // 需要迁移 Token 和密码存储
        tracing::info!("更新账号 ID: {} -> {}", temp_id, new_id);
        Ok(())
    }

    /// 获取服务商池
    pub fn provider_pool(&self) -> &ProviderPool {
        &self.provider_pool
    }

    /// 获取 Token 管理器
    pub fn token_manager(&self) -> &TokenManager {
        &self.token_manager
    }

    /// 获取密码认证处理器
    pub fn password_auth(&self) -> &PasswordAuth {
        &self.password_auth
    }

    /// 获取企业认证处理器
    pub fn enterprise_auth(&self) -> &EnterpriseAuth {
        &self.enterprise_auth
    }

    /// 获取 OAuth 会话管理器
    pub fn session_manager(&self) -> Arc<OAuthSessionManager> {
        Arc::clone(&self.session_manager)
    }

    /// 获取当前使用的 redirect_uri
    ///
    /// 返回 HTTP localhost 形式的回调 URL
    pub fn get_redirect_uri(&self) -> String {
        self.http_server.get_callback_url()
    }

    /// 获取 HTTP 服务器引用
    ///
    /// 供命令层检查服务器状态
    pub fn get_http_server(&self) -> &Arc<OAuthHttpServer> {
        &self.http_server
    }

    /// 优雅关闭
    ///
    /// 停止 HTTP 服务器并清理资源
    pub async fn shutdown(&self) {
        self.http_server.stop().await;
        tracing::info!("OAuth HTTP 服务器已停止");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_manager_new() {
        // 注意：测试需要 Tauri AppHandle，这里只测试结构
        // 实际测试需要集成测试环境
        // TODO: 实现实际的集成测试
    }

    #[test]
    fn test_auth_result_creation() {
        let result = AuthResult {
            account_id: Some(1),
            email: "user@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            auth_type: AuthType::OAuth2,
            provider: "gmail".to_string(),
            expires_at: Some(1234567890),
            id_token: Some("test_id_token".to_string()),
        };

        assert_eq!(result.account_id, Some(1));
        assert_eq!(result.email, "user@example.com");
        assert_eq!(result.display_name, Some("Test User".to_string()));
        assert_eq!(result.auth_type, AuthType::OAuth2);
        assert_eq!(result.provider, "gmail");
        assert_eq!(result.expires_at, Some(1234567890));
        assert_eq!(result.id_token, Some("test_id_token".to_string()));
    }

    #[test]
    fn test_imap_auth_info_password() {
        let auth_info = ImapAuthInfo::Password {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
        };

        match auth_info {
            ImapAuthInfo::Password { username, password } => {
                assert_eq!(username, "user@example.com");
                assert_eq!(password, "password");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_imap_auth_info_oauth() {
        let auth_info = ImapAuthInfo::OAuth {
            email: "user@example.com".to_string(),
            access_token: "test_xoauth2".to_string(),
        };

        match auth_info {
            ImapAuthInfo::OAuth {
                email,
                access_token: xoauth2,
            } => {
                assert_eq!(email, "user@example.com");
                assert_eq!(xoauth2, "test_xoauth2");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_smtp_auth_info_password() {
        let auth_info = SmtpAuthInfo::Password {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
        };

        match auth_info {
            SmtpAuthInfo::Password { username, password } => {
                assert_eq!(username, "user@example.com");
                assert_eq!(password, "password");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_smtp_auth_info_oauth() {
        let auth_info = SmtpAuthInfo::OAuth {
            email: "user@example.com".to_string(),
            access_token: "test_xoauth2".to_string(),
        };

        match auth_info {
            SmtpAuthInfo::OAuth {
                email,
                access_token: xoauth2,
            } => {
                assert_eq!(email, "user@example.com");
                assert_eq!(xoauth2, "test_xoauth2");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_auth_state_authenticated() {
        assert_eq!(AuthState::Authenticated, AuthState::Authenticated);
    }

    #[test]
    fn test_auth_state_expiring_soon() {
        assert_eq!(AuthState::ExpiringSoon, AuthState::ExpiringSoon);
    }

    #[test]
    fn test_auth_state_expired() {
        assert_eq!(AuthState::Expired, AuthState::Expired);
    }
}
