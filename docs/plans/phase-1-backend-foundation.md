# Phase 1: 后端基础层

> 前置：Phase 0 完成
> 完成标志：MailError + Provider traits + 个人服务商实现 + IMAP/SMTP 协议层 + AuthManager 编译通过

---

### Task 1.1: 实现 MailError 统一错误类型

**Files:**
- Modify: `src-tauri/src/error/mod.rs`
- Create: `src-tauri/src/error/types.rs`

**Step 1: 写 error/types.rs**

```rust
use serde::Serialize;
use specta::Type;

/// 统一错误类型 — 前端通过 tauri-specta Result 模式拿到类型化错误
#[derive(Debug, thiserror::Error, Type, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum MailError {
    #[error("账户不存在: {0}")]
    AccountNotFound(i32),

    #[error("认证失败: {0}")]
    AuthFailed(String),

    #[error("IMAP 连接失败: {0}")]
    ImapConnectionFailed(String),

    #[error("SMTP 发送失败: {0}")]
    SmtpSendFailed(String),

    #[error("同步失败: {0}")]
    SyncFailed(String),

    #[error("数据库错误: {0}")]
    DatabaseError(String),

    #[error("Keyring 错误: {0}")]
    KeyringError(String),

    #[error("服务商不支持: {0}")]
    ProviderNotSupported(String),

    #[error("参数无效: {0}")]
    InvalidParam(String),

    #[error("邮件不存在: {0}")]
    EmailNotFound(i32),

    #[error("文件夹不存在: {0}")]
    FolderNotFound(String),

    #[error("OAuth 错误: {0}")]
    OAuthError(String),

    #[error("未实现: {0}")]
    NotImplemented(String),
}

// SeaORM 错误转换
impl From<sea_orm::DbErr> for MailError {
    fn from(err: sea_orm::DbErr) -> Self {
        MailError::DatabaseError(err.to_string())
    }
}

// JSON 错误转换
impl From<serde_json::Error> for MailError {
    fn from(err: serde_json::Error) -> Self {
        MailError::InvalidParam(err.to_string())
    }
}
```

**Step 2: 更新 error/mod.rs**

```rust
pub mod types;

pub use types::MailError;
```

**Step 3: 验证**

Run: `cd D:\Xuan\postium-mail\src-tauri && cargo check`
Expected: 编译成功

**Step 4: Commit**

```bash
git add src-tauri/src/error/
git commit -m "feat: implement MailError unified error type"
```

---

### Task 1.2: 定义 Provider 核心类型

**Files:**
- Create: `src-tauri/src/domain/providers/traits.rs`

**Step 1: 写 Provider traits**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

