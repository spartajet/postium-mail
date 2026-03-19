# Postium Mail 架构迁移完整计划

> **目标**: 从现有架构迁移到新的 FlowEngine 架构体系
> **范围**: 前后端完整迁移，允许破坏性 API 变更
> **预计时间**: 2-3周

---

## 📋 目录

1. [架构对比分析](#架构对比分析)
2. [后端迁移计划](#后端迁移计划)
3. [前端迁移计划](#前端迁移计划)
4. [数据库迁移](#数据库迁移)
5. [测试策略](#测试策略)
6. [回滚方案](#回滚方案)

---

## 📊 架构对比分析

### 现有架构问题

```
┌─────────────────────────────────────────────────────────┐
│                    现有架构（需要重构）                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  前端 Vue                                              │
│    ↓ 直接调用 Tauri Commands                           │
│  命令层 (command/)                                     │
│    ↓ 直接调用                                          │
│  服务层 (services/) - ⚠️ 问题区域                      │
│    ├─ account_service.rs    - 业务逻辑混杂             │
│    ├─ email_service.rs      - 直接操作数据库            │
│    ├─ sync_manager.rs       - 同步逻辑耦合             │
│    ├─ imap/service.rs       - IMAP 协议实现             │
│    └─ oauth_service.rs      - OAuth 硬编码              │
│    ↓                                                   │
│  数据层 (db.rs)                                        │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 新架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    新架构（目标）                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  前端 Vue                                                  │
│    ↓ 通过统一的 API 调用                                    │
│  命令层 (command/) - 薄层，仅做转换                         │
│    ↓                                                       │
│  引擎层 (engine/) - FlowEngine 协调中心                    │
│    ├─ flow_engine.rs      - 核心引擎                       │
│    ├─ task_scheduler.rs   - 定时任务                       │
│    └─ notification_manager.rs - 通知管理                   │
│    ↓                                                       │
│  业务层 (services/) - 重新设计的业务层                    │
│    ├─ auth/               - 认证管理                       │
│    │   ├─ auth_manager.rs      - 统一认证入口              │
│    │   ├─ token_manager.rs     - Token 管理               │
│    │   └─ oauth_handler.rs     - OAuth 处理               │
│    ├─ providers/          - 服务商抽象层                   │
│    │   ├─ traits.rs            - MailProvider trait       │
│    │   ├─ provider_pool.rs     - 服务商池                  │
│    │   ├─ gmail.rs             - Gmail 实现               │
│    │   ├─ outlook.rs           - Outlook 实现             │
│    │   ├─ microsoft_365.rs     - Microsoft 365 实现       │
│    │   └─ native.rs            - 本地服务商实现           │
│    ├─ sync/               - 同步引擎                       │
│    │   ├─ sync_manager.rs      - 同步管理器               │
│    │   ├─ delta_sync.rs        - 增量同步                 │
│    │   └─ change_detector.rs   - 变更检测                 │
│    └─ operations/         - 操作管理                       │
│        ├─ operation_manager.rs - 操作队列                  │
│        └─ conflict_resolver.rs  - 冲突解决                │
│    ↓                                                       │
│  协议层 (protocols/) - IMAP/SMTP 实现                      │
│    ├─ imap/client.rs      - 异步 IMAP 客户端              │
│    └─ smtp/sender.rs      - SMTP 发送器                   │
│    ↓                                                       │
│  存储层 (storage/)                                         │
│    ├─ database.rs         - 数据库操作                     │
│    └─ cache.rs            - 缓存管理                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 🔧 后端迁移计划

### 阶段 1: 基础设施搭建 (2-3天)

#### 1.1 创建新的目录结构

```bash
src-tauri/src/
├── engine/              # 引擎层
│   ├── mod.rs
│   ├── flow_engine.rs
│   ├── task_scheduler.rs
│   └── notification_manager.rs
├── services/            # 业务层（重新组织）
│   ├── mod.rs
│   ├── auth/            # 认证模块
│   │   ├── mod.rs
│   │   ├── auth_manager.rs
│   │   ├── token_manager.rs
│   │   └── oauth_handler.rs
│   ├── providers/       # 服务商抽象
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── provider_pool.rs
│   │   ├── gmail.rs
│   │   ├── outlook.rs
│   │   ├── microsoft_365.rs
│   │   ├── google_workspace.rs
│   │   ├── yahoo.rs
│   │   └── native.rs
│   ├── sync/            # 同步引擎
│   │   ├── mod.rs
│   │   ├── sync_manager.rs
│   │   ├── delta_sync.rs
│   │   └── change_detector.rs
│   ├── operations/      # 操作管理
│   │   ├── mod.rs
│   │   ├── operation_manager.rs
│   │   └── conflict_resolver.rs
│   └── email/           # 邮件操作
│       ├── mod.rs
│       └── email_service.rs
├── protocols/           # 协议层
│   ├── mod.rs
│   ├── imap/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── types.rs
│   └── smtp/
│       ├── mod.rs
│       └── sender.rs
└── storage/             # 存储层
    ├── mod.rs
    ├── database.rs
    └── cache.rs
```

#### 1.2 实现核心错误类型

**文件**: `src-tauri/src/error.rs`

```rust
//! 统一错误类型定义
//!
//! 新架构使用结构化错误，支持：
//! - 错误分类（网络、认证、同步等）
//! - 错误严重程度
//! - 自动重试策略
//! - 用户友好的错误消息

use thiserror::Error;

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Low,      // 低：不影响核心功能
    Medium,   // 中：影响部分功能
    High,     // 高：影响核心功能
    Critical, // 严重：应用无法使用
}

/// 邮件操作主错误类型
#[derive(Error, Debug)]
pub enum MailError {
    #[error("连接错误: {message}")]
    Connection {
        message: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
        retry_after: Option<u64>, // 秒
    },

    #[error("认证失败: {message}")]
    Authentication {
        message: String,
        need_reauth: bool,
    },

    #[error("同步错误: {message}")]
    Sync {
        message: String,
        folder: Option<String>,
    },

    #[error("OAuth 错误: {message}")]
    OAuth {
        message: String,
        error_code: Option<String>,
    },

    #[error("存储错误: {message}")]
    Storage {
        message: String,
        path: Option<String>,
    },

    #[error("限流: 请稍后再试")]
    RateLimit {
        retry_after: u64, // 秒
    },

    #[error("协议错误: {message}")]
    Protocol {
        message: String,
        command: Option<String>,
    },

    #[error("配置错误: {message}")]
    Config {
        message: String,
        field: Option<String>,
    },

    #[error("权限错误: {message}")]
    Permission {
        message: String,
        required_permission: String,
    },

    #[error("配额超限: {message}")]
    Quota {
        message: String,
        current_usage: u64,
        limit: u64,
    },

    #[error("超时: 操作 {operation} 超时")]
    Timeout {
        operation: String,
        timeout_secs: u64,
    },
}

impl MailError {
    /// 获取错误严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            MailError::Connection { .. } => ErrorSeverity::High,
            MailError::Authentication { .. } => ErrorSeverity::High,
            MailError::Sync { .. } => ErrorSeverity::Medium,
            MailError::OAuth { .. } => ErrorSeverity::High,
            MailError::Storage { .. } => ErrorSeverity::Critical,
            MailError::RateLimit { .. } => ErrorSeverity::Medium,
            MailError::Protocol { .. } => ErrorSeverity::Medium,
            MailError::Config { .. } => ErrorSeverity::Critical,
            MailError::Permission { .. } => ErrorSeverity::High,
            MailError::Quota { .. } => ErrorSeverity::Medium,
            MailError::Timeout { .. } => ErrorSeverity::Medium,
        }
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        match self {
            MailError::Connection { .. } => true,
            MailError::RateLimit { .. } => true,
            MailError::Timeout { .. } => true,
            MailError::OAuth { error_code, .. } => {
                // 某些 OAuth 错误可重试
                error_code.as_ref().map_or(false, |c| {
                    matches!(c.as_str(), "temporarily_unavailable" | "server_error")
                })
            }
            _ => false,
        }
    }

    /// 获取重试延迟（秒）
    pub fn retry_delay(&self) -> Option<u64> {
        match self {
            MailError::Connection { retry_after, .. } => *retry_after,
            MailError::RateLimit { retry_after } => Some(*retry_after),
            MailError::Timeout { .. } => Some(5),
            _ => None,
        }
    }

    /// 获取用户友好的错误消息
    pub fn user_message(&self) -> String {
        match self {
            MailError::Connection { message, .. } => {
                format!("无法连接到服务器: {}", message)
            }
            MailError::Authentication { message, .. } => {
                format!("认证失败: {}", message)
            }
            MailError::OAuth { message, .. } => {
                format!("登录授权失败: {}", message)
            }
            MailError::RateLimit { .. } => {
                "操作过于频繁，请稍后再试".to_string()
            }
            MailError::Timeout { operation, .. } => {
                format!("操作超时: {}", operation)
            }
            _ => self.to_string(),
        }
    }
}

/// 从现有错误类型转换
impl From<sqlx::Error> for MailError {
    fn from(err: sqlx::Error) -> Self {
        MailError::Storage {
            message: err.to_string(),
            path: None,
        }
    }
}

impl From<async_imap::Error> for MailError {
    fn from(err: async_imap::Error) -> Self {
        match err {
            async_imap::Error::ConnectionLost => MailError::Connection {
                message: "连接中断".to_string(),
                source: Some(Box::new(err)),
                retry_after: Some(5),
            },
            async_imap::Error::No(_) => MailError::Timeout {
                operation: "IMAP 操作".to_string(),
                timeout_secs: 30,
            },
            _ => MailError::Protocol {
                message: err.to_string(),
                command: None,
            },
        }
    }
}
```

### 阶段 2: 服务商层实现 (3-4天)

#### 2.1 实现 MailProvider Trait

**文件**: `src-tauri/src/services/providers/traits.rs`

```rust
//! 邮件服务商 Trait 定义
//!
//! 提供统一的邮件服务商抽象，支持个人和企业邮箱

use async_trait::async_trait;

/// 账号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Personal,  // 个人邮箱
    Enterprise, // 企业邮箱
}

impl Default for AccountType {
    fn default() -> Self {
        Self::Personal
    }
}

/// 认证类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthType {
    Password,        // 密码认证
    OAuth2,          // OAuth 2.0
    AppPassword,     // 应用专用密码
    DomainAuth,      // 域认证（企业）
    SamlSso,         // SAML SSO（企业）
}

/// SSL 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    None,       // 无加密
    StartTls,   // STARTTLS
    Implicit,   // 隐式 SSL/TLS
}

