use crate::domain::auth::AuthManager;
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::accounts;
use crate::infrastructure::storage::repository::{account_repo, email_repo, label_repo, sync_repo};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

// ═════════════════════════════════════════════════════════════════════════
// 账号服务模块 (Account Service)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责邮件账号的完整生命周期管理，包括：
// 1. 账号的创建、查询、更新和删除（CRUD 操作）
// 2. 账号密码的安全存储（使用系统 Keyring）
// 3. OAuth2 认证账号的创建和 token 管理
// 4. 账号信息的管理（名称、颜色、同步状态等）
//
// 架构设计：
// - Service 层：处理业务逻辑和流程控制
// - Repository 层：负责数据库持久化操作
// - AuthManager：处理认证相关逻辑（密码存储、OAuth2 token 管理）
//
// 安全特性：
// - 密码使用系统 Keyring 存储，不存入数据库
// - OAuth2 token 同样使用 Keyring 安全存储
// - 删除账号时会自动清理关联的认证信息
// ═════════════════════════════════════════════════════════════════════════

// ═════════════════════════════════════════════════════════════════════════
// 数据传输对象 (DTO) 部分
// ═════════════════════════════════════════════════════════════════════════

/// 账号数据传输对象
///
/// 这是账号信息对外展示的标准格式，用于前后端数据交换。
/// 不包含敏感信息如密码和认证 token。
///
/// # 字段说明
///
/// - `id`: 账号在数据库中的唯一标识
/// - `name`: 账号名称（用户自定义的昵称）
/// - `email`: 邮箱地址，作为账号的唯一标识
/// - `display_name`: 发送邮件时显示的名称（可选）
/// - `provider`: 邮件服务提供商（如 "gmail", "outlook", "qq"）
/// - `color`: 账号在界面显示的主题颜色（可选，十六进制颜色值）
/// - `sync_enabled`: 是否启用自动同步
/// - `auth_type`: 认证类型（"password" 或 "OAuth2"）
/// - `account_type`: 账号类型（如 "personal", "work"）
/// - `last_sync_at`: 上次同步的时间戳（Unix 时间戳，秒）
/// - `created_at`: 账号创建时间（Unix 时间戳，秒）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AccountDto {
    /// 数据库主键 ID
    pub id: i32,
    /// 账号名称
    pub name: String,
    /// 邮箱地址
    pub email: String,
    /// 显示名称（用于发送邮件时）
    pub display_name: Option<String>,
    /// 邮件服务提供商标识
    pub provider: String,
    /// 账号主题颜色
    pub color: Option<String>,
    /// 是否启用同步
    pub sync_enabled: bool,
    /// 认证类型
    pub auth_type: String,
    /// 账号类型
    pub account_type: String,
    /// 上次同步时间
    pub last_sync_at: Option<i64>,
    /// 创建时间
    pub created_at: i64,
}

/// 从数据库模型转换为 DTO 的实现
///
/// 这个转换将数据库实体转换为前端可展示的格式，
/// 自动处理 Optional 类型的默认值。
impl From<accounts::Model> for AccountDto {
    fn from(m: accounts::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            email: m.email,
            display_name: m.display_name,
            provider: m.provider,
            color: m.color,
            // 默认启用同步
            sync_enabled: m.sync_enabled.unwrap_or(true),
            // 默认使用密码认证
            auth_type: m.auth_type.unwrap_or_else(|| "password".into()),
            account_type: m.account_type,
            last_sync_at: m.last_sync_at,
            created_at: m.created_at,
        }
    }
}