// ─── 值对象 ───

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub auth_types: Vec<AuthType>,
    pub logo_url: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum AuthType {
    Password,
    OAuth2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImapServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SmtpServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub enum SslMode {
    None,
    StartTls,
    Tls,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StandardFolder {
    pub inbox: Option<String>,
    pub sent: Option<String>,
    pub drafts: Option<String>,
    pub spam: Option<String>,
    pub trash: Option<String>,
    pub archive: Option<String>,
    pub starred: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderCapabilities {
    pub oauth2: bool,
    pub idle: bool,
    pub quota: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self { oauth2: false, idle: false, quota: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
}

// ─── 核心配置 Trait ───

pub trait MailProvider: Send + Sync {
    fn info(&self) -> &ProviderInfo;
    fn imap_config(&self, _email: &str) -> ImapServerConfig;
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig;
    fn capabilities(&self) -> ProviderCapabilities;
    fn folder_mapping(&self) -> StandardFolder;
}

// ─── OAuth Trait (独立) ───

pub trait OAuthProvider: MailProvider {
    fn oauth_config(&self) -> OAuthConfig;
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String;
    fn redirect_uri(&self, port: u16) -> String;
}

// ─── 检测 Trait (独立) ───

pub trait ProviderDetector: Send + Sync {
    fn domains(&self) -> &[&'static str];
    fn detect(&self, email: &str) -> bool {
        let email_domain = email.split('@').last().unwrap_or("").to_lowercase();
        self.domains().iter().any(|d| d.to_lowercase() == email_domain)
    }
}
```

**Step 2: 验证**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src-tauri/src/domain/providers/traits.rs
git commit -m "feat: define provider traits and core types"
```

---

### Task 1.3: 实现个人服务商 (Gmail + Outlook + QQ)

**Files:**
- Modify: `src-tauri/src/domain/providers/personal/mod.rs`
- Create: `src-tauri/src/domain/providers/personal/gmail.rs`
- Create: `src-tauri/src/domain/providers/personal/outlook.rs`
- Create: `src-tauri/src/domain/providers/personal/qq.rs`

**Step 1: 写 gmail.rs**

```rust
use crate::domain::providers::*;

pub struct GmailProvider;

impl GmailProvider {
    pub fn new() -> Self { Self }
}

impl MailProvider for GmailProvider {
    fn info(&self) -> &ProviderInfo {
        static INFO: once_cell::sync::Lazy<ProviderInfo> = once_cell::sync::Lazy::new(|| ProviderInfo {
            id: "gmail".into(),
            name: "Gmail".into(),
            auth_types: vec![AuthType::Password, AuthType::OAuth2],
            logo_url: None,
            color: Some("#EA4335".into()),
        });
        &INFO
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig { host: "imap.gmail.com".into(), port: 993, ssl: SslMode::Tls }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig { host: "smtp.gmail.com".into(), port: 465, ssl: SslMode::Tls }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { oauth2: true, idle: true, quota: true }
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: Some("INBOX".into()),
            sent: Some("[Gmail]/Sent Mail".into()),
            drafts: Some("[Gmail]/Drafts".into()),
            spam: Some("[Gmail]/Spam".into()),
            trash: Some("[Gmail]/Trash".into()),
            archive: Some("[Gmail]/All Mail".into()),
            starred: Some("[Gmail]/Starred".into()),
        }
    }
}

impl OAuthProvider for GmailProvider {
    fn oauth_config(&self) -> OAuthConfig {
        OAuthConfig {
            client_id: String::new(), // 从环境变量读取
            client_secret: String::new(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec![
                "https://mail.google.com/".into(),
                "https://www.googleapis.com/auth/userinfo.email".into(),
            ],
        }
    }

    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        // XOAuth2 SASL: user={user}\x01auth=Bearer {token}\x01\x01
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }

    fn redirect_uri(&self, port: u16) -> String {
        format!("http://127.0.0.1:{}/oauth/callback", port)
    }
}

impl ProviderDetector for GmailProvider {
    fn domains(&self) -> &[&'static str] {
        &["gmail.com", "googlemail.com"]
    }
}
```

**Step 2: 写 outlook.rs**

```rust
use crate::domain::providers::*;

pub struct OutlookProvider;

impl OutlookProvider {
    pub fn new() -> Self { Self }
}

impl MailProvider for OutlookProvider {
    fn info(&self) -> &ProviderInfo {
        static INFO: once_cell::sync::Lazy<ProviderInfo> = once_cell::sync::Lazy::new(|| ProviderInfo {
            id: "outlook".into(),
            name: "Outlook".into(),
            auth_types: vec![AuthType::Password, AuthType::OAuth2],
            logo_url: None,
            color: Some("#0078D4".into()),
        });
        &INFO
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig { host: "outlook.office365.com".into(), port: 993, ssl: SslMode::Tls }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig { host: "smtp.office365.com".into(), port: 587, ssl: SslMode::StartTls }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { oauth2: true, idle: false, quota: false }
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: Some("INBOX".into()),
            sent: Some("Sent".into()),
            drafts: Some("Drafts".into()),
            spam: Some("Junk".into()),
            trash: Some("Deleted".into()),
            archive: None,
            starred: None,
        }
    }
}

impl ProviderDetector for OutlookProvider {
    fn domains(&self) -> &[&'static str] {
        &["outlook.com", "hotmail.com", "live.com", "msn.com"]
    }
}
```

**Step 3: 写 qq.rs**

```rust
use crate::domain::providers::*;

pub struct QqMailProvider;

impl QqMailProvider {
    pub fn new() -> Self { Self }
}

impl MailProvider for QqMailProvider {
    fn info(&self) -> &ProviderInfo {
        static INFO: once_cell::sync::Lazy<ProviderInfo> = once_cell::sync::Lazy::new(|| ProviderInfo {
            id: "qq".into(),
            name: "QQ Mail".into(),
            auth_types: vec![AuthType::Password],
            logo_url: None,
            color: Some("#12B7F5".into()),
        });
        &INFO
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig { host: "imap.qq.com".into(), port: 993, ssl: SslMode::Tls }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig { host: "smtp.qq.com".into(), port: 465, ssl: SslMode::Tls }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { oauth2: false, idle: false, quota: false }
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: Some("INBOX".into()),
            sent: Some("Sent Messages".into()),
            drafts: Some("Drafts".into()),
            spam: Some("Junk".into()),
            trash: Some("Deleted Messages".into()),
            archive: None,
            starred: None,
        }
    }
}

impl ProviderDetector for QqMailProvider {
    fn domains(&self) -> &[&'static str] {
        &["qq.com", "foxmail.com"]
    }
}
```

**Step 4: 更新 personal/mod.rs**

```rust
pub mod gmail;
pub mod outlook;
pub mod qq;

use crate::domain::providers::traits::{MailProvider, ProviderDetector};
use std::sync::Arc;

/// 获取所有个人服务商
pub fn all_providers() -> Vec<Arc<dyn MailProvider>> {
    vec![
        Arc::new(gmail::GmailProvider::new()),
        Arc::new(outlook::OutlookProvider::new()),
        Arc::new(qq::QqMailProvider::new()),
    ]
}

/// 获取所有个人服务商检测器
pub fn all_detectors() -> Vec<Arc<dyn ProviderDetector>> {
    vec![
        Arc::new(gmail::GmailProvider::new()),
        Arc::new(outlook::OutlookProvider::new()),
        Arc::new(qq::QqMailProvider::new()),
    ]
}
```

**Step 5: 验证**

Run: `cargo check`

**Step 6: Commit**

```bash
git add src-tauri/src/domain/providers/personal/
git commit -m "feat: implement personal providers (Gmail, Outlook, QQ)"
```

---

### Task 1.4: 实现更多个人服务商

**Files:**
- Create: `src-tauri/src/domain/providers/personal/163.rs`
- Create: `src-tauri/src/domain/providers/personal/126.rs`
- Create: `src-tauri/src/domain/providers/personal/icloud.rs`
- Create: `src-tauri/src/domain/providers/personal/yahoo.rs`
- Create: `src-tauri/src/domain/providers/personal/zoho.rs`
- Modify: `src-tauri/src/domain/providers/personal/mod.rs`

**Step 1: 写 163.rs**

```rust
use crate::domain::providers::*;

pub struct Mail163Provider;

impl Mail163Provider { pub fn new() -> Self { Self } }

impl MailProvider for Mail163Provider {
    fn info(&self) -> &ProviderInfo {
        static INFO: once_cell::sync::Lazy<ProviderInfo> = once_cell::sync::Lazy::new(|| ProviderInfo {
            id: "163".into(), name: "163 Mail".into(),
            auth_types: vec![AuthType::Password], logo_url: None, color: Some("#D9291D".into()),
        });
        &INFO
    }
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig { host: "imap.163.com".into(), port: 993, ssl: SslMode::Tls }
    }
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig { host: "smtp.163.com".into(), port: 465, ssl: SslMode::Tls }
    }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::default() }
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: Some("INBOX".into()), sent: Some("Sent Messages".into()),
            drafts: Some("Drafts".into()), spam: Some("Junk".into()),
            trash: Some("Deleted Messages".into()), archive: None, starred: None,
        }
    }
}

impl ProviderDetector for Mail163Provider {
    fn domains(&self) -> &[&'static str] { &["163.com"] }
}
```

**Step 2: 写 126.rs**

```rust
use crate::domain::providers::*;

pub struct Mail126Provider;

impl Mail126Provider { pub fn new() -> Self { Self } }

impl MailProvider for Mail126Provider {
    fn info(&self) -> &ProviderInfo {
        static INFO: once_cell::sync::Lazy<ProviderInfo> = once_cell::sync::Lazy::new(|| ProviderInfo {
            id: "126".into(), name: "126 Mail".into(),
            auth_types: vec![AuthType::Password], logo_url: None, color: Some("#D9291D".into()),
        });
        &INFO
    }
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig { host: "imap.126.com".into(), port: 993, ssl: SslMode::Tls }
    }
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig { host: "smtp.126.com".into(), port: 465, ssl: SslMode::Tls }
    }
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::default() }
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: Some("INBOX".into()), sent: Some("Sent Messages".into()),
            drafts: Some("Drafts".into()), spam: Some("Junk".into()),
            trash: Some("Deleted Messages".into()), archive: None, starred: None,
        }
    }
}

impl ProviderDetector for Mail126Provider {
    fn domains(&self) -> &[&'static str] { &["126.com", "yeah.net"] }
}
```

**Step 3: 写 icloud.rs、yahoo.rs、zoho.rs**

模式相同，配置如下：

- **iCloud**: imap.mail.mecloud.systems:993 TLS / smtp.mail.mecloud.systems:587 StartTls / 域名: `icloud.com`, `me.com`, `mac.com`
- **Yahoo**: imap.mail.yahoo.com:993 TLS / smtp.mail.yahoo.com:465 TLS / 域名: `yahoo.com`, `yahoo.co.jp`
- **Zoho**: imap.zoho.com:993 TLS / smtp.zoho.com:465 TLS / 域名: `zoho.com`

每个文件结构同 163.rs，替换 host/port/domains/info 即可。

**Step 4: 更新 mod.rs 注册新模块**

在 `personal/mod.rs` 添加对应 pub mod 和 `all_providers()` / `all_detectors()` 中的 `Arc::new(...)` 条目。

**Step 5: 验证**

Run: `cargo check`

**Step 6: Commit**

```bash
git add src-tauri/src/domain/providers/personal/
git commit -m "feat: add more personal providers (163, 126, iCloud, Yahoo, Zoho)"
```

---

### Task 1.5: 实现 Provider 检测器

**Files:**
- Create: `src-tauri/src/domain/providers/detect.rs`
- Modify: `src-tauri/src/domain/providers/mod.rs`

**Step 1: 写 detect.rs**

```rust
use crate::domain::providers::traits::{MailProvider, ProviderDetector, ProviderInfo};
use super::personal;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, specta::Type)]
use serde::Serialize;

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ProviderDetectionResult {
    pub detected: bool,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub auth_types: Vec<String>,
}

/// 检测邮箱对应的服务商
pub fn detect_provider(email: &str) -> ProviderDetectionResult {
    let detectors: Vec<Arc<dyn ProviderDetector>> = personal::all_detectors();

    for detector in detectors {
        if detector.detect(email) {
            // 找到匹配的 detector，再从 providers 中找对应的 info
            let providers = personal::all_providers();
            if let Some(provider) = providers.iter().find(|p| {
                // 利用 detector 的 domains 来匹配 provider id
                // 简化：直接遍历
                detector.domains().iter().any(|_| true) && detector.detect(email)
            }) {
                return ProviderDetectionResult {
                    detected: true,
                    provider_id: Some(provider.info().id.clone()),
                    provider_name: Some(provider.info().name.clone()),
                    auth_types: provider.info().auth_types.iter().map(|t| format!("{:?}", t)).collect(),
                };
            }
        }
    }

    ProviderDetectionResult {
        detected: false,
        provider_id: None,
        provider_name: None,
        auth_types: vec![],
    }
}

/// 列出所有支持的服务商
pub fn list_providers() -> Vec<ProviderInfo> {
    personal::all_providers().iter().map(|p| p.info().clone()).collect()
}
```

**Step 2: 更新 providers/mod.rs**

```rust
pub mod traits;
pub mod detect;
pub mod personal;
pub mod enterprise;
```

**Step 3: 验证**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src-tauri/src/domain/providers/
git commit -m "feat: implement provider detection"
```

---

### Task 1.6: 实现 IMAP 协议层

**Files:**
- Create: `src-tauri/src/infrastructure/protocols/imap.rs`
- Modify: `src-tauri/src/infrastructure/protocols/mod.rs`

**Step 1: 写 imap.rs**

```rust
use async_imap::types::Fetches;
use crate::domain::providers::traits::{ImapServerConfig, SslMode};
use crate::error::MailError;

/// IMAP 连接客户端封装
pub struct ImapClient {
    client: async_imap::Client<async_native_tls::TlsStream<tokio::net::TcpStream>>,
    mailbox: Option<async_imap::Mailbox>,
}

impl ImapClient {
    /// 建立 IMAP 连接并登录
    pub async fn connect(
        config: &ImapServerConfig,
        email: &str,
        password: &str,
    ) -> Result<Self, MailError> {
        let tls = async_native_tls::TlsConnector::new();
        let stream = tokio::net::TcpStream::connect((config.host.as_str(), config.port))
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("TCP 连接失败: {}", e)))?;

        let tls_stream = tls.connect(&config.host, stream)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("TLS 握手失败: {}", e)))?;

        let client = async_imap::connect(tls_stream)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("IMAP 连接失败: {}", e)))?;

        let client = client
            .login(email, password)
            .await
            .map_err(|e| MailError::AuthFailed(format!("IMAP 登录失败: {:?}", e)))?;

        Ok(Self { client, mailbox: None })
    }

    /// 使用 XOAUTH2 登录 (Gmail 等)
    pub async fn connect_xoauth2(
        config: &ImapServerConfig,
        email: &str,
        auth_string: &str, // base64 编码的 XOAuth2 string
    ) -> Result<Self, MailError> {
        let tls = async_native_tls::TlsConnector::new();
        let stream = tokio::net::TcpStream::connect((config.host.as_str(), config.port))
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        let tls_stream = tls.connect(&config.host, stream)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        let client = async_imap::connect(tls_stream)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        let client = client
            .authenticate("XOAUTH2", async_imap::Authenticator::new(auth_string))
            .await
            .map_err(|e| MailError::AuthFailed(format!("{:?}", e)))?;

        Ok(Self { client, mailbox: None })
    }

    /// 选择文件夹
    pub async fn select_folder(&mut self, folder: &str) -> Result<async_imap::Mailbox, MailError> {
        let mailbox = self.client.select(folder)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("选择文件夹失败: {}", e)))?;
        self.mailbox = Some(mailbox.clone());
        Ok(mailbox)
    }

    /// 获取文件夹列表
    pub async fn list_folders(&mut self) -> Result<Vec<FolderInfo>, MailError> {
        let list = self.client.list(Some(""), Some("*"))
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        let folders: Vec<FolderInfo> = list
            .iter()
            .map(|f| FolderInfo {
                name: f.name().to_string(),
                delimiter: f.delimiter().map(|d| d.to_string()),
                flags: f.attributes().iter().map(|a| format!("{:?}", a)).collect(),
            })
            .collect();

        Ok(folders)
    }

    /// 按 UID 范围获取邮件
    pub async fn fetch_uids(&mut self, start: u32, end: u32) -> Result<Vec<RawEmail>, MailError> {
        let fetches = self.client.uid_fetch(
            format!("{}:{}", start, end),
            "(UID FLAGS ENVELOPE BODY.PEEK[HEADER.FIELDS (SUBJECT FROM TO CC DATE MESSAGE-ID)] BODYSTRUCTURE)",
        ).await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        let mut emails = Vec::new();
        for fetch in fetches.iter() {
            if let Some(uid) = fetch.uid {
                emails.push(RawEmail {
                    uid,
                    flags: fetch.flags().iter().map(|f| format!("{:?}", f)).collect(),
                    envelope: fetch.envelope().cloned(),
                    header: fetch.header().map(|h| h.to_vec()),
                });
            }
        }
        Ok(emails)
    }

    /// 获取邮件完整内容 (body)
    pub async fn fetch_body(&mut self, uid: u32) -> Result<Option<Vec<u8>>, MailError> {
        let fetches = self.client.uid_fetch(
            uid.to_string(),
            "BODY.PEEK[]",
        ).await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;

        if let Some(fetch) = fetches.iter().next() {
            return Ok(fetch.body().map(|b| b.to_vec()));
        }
        Ok(None)
    }

    /// 设置邮件标志
    pub async fn set_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        self.client.uid_store(uid.to_string(), flags)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;
        Ok(())
    }

    /// 登出
    pub async fn logout(mut self) -> Result<(), MailError> {
        self.client.logout().await
            .map_err(|e| MailError::ImapConnectionFailed(format!("{}", e)))?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct FolderInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug)]