/// IMAP 配置
#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

/// SMTP 配置
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

/// OAuth 配置
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub pkce_enabled: bool,
    pub tenant_id: Option<String>, // 企业租户 ID
}

/// 企业配置
#[derive(Debug, Clone)]
pub struct EnterpriseConfig {
    pub tenant_id: Option<String>,
    pub domain: Option<String>,
    pub conditional_access: bool,
    pub mfa_required: bool,
    pub custom_server: Option<String>,
    pub custom_imap: Option<ImapConfig>,
    pub custom_smtp: Option<SmtpConfig>,
}

/// OAuth Token
#[derive(Debug, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub id_token: Option<String>,
    pub tenant_id: Option<String>,
}

/// 服务商能力
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub supports_idle: bool,
    pub supports_condstore: bool,
    pub supports_push: bool,
    pub supports_oauth: bool,
    pub supports_enterprise: bool,
    pub max_message_size: Option<usize>,
}

/// 邮件服务商 Trait
#[async_trait]
pub trait MailProvider: Send + Sync {
    /// 获取服务商唯一标识
    fn provider_id(&self) -> &'static str;

    /// 获取服务商名称
    fn provider_name(&self) -> String;

    /// 获取账号类型
    fn account_type(&self) -> AccountType;

    /// 支持的认证类型
    fn auth_types(&self) -> Vec<AuthType>;

    /// 默认 IMAP 配置
    fn default_imap_config(&self) -> ImapConfig;

    /// 默认 SMTP 配置
    fn default_smtp_config(&self) -> SmtpConfig;

    /// OAuth 配置（如果支持）
    fn oauth_config(&self) -> Option<&OAuthConfig>;

    /// 企业配置（如果支持）
    fn enterprise_config(&self) -> Option<&EnterpriseConfig>;

    /// 服务商能力
    fn capabilities(&self) -> &ProviderCapabilities;

    /// 自动检测邮箱是否属于此服务商
    async fn detect(&self, email: &str) -> Result<bool, crate::error::MailError>;

    /// 支持的域名列表
    fn supported_domains(&self) -> Vec<&'static str>;

    /// 生成 XOAUTH2 字符串
    fn generate_xoauth2(&self, email: &str, token: &str) -> String {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            email, token
        )
    }

    /// 克隆为 trait object
    fn box_clone(&self) -> Box<dyn MailProvider>;
}

/// 克隆实现
impl Clone for Box<dyn MailProvider> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
```

#### 2.2 实现 Gmail 服务商

**文件**: `src-tauri/src/services/providers/gmail.rs`

```rust
//! Gmail 服务商实现

use super::traits::*;
use crate::error::MailError;

pub struct GmailProvider {
    oauth_config: OAuthConfig,
}

impl GmailProvider {
    pub fn new(oauth_config: OAuthConfig) -> Self {
        Self { oauth_config }
    }

    pub fn from_env() -> Result<Self, MailError> {
        let client_id = std::env::var("GMAIL_CLIENT_ID")
            .map_err(|_| MailError::Config {
                message: "GMAIL_CLIENT_ID 未设置".to_string(),
                field: Some("GMAIL_CLIENT_ID".to_string()),
            })?;

        let oauth_config = OAuthConfig {
            client_id,
            client_secret: std::env::var("GMAIL_CLIENT_SECRET").ok(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri: "http://localhost:1420/callback".to_string(),
            scopes: vec![
                "https://mail.google.com/".to_string(),
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        };

        Ok(Self { oauth_config })
    }
}

#[async_trait]
impl MailProvider for GmailProvider {
    fn provider_id(&self) -> &'static str {
        "gmail"
    }

    fn provider_name(&self) -> String {
        "Gmail".to_string()
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::OAuth2]
    }

