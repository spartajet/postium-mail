//!
//! # 认证管理模块 (Authentication Module)
//!
//! 本模块负责管理邮件账号的认证凭证，是应用程序安全层的核心组件。
//!
//! ## 主要功能
//!
//! 1. **凭证管理**：统一管理密码和 OAuth2 两种认证方式的凭证
//! 2. **Token 缓存**：内存缓存 OAuth2 access_token，减少网络请求
//! 3. **自动刷新**：自动处理 OAuth2 token 的刷新和轮换
//! 4. **安全存储**：集成系统 Keyring，安全持久化密码和 refresh_token
//!
//! ## 模块结构
//!
//! ### manager - 认证管理器
//!
//! 核心组件，提供统一的凭证获取接口：
//! - `AuthManager`: 认证管理器，管理所有账号的认证状态
//! - `Credentials`: 凭证枚举，统一表示密码和 OAuth2 令牌
//!
//! ### token_cache - Token 缓存
//!
//! 提供高效的 OAuth2 access_token 内存缓存：
//! - 减少对 OAuth2 服务器的请求次数
//! - 自动处理 token 过期
//! - 线程安全的缓存实现
//!
//! ## 认证流程
//!
//! ### 密码认证
//!
//! ```text
//! 用户输入密码 → Keyring 存储 → 获取时从 Keyring 读取 → 传递给 IMAP/SMTP
//! ```
//!
//! ### OAuth2 认证
//!
//! ```text
//! 浏览器授权 → 获取授权码 → 交换令牌 → 存储 refresh_token 到 Keyring
//! → access_token 缓存到内存 → 使用时检查有效期 → 过期自动刷新
//! ```
//!
//! ## 安全特性
//!
//! - **零数据库存储**：密码和 refresh_token 永不存入数据库
//! - **系统级加密**：使用操作系统的 Keyring 机制加密存储敏感信息
//! - **内存缓存**：access_token 仅缓存在内存中，应用重启后自动清除
//! - **自动刷新**：OAuth2 token 过期时自动使用 refresh_token 获取新的 access_token
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use crate::domain::auth::{AuthManager, Credentials};
//!
//! // 创建认证管理器
//! let auth_manager = AuthManager::default();
//!
//! // 存储密码凭证
//! auth_manager.store_password(account_id, "my_password").await?;
//!
//! // 获取凭证（自动处理 OAuth2 刷新）
//! let credentials = auth_manager.get_credentials(account_id).await?;
//!
//! match credentials {
//!     Credentials::Password(password) => {
//!         // 使用密码连接 IMAP/SMTP
//!     }
//!     Credentials::OAuth2 { access_token } => {
//!         // 使用 OAuth2 access_token 连接
//!     }
//! }
//! ```
//!
//! ## 线程安全
//!
//! - `AuthManager` 内部使用 `Arc` 和 `Mutex` 保证线程安全
//! - `TokenCache` 使用 `RwLock` 实现高效的读写并发
//! - 所有公共方法都是线程安全的，可以在多线程环境中使用
//!

/// 认证管理器子模块
///
/// 包含 `AuthManager` 结构体和 `Credentials` 枚举的定义，
/// 提供统一的认证凭证管理接口。
pub mod manager;

/// Token 缓存子模块
///
/// 提供 OAuth2 access_token 的内存缓存实现，
/// 支持自动过期检查和线程安全的并发访问。
pub mod token_cache;

// ========== 公共导出 ==========

/// 重新导出认证管理器和凭证类型
///
/// - `AuthManager`: 认证管理器，用于获取和管理账号凭证
/// - `Credentials`: 凭证枚举，表示密码或 OAuth2 令牌
///
/// # 使用示例
///
/// ```rust,ignore
/// use crate::domain::auth::{AuthManager, Credentials};
///
/// let auth = AuthManager::default();
/// let creds = auth.get_credentials(account_id).await?;
/// ```
pub use manager::{AuthManager, Credentials};