pub struct RawEmail {
    pub uid: u32,
    pub flags: Vec<String>,
    pub envelope: Option<async_imap::types::Envelope>,
    pub header: Option<Vec<u8>>,
}
```

**Step 2: 更新 protocols/mod.rs**

```rust
pub mod imap;
pub mod smtp;
```

**Step 3: 验证**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src-tauri/src/infrastructure/protocols/
git commit -m "feat: implement IMAP protocol layer"
```

---

### Task 1.7: 实现 SMTP 协议层

**Files:**
- Create: `src-tauri/src/infrastructure/protocols/smtp.rs`

**Step 1: 写 smtp.rs**

```rust
use crate::domain::providers::traits::SmtpServerConfig;
use crate::error::MailError;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

/// 发送邮件
pub async fn send_email(
    config: &SmtpServerConfig,
    email: &str,
    password: &str,
    from: &str,
    to: &[String],
    cc: &[String],
    bcc: &[String],
    subject: &str,
    body_html: &str,
    body_text: &str,
) -> Result<String, MailError> {
    let mut builder = Message::builder()
        .from(from.parse().map_err(|e| MailError::InvalidParam(format!("发件人地址无效: {}", e)))?)
        .subject(subject);

    for addr in to {
        builder = builder.to(addr.parse().map_err(|e| MailError::InvalidParam(format!("收件人地址无效: {}", e)))?);
    }
    for addr in cc {
        builder = builder.cc(addr.parse().map_err(|e| MailError::InvalidParam(format!("CC 地址无效: {}", e)))?);
    }
    for addr in bcc {
        builder = builder.bcc(addr.parse().map_err(|e| MailError::InvalidParam(format!("BCC 地址无效: {}", e)))?);
    }

    let email_msg = builder
        .multipart(
            lettre::message::MultiPart::alternative()
                .singlepart(
                    lettre::message::SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(body_text.to_string())
                )
                .singlepart(
                    lettre::message::SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(body_html.to_string())
                )
        )
        .map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {}", e)))?;

    let creds = Credentials::new(email.to_string(), password.to_string());

    let mailer = match config.ssl {
        crate::domain::providers::traits::SslMode::Tls => {
            SmtpTransport::relay(&config.host)
                .map_err(|e| MailError::SmtpSendFailed(format!("{}", e)))?
                .port(config.port)
                .credentials(creds)
                .build()
        }
        crate::domain::providers::traits::SslMode::StartTls => {
            SmtpTransport::starttls_relay(&config.host)
                .map_err(|e| MailError::SmtpSendFailed(format!("{}", e)))?
                .port(config.port)
                .credentials(creds)
                .build()
        }
        _ => {
            SmtpTransport::builder_dangerous(&config.host)
                .port(config.port)
                .credentials(creds)
                .build()
        }
    };

    let result = tokio::task::spawn_blocking(move || mailer.send(&email_msg))
        .await
        .map_err(|e| MailError::SmtpSendFailed(format!("发送任务失败: {}", e)))?
        .map_err(|e| MailError::SmtpSendFailed(format!("SMTP 发送失败: {}", e)))?;

    Ok(format!("{:?}", result))
}
```

