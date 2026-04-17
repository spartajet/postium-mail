// ═════════════════════════════════════════════════════════════════════════
// 认证管理器模块 (Authentication Manager)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责管理邮件账号的认证凭证，包括：
// 1. 密码认证 - 支持传统的用户名/密码登录
// 2. OAuth2 认证 - 支持 Google、Outlook 等 OAuth2 提供商
// 3. Token 缓存 - 内存缓存 access_token 以减少网络请求
// 4. Keyring 集成 - 使用系统 Keyring 安全存储密码和 refresh_token
//
// 设计原则：
// - 凭证永不存入数据库，只使用系统 Keyring 加密存储
// - access_token 内存缓存，refresh_token 存储在 Keyring
// - 自动处理 OAuth2 token 的刷新逻辑
// - 统一的凭证获取接口，简化上层调用
//
// 安全特性：
// - 密码和 refresh_token 使用系统 Keyring 加密存储
// - access_token 仅缓存到内存，应用重启后重新获取
// - Token 刷新时自动轮换 refresh_token（如果提供商返回新的）
// - 所有 Keyring 操作都有错误处理和日志记录
// ═════════════════════════════════════════════════════════════════════════

use crate::domain::auth::token_cache::TokenCache;
use crate::domain::providers::AuthType;
use crate::error::MailError;
use crate::infrastructure::auth::oauth2::OAuth2Manager;
use std::sync::Arc;
use std::sync::OnceLock;

// ═════════════════════════════════════════════════════════════════════════
// 凭证类型枚举
// ═════════════════════════════════════════════════════════════════════════

/// 凭证类型
///
/// 表示两种认证方式的凭证，用于统一接口返回。
///
/// # 变体说明
///
/// - `Password(String)`: 密码凭证，用于传统用户名/密码认证
/// - `OAuth2 { access_token }`: OAuth2 访问令牌，用于 OAuth2 认证
///
/// # 使用场景
///
/// - IMAP/SMTP 连接时传递凭证
/// - 统一处理不同认证方式的逻辑
/// - 在 protocol 层根据类型选择不同的认证方法
pub enum Credentials {
    /// 密码凭证（包含明文密码）
    Password(String),

    /// OAuth2 访问令牌
    OAuth2 { access_token: String },
}

// ═════════════════════════════════════════════════════════════════════════
// 认证管理器结构体
// ═════════════════════════════════════════════════════════════════════════

/// 认证管理器
///
/// 负责管理所有账号的认证凭证，提供统一的凭证获取接口。
/// 支持 OAuth2 token 的自动刷新和缓存管理。
///
/// # 字段说明
///
/// - `token_cache`: access_token 的内存缓存，避免频繁请求 OAuth2 服务器
/// - `oauth2_manager`: OAuth2 管理器（懒加载，仅在需要 OAuth2 时初始化）
///
/// # 线程安全
///
/// 该结构体可以在线程间共享，内部使用的 TokenCache 是线程安全的，
/// oauth2_manager 使用 OnceLock 确保只初始化一次。
///
/// # 生命周期管理
///
/// - TokenCache: 内存缓存，应用重启后清空
/// - Keyring: 持久化存储，即使应用重启后密码/token 仍然存在
/// - access_token: 存储在内存中，过期后自动刷新
/// - refresh_token: 存储在 Keyring 中，用于获取新的 access_token
pub struct AuthManager {
    /// access_token 内存缓存
    token_cache: TokenCache,

    /// OAuth2 管理器（懒加载）
    oauth2_manager: OnceLock<Arc<OAuth2Manager>>,
}

impl Default for AuthManager {
    fn default() -> Self {
        Self {
            // 创建空的 token 缓存
            token_cache: TokenCache::new(),
            // 创建未初始化的 oauth2_manager
            oauth2_manager: OnceLock::new(),
        }
    }
}

impl AuthManager {
    // ═══════════════════════════════════════════════════════════════════════
    // 初始化和配置方法
    // ═══════════════════════════════════════════════════════════════════════