/// 创建账号请求
///
/// 前端调用创建账号 API 时传递的参数。
///
/// # 必填字段
///
/// - `name`: 账号名称
/// - `email`: 邮箱地址
/// - `provider`: 服务提供商标识
/// - `auth_type`: 认证类型
/// - `password`: 账号密码或认证凭证
///
/// # 可选字段
///
/// - `display_name`: 显示名称
/// - `color`: 主题颜色
/// - `imap_host/smtp_host`: 自定义服务器地址（如果提供商不支持自动配置）
/// - `imap_port/smtp_port`: 自定义服务器端口
/// - `imap_ssl_mode/smtp_ssl_mode`: SSL 模式
/// - `account_type`: 账号类型
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateAccountRequest {
    /// 账号名称（用户自定义）
    pub name: String,
    /// 邮箱地址
    pub email: String,
    /// 发送邮件时的显示名称
    pub display_name: Option<String>,
    /// 邮件服务提供商
    pub provider: String,
    /// 认证类型："password" 或 "OAuth2"
    pub auth_type: String,
    /// 账号密码或初始凭证
    pub password: String,
    /// IMAP 服务器地址（可选）
    pub imap_host: Option<String>,
    /// IMAP 服务器端口（可选）
    pub imap_port: Option<i32>,
    /// IMAP SSL 模式（可选）
    pub imap_ssl_mode: Option<String>,
    /// SMTP 服务器地址（可选）
    pub smtp_host: Option<String>,
    /// SMTP 服务器端口（可选）
    pub smtp_port: Option<i32>,
    /// SMTP SSL 模式（可选）
    pub smtp_ssl_mode: Option<String>,
    /// 账号主题颜色（可选）
    pub color: Option<String>,
    /// 账号类型（可选）
    pub account_type: Option<String>,
}

/// 更新账号请求
///
/// 用于更新账号的部分信息，所有字段都是可选的。
/// 只更新提供的字段，未提供的字段保持不变。
///
/// # 可更新字段
///
/// - `name`: 账号名称
/// - `display_name`: 显示名称
/// - `color`: 主题颜色
/// - `sync_enabled`: 同步开关
///
/// # 注意事项
///
/// - 邮箱地址（email）不可更新，需要删除后重新创建
/// - 密码更新使用专门的 `update_password` 方法
/// - 认证信息不可通过此方法更新
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAccountRequest {
    /// 账号 ID（必填）
    pub id: i32,
    /// 新的账号名称
    pub name: Option<String>,
    /// 新的显示名称
    pub display_name: Option<String>,
    /// 新的主题颜色
    pub color: Option<String>,
    /// 是否启用同步
    pub sync_enabled: Option<bool>,
}

// ═════════════════════════════════════════════════════════════════════════
// 账号服务实现
// ═════════════════════════════════════════════════════════════════════════

/// 账号服务
///
/// 负责处理所有与账号相关的业务逻辑。
///
/// # 字段
///
/// - `db`: 数据库连接，用于持久化操作
/// - `auth`: 认证管理器，处理密码和 token 的安全存储
///
/// # 线程安全
///
/// 该服务使用 Arc 包装 AuthManager，可以在多线程环境中共享使用。
pub struct AccountService {
    /// 数据库连接
    db: DbConn,
    /// 认证管理器（线程安全）
    auth: Arc<AuthManager>,
}