**Step 2: 验证**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src-tauri/src/infrastructure/protocols/smtp.rs
git commit -m "feat: implement SMTP protocol layer"
```

---

### Task 1.8: 实现 AuthManager (认证管理)

**Files:**
- Create: `src-tauri/src/domain/auth/mod.rs`
- Create: `src-tauri/src/domain/auth/manager.rs`

**Step 1: 写 manager.rs**

```rust
use crate::domain::providers::traits::{MailProvider, OAuthProvider};
use crate::error::MailError;
use std::collections::HashMap;
use std::sync::Arc;

/// 认证管理器 — 管理密码和 OAuth 认证
pub struct AuthManager {
    providers: HashMap<String, Arc<dyn MailProvider>>,
}

impl AuthManager {
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        for p in crate::domain::providers::personal::all_providers() {
            providers.insert(p.info().id.clone(), p);
        }
        Self { providers }
    }

    /// 获取服务商
    pub fn get_provider(&self, provider_id: &str) -> Option<Arc<dyn MailProvider>> {
        self.providers.get(provider_id).cloned()
    }

    /// 从 Keyring 获取密码
    pub async fn get_password(&self, email: &str) -> Result<String, MailError> {
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        entry.get_password()
            .map_err(|e| MailError::KeyringError(format!("{}", e)))
    }

    /// 保存密码到 Keyring
    pub async fn save_password(&self, email: &str, password: &str) -> Result<(), MailError> {
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        entry.set_password(password)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))
    }

    /// 删除 Keyring 中的密码
    pub async fn delete_password(&self, email: &str) -> Result<(), MailError> {
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        entry.delete_credential()
            .map_err(|e| MailError::KeyringError(format!("{}", e)))
    }

    /// 获取所有服务商信息
    pub fn list_providers(&self) -> Vec<crate::domain::providers::traits::ProviderInfo> {
        self.providers.values().map(|p| p.info().clone()).collect()
    }
}
```

**Step 2: 更新 auth/mod.rs**

```rust
pub mod manager;

