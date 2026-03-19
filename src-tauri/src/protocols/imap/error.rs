//! IMAP 错误类型定义
//!
//! 定义了 IMAP 协议操作中可能出现的所有错误类型。
//! 使用 `thiserror` 提供结构化的错误信息和良好的错误链追踪。

use thiserror::Error;

/// IMAP 错误类型
///
/// 表示 IMAP 协议操作中可能出现的各种错误情况。
/// 每个错误变体都提供了详细的人类可读的错误消息。
///
/// # 错误分类
///
/// ## 连接错误
/// - `ConnectionFailed`: 无法建立到 IMAP 服务器的连接
///
/// ## 认证错误
/// - `AuthenticationFailed`: 用户名密码错误或 OAuth 令牌无效
///
/// ## 操作错误
/// - `OperationFailed`: IMAP 命令执行失败（如文件夹不存在、权限不足等）
///
/// ## 解析错误
/// - `ParseError`: 无法解析服务器响应
///
/// ## 状态错误
/// - `NotConnected`: 尝试在未连接状态下执行操作
///
/// ## 其他错误
/// - `Other`: 包装其他类型的错误（如网络错误、IO 错误等）
///
/// # 示例
///
/// ```rust
/// use imap::error::{ImapError, Result};
///
/// fn connect_to_server() -> Result<()> {
///     // ... 连接逻辑 ...
///     Err(ImapError::ConnectionFailed("超时".to_string()))
/// }
/// ```
#[derive(Error, Debug)]
pub enum ImapError {
    /// 连接失败
    ///
    /// 表示无法建立到 IMAP 服务器的网络连接。
    /// 可能的原因包括：
    /// - 网络不可达
    /// - 服务器地址错误
    /// - 连接超时
    /// - 证书验证失败
    #[error("连接失败: {0}")]
    ConnectionFailed(String),

    /// 认证失败
    ///
    /// 表示用户身份验证失败。
    /// 可能的原因包括：
    /// - 用户名或密码错误
    /// - OAuth 令牌无效或过期
    /// - 账号被锁定
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),

    /// IMAP 操作失败
    ///
    /// 表示 IMAP 命令执行失败。
    /// 可能的原因包括：
    /// - 文件夹不存在
    /// - 权限不足
    /// - 命令参数错误
    /// - 服务器返回 NO 或 BAD 响应
    #[error("IMAP 操作失败: {0}")]
    OperationFailed(String),

    /// 解析失败
    ///
    /// 表示无法解析服务器的响应。
    /// 可能的原因包括：
    /// - 响应格式不符合 RFC 3501
    /// - 字符编码问题
    /// - 意外的响应结构
    #[error("解析失败: {0}")]
    ParseError(String),

    /// 未连接
    ///
    /// 表示尝试在未连接状态下执行操作。
    /// 通常需要先调用 `connect()` 方法建立连接。
    #[error("未连接")]
    NotConnected,

    /// 其他错误
    ///
    /// 包装其他类型的错误，如：
    /// - 网络库错误
    /// - IO 错误
    /// - TLS/SSL 错误
    #[error("其他错误: {0}")]
    Other(#[from] anyhow::Error),
}

/// IMAP Result 类型
///
/// IMAP 操作的标准返回类型，简化错误处理。
///
/// # 示例
///
/// ```rust
/// use imap::error::Result;
///
/// fn list_folders() -> Result<Vec<String>> {
///     // ... 返回 Vec<String> 或 ImapError
/// }
/// ```
pub type Result<T> = std::result::Result<T, ImapError>;