impl AccountService {
    /// 创建新的账号服务实例
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `auth`: 认证管理器（使用 Arc 包装以便共享）
    ///
    /// # 返回
    ///
    /// 返回初始化好的 AccountService 实例
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self { db, auth }
    }

    /// 获取所有账号列表
    ///
    /// 查询数据库中的所有账号，按创建时间倒序排列。
    ///
    /// # 返回
    ///
    /// 成功时返回账号 DTO 向量，失败时返回错误。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let accounts = account_service.list().await?;
    /// for account in accounts {
    ///     println!("账号: {} ({})", account.name, account.email);
    /// }
    /// ```
    pub async fn list(&self) -> Result<Vec<AccountDto>, MailError> {
        // 从数据库查询所有账号
        let accounts = account_repo::list(&self.db).await?;
        // 转换为 DTO 格式
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    /// 根据 ID 获取账号详情
    ///
    /// # 参数
    ///
    /// - `id`: 账号的数据库 ID
    ///
    /// # 返回
    ///
    /// 成功时返回账号 DTO，如果账号不存在则返回 `AccountNotFound` 错误。
    pub async fn get(&self, id: i32) -> Result<AccountDto, MailError> {
        // 从数据库查询指定 ID 的账号
        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;
        // 转换为 DTO 格式
        Ok(account.into())
    }

    /// 创建新账号
    ///
    /// # 流程说明
    ///
    /// 1. 验证并创建账号记录到数据库
    /// 2. 将密码安全存储到系统 Keyring
    /// 3. 对于 OAuth2 账号，token 会在 OAuth2 流程完成后单独存储
    ///
    /// # 参数
    ///
    /// - `req`: 创建账号请求对象
    ///
    /// # 返回
    ///
    /// 成功时返回创建的账号 DTO，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户添加新的邮箱账号
    /// - OAuth2 认证成功后创建账号（password 字段为 refresh_token）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let req = CreateAccountRequest {
    ///     name: "我的邮箱".to_string(),
    ///     email: "user@example.com".to_string(),
    ///     provider: "gmail".to_string(),
    ///     auth_type: "password".to_string(),
    ///     password: "mypassword123".to_string(),
    ///     ..Default::default()
    /// };
    /// let account = account_service.create(req).await?;
    /// ```
    pub async fn create(&self, req: CreateAccountRequest) -> Result<AccountDto, MailError> {
        // 记录日志，便于追踪
        tracing::info!(email = %req.email, provider = %req.provider, "创建账号");

        // 获取当前时间戳
        let now = chrono::Utc::now().timestamp();

        let model = account_repo::AccountWrite {
            name: req.name,
            email: req.email,
            display_name: req.display_name,
            provider: req.provider.clone(),
            // IMAP 服务器配置
            imap_host: req.imap_host,
            imap_port: req.imap_port,
            imap_ssl: Some(true),
            imap_ssl_mode: req.imap_ssl_mode,
            // SMTP 服务器配置
            smtp_host: req.smtp_host,
            smtp_port: req.smtp_port,
            smtp_ssl: Some(true),
            smtp_ssl_mode: req.smtp_ssl_mode,
            // 其他属性
            color: req.color,
            auth_type: Some(req.auth_type.clone()),
            account_type: req.account_type.unwrap_or_else(|| "personal".into()),
            sync_enabled: Some(true),
            last_sync_at: None,
            created_at: now,
            updated_at: now,
        };

        // 持久化到数据库
        let account = account_repo::create(&self.db, model).await?;

        // 将密码安全存储到系统 Keyring
        // 对于 OAuth2 账号，这里存储的是 refresh_token
        self.auth.save_password(&account.email, &req.password)?;

        Ok(account.into())
    }

    /// 更新账号信息
    ///
    /// 更新账号的可编辑字段：名称、显示名称、颜色、同步状态等。
    /// 不包括邮箱地址和密码。
    ///
    /// # 参数
    ///
    /// - `req`: 更新账号请求对象（必须包含 id）
    ///
    /// # 返回
    ///
    /// 成功时返回更新后的账号 DTO，失败时返回错误。
    ///
    /// # 注意事项
    ///
    /// - 邮箱地址（email）不可更新
    /// - 密码更新需要使用专门的 `update_password` 方法
    /// - 认证信息不可通过此方法更新
    pub async fn update(&self, req: UpdateAccountRequest) -> Result<AccountDto, MailError> {
        tracing::info!(id = req.id, "更新账号");

        // 检查账号是否存在
        let existing = account_repo::get_by_id(&self.db, req.id)
            .await?
            .ok_or(MailError::AccountNotFound(req.id))?;

        let model = account_repo::AccountWrite {
            name: req.name.unwrap_or(existing.name),
            email: existing.email,
            display_name: req.display_name.or(existing.display_name),
            provider: existing.provider,
            imap_host: existing.imap_host,
            imap_port: existing.imap_port,
            imap_ssl: existing.imap_ssl,
            imap_ssl_mode: existing.imap_ssl_mode,
            smtp_host: existing.smtp_host,
            smtp_port: existing.smtp_port,
            smtp_ssl: existing.smtp_ssl,
            smtp_ssl_mode: existing.smtp_ssl_mode,
            color: req.color.or(existing.color),
            sync_enabled: req.sync_enabled.or(existing.sync_enabled),
            last_sync_at: existing.last_sync_at,
            auth_type: existing.auth_type,
            account_type: existing.account_type,
            created_at: existing.created_at,
            updated_at: chrono::Utc::now().timestamp(),
        };

        // 保存到数据库
        let account = account_repo::update(&self.db, req.id, model).await?;
        Ok(account.into())
    }

    /// 删除账号
    ///
    /// ⚠️ **危险操作**：删除账号会：
    /// 1. 从数据库删除账号记录
    /// 2. 从系统 Keyring 删除密码/token
    /// 3. 关联的邮件、标签等数据需要额外清理
    ///
    /// # 参数
    ///
    /// - `id`: 要删除的账号 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 安全提示
    ///
    /// - 此操作不可逆
    /// - 建议在前端显示确认对话框
    /// - 建议先检查是否有关联的邮件需要处理
    ///
    /// # 示例（前端确认对话框）
    ///
    /// ```typescript,ignore
    /// if (confirm(`确定要删除账号 ${account.email} 吗？此操作不可恢复！`)) {
    ///     await deleteAccount(account.id);
    /// }
    /// ```
    pub async fn delete(&self, id: i32) -> Result<(), MailError> {
        tracing::info!(id, "删除账号");

        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;

        self.db
            .transaction(move |tx| {
                label_repo::delete_by_account_tx(tx, id)?;
                email_repo::delete_by_account_tx(tx, id)?;
                sync_repo::delete_by_account_tx(tx, id)?;
                account_repo::delete_tx(tx, id)?;
                Ok(())
            })
            .await?;

        let _ = self.auth.delete_password(&account.email);

        Ok(())
    }

    /// 更新账号密码
    ///
    /// 用于密码认证的账号更新密码，或者 OAuth2 账号更新 refresh_token。
    ///
    /// # 参数
    ///
    /// - `id`: 账号 ID
    /// - `password`: 新的密码或认证凭证
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 安全说明
    ///
    /// - 密码直接存储到系统 Keyring，不在数据库中
    /// - Keyring 使用操作系统提供的加密存储
    /// - 支持的 Keyring：Windows Credential Manager、macOS Keychain、Linux Secret Service
    ///
    /// # 使用场景
    ///
    /// - 用户修改密码
    /// - OAuth2 token 刷新
    /// - 密码失效后重新输入
    pub async fn update_password(&self, id: i32, password: String) -> Result<(), MailError> {
        tracing::info!(id, "更新账号密码");

        // 检查账号是否存在
        let account = account_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::AccountNotFound(id))?;

        // 更新系统 Keyring 中的密码/token
        self.auth.save_password(&account.email, &password)?;
        Ok(())
    }

    /// 从 OAuth2 回调创建账号
    ///
    /// 这是 OAuth2 认证流程的最后一步，当用户完成 OAuth2 授权后，
    /// 系统自动使用获取的信息创建账号。
    ///
    /// # 流程说明
    ///
    /// 1. 用户点击"添加账号"，选择 OAuth2 方式
    /// 2. 系统打开浏览器进行 OAuth2 授权
    /// 3. 用户授权后，服务器获取 access_token 和 refresh_token
    /// 4. 调用此方法创建账号（邮箱地址已从 OAuth2 响应中获取）
    /// 5. refresh_token 存储到 Keyring
    ///
    /// # 参数
    ///
    /// - `params`: OAuth2 账号创建参数（包含邮箱、服务器配置等）
    ///
    /// # 返回
    ///
    /// 成功时返回创建的账号 DTO，失败时返回错误。
    ///
    /// # 注意事项
    ///
    /// - 此方法不存储密码，因为 OAuth2 认证不需要密码
    /// - OAuth2 的 refresh_token 会在 OAuth2 流程中单独存储到 Keyring
    /// - 账号名称默认使用邮箱地址 @ 符号前的部分
    pub async fn create_oauth2_account(
        &self,
        params: CreateOAuth2AccountParams,
    ) -> Result<AccountDto, MailError> {
        tracing::info!(email = %params.email, "创建 OAuth2 账号");

        // 获取当前时间戳
        let now = chrono::Utc::now().timestamp();

        // 从邮箱地址生成账号名称（取 @ 符号前的部分）
        let account_name = params
            .email
            .split('@')
            .next()
            .unwrap_or(&params.email)
            .to_string();

        let model = account_repo::AccountWrite {
            name: account_name,
            email: params.email,
            display_name: params.display_name,
            provider: params.provider_id,
            // IMAP 服务器配置
            imap_host: Some(params.imap_host),
            imap_port: Some(params.imap_port as i32),
            imap_ssl: Some(true),
            imap_ssl_mode: Some(params.imap_ssl_mode),
            // SMTP 服务器配置
            smtp_host: Some(params.smtp_host),
            smtp_port: Some(params.smtp_port as i32),
            smtp_ssl: Some(true),
            smtp_ssl_mode: Some(params.smtp_ssl_mode),
            // 其他属性
            color: params.color,
            auth_type: Some("OAuth2".to_string()),
            account_type: "personal".to_string(),
            sync_enabled: Some(true),
            last_sync_at: None,
            created_at: now,
            updated_at: now,
        };

        // 持久化到数据库
        let account = account_repo::create(&self.db, model).await?;
        Ok(account.into())
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 辅助结构体
// ═════════════════════════════════════════════════════════════════════════

/// OAuth2 账号创建参数
///
/// 这个结构体由 OAuth2 模块填充，传递给 AccountService 用于创建账号。
/// 包含了从 OAuth2 提供商获取的信息以及预设的服务器配置。
///
/// # 字段说明
///
/// - `email`: 从 OAuth2 响应中获取的用户邮箱地址
/// - `display_name`: 用户的显示名称（从 OAuth2 响应获取）
/// - `provider_id`: 服务提供商标识（如 "gmail"）
/// - `imap_host/smtp_host`: 预配置的服务器地址
/// - `imap_port/smtp_port`: 预配置的服务器端口
/// - `imap_ssl_mode/smtp_ssl_mode`: SSL 连接模式
/// - `color`: 账号主题颜色（可选）
///
/// # 使用流程
///
/// ```rust,ignore
/// // 1. OAuth2 流程完成，获取用户信息
/// let user_info = oauth2_service.get_user_info(access_token).await?;
///
/// // 2. 获取预配置的服务器参数
/// let server_config = provider_pool.get(&provider).get_server_config();
///
/// // 3. 创建账号参数
/// let params = CreateOAuth2AccountParams {
///     email: user_info.email,
///     display_name: user_info.name,
///     provider_id: "gmail".to_string(),
///     imap_host: server_config.imap.host,
///     imap_port: server_config.imap.port,
///     imap_ssl_mode: "TLS".to_string(),
///     smtp_host: server_config.smtp.host,
///     smtp_port: server_config.smtp.port,
///     smtp_ssl_mode: "TLS".to_string(),
///     color: Some("#ea4335".to_string()),
/// };
///
/// // 4. 调用 Service 创建账号
/// let account = account_service.create_oauth2_account(params).await?;
/// ```
#[derive(Debug, Clone)]
pub struct CreateOAuth2AccountParams {
    /// 用户邮箱地址
    pub email: String,
    /// 用户显示名称
    pub display_name: Option<String>,
    /// 服务提供商标识
    pub provider_id: String,
    /// IMAP 服务器地址
    pub imap_host: String,
    /// IMAP 服务器端口
    pub imap_port: u16,
    /// IMAP SSL 模式
    pub imap_ssl_mode: String,
    /// SMTP 服务器地址
    pub smtp_host: String,
    /// SMTP 服务器端口
    pub smtp_port: u16,
    /// SMTP SSL 模式
    pub smtp_ssl_mode: String,
    /// 主题颜色（可选）
    pub color: Option<String>,
}