pub use manager::AuthManager;
```

**Step 3: 在 Cargo.toml 添加 keyring 依赖**

```toml
keyring = "3"
```

**Step 4: 验证**

Run: `cargo check`

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement AuthManager for credential management"
```

---

### Task 1.9: 实现 Sync 基础类型

**Files:**
- Create: `src-tauri/src/domain/sync/mod.rs`
- Create: `src-tauri/src/domain/sync/types.rs`
- Create: `src-tauri/src/domain/sync/orchestrator.rs`
- Create: `src-tauri/src/domain/sync/progress.rs`

**Step 1: 写 types.rs**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncProgress {
    pub account_id: i32,
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncResult {
    pub new_emails: usize,
    pub updated_emails: usize,
    pub deleted_emails: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderStat {
    pub folder: String,
    pub total: usize,
    pub unread: usize,
}
```

**Step 2: 写 orchestrator.rs (骨架)**

```rust
use crate::error::MailError;
use super::types::SyncResult;

/// 同步编排器 — 只负责同步流程编排，不负责进度通知
pub struct SyncOrchestrator {
    // 将在 Phase 2 填充实现
}

impl SyncOrchestrator {
    pub fn new() -> Self { Self {} }

    /// 同步账号的所有文件夹
    pub async fn sync_account(&self, _account_id: i32) -> Result<SyncResult, MailError> {
        Err(MailError::NotImplemented("sync_account".into()))
    }
}
```

**Step 3: 写 progress.rs**

```rust
use tauri::{AppHandle, Emitter};
use crate::domain::sync::types::SyncProgress;
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
pub struct SyncProgressEvent {
    pub progress: SyncProgress,
}

/// 同步进度发射器 — 独立于 SyncOrchestrator
pub struct SyncProgressEmitter {
    app_handle: AppHandle,
}

impl SyncProgressEmitter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn emit(&self, progress: SyncProgress) {
        let event = SyncProgressEvent { progress };
        let _ = self.app_handle.emit("sync-progress", &event);
    }
}
```

**Step 4: 更新 sync/mod.rs**

```rust
pub mod types;
pub mod orchestrator;
pub mod progress;
```

**Step 5: 验证**

Run: `cargo check`

**Step 6: Commit**

```bash
git add src-tauri/src/domain/sync/
git commit -m "feat: implement sync types, orchestrator skeleton, and progress emitter"
```

---

### Task 1.10: 实现 enterprise 服务商骨架

**Files:**
- Modify: `src-tauri/src/domain/providers/enterprise/mod.rs`
- Create: `src-tauri/src/domain/providers/enterprise/google_workspace.rs`
- Create: `src-tauri/src/domain/providers/enterprise/microsoft_365.rs`
- Create: `src-tauri/src/domain/providers/enterprise/custom.rs`

**Step 1: 写 enterprise 骨架**

每个文件实现 `MailProvider` trait，使用基本配置：

- `google_workspace.rs` — IMAP: imap.gmail.com:993, 域名: 无固定 (手动配置)
- `microsoft_365.rs` — IMAP: outlook.office365.com:993, 域名: 无固定
- `custom.rs` — 所有配置从用户输入读取

`enterprise/mod.rs`:
```rust
pub mod google_workspace;
pub mod microsoft_365;
pub mod custom;

use crate::domain::providers::traits::MailProvider;
use std::sync::Arc;

pub fn all_providers() -> Vec<Arc<dyn MailProvider>> {
    vec![
        Arc::new(google_workspace::GoogleWorkspaceProvider::new()),
        Arc::new(microsoft_365::Microsoft365Provider::new()),
    ]
}
```

**Step 2: 验证**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src-tauri/src/domain/providers/enterprise/
git commit -m "feat: add enterprise provider skeletons"
```