    fn default_imap_config(&self) -> ImapConfig {
        ImapConfig {
            host: "imap.gmail.com".to_string(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpConfig {
        SmtpConfig {
            host: "smtp.gmail.com".to_string(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<&OAuthConfig> {
        Some(&self.oauth_config)
    }

    fn enterprise_config(&self) -> Option<&EnterpriseConfig> {
        None
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        static CAPABILITIES: ProviderCapabilities = ProviderCapabilities {
            supports_idle: true,
            supports_condstore: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: false,
            max_message_size: Some(25 * 1024 * 1024), // 25MB
        };
        &CAPABILITIES
    }

    async fn detect(&self, email: &str) -> Result<bool, MailError> {
        Ok(email.ends_with("@gmail.com") || email.ends_with("@googlemail.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmail.com", "googlemail.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Self::new(self.oauth_config.clone()))
    }
}
```

#### 2.3 实现 ProviderPool

**文件**: `src-tauri/src/services/providers/provider_pool.rs`

```rust
//! 服务商池管理

use super::traits::*;
use crate::error::MailError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ProviderPool {
    personal_providers: HashMap<String, Arc<dyn MailProvider>>,
    enterprise_providers: HashMap<String, Arc<dyn MailProvider>>,
    detection_order: Vec<String>,
}

impl ProviderPool {
    pub fn new() -> Self {
        let mut pool = Self {
            personal_providers: HashMap::new(),
            enterprise_providers: HashMap::new(),
            detection_order: Vec::new(),
        };

        // 注册默认服务商
        pool.register_defaults();

        pool
    }

    fn register_defaults(&mut self) {
        // 个人邮箱
        self.register_personal("gmail", Arc::new(GmailProvider::default()));
        self.register_personal("outlook", Arc::new(OutlookProvider::default()));
        self.register_personal("yahoo", Arc::new(YahooProvider::default()));

        // 企业邮箱
        self.register_enterprise("microsoft365", Arc::new(Microsoft365Provider::default()));
        self.register_enterprise("googleworkspace", Arc::new(GoogleWorkspaceProvider::default()));
    }

    pub fn register_personal(&mut self, id: String, provider: Arc<dyn MailProvider>) {
        self.detection_order.push(id.clone());
        self.personal_providers.insert(id, provider);
    }

    pub fn register_enterprise(&mut self, id: String, provider: Arc<dyn MailProvider>) {
        self.enterprise_providers.insert(id, provider);
    }

    /// 自动检测服务商
    pub async fn detect_provider(
        &self,
        email: &str,
    ) -> Result<Arc<dyn MailProvider>, MailError> {
        let domain = email.split('@').last().unwrap_or("");

        // 1. 先检查是否是已知的企业域名
        for provider in self.enterprise_providers.values() {
            if provider.supported_domains().contains(&domain) {
                return Ok(provider.clone());
            }
        }

        // 2. 再检查个人邮箱
        for provider in self.personal_providers.values() {
            if provider.supported_domains().contains(&domain) {
                return Ok(provider.clone());
            }
        }

        // 3. 尝试自动检测
        for provider in self.personal_providers.values() {
            if provider.detect(email).await? {
                return Ok(provider.clone());
            }
        }

        // 4. 默认返回通用 IMAP
        Ok(Arc::new(NativeProvider::new_generic()))
    }

    /// 获取服务商
    pub async fn get_provider(&self, provider_id: &str) -> Option<Arc<dyn MailProvider>> {
        self.personal_providers
            .get(provider_id)
            .or_else(|| self.enterprise_providers.get(provider_id))
            .cloned()
    }

    /// 创建自定义企业邮箱服务商
    pub fn create_custom_provider(
        &self,
        imap_config: ImapConfig,
        smtp_config: SmtpConfig,
        name: String,
    ) -> Arc<dyn MailProvider> {
        Arc::new(CustomEnterpriseProvider::new(imap_config, smtp_config, name))
    }
}

impl Default for ProviderPool {
    fn default() -> Self {
        Self::new()
    }
}
```

### 阶段 3: 认证层重构 (2-3天)

#### 3.1 实现 AuthManager

**文件**: `src-tauri/src/services/auth/auth_manager.rs`

```rust
//! 认证管理器
//!
//! 统一处理个人和企业邮箱的认证

use super::token_manager::TokenManager;
use super::oauth_handler::OAuthHandler;
use super::traits::*;
use crate::error::MailError;
use std::sync::Arc;

pub struct AuthResult {
    pub account_id: i64,
    pub email: String,
    pub provider_id: String,
    pub auth_type: AuthType,
}

pub struct AuthManager {
    provider_pool: Arc<ProviderPool>,
    oauth_handler: OAuthHandler,
    token_manager: TokenManager,
    password_auth: PasswordAuth,
}

impl AuthManager {
    pub fn new(
        provider_pool: Arc<ProviderPool>,
        db: Arc<Database>,
        keyring: Arc<Keyring>,
    ) -> Self {
        Self {
            provider_pool,
            oauth_handler: OAuthHandler::new(),
            token_manager: TokenManager::new(db, keyring),
            password_auth: PasswordAuth::new(),
        }
    }

    /// 统一认证入口
    pub async fn authenticate(
        &self,
        email: &str,
        auth_type: AuthType,
        password: Option<String>,
        oauth_code: Option<String>,
    ) -> Result<AuthResult, MailError> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;

        // 2. 根据认证类型处理
        match auth_type {
            AuthType::Password => {
                self.password_auth
                    .authenticate(email, password.unwrap(), &provider)
                    .await
            }
            AuthType::OAuth2 => {
                self.oauth_handler
                    .authenticate(email, oauth_code.unwrap(), &provider)
                    .await
            }
            _ => Err(MailError::Authentication {
                message: "不支持的认证类型".to_string(),
                need_reauth: false,
            }),
        }
    }

    /// 企业邮箱认证
    pub async fn authenticate_enterprise(
        &self,
        email: &str,
        domain: &str,
        auth_type: AuthType,
    ) -> Result<AuthResult, MailError> {
        // 企业邮箱特殊处理
        todo!()
    }
}
```

### 阶段 4: 同步引擎重构 (3-4天)

#### 4.1 重新实现 SyncManager

**文件**: `src-tauri/src/services/sync/sync_manager.rs`

```rust
//! 同步管理器
//!
//! 基于新架构的同步引擎

use super::delta_sync::DeltaSync;
use super::change_detector::ChangeDetector;
use crate::error::MailError;
use crate::protocols::imap::AsyncImapClient;
use crate::engine::FlowEngine;

pub struct SyncManager {
    db: Arc<Database>,
    delta_sync: DeltaSync,
    change_detector: ChangeDetector,
    engine: Arc<FlowEngine>,
}

impl SyncManager {
    pub fn new(db: Arc<Database>, engine: Arc<FlowEngine>) -> Self {
        Self {
            db: db.clone(),
            delta_sync: DeltaSync::new(db.clone()),
            change_detector: ChangeDetector::new(db),
            engine,
        }
    }

    /// 执行完整同步
    pub async fn sync_account(
        &self,
        account_id: i64,
        progress_callback: impl Fn(SyncProgress),
    ) -> Result<SyncResult, MailError> {
        // 1. 获取账号信息
        let account = self.db.get_account(account_id).await?;

        // 2. 创建 IMAP 客户端
        let mut client = AsyncImapClient::connect(&account).await?;

        // 3. 获取服务商
        let provider = self
            .engine
            .provider_pool
            .detect_provider(&account.email)
            .await?;

        // 4. 同步文件夹
        progress_callback(SyncProgress {
            stage: SyncStage::SyncingFolders,
            current: 0,
            total: 0,
            message: "同步文件夹...".to_string(),
        });
        let folders = self.sync_folders(&mut client, account_id).await?;

        // 5. 增量同步邮件
        let mut total_synced = 0;
        for folder in &folders {
            progress_callback(SyncProgress {
                stage: SyncStage::SyncingEmails,
                current: 0,
                total: 0,
                message: format!("同步 {} 文件夹...", folder.name),
            });

            let synced = self
                .sync_folder(&mut client, account_id, folder, &provider)
                .await?;

            total_synced += synced;
        }

        // 6. 更新同步时间
        self.db
            .update_account_sync_time(account_id)
            .await?;

        Ok(SyncResult {
            total_synced,
            folders_synced: folders.len() as u32,
            errors: 0,
            duration_ms: 0,
        })
    }

    /// 同步单个文件夹
    async fn sync_folder(
        &self,
        client: &mut AsyncImapClient,
        account_id: i64,
        folder: &Folder,
        provider: &Arc<dyn MailProvider>,
    ) -> Result<u32, MailError> {
        // 1. 检查是否支持 CONDSTORE
        let supports_condstore = provider.capabilities().supports_condstore
            && client.check_condstore_support().await?;

        // 2. 执行增量同步
        let result = if supports_condstore {
            self.delta_sync
                .sync_with_condstore(client, account_id, &folder.imap_name)
                .await?
        } else {
            self.delta_sync
                .sync_with_uid_search(client, account_id, &folder.imap_name)
                .await?
        };

        // 3. 检测变更
        let changes = self
            .change_detector
            .detect_changes(client, account_id, &folder.imap_name)
            .await?;

        // 4. 处理变更
        self.process_changes(account_id, changes).await?;

        Ok(result.new_or_updated.len() as u32)
    }
}
```

### 阶段 5: 命令层适配 (2天)

#### 5.1 重构账号命令

**文件**: `src-tauri/src/command/account.rs`

```rust
//! 账号相关命令
//!
//! 新架构下，命令层只负责参数转换和调用引擎

use crate::engine::FlowEngine;
use crate::error::MailError;

#[tauri::command]
pub async fn list_accounts(
    engine: tauri::State<'_, FlowEngine>,
) -> Result<Vec<AccountDto>, MailError> {
    let accounts = engine.list_accounts().await?;
    Ok(accounts.into_iter().map(AccountDto::from).collect())
}

#[tauri::command]
pub async fn add_account(
    engine: tauri::State<'_, FlowEngine>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    let account = engine.add_account(request.into()).await?;
    Ok(AccountDto::from(account))
}

#[tauri::command]
pub async fn test_account_connection(
    engine: tauri::State<'_, FlowEngine>,
    request: CreateAccountRequest,
) -> Result<ConnectionTestResult, MailError> {
    let result = engine.test_connection(request.into()).await?;
    Ok(ConnectionTestResult::from(result))
}

// DTO 类型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub email: String,
    pub provider: String,
    pub password: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDto {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub connect_time: u64,
    pub login_time: u64,
    pub email_count: u32,
    pub error: Option<String>,
}
```

---

## 🎨 前端迁移计划

### 阶段 1: 类型定义更新 (1天)

#### 1.1 更新 API 接口类型

**文件**: `src/types/api.ts`

```typescript
/**
 * 新架构 API 类型定义
 *
 * 与后端新架构保持一致
 */

// ========================================
// 账号相关类型
// ========================================

export enum AccountType {
  Personal = 'personal',
  Enterprise = 'enterprise',
}

export enum AuthType {
  Password = 'password',
  OAuth2 = 'oauth2',
  AppPassword = 'app_password',
  DomainAuth = 'domain_auth',
  SamlSso = 'saml_sso',
}

export enum SslMode {
  None = 'none',
  StartTls = 'start_tls',
  Implicit = 'implicit',
}

export interface Account {
  id: string;
  name: string;
  email: string;
  provider: string;
  accountType: AccountType;
  authType: AuthType;

  // IMAP 配置
  imapHost?: string;
  imapPort?: number;
  imapSsl?: SslMode;

  // SMTP 配置
  smtpHost?: string;
  smtpPort?: number;
  smtpSsl?: SslMode;

  // UI 相关
  color: string;
  unreadCount: number;

  // 同步状态
  syncEnabled: boolean;
  lastSyncAt?: Date;

  // 企业邮箱特有
  enterpriseConfig?: {
    tenantId?: string;
    domain?: string;
    conditionalAccess?: boolean;
    mfaRequired?: boolean;
  };

  // OAuth 相关
  oauthToken?: {
    accessToken: string;
    refreshToken?: string;
    expiresAt?: number;
  };

  created_at: Date;
  updated_at: Date;
}

export interface CreateAccountRequest {
  name: string;
  email: string;
  provider?: string;
  accountType?: AccountType;
  authType: AuthType;

  // 认证信息（二选一）
  password?: string;
  oauthCode?: string;

  // 自定义配置（可选）
  imapHost?: string;
  imapPort?: number;
  imapSsl?: SslMode;
  smtpHost?: string;
  smtpPort?: number;
  smtpSsl?: SslMode;
  color?: string;

  // 企业配置
  enterpriseTenantId?: string;
  enterpriseDomain?: string;
}

// ========================================
// 同步相关类型
// ========================================

export enum SyncStage {
  Connecting = 'connecting',
  Authenticating = 'authenticating',
  SyncingFolders = 'syncing_folders',
  SyncingEmails = 'syncing_emails',
  Completed = 'completed',
  Error = 'error',
}

export interface SyncProgress {
  accountId: number;
  stage: SyncStage;
  folder?: string;
  current: number;
  total: number;
  message: string;
}

export interface SyncResult {
  totalSynced: number;
  foldersSynced: number;
  errors: number;
  durationMs: number;
}

// ========================================
// 邮件相关类型
// ========================================

export interface Email {
  id: string;
  accountId: string;
  folder: string;
  uid?: number;

  // 内容
  subject: string;
  sender: string;
  senderEmail: string;
  recipient: string;
  cc?: string;
  bcc?: string;
  body: string;
  snippet?: string;

  // 状态
  unread: boolean;
  starred: boolean;
  flagged?: boolean;
  draft: boolean;

  // 时间
  date: Date;
  receivedAt: Date;
  sentAt?: Date;

  // 附件
  attachments: Attachment[];
  hasAttachment: boolean;
  attachmentCount: number;

  // 标签（Gmail 等）
  labels: string[];

  // 线程信息
  threadId?: string;
  inReplyTo?: string;
  references?: string[];
}

export interface Attachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  path?: string;
  url?: string;
}

// ========================================
// 文件夹相关类型
// ========================================

export interface Folder {
  id: string;
  accountId: string;
  name: string;
  imapName: string;
  emailCount: number;
  unreadCount: number;
  selectable: boolean;
}

// ========================================
// 操作相关类型
// ========================================

export enum OperationType {
  MarkRead = 'mark_read',
  MarkUnread = 'mark_unread',
  ToggleFlag = 'toggle_flag',
  MoveEmail = 'move_email',
  DeleteEmail = 'delete_email',
  PermanentDelete = 'permanent_delete',
  RestoreEmail = 'restore_email',
}

export interface Operation {
  id: string;
  accountId: string;
  type: OperationType;
  emailIds: string[];
  status: 'pending' | 'processing' | 'completed' | 'failed';
  error?: string;
  createdAt: Date;
  completedAt?: Date;
}

// ========================================
// 错误类型
// ========================================

export enum ErrorSeverity {
  Low = 'low',
  Medium = 'medium',
  High = 'high',
  Critical = 'critical',
}

export interface MailError {
  code: string;
  message: string;
  userMessage: string;
  severity: ErrorSeverity;
  retryable: boolean;
  retryAfter?: number;
}

// ========================================
// 通知类型
// ========================================

export interface NewMailNotification {
  accountId: string;
  accountName: string;
  count: number;
  messages: {
    from: string;
    subject: string;
    date: Date;
  }[];
  summary: string;
}

// ========================================
// 搜索相关类型
// ========================================

export interface SearchQuery {
  query: string;
  folder?: string;
  accountId?: string;
  fields?: SearchField[];
  dateRange?: {
    start: Date;
    end: Date;
  };
  hasAttachment?: boolean;
}

export enum SearchField {
  From = 'from',
  To = 'to',
  Cc = 'cc',
  Subject = 'subject',
  Body = 'body',
  AttachmentName = 'attachment_name',
}

export interface SearchResult {
  items: Email[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
  queryTimeMs: number;
}

// ========================================
// 发送相关类型
// ========================================

export interface SendEmailRequest {
  accountId: string;
  from: string;
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  body: string;
  bodyType: 'plain' | 'html';
  attachments?: {
    filename: string;
    contentType: string;
    data: ArrayBuffer;
  }[];
  inReplyTo?: string;
  references?: string[];
}

export interface SendStatus {
  status: 'draft' | 'queued' | 'sending' | 'sent' | 'failed';
  progress?: number;
  sentAt?: Date;
  error?: string;
  retryCount?: number;
}
```

### 阶段 2: API 客户端重构 (2天)

#### 2.1 创建统一的 API 客户端

**文件**: `src/api/client.ts`

```typescript
/**
 * 统一的 API 客户端
 *
 * 封装所有 Tauri 命令调用，提供类型安全
 */

import { invoke } from '@tauri-apps/api/core'
import type {
  Account,
  CreateAccountRequest,
  Email,
  Folder,
  SyncProgress,
  SyncResult,
  SearchQuery,
  SearchResult,
  SendEmailRequest,
  Operation,
  NewMailNotification,
  MailError,
  ConnectionTestResult,
} from '@/types/api'

/**
 * API 错误处理
 */
export class ApiError extends Error {
  constructor(
    public code: string,
    public userMessage: string,
    public severity: 'low' | 'medium' | 'high' | 'critical',
    public retryable: boolean,
    public retryAfter?: number,
  ) {
    super(userMessage)
    this.name = 'ApiError'
  }

  static fromError(error: any): ApiError {
    // 尝试解析后端返回的结构化错误
    if (error?.code) {
      return new ApiError(
        error.code,
        error.userMessage || error.message,
        error.severity || 'medium',
        error.retryable || false,
        error.retryAfter,
      )
    }

    // 默认错误处理
    return new ApiError(
      'UNKNOWN_ERROR',
      error?.message || '未知错误',
      'medium',
      false,
    )
  }
}

/**
 * 邮件 API 客户端
 */
export class MailApi {
  // ========================================
  // 账号相关
  // ========================================

  /**
   * 获取账号列表
   */
  static async listAccounts(): Promise<Account[]> {
    try {
      const dtos = await invoke<any[]>('list_accounts')
      return dtos.map(this.dtoToAccount)
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 添加账号
   */
  static async addAccount(request: CreateAccountRequest): Promise<Account> {
    try {
      const dto = await invoke('add_account', { request })
      return this.dtoToAccount(dto)
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 更新账号
   */
  static async updateAccount(
    id: string,
    updates: Partial<Account>,
  ): Promise<Account> {
    try {
      const dto = await invoke('update_account', {
        id: parseInt(id),
        updates,
      })
      return this.dtoToAccount(dto)
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 删除账号
   */
  static async deleteAccount(id: string): Promise<void> {
    try {
      await invoke('delete_account', { id: parseInt(id) })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 测试账号连接
   */
  static async testConnection(
    request: CreateAccountRequest,
  ): Promise<ConnectionTestResult> {
    try {
      return await invoke('test_account_connection', { request })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 触发账号同步
   */
  static async syncAccount(accountId: string): Promise<SyncResult> {
    try {
      return await invoke('sync_account_with_progress', {
        accountId: parseInt(accountId),
      })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  // ========================================
  // 邮件相关
  // ========================================

  /**
   * 获取邮件列表
   */
  static async listEmails(params: {
    accountId: string
    folder: string
    page?: number
    limit?: number
  }): Promise<{ emails: Email[]; total: number; page: number }> {
    try {
      const response = await invoke('list_emails', {
        accountId: parseInt(params.accountId),
        folder: params.folder,
        page: params.page || 0,
        limit: params.limit || 100,
      })
      return {
        emails: response.emails.map(this.dtoToEmail),
        total: response.total,
        page: response.page,
      }
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 获取邮件详情
   */
  static async getEmail(id: string): Promise<Email> {
    try {
      const dto = await invoke('get_email', { id: parseInt(id) })
      return this.dtoToEmail(dto)
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 标记已读/未读
   */
  static async markAsRead(emailId: string, isRead: boolean): Promise<void> {
    try {
      await invoke('mark_as_read', {
        emailId: parseInt(emailId),
        isRead,
      })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 切换星标
   */
  static async toggleStar(emailId: string): Promise<void> {
    try {
      await invoke('toggle_star', { emailId: parseInt(emailId) })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 移动邮件到文件夹
   */
  static async moveEmail(emailId: string, folder: string): Promise<void> {
    try {
      await invoke('move_email_to_folder', {
        emailId: parseInt(emailId),
        folder,
      })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  /**
   * 删除邮件（移动到垃圾箱）
   */
  static async deleteEmail(emailId: string): Promise<void> {
    try {
      await invoke('delete_emails', {
        emailIds: [parseInt(emailId)],
      })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  // ========================================
  // 搜索相关
  // ========================================

  /**
   * 搜索邮件
   */
  static async searchEmails(query: SearchQuery): Promise<SearchResult> {
    try {
      const result = await invoke('search_emails', { query })
      return {
        items: result.items.map(this.dtoToEmail),
        total: result.total,
        page: result.page,
        pageSize: result.page_size,
        hasMore: result.has_more,
        queryTimeMs: result.query_time_ms,
      }
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  // ========================================
  // 发送相关
  // ========================================

  /**
   * 发送邮件
   */
  static async sendEmail(request: SendEmailRequest): Promise<void> {
    try {
      await invoke('send_email', { request })
    } catch (error) {
      throw ApiError.fromError(error)
    }
  }

  // ========================================
  // 事件监听
  // ========================================

  /**
   * 监听同步进度
   */
  static onSyncProgress(
    accountId: string,
    callback: (progress: SyncProgress) => void,
  ): Promise<() => void> {
    const { listen } = require('@tauri-apps/api/event')

    return listen(`sync-progress-${accountId}`, (event: any) => {
      callback(event.payload as SyncProgress)
    }).then((unlisten: any) => unlisten)
  }

  /**
   * 监听新邮件通知
   */
  static onNewMail(
    callback: (notification: NewMailNotification) => void,
  ): Promise<() => void> {
    const { listen } = require('@tauri-apps/api/event')

    return listen('new-mail', (event: any) => {
      callback(event.payload as NewMailNotification)
    }).then((unlisten: any) => unlisten)
  }

  // ========================================
  // 辅助方法
  // ========================================

  /**
   * DTO 转换为 Account
   */
  private static dtoToAccount(dto: any): Account {
    return {
      id: dto.id.toString(),
      name: dto.name,
      email: dto.email,
      provider: dto.provider,
      accountType: dto.account_type || 'personal',
      authType: dto.auth_type || 'password',
      imapHost: dto.imap_host,
      imapPort: dto.imap_port,
      imapSsl: dto.imap_ssl,
      smtpHost: dto.smtp_host,
      smtpPort: dto.smtp_port,
      smtpSsl: dto.smtp_ssl,
      color: dto.color || '#7C3AED',
      unreadCount: 0, // 需要从后端获取
      syncEnabled: dto.sync_enabled ?? true,
      lastSyncAt: dto.last_sync_at ? new Date(dto.last_sync_at) : undefined,
      enterpriseConfig: dto.enterprise_config,
      oauthToken: dto.oauth_token,
      created_at: new Date(dto.created_at),
      updated_at: new Date(dto.updated_at),
    }
  }

  /**
   * DTO 转换为 Email
   */
  private static dtoToEmail(dto: any): Email {
    return {
      id: dto.id.toString(),
      accountId: dto.account_id.toString(),
      folder: dto.folder.toLowerCase(),
      uid: dto.uid,
      subject: dto.subject || '无主题',
      sender: dto.sender_name || dto.sender_email,
      senderEmail: dto.sender_email,
      recipient: dto.recipient_emails
        ? JSON.parse(dto.recipient_emails).map((r: any) => r.email).join(', ')
        : '',
      body: dto.body_html || dto.body_text || '',
      snippet: dto.snippet,
      unread: !dto.is_read,
      starred: dto.is_starred,
      flagged: dto.is_flagged,
      draft: dto.is_draft,
      date: new Date(dto.received_at * 1000),
      receivedAt: new Date(dto.received_at * 1000),
      sentAt: dto.sent_at ? new Date(dto.sent_at * 1000) : undefined,
      attachments: (dto.attachments || []).map((a: any) => ({
        id: a.id.toString(),
        filename: a.filename,
        contentType: a.content_type,
        size: a.size,
        path: a.path,
      })),
      hasAttachment: dto.has_attachment,
      attachmentCount: dto.attachment_count || 0,
      labels: [],
      threadId: dto.thread_id,
      inReplyTo: dto.in_reply_to,
      references: dto.references ? JSON.parse(dto.references) : undefined,
    }
  }
}
```

### 阶段 3: Store 重构 (2-3天)

#### 3.1 重构 AccountStore

**文件**: `src/stores/account.ts`

```typescript
/**
 * 账号 Store - 新架构版本
 *
 * 主要变更：
 * 1. 使用新的 API 客户端
 * 2. 支持企业邮箱配置
 * 3. 改进的错误处理
 * 4. 更好的类型安全
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Account, CreateAccountRequest, AccountType, AuthType } from '@/types/api'
import { MailApi, ApiError } from '@/api/client'
import { useSyncStore } from './sync'
import { useNotificationStore } from '@/stores/notification'

export const useAccountStore = defineStore('account', () => {
  // ========================================
  // State
  // ========================================

  const accounts = ref<Account[]>([])
  const currentAccount = ref<Account | null>(null)
  const isLoading = ref(false)
  const isDropdownOpen = ref(false)

  // ========================================
  // Getters
  // ========================================

  const hasAccounts = computed(() => accounts.value.length > 0)

  const totalUnreadCount = computed(() =>
    accounts.value.reduce((sum, account) => sum + account.unreadCount, 0),
  )

  const accountsByType = computed(() => {
    const personal = accounts.value.filter(a => a.accountType === 'personal')
    const enterprise = accounts.value.filter(a => a.accountType === 'enterprise')
    return { personal, enterprise }
  })

  const accountsByProvider = computed(() => {
    const grouped: Record<string, Account[]> = {}
    accounts.value.forEach(account => {
      const provider = account.provider
      if (!grouped[provider]) grouped[provider] = []
      grouped[provider].push(account)
    })
    return grouped
  })

  // ========================================
  // Actions
  // ========================================

  /**
   * 获取账号列表
   */
  async function fetchAccounts() {
    isLoading.value = true
    try {
      accounts.value = await MailApi.listAccounts()

      // 保持当前账号选择
      if (currentAccount.value) {
        const exists = accounts.value.some(
          a => a.id === currentAccount.value!.id,
        )
        if (!exists) {
          currentAccount.value = accounts.value[0] || null
        }
      } else if (accounts.value.length > 0) {
        currentAccount.value = accounts.value[0]
      }
    } catch (error) {
      if (error instanceof ApiError) {
        useNotificationStore().showError(error.userMessage)
      }
      throw error
    } finally {
      isLoading.value = false
    }
  }

  /**
   * 选择账号
   */
  function selectAccount(account: Account) {
    currentAccount.value = account
    isDropdownOpen.value = false
  }

  /**
   * 通过 ID 选择账号
   */
  function selectAccountById(accountId: string) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      selectAccount(account)
    }
  }

  /**
   * 添加账号
   */
  async function addAccount(request: CreateAccountRequest): Promise<Account> {
    isLoading.value = true
    try {
      const account = await MailApi.addAccount(request)
      accounts.value.push(account)

      // 如果是第一个账号，自动选择
      if (accounts.value.length === 1) {
        currentAccount.value = account
      }

      useNotificationStore().showSuccess('账号添加成功')

      return account
    } catch (error) {
      if (error instanceof ApiError) {
        useNotificationStore().showError(error.userMessage)
      }
      throw error
    } finally {
      isLoading.value = false
    }
  }

  /**
   * 更新账号
   */
  async function updateAccount(
    accountId: string,
    updates: Partial<Account>,
  ): Promise<Account> {
    try {
      const account = await MailApi.updateAccount(accountId, updates)

      // 更新本地列表
      const index = accounts.value.findIndex(a => a.id === accountId)
      if (index !== -1) {
        accounts.value[index] = account
      }

      // 更新当前账号
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = account
      }

      useNotificationStore().showSuccess('账号更新成功')
      return account
    } catch (error) {
      if (error instanceof ApiError) {
        useNotificationStore().showError(error.userMessage)
      }
      throw error
    }
  }

  /**
   * 删除账号
   */
  async function removeAccount(accountId: string) {
    try {
      await MailApi.deleteAccount(accountId)

      const index = accounts.value.findIndex(a => a.id === accountId)
      if (index !== -1) {
        accounts.value.splice(index, 1)
      }

      // 切换到其他账号
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = accounts.value[0] || null
      }

      useNotificationStore().showSuccess('账号删除成功')
    } catch (error) {
      if (error instanceof ApiError) {
        useNotificationStore().showError(error.userMessage)
      }
      throw error
    }
  }

  /**
   * 测试连接
   */
  async function testConnection(
    request: CreateAccountRequest,
  ): Promise<{ success: boolean; message: string }> {
    try {
      const result = await MailApi.testConnection(request)
      if (result.success) {
        return {
          success: true,
          message: `连接成功！找到 ${result.emailCount} 封邮件`,
        }
      } else {
        return {
          success: false,
          message: result.error || '连接失败',
        }
      }
    } catch (error) {
      if (error instanceof ApiError) {
        return {
          success: false,
          message: error.userMessage,
        }
      }
      return {
        success: false,
        message: '连接测试失败',
      }
    }
  }

  /**
   * 同步账号
   */
  async function syncAccount(accountId?: string) {
    const id = accountId || currentAccount.value?.id
    if (!id) {
      throw new Error('没有可同步的账号')
    }

    const syncStore = useSyncStore()
    await syncStore.syncAccount(id)

    // 刷新账号列表
    await fetchAccounts()
  }

  /**
   * 更新未读数量
   */
  function updateUnreadCount(accountId: string, count: number) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      account.unreadCount = count
    }
  }

  /**
   * 清空所有账号
   */
  function clearAccounts() {
    accounts.value = []
    currentAccount.value = null
  }

  return {
    // State
    accounts,
    currentAccount,
    isLoading,
    isDropdownOpen,

    // Getters
    hasAccounts,
    totalUnreadCount,
    accountsByType,
    accountsByProvider,

    // Actions
    fetchAccounts,
    selectAccount,
    selectAccountById,
    addAccount,
    updateAccount,
    removeAccount,
    testConnection,
    syncAccount,
    updateUnreadCount,
    clearAccounts,
    toggleDropdown: () => { isDropdownOpen.value = !isDropdownOpen.value },
    closeDropdown: () => { isDropdownOpen.value = false },
  }
})
```

#### 3.2 重构 SyncStore

**文件**: `src/stores/sync.ts`

```typescript
/**
 * 同步 Store - 新架构版本
 *
 * 主要变更：
 * 1. 使用新的同步进度事件格式
 * 2. 支持多账号并发同步
 * 3. 更好的错误恢复
 * 4. 同步历史记录
 */

import { defineStore } from 'pinia'
import type {
  SyncProgress,
  SyncResult,
  SyncStage,
} from '@/types/api'
import { MailApi } from '@/api/client'

interface SyncStatus {
  accountId: number
  stage: 'idle' | 'syncing' | 'completed' | 'error'
  progress: number
  message: string
  currentFolder?: string
  result?: SyncResult
  error?: string
  startedAt: Date
  completedAt?: Date
}

export const useSyncStore = defineStore('sync', {
  state: () => ({
    syncingAccounts: new Set<number>(),
    syncStatuses: new Map<number, SyncStatus>(),
    syncHistory: [] as SyncStatus[],
    unlisteners: new Map<number, Promise<() => void>>(),
  }),

  getters: {
    isSyncing: (state) => (accountId: number) => {
      return state.syncingAccounts.has(accountId)
    },

    hasAnySyncing: (state) => {
      return state.syncingAccounts.size > 0
    },

    getStatus: (state) => (accountId: number) => {
      return state.syncStatuses.get(accountId)
    },

    allStatuses: (state) => {
      return Array.from(state.syncStatuses.values())
    },

    completedSyncs: (state) => {
      return state.syncHistory.filter(s => s.stage === 'completed')
    },
  },

  actions: {
    /**
     * 开始同步
     */
    async syncAccount(accountId: number | string): Promise<SyncResult | null> {
      const numericAccountId = typeof accountId === 'string'
        ? parseInt(accountId, 10)
        : accountId

      try {
        // 初始化状态
        this.syncStatuses.set(numericAccountId, {
          accountId: numericAccountId,
          stage: 'syncing',
          progress: 0,
          message: '准备同步...',
          startedAt: new Date(),
        })
        this.syncingAccounts.add(numericAccountId)

        // 开始监听进度
        await this.startListening(numericAccountId)

        // 调用后端同步
        const result = await MailApi.syncAccount(numericAccountId.toString())

        // 等待最后的进度事件
        await new Promise(resolve => setTimeout(resolve, 500))

        const status = this.syncStatuses.get(numericAccountId)
        if (status?.stage === 'completed') {
          return {
            totalSynced: status.progress || 0,
            foldersSynced: 0,
            errors: 0,
            durationMs: status.completedAt
              ? status.completedAt.getTime() - status.startedAt.getTime()
              : 0,
          }
        }

        return null
      } catch (error: any) {
        const errorMessage = this.parseErrorMessage(error)

        this.syncStatuses.set(numericAccountId, {
          accountId: numericAccountId,
          stage: 'error',
          progress: 0,
          message: errorMessage,
          error: String(error),
          startedAt: new Date(),
        })
        this.syncingAccounts.delete(numericAccountId)

        return null
      }
    },

    /**
     * 开始监听进度
     */
    async startListening(accountId: number) {
      // 停止之前的监听
      this.stopListening(accountId)

      const unlisten = await MailApi.onSyncProgress(
        accountId.toString(),
        (progress: SyncProgress) => {
          this.handleProgressEvent(progress)
        },
      )

      this.unlisteners.set(accountId, Promise.resolve(unlisten))
    },

    /**
     * 停止监听
     */
    stopListening(accountId: number) {
      const unlisten = this.unlisteners.get(accountId)
      if (unlisten) {
        unlisten.then(fn => fn())
        this.unlisteners.delete(accountId)
      }
    },

    /**
     * 处理进度事件
     */
    handleProgressEvent(progress: SyncProgress) {
      const accountId = typeof progress.accountId === 'string'
        ? parseInt(progress.accountId, 10)
        : progress.accountId

      let stage: 'idle' | 'syncing' | 'completed' | 'error'

      switch (progress.stage) {
        case 'connecting':
        case 'authenticating':
        case 'syncing_folders':
        case 'syncing_emails':
          stage = 'syncing'
          this.syncingAccounts.add(accountId)
          break
        case 'completed':
          stage = 'completed'
          this.syncingAccounts.delete(accountId)
          break
        case 'error':
          stage = 'error'
          this.syncingAccounts.delete(accountId)
          break
        default:
          stage = 'idle'
      }

      const progressPercent = progress.total > 0
        ? Math.floor((progress.current / progress.total) * 100)
        : 0

      const currentStatus = this.syncStatuses.get(accountId)

      this.syncStatuses.set(accountId, {
        accountId,
        stage,
        progress: progressPercent,
        message: progress.message,
        currentFolder: progress.folder,
        startedAt: currentStatus?.startedAt || new Date(),
        completedAt: stage === 'completed' ? new Date() : currentStatus?.completedAt,
      })

      // 如果完成，添加到历史记录
      if (stage === 'completed') {
        const status = this.syncStatuses.get(accountId)
        if (status) {
          this.syncHistory.unshift(status)
          // 只保留最近 50 条
          if (this.syncHistory.length > 50) {
            this.syncHistory = this.syncHistory.slice(0, 50)
          }
        }
      }
    },

    /**
     * 解析错误消息
     */
    parseErrorMessage(error: any): string {
      const message = String(error)

      if (message.includes('账号密码不存在') || message.includes('密码未找到')) {
        return '账号密码未保存，请删除账号后重新添加以设置密码'
      }
      if (message.includes('连接 IMAP 服务器失败')) {
        return '无法连接到邮件服务器，请检查网络或账号配置'
      }
      if (message.includes('账号不存在')) {
        return '账号不存在，请重新添加账号'
      }
      if (message.includes('认证失败')) {
        return '认证失败，请检查账号密码或重新授权'
      }

      return message || '同步失败，请稍后重试'
    },

    /**
     * 清除状态
     */
    clearStatus(accountId: number) {
      this.syncStatuses.delete(accountId)
      this.syncingAccounts.delete(accountId)
      this.stopListening(accountId)
    },

    /**
     * 清除所有状态
     */
    clearAllStatuses() {
      this.syncStatuses.clear()
      this.syncingAccounts.clear()
      this.unlisteners.forEach((unlisten, _accountId) => {
        unlisten.then(fn => fn())
      })
      this.unlisteners.clear()
    },
  },
})
```

### 阶段 4: 组件适配 (2-3天)

#### 4.1 更新账号添加组件

**文件**: `src/components/settings/AccountAddDialog.vue`

```vue
<template>
  <Dialog v-model:open="open" @update:open="handleClose">
    <DialogContent class="max-w-2xl">
      <DialogHeader>
        <DialogTitle>添加邮箱账号</DialogTitle>
        <DialogDescription>
          支持个人邮箱和企业邮箱的添加
        </DialogDescription>
      </DialogHeader>

      <form @submit.prevent="handleSubmit" class="space-y-6">
        <!-- 账号类型选择 -->
        <div class="space-y-3">
          <Label>账号类型</Label>
          <RadioGroup v-model="form.accountType">
            <div class="flex gap-4">
              <label class="flex items-center gap-2 cursor-pointer">
                <RadioGroupItem value="personal" />
                <span>个人邮箱</span>
              </label>
              <label class="flex items-center gap-2 cursor-pointer">
                <RadioGroupItem value="enterprise" />
                <span>企业邮箱</span>
              </label>
            </div>
          </RadioGroup>
        </div>

        <!-- 邮箱地址 -->
        <div class="space-y-2">
          <Label for="email">邮箱地址</Label>
          <Input
            id="email"
            v-model="form.email"
            type="email"
            placeholder="example@gmail.com"
            required
            :disabled="isLoading"
          />
          <p v-if="providerHint" class="text-sm text-muted-foreground">
            {{ providerHint }}
          </p>
        </div>

        <!-- 认证方式 -->
        <div class="space-y-2">
          <Label>认证方式</Label>
          <Select v-model="form.authType">
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="oauth2">OAuth 授权（推荐）</SelectItem>
              <SelectItem value="password">密码登录</SelectItem>
              <SelectItem value="app_password">应用专用密码</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <!-- OAuth 授权流程 -->
        <div v-if="form.authType === 'oauth2'" class="space-y-4">
          <p class="text-sm text-muted-foreground">
            点击下方按钮将在浏览器中打开授权页面
          </p>
          <Button
            type="button"
            variant="outline"
            @click="handleOAuth"
            :disabled="isLoading || !form.email"
            class="w-full"
          >
            <Icon name="external-link" class="mr-2 h-4 w-4" />
            打开授权页面
          </Button>

          <!-- OAuth 回调处理 -->
          <div v-if="oauthStatus" class="p-4 rounded-md bg-muted">
            <p class="text-sm">{{ oauthStatus }}</p>
          </div>
        </div>

        <!-- 密码输入 -->
        <div v-else class="space-y-2">
          <Label for="password">密码</Label>
          <Input
            id="password"
            v-model="form.password"
            type="password"
            placeholder="请输入密码"
            required
            :disabled="isLoading"
          />
        </div>

        <!-- 企业邮箱配置 -->
        <div v-if="form.accountType === 'enterprise'" class="space-y-4 p-4 rounded-md border">
          <h4 class="font-medium">企业邮箱配置</h4>

          <div class="space-y-2">
            <Label for="tenant">租户 ID（可选）</Label>
            <Input
              id="tenant"
              v-model="form.enterpriseTenantId"
              placeholder="例如：contoso.onmicrosoft.com"
              :disabled="isLoading"
            />
          </div>

          <div class="space-y-2">
            <Label for="domain">企业域名（可选）</Label>
            <Input
              id="domain"
              v-model="form.enterpriseDomain"
              placeholder="例如：company.com"
              :disabled="isLoading"
            />
          </div>
        </div>

        <!-- 自定义服务器配置 -->
        <Collapsible v-if="showAdvanced">
          <CollapsibleTrigger class="flex items-center gap-2 text-sm">
            <Icon name="settings" class="h-4 w-4" />
            高级配置
          </CollapsibleTrigger>
          <CollapsibleContent class="space-y-4 pt-4">
            <div class="grid grid-cols-3 gap-4">
              <div class="space-y-2">
                <Label>IMAP 服务器</Label>
                <Input v-model="form.imapHost" placeholder="imap.example.com" />
              </div>
              <div class="space-y-2">
                <Label>端口</Label>
                <Input v-model.number="form.imapPort" type="number" placeholder="993" />
              </div>
              <div class="space-y-2">
                <Label>SSL</Label>
                <Select v-model="form.imapSsl">
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="implicit">SSL/TLS</SelectItem>
                    <SelectItem value="starttls">STARTTLS</SelectItem>
                    <SelectItem value="none">无加密</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div class="grid grid-cols-3 gap-4">
              <div class="space-y-2">
                <Label>SMTP 服务器</Label>
                <Input v-model="form.smtpHost" placeholder="smtp.example.com" />
              </div>
              <div class="space-y-2">
                <Label>端口</Label>
                <Input v-model.number="form.smtpPort" type="number" placeholder="465" />
              </div>
              <div class="space-y-2">
                <Label>SSL</Label>
                <Select v-model="form.smtpSsl">
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="implicit">SSL/TLS</SelectItem>
                    <SelectItem value="starttls">STARTTLS</SelectItem>
                    <SelectItem value="none">无加密</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </CollapsibleContent>
        </Collapsible>

        <!-- 测试连接 -->
        <div v-if="form.password && form.email" class="space-y-2">
          <Button
            type="button"
            variant="outline"
            @click="handleTestConnection"
            :disabled="isTesting"
            class="w-full"
          >
            <Icon
              :name="isTesting ? 'loader' : 'network'"
              :class="{ 'animate-spin': isTesting }"
              class="mr-2 h-4 w-4"
            />
            {{ isTesting ? '测试中...' : '测试连接' }}
          </Button>

          <Alert v-if="testResult" :variant="testResult.success ? 'default' : 'destructive'">
            <Icon :name="testResult.success ? 'check' : 'x'" class="h-4 w-4" />
            <AlertDescription>{{ testResult.message }}</AlertDescription>
          </Alert>
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" @click="handleClose">
            取消
          </Button>
          <Button type="submit" :disabled="isLoading">
            {{ isLoading ? '添加中...' : '添加账号' }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useAccountStore } from '@/stores/account'
import { MailApi } from '@/api/client'
import type { CreateAccountRequest, AccountType, AuthType, SslMode } from '@/types/api'

interface Props {
  open: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:open': [value: boolean]
  'success': []
}>()

const accountStore = useAccountStore()

const isLoading = ref(false)
const isTesting = ref(false)
const showAdvanced = ref(false)
const oauthStatus = ref('')

const form = ref<CreateAccountRequest>({
  name: '',
  email: '',
  accountType: 'personal' as AccountType,
  authType: 'oauth2' as AuthType,
  password: '',
  oauthCode: '',
  imapHost: '',
  imapPort: 993,
  imapSsl: 'implicit' as SslMode,
  smtpHost: '',
  smtpPort: 465,
  smtpSsl: 'implicit' as SslMode,
  color: '#7C3AED',
  enterpriseTenantId: '',
  enterpriseDomain: '',
})

const testResult = ref<{ success: boolean; message: string } | null>(null)

// 根据邮箱地址自动检测服务商
const providerHint = computed(() => {
  const email = form.value.email
  if (!email) return ''

  const domain = email.split('@')[1]?.toLowerCase()
  if (!domain) return ''

  const hints: Record<string, string> = {
    'gmail.com': '检测到 Gmail 邮箱，推荐使用 OAuth 授权',
    'googlemail.com': '检测到 Gmail 邮箱，推荐使用 OAuth 授权',
    'outlook.com': '检测到 Outlook 邮箱',
    'hotmail.com': '检测到 Outlook 邮箱',
    'live.com': '检测到 Outlook 邮箱',
    'yahoo.com': '检测到 Yahoo 邮箱',
    'icloud.com': '检测到 iCloud 邮箱',
    'qq.com': '检测到 QQ 邮箱',
    '163.com': '检测到网易邮箱',
    '126.com': '检测到网易邮箱',
  }

  return hints[domain] || ''
})

// 监听邮箱变化，自动填写名称
watch(() => form.value.email, (email) => {
  if (email && !form.value.name) {
    form.value.name = email.split('@')[0] || email
  }
})

// OAuth 授权
async function handleOAuth() {
  if (!form.value.email) return

  oauthStatus.value = '正在打开授权页面...'

  try {
    const authUrl = await MailApi.getOAuthUrl(
      form.value.email,
      form.value.accountType,
    )

    // 打开系统浏览器
    const { open } = require('@tauri-apps/api/shell')
    await open(authUrl)

    oauthStatus.value = '请在浏览器中完成授权，授权完成后会自动返回...'
  } catch (error) {
    oauthStatus.value = '打开授权页面失败：' + error
  }
}

// 测试连接
async function handleTestConnection() {
  if (!form.value.email || !form.value.password) return

  isTesting.value = true
  testResult.value = null

  try {
    testResult.value = await accountStore.testConnection(form.value)
  } finally {
    isTesting.value = false
  }
}

// 提交表单
async function handleSubmit() {
  isLoading.value = true

  try {
    await accountStore.addAccount(form.value)
    emit('success')
    handleClose()
  } finally {
    isLoading.value = false
  }
}

// 关闭对话框
function handleClose() {
  emit('update:open', false)
  // 重置表单
  form.value = {
    name: '',
    email: '',
    accountType: 'personal',
    authType: 'oauth2',
    password: '',
    oauthCode: '',
    imapHost: '',
    imapPort: 993,
    imapSsl: 'implicit',
    smtpHost: '',
    smtpPort: 465,
    smtpSsl: 'implicit',
    color: '#7C3AED',
    enterpriseTenantId: '',
    enterpriseDomain: '',
  }
  testResult.value = null
  oauthStatus.value = ''
}
</script>
```

---

## 🗄️ 数据库迁移

### 新增表和字段

```sql
-- 账号表新增字段
ALTER TABLE accounts ADD COLUMN account_type TEXT DEFAULT 'personal';
ALTER TABLE accounts ADD COLUMN auth_type TEXT DEFAULT 'password';
ALTER TABLE accounts ADD COLUMN enterprise_config TEXT; -- JSON

-- OAuth Token 表
CREATE TABLE IF NOT EXISTS oauth_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL UNIQUE,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER,
    id_token TEXT,
    token_type TEXT DEFAULT 'Bearer',
    scope TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- 同步状态表
CREATE TABLE IF NOT EXISTS sync_states (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    folder_path TEXT NOT NULL,
    last_sync_time INTEGER,
    last_uid INTEGER,
    last_modseq INTEGER,
    synced_count INTEGER DEFAULT 0,
    status TEXT DEFAULT 'never', -- never, syncing, synced, failed
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(account_id, folder_path),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- 操作队列表
CREATE TABLE IF NOT EXISTS operation_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    payload TEXT NOT NULL, -- JSON
    status TEXT DEFAULT 'pending', -- pending, processing, completed, failed
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    error_message TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    started_at INTEGER,
    completed_at INTEGER,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_oauth_tokens_account_id ON oauth_tokens(account_id);
CREATE INDEX IF NOT EXISTS idx_sync_states_account_id ON sync_states(account_id);
CREATE INDEX IF NOT EXISTS idx_operation_queue_account_id ON operation_queue(account_id);
CREATE INDEX IF NOT EXISTS idx_operation_queue_status ON operation_queue(status);
```

---

## 🧪 测试策略

### 单元测试

```rust
// src-tauri/src/services/providers/tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gmail_provider_detection() {
        let provider = GmailProvider::default();

        assert!(provider.detect("test@gmail.com").await.unwrap());
        assert!(provider.detect("test@googlemail.com").await.unwrap());
        assert!(!provider.detect("test@outlook.com").await.unwrap());
    }

    #[tokio::test]
    async fn test_provider_pool() {
        let pool = ProviderPool::new();

        let gmail = pool.detect_provider("test@gmail.com").await.unwrap();
        assert_eq!(gmail.provider_id(), "gmail");

        let outlook = pool.detect_provider("test@outlook.com").await.unwrap();
        assert_eq!(outlook.provider_id(), "outlook");
    }
}
```

### 集成测试

```typescript
// tests/api.test.ts

import { describe, it, expect, beforeEach } from 'vitest'
import { MailApi } from '@/api/client'

describe('Mail API', () => {
  beforeEach(() => {
    // 初始化测试环境
  })

  it('should list accounts', async () => {
    const accounts = await MailApi.listAccounts()
    expect(Array.isArray(accounts)).toBe(true)
  })

  it('should add account with OAuth', async () => {
    const request: CreateAccountRequest = {
      name: 'Test Account',
      email: 'test@example.com',
      authType: 'oauth2',
      oauthCode: 'test-code',
    }

    const account = await MailApi.addAccount(request)
    expect(account.email).toBe('test@example.com')
  })
})
```

---

## 🔄 回滚方案

### 1. 分支策略

```bash
# 创建迁移分支
git checkout -b feature/migration-new-architecture

# 保持 main 分支可用
git checkout main
```

### 2. 数据库迁移回滚

```sql
-- 回滚脚本
DROP TABLE IF EXISTS oauth_tokens;
DROP TABLE IF EXISTS sync_states;
DROP TABLE IF EXISTS operation_queue;

ALTER TABLE accounts DROP COLUMN account_type;
ALTER TABLE accounts DROP COLUMN auth_type;
ALTER TABLE accounts DROP COLUMN enterprise_config;
```

### 3. 功能开关

```rust
// src-tauri/src/feature_flags.rs

pub struct FeatureFlags {
    pub use_new_architecture: bool,
    pub enable_condstore: bool,
    pub enable_idle: bool,
}

impl FeatureFlags {
    pub fn from_env() -> Self {
        Self {
            use_new_architecture: std::env::var("USE_NEW_ARCH")
                .unwrap_or("false".to_string()) == "true",
            enable_condstore: true,
            enable_idle: true,
        }
    }
}
```

---

## 📝 迁移检查清单

### 后端

- [ ] 创建新的目录结构
- [ ] 实现核心错误类型
- [ ] 实现 MailProvider trait
- [ ] 实现各服务商（Gmail, Outlook, Yahoo, Native）
- [ ] 实现 ProviderPool
- [ ] 实现 AuthManager
- [ ] 实现 TokenManager
- [ ] 实现 SyncManager
- [ ] 实现 DeltaSync
- [ ] 实现 ChangeDetector
- [ ] 实现 OperationManager
- [ ] 实现 NotificationManager
- [ ] 实现 TaskScheduler
- [ ] 实现 FlowEngine
- [ ] 重构所有 Tauri 命令
- [ ] 数据库迁移脚本
- [ ] 单元测试
- [ ] 集成测试

### 前端

- [ ] 更新类型定义
- [ ] 实现 MailApi 客户端
- [ ] 重构 AccountStore
- [ ] 重构 EmailStore
- [ ] 重构 SyncStore
- [ ] 更新 AccountAddDialog 组件
- [ ] 更新 EmailList 组件
- [ ] 更新 SyncProgress 组件
- [ ] 添加错误处理组件
- [ ] 添加通知组件
- [ ] 端到端测试

---

## ⏱️ 时间估算

| 阶段 | 任务 | 后端 | 前端 | 总计 |
|------|------|------|------|------|
| 1 | 基础设施搭建 | 2-3天 | 1天 | 3-4天 |
| 2 | 服务商层实现 | 3-4天 | - | 3-4天 |
| 3 | 认证层重构 | 2-3天 | - | 2-3天 |
| 4 | 同步引擎重构 | 3-4天 | - | 3-4天 |
| 5 | 命令层适配 | 2天 | - | 2天 |
| 6 | 类型定义更新 | - | 1天 | 1天 |
| 7 | API 客户端重构 | - | 2天 | 2天 |
| 8 | Store 重构 | - | 2-3天 | 2-3天 |
| 9 | 组件适配 | - | 2-3天 | 2-3天 |
| 10 | 测试与修复 | 2-3天 | 2天 | 4-5天 |
| **总计** | | **16-21天** | **10-13天** | **26-34天** |

---

## 🎯 验收标准

### 功能验收

1. **账号管理**
   - [ ] 支持添加个人邮箱（Gmail, Outlook, Yahoo, 等）
   - [ ] 支持添加企业邮箱（Microsoft 365, Google Workspace）
   - [ ] OAuth 授权流程正常
   - [ ] 密码认证正常
   - [ ] 账号连接测试正常

2. **邮件同步**
   - [ ] 首次完整同步正常
   - [ ] CONDSTORE 增量同步正常
   - [ ] IDLE 实时通知正常
   - [ ] 同步进度显示正常
   - [ ] 多账号并发同步正常

3. **邮件操作**
   - [ ] 标记已读/未读
   - [ ] 切换星标
   - [ ] 移动到文件夹
   - [ ] 删除邮件
   - [ ] 批量操作

4. **搜索功能**
   - [ ] 全文搜索
   - [ ] 字段搜索
   - [ ] 日期范围搜索

5. **发送功能**
   - [ ] SMTP 发送
   - [ ] 附件上传
   - [ ] 草稿保存

### 性能验收

1. **启动性能**
   - 应用启动时间 < 2秒
   - 首屏渲染 < 1秒

2. **同步性能**
   - 首次同步: > 100封邮件/分钟
   - 增量同步: < 5秒
   - UI 响应: < 100ms

3. **内存占用**
   - 空闲: < 200MB
   - 同步中: < 500MB

### 安全验收

1. **认证安全**
   - OAuth Token 安全存储
   - 密码加密存储
   - Token 自动刷新

2. **数据安全**
   - 敏感数据脱敏
   - 安全审计日志

---

## 📚 附录

### A. 迁移文件清单

#### 新增文件

**后端**
```
src-tauri/src/
├── engine/
│   ├── mod.rs
│   ├── flow_engine.rs
│   ├── task_scheduler.rs
│   └── notification_manager.rs
├── services/
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── auth_manager.rs
│   │   ├── token_manager.rs
│   │   └── oauth_handler.rs
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── provider_pool.rs
│   │   ├── gmail.rs
│   │   ├── outlook.rs
│   │   ├── microsoft_365.rs
│   │   ├── google_workspace.rs
│   │   ├── yahoo.rs
│   │   ├── native.rs
│   │   └── custom_enterprise.rs
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── sync_manager.rs
│   │   ├── delta_sync.rs
│   │   └── change_detector.rs
│   ├── operations/
│   │   ├── mod.rs
│   │   ├── operation_manager.rs
│   │   └── conflict_resolver.rs
│   └── email/
│       ├── mod.rs
│       └── email_service.rs
├── protocols/
│   ├── mod.rs
│   ├── imap/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── types.rs
│   └── smtp/
│       ├── mod.rs
│       └── sender.rs
└── storage/
    ├── mod.rs
    ├── database.rs
    └── cache.rs
```

**前端**
```
src/
├── types/
│   └── api.ts
├── api/
│   ├── client.ts
│   └── types.ts
└── stores/
    ├── account.ts (重构)
    ├── email.ts (重构)
    └── sync.ts (重构)
```

#### 修改文件

**后端**
```
src-tauri/src/
├── error.rs (重写)
├── command/
│   ├── account.rs (重构)
│   ├── email.rs (重构)
│   └── sync.rs (重构)
├── lib.rs (修改)
└── main.rs (修改)
```

**前端**
```
src/
├── components/
│   ├── settings/
│   │   └── AccountAddDialog.vue (重构)
│   └── email/
│       └── EmailList.vue (修改)
└── stores/
    ├── account.ts (重构)
    ├── email.ts (重构)
    └── sync.ts (重构)
```

### B. 破坏性 API 变更清单

#### 后端命令变更

| 旧命令 | 新命令 | 变更说明 |
|--------|--------|----------|
| `add_account` | `add_account` | 请求参数增加 `account_type`, `auth_type` |
| `sync_account` | `sync_account_with_progress` | 返回同步结果，通过事件发送进度 |
| `list_emails` | `list_emails` | 响应结构变化 |
| - | `get_oauth_url` | 新增：获取 OAuth 授权 URL |
| - | `handle_oauth_callback` | 新增：处理 OAuth 回调 |

#### 前端类型变更

| 旧类型 | 新类型 | 变更说明 |
|--------|--------|----------|
| `Account` | `Account` | 增加 `accountType`, `authType`, `enterpriseConfig` |
| `SyncProgress` | `SyncProgress` | 阶段名称变化 |
| - | `OAuthToken` | 新增类型 |
| - | `Operation` | 新增类型 |

### C. 环境变量配置

```bash
# .env

# Gmail OAuth
GMAIL_CLIENT_ID=your_client_id
GMAIL_CLIENT_SECRET=your_client_secret

# Outlook OAuth
OUTLOOK_CLIENT_ID=your_client_id
OUTLOOK_CLIENT_SECRET=your_client_secret

# Microsoft 365 OAuth
MICROSOFT365_CLIENT_ID=your_client_id
MICROSOFT365_CLIENT_SECRET=your_client_secret
MICROSOFT365_TENANT_ID=common

# Google Workspace
GOOGLE_WORKSPACE_CLIENT_ID=your_client_id
GOOGLE_WORKSPACE_CLIENT_SECRET=your_client_secret

# Yahoo OAuth
YAHOO_CLIENT_ID=your_client_id
YAHOO_CLIENT_SECRET=your_client_secret

# 功能开关
USE_NEW_ARCH=true
ENABLE_CONDSTORE=true
ENABLE_IDLE=true
```

---

**文档版本**: 1.0
**最后更新**: 2026-03-19
**状态**: 待审核
