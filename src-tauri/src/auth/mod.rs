//! 认证管理模块
//!
//! 提供统一的邮件账号认证管理功能，支持密码认证和 OAuth2 认证。
//!
//! # 核心功能
//!
//! - **认证管理**: [`AuthManager`] 统一管理所有认证方式
//! - **密码认证**: 传统的用户名/密码认证
//! - **OAuth2 认证**: 支持 Gmail、Outlook 等 OAuth2 登录
//! - **企业认证**: 支持 Microsoft 365、Google Workspace 企业认证
//! - **Token 管理**: OAuth Token 自动刷新和过期处理
//!
//! # 架构设计
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              AuthManager               │
//! │  (认证管理器、策略选择、Token 刷新)     │
//! └────────────┬────────────────────────────┘
//!              │
//!    ┌─────────┼─────────┬──────────┐
//!    │         │         │          │
//! ┌───▼───┐ ┌──▼────┐ ┌─▼──────┐ ┌─▼──────┐
//! │Password│ │OAuth  │ │Enterprise│ │Token  │
//! │ Auth   │ │Handler│ │ Auth    │ │Manager│
//! └───────┘ └────────┘ └────────┘ └────────┘
//! ```
//!
//! # 认证方式
//!
//! ## 密码认证
//!
//! 使用应用专用密码（App Password）进行 IMAP/SMTP 认证：
//!
//! ```text
//! 用户输入密码 → Keyring 存储 → 使用时读取
//! ```
//!
//! **适用场景**:
//! - Gmail 应用专用密码
//! - Outlook 传统密码
//! - QQ/163 等国内邮箱
//!
//! ## OAuth2 认证
//!
//! 使用 OAuth2 授权码流程：
//!
//! ```text
//! 1. 生成授权 URL
//! 2. 用户在浏览器中授权
//! 3. 接收授权码回调
//! 4. 交换 Access Token
//! 5. 定期刷新 Token
//! ```
//!
//! **支持的服务商**:
//! - Google (Gmail, Google Workspace)
//! - Microsoft (Outlook, Microsoft 365)
//!
//! ## 企业认证
//!
//! 企业邮箱的额外认证方式：
//!
//! - **SAML SSO**: 单点登录
//! - **域认证**: 企业域账号认证
//!
//! # Token 管理
//!
//! ## OAuth Token 结构
//!
//! ```rust
//! pub struct OAuthToken {
//!     pub refresh_token: String,  // 用于刷新
//!     pub expires_at: i64,        // 过期时间（Unix 时间戳）
//! }
//! ```
//!
//! ## Token 刷新流程
//!
//! ```text
//! 检查过期时间 → 已过期？
//!     │              │
//!     ↓              ↓
//!    未过期         使用 refresh_token
//!     │              刷新
//!     ↓              ↓
//!  直接使用       更新 Keyring
//! ```
//!
//! # 使用示例
//!
//! ## 创建 AuthManager
//!
//! ```rust,no_run
//! # use crate::auth::AuthManager;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! # let app_handle = todo!();
//! let manager = AuthManager::new(&db, &app_handle)?;
//!
//! // 获取 IMAP 认证信息
//! let auth_info = manager.get_imap_auth(1).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## OAuth2 流程
//!
//! ```rust,no_run
//! # use crate::auth::AuthManager;
//! # async fn example() -> anyhow::Result<()> {
//! # let manager = todo!();
//! // 1. 生成授权 URL
//! let auth_url = manager.get_oauth_auth_url("gmail", "state").await?;
//!
//! // 2. 用户授权后，交换 Token
//! let token_response = manager.exchange_oauth_code("gmail", "code", "state").await?;
//!
//! // 3. Token 自动存储到 Keyring
//! # Ok(())
//! # }
//! ```
//!
//! ## 刷新 Token
//!
//! ```rust,no_run
//! # use crate::auth::AuthManager;
//! # async fn example() -> anyhow::Result<()> {
//! # let manager = todo!();
//! // 检查并刷新过期的 Token
//! manager.refresh_oauth_token(1).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Keyring 存储
//!
//! 敏感信息使用系统 Keyring 加密存储：
//!
//! ```text
//! Service: "com.postium.mail"
//! Username: "password:<account_id>"    → 密码
//! Username: "oauth:<account_id>"       → OAuth Token
//! ```

mod auth_manager;
mod enterprise_auth;
mod oauth_handler;
mod password_auth;
mod token_manager;

// 重新导出主要类型
pub use auth_manager::{AuthManager, ImapAuthInfo};