    /// 注入 OAuth2 管理器
    ///
    /// 在应用启动时（lib.rs 的 run 函数中）调用一次，
    /// 将 OAuth2Manager 注入到 AuthManager 中。
    ///
    /// # 参数
    ///
    /// - `manager`: OAuth2 管理器实例（使用 Arc 包装）
    ///
    /// # 为什么使用 OnceLock
    ///
    /// - 确保只初始化一次，避免多次初始化造成的资源浪费
    /// - 懒加载：只有使用 OAuth2 时才需要初始化
    /// - 线程安全：OnceLock 提供线程安全的初始化保证
    ///
    /// # 注意事项
    ///
    /// - 此方法应该在应用启动时调用一次
    /// - 如果多次调用，第二次及以后的调用会被忽略（OnceLock 的特性）
    /// - 必须在使用 OAuth2 认证前调用，否则会报错
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 在 lib.rs 的 run 函数中
    /// let oauth2_manager = Arc::new(OAuth2Manager::new()?);
    /// auth_manager.set_oauth2_manager(oauth2_manager);
    /// ```
    pub fn set_oauth2_manager(&self, manager: Arc<OAuth2Manager>) {
        // 使用 OnceLock.set 方法，如果已经设置过则忽略
        let _ = self.oauth2_manager.set(manager);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 凭证获取方法
    // ═══════════════════════════════════════════════════════════════════════

    /// 统一凭证获取接口
    ///
    /// 根据认证类型返回相应的凭证，自动处理 OAuth2 token 的缓存和刷新。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱地址（作为 Keyring 的 key）
    /// - `auth_type`: 认证类型（Password 或 OAuth2）
    /// - `oauth_provider`: OAuth2 提供商标识（如 "gmail", "outlook"），OAuth2 模式必填
    ///
    /// # 返回
    ///
    /// 成功时返回 Credentials 枚举，失败时返回错误。
    ///
    /// # 工作流程 - 密码认证
    ///
    /// 1. 直接从 Keyring 获取密码
    /// 2. 返回 Credentials::Password
    ///
    /// # 工作流程 - OAuth2 认证
    ///
    /// 1. **检查缓存**: 尝试从内存缓存获取有效的 access_token
    ///    - 如果缓存命中且未过期，直接返回
    /// 2. **获取 refresh_token**: 从 Keyring 获取 refresh_token
    /// 3. **刷新 token**: 使用 refresh_token 向 OAuth2 服务器请求新的 access_token
    /// 4. **轮换 refresh_token**: 如果服务器返回新的 refresh_token，更新 Keyring
    /// 5. **缓存 access_token**: 将新的 access_token 存入内存缓存
    /// 6. **返回凭证**: 返回新的 access_token
    ///
    /// # Token 刷新策略
    ///
    /// - access_token 通常有效期 1 小时
    /// - refresh_token 长期有效（可能永久有效）
    /// - 某些 OAuth2 提供商在刷新 token 时会返回新的 refresh_token
    /// - 新的 refresh_token 必须保存到 Keyring，旧的失效
    ///
    /// # 错误处理
    ///
    /// - `OAuth2Error("缺少 oauth_provider")`: OAuth2 模式下未提供 provider
    /// - `KeyringError`: Keyring 读取失败
    /// - `OAuth2Error("OAuth2Manager 未初始化")`: 未调用 set_oauth2_manager
    /// - `OAuth2Error`: Token 刷新失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 密码认证
    /// let creds = auth_manager.get_credentials(
    ///     "user@example.com",
    ///     &AuthType::Password,
    ///     None
    /// ).await?;
    ///
    /// // OAuth2 认证
    /// let creds = auth_manager.get_credentials(
    ///     "user@gmail.com",
    ///     &AuthType::OAuth2,
    ///     Some("gmail")
    /// ).await?;
    /// ```
    pub async fn get_credentials(
        &self,
        email: &str,
        auth_type: &AuthType,
        oauth_provider: Option<&str>,
    ) -> Result<Credentials, MailError> {
        match auth_type {
            // ─── OAuth2 认证流程 ───
            AuthType::OAuth2 => {
                // 1. 验证必填参数
                let provider_id = oauth_provider
                    .ok_or_else(|| MailError::OAuth2Error("缺少 oauth_provider".into()))?;

                // 2. 尝试从内存缓存获取 access_token
                if let Some(token) = self.token_cache.get(email) {
                    tracing::debug!(email, "使用缓存的 access_token");
                    return Ok(Credentials::OAuth2 {
                        access_token: token,
                    });
                }

                // 3. 缓存未命中或过期 → 从 Keyring 获取 refresh_token
                let refresh_token = self.get_password(email)?;

                // 4. 刷新 access_token
                let manager = self
                    .oauth2_manager
                    .get()
                    .ok_or_else(|| MailError::OAuth2Error("OAuth2Manager 未初始化".into()))?;

                tracing::info!(email, provider_id, "刷新 access_token");
                let token_result = manager.refresh_token(provider_id, &refresh_token).await?;
                tracing::info!("access_token 刷新成功: {:?}", token_result);

                // 5. 如果返回新的 refresh_token，更新 Keyring
                // 某些 OAuth2 提供商（如 Google）在刷新 token 时会返回新的 refresh_token
                if let Some(new_rt) = &token_result.refresh_token
                    && new_rt != &refresh_token
                {
                    tracing::info!(email, "refresh_token 已轮换，更新 Keyring");
                    self.save_password(email, new_rt)?;
                }

                // 6. 缓存新 access_token 到内存
                // 计算 token 的过期时间（当前时间 + 有效期）
                let expires_at = if let Some(expires_in) = token_result.expires_in {
                    chrono::Utc::now().timestamp() + expires_in
                } else {
                    // 如果未提供有效期，默认 3600 秒（1小时）
                    chrono::Utc::now().timestamp() + 3600
                };
                self.token_cache
                    .store(email, token_result.access_token.clone(), expires_at);
                tracing::info!(expires_at, "access_token 已缓存");

                // 7. 返回新的 access_token
                Ok(Credentials::OAuth2 {
                    access_token: token_result.access_token,
                })
            }

            // ─── 密码认证流程 ───
            AuthType::Password => {
                // 直接从 Keyring 获取密码
                let password = self.get_password(email)?;
                Ok(Credentials::Password(password))
            }
        }
    }

    /// 缓存 access_token（创建账号后立即调用）
    ///
    /// 在 OAuth2 认证流程完成后，获取的第一个 access_token 需要立即缓存，
    /// 避免后续操作时重新请求。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱地址
    /// - `access_token`: OAuth2 access_token
    /// - `expires_in`: token 有效期（秒）
    ///
    /// # 使用场景
    ///
    /// - OAuth2 认证成功后立即缓存 token
    /// - 避免首次使用时再次请求 token
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // OAuth2 回调处理中
    /// let access_token = oauth2_response.access_token;
    /// let expires_in = oauth2_response.expires_in.unwrap_or(3600);
    /// auth_manager.cache_access_token(email, &access_token, expires_in);
    /// ```
    pub fn cache_access_token(&self, email: &str, access_token: &str, expires_in: i64) {
        // 计算过期时间（当前时间 + 有效期）
        let expires_at = chrono::Utc::now().timestamp() + expires_in;

        // 存入内存缓存
        self.token_cache
            .store(email, access_token.to_string(), expires_at);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Keyring 操作方法
    // ═══════════════════════════════════════════════════════════════════════

    /// 从 Keyring 获取密码或 refresh_token
    ///
    /// 使用操作系统提供的 Keyring 安全存储服务读取凭证。
    ///
    /// # 支持的 Keyring
    ///
    /// - **Windows**: Windows Credential Manager
    /// - **macOS**: Keychain
    /// - **Linux**: Secret Service API (libsecret)
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱地址，作为 Keyring 的唯一标识
    ///
    /// # 返回
    ///
    /// 成功时返回密码字符串，失败时返回错误。
    ///
    /// # Keyring 结构
    ///
    /// - Service: "postium-mail"（固定值）
    /// - Username: 用户邮箱地址
    /// - Password: 实际的密码或 refresh_token
    ///
    /// # 错误处理
    ///
    /// - `KeyringError`: Keyring 初始化失败或读取失败
    /// - 日志级别: WARN（读取失败是预期的，可能是首次使用）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let password = auth_manager.get_password("user@example.com")?;
    /// println!("密码长度: {}", password.len());
    /// ```
    pub fn get_password(&self, email: &str) -> Result<String, MailError> {
        tracing::debug!(email, "Keyring: 获取密码");

        // Keyring 的服务名称（应用标识）
        let service = "postium-mail";

        // 创建 Keyring 条目
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;

        // 从 Keyring 读取密码
        let password = entry.get_password().map_err(|e| {
            tracing::warn!(email, error = %e, "Keyring: 获取密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;

        tracing::debug!(email, "Keyring: 密码获取成功");
        Ok(password)
    }

    /// 保存密码或 refresh_token 到 Keyring
    ///
    /// 使用操作系统提供的 Keyring 安全存储服务保存凭证。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱地址，作为 Keyring 的唯一标识
    /// - `password`: 要保存的密码或 refresh_token
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 安全特性
    ///
    /// - Keyring 使用操作系统提供的加密存储
    /// - 密码明文不会被写入磁盘
    /// - 不同用户有独立的 Keyring 空间
    ///
    /// # 错误处理
    ///
    /// - `KeyringError`: Keyring 初始化失败或写入失败
    /// - 日志级别: ERROR（写入失败是严重的）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// auth_manager.save_password("user@example.com", "mypassword123")?;
    /// ```
    pub fn save_password(&self, email: &str, password: &str) -> Result<(), MailError> {
        tracing::debug!(email, "Keyring: 保存密码");

        // Keyring 的服务名称
        let service = "postium-mail";

        // 创建 Keyring 条目
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;

        // 保存密码到 Keyring
        entry.set_password(password).map_err(|e| {
            tracing::error!(email, error = %e, "Keyring: 保存密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;

        tracing::debug!(email, "Keyring: 密码保存成功");
        Ok(())
    }

    /// 删除 Keyring 中的密码或 refresh_token
    ///
    /// 在删除账号或重置认证信息时调用，清理 Keyring 中的敏感数据。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱地址
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户删除账号时清理凭证
    /// - 密码重置时删除旧密码
    /// - OAuth2 重新授权时删除旧 token
    ///
    /// # 错误处理
    ///
    /// - `KeyringError`: Keyring 操作失败
    /// - 如果凭证不存在，仍然返回错误（预期的行为）
    /// - 日志级别: WARN（删除失败通常不影响后续操作）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 删除账号时
    /// auth_manager.delete_password("user@example.com")?;
    /// ```
    pub fn delete_password(&self, email: &str) -> Result<(), MailError> {
        tracing::debug!(email, "Keyring: 删除密码");

        // Keyring 的服务名称
        let service = "postium-mail";

        // 创建 Keyring 条目
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;

        // 删除 Keyring 中的凭证
        entry.delete_credential().map_err(|e| {
            tracing::warn!(email, error = %e, "Keyring: 删除密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;

        tracing::debug!(email, "Keyring: 密码删除成功");
        Ok(())
    }
}
