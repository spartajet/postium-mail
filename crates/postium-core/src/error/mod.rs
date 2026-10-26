//!
//! # 错误处理模块 (Error Handling Module)
//!
//! 本模块定义了应用程序的统一错误类型，用于在整个应用中传递和处理错误。
//!
//! ## 设计原则
//!
//! 1. **统一错误类型**: 使用 `MailError` 枚举作为所有业务错误的统一类型
//! 2. **类型安全**: 通过 Rust 的类型系统确保错误处理的完整性
//! 3. **前端友好**: 错误信息序列化为 JSON 格式，便于前端解析和展示
//! 4. **详细错误信息**: 每个错误变体都包含上下文信息，便于调试
//!
//! ## 错误分类
//!
//! `MailError` 枚举包含以下几类错误：
//!
//! ### 账号相关错误
//! - `AccountNotFound`: 账号不存在
//!
//! ### 认证错误
//! - `AuthFailed`: 认证失败（密码错误、令牌无效等）
//! - `OAuthError`: OAuth 授权错误
//! - `OAuth2Error`: OAuth2 流程错误
//! - `KeyringError`: 系统密钥环操作失败
//!
//! ### 连接错误
//! - `ImapConnectionFailed`: IMAP 服务器连接失败
//! - `SmtpSendFailed`: SMTP 邮件发送失败
//! - `ImapError`: IMAP 协议操作错误
//!
//! ### 同步错误
//! - `SyncFailed`: 邮件同步失败
//!
//! ### 数据错误
//! - `DatabaseError`: 数据库操作错误
//! - `EmailNotFound`: 邮件不存在
//! - `FolderNotFound`: 文件夹不存在
//! - `LabelNotFound`: 标签不存在
//! - `EmailMissingUid`: 邮件缺少 UID 标识
//!
//! ### 参数错误
//! - `InvalidParam`: 参数验证失败
//! - `InvalidProvider`: 无效的服务提供商
//! - `ProviderNotSupported`: 不支持的服务提供商
//! - `NotImplemented`: 功能未实现
//!
//! ## 错误处理流程
//!
//! ```text
//! Service/Domain 层产生错误
//!         ↓
//! 转换为 MailError 枚举
//!         ↓
//! 通过 Tauri Command 返回给前端
//!         ↓
//! 前端通过 tauri-specta 获取类型化错误
//!         ↓
//! 在 UI 中展示错误信息
//! ```
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! // 在 Service 层返回错误
//! pub async fn get_email(&self, id: i32) -> Result<EmailDto, MailError> {
//!     let email = email_repo::find_by_id(&self.db, id)
//!         .await?
//!         .ok_or(MailError::EmailNotFound(id))?;
//!     Ok(email.into())
//! }
//!
//! // 自动错误转换（实现了 From trait）
//! fn example() -> Result<(), MailError> {
//!     // SeaORM 错误自动转换为 MailError::DatabaseError
//!     let account = account_repo::get_by_id(&db, id).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## 前端错误处理
//!
//! 前端通过 tauri-specta 的 Result 模式获取类型化的错误：
//!
//! ```typescript
//! try {
//!     const email = await invoke('get_email', { id: 123 });
//! } catch (error) {
//!     // error 的类型为 MailError
//!     switch (error.type) {
//!         case 'EmailNotFound':
//!             console.error(`邮件 ${error.message} 不存在`);
//!             break;
//!         case 'DatabaseError':
//!             console.error('数据库错误:', error.message);
//!             break;
//!     }
//! }
//! ```
//!
//! ## 序列化格式
//!
//! 错误以 JSON 格式序列化，包含 `type` 和 `message` 两个字段：
//!
//! ```json
//! {
//!     "type": "EmailNotFound",
//!     "message": "123"
//! }
//! ```
//!

/// 错误类型定义模块
///
/// 包含 `MailError` 枚举的完整定义和相关的错误转换实现。
pub mod types;

/// 重新导出统一错误类型
///
/// 允许外部模块直接使用 `crate::error::MailError` 而不需要指定完整路径。
pub use types::MailError;
