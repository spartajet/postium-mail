//! 错误类型定义
//!
//! 提供统一的错误类型体系

use std::time::Duration;
use thiserror::Error;

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// 低严重度：用户可忽略，自动恢复
    Low,
    /// 中等严重度：影响部分功能，需要重试
    Medium,
    /// 高严重度：影响核心功能，需要用户干预
    High,
    /// 致命错误：应用无法继续
    Critical,
}

/// 邮件客户端统一错误类型
#[derive(Error, Debug)]
pub enum MailError {
    /// 连接错误
    #[error("连接错误: {0}")]
    Connection(#[from] ConnectionError),

    #[error("IMAP 错误: {0}")]
    Imap(#[from] ImapError),

    /// 认证错误
    #[error("认证错误: {0}")]
    Authentication(#[from] AuthError),

    /// 同步错误
    #[error("同步错误: {0}")]
    Sync(#[from] SyncError),

    /// OAuth 错误
    #[error("OAuth 错误: {0}")]
    OAuth(#[from] OAuthError),

    /// 存储错误
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),

    /// 限流错误
    #[error("请求过于频繁，请 {retry_after} 秒后重试")]
    RateLimit { retry_after: u64 },

    /// 通用错误
    #[error("内部错误: {0}")]
    Internal(String),
}

/// 连接错误
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("连接超时")]
    Timeout,

    #[error("SSL/TLS 错误: {0}")]
    Ssl(String),

    #[error("主机无法访问: {0}")]
    HostUnreachable(String),

    #[error("连接被拒绝")]
    ConnectionRefused,

    #[error("DNS 解析失败: {0}")]
    DnsFailed(String),
}

impl ConnectionError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ConnectionError::Timeout => ErrorSeverity::Medium,
            ConnectionError::HostUnreachable(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ConnectionError::Timeout | ConnectionError::ConnectionRefused
        )
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            ConnectionError::Timeout => Duration::from_secs(5),
            ConnectionError::ConnectionRefused => Duration::from_secs(10),
            _ => Duration::from_secs(5),
        }
    }
}

/// 认证错误
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("用户名或密码错误")]
    InvalidCredentials,

    #[error("账号被锁定")]
    AccountLocked,

    #[error("需要应用专用密码")]
    AppPasswordRequired,

    #[error("需要双因素认证")]
    TwoFactorRequired,

    #[error("OAuth Token 已过期")]
    OAuthExpired,

    #[error("OAuth 授权已撤销")]
    OAuthRevoked,
}

impl AuthError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AuthError::OAuthExpired => ErrorSeverity::Medium,
            AuthError::TwoFactorRequired => ErrorSeverity::High,
            _ => ErrorSeverity::High,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, AuthError::OAuthExpired)
    }

    pub fn retry_delay(&self) -> Duration {
        Duration::from_secs(0)
    }
}

/// 同步错误
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("文件夹不存在: {0}")]
    FolderNotFound(String),

    #[error("邮件数据损坏: {0}")]
    MessageCorrupted(String),

    #[error("无效的 UID: {0}")]
    UidInvalid(u32),

    #[error("存储配额已满")]
    QuotaExceeded,

    #[error("部分同步失败: {success}/{total}")]
    PartialFailure { success: usize, total: usize },

    #[error("UIDVALIDITY 变化，需要重新同步")]
    UidValidityChanged,

    #[error("同步状态不一致")]
    StateInconsistent,

    #[error("同步已被取消")]
    Cancelled,
}

impl SyncError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            SyncError::Cancelled => ErrorSeverity::Low,
            SyncError::UidValidityChanged => ErrorSeverity::Medium,
            SyncError::PartialFailure { .. } => ErrorSeverity::Medium,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        !matches!(self, SyncError::Cancelled)
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            SyncError::UidValidityChanged => Duration::from_secs(0),
            _ => Duration::from_secs(5),
        }
    }
}

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

    /// TLS 连接器创建失败
    #[error("创建 TLS 连接器失败: {0}")]
    TlsConnectorFailed(String),

    /// TLS 握手失败
    #[error("TLS 握手失败: {0}")]
    TlsHandshakeFailed(String),

    /// 认证失败
    ///
    /// 表示用户身份验证失败。
    /// 可能的原因包括：
    /// - 用户名或密码错误
    /// - OAuth 令牌无效或过期
    /// - 账号被锁定
    #[error("认证失败: {0}")]
    AuthenticationFailed(String),

    /// 密码登录失败
    #[error("密码登录失败: {0}")]
    PasswordLoginFailed(String),

    /// OAuth2 登录失败
    #[error("OAuth2 登录失败: {0}")]
    OAuthLoginFailed(String),

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

    /// 列出文件夹失败
    #[error("列出文件夹失败: {0}")]
    ListFoldersFailed(String),

    /// 收集文件夹列表失败
    #[error("收集文件夹列表失败: {0}")]
    CollectFoldersFailed(String),

    /// 选择文件夹失败
    #[error("选择文件夹失败: {0}")]
    SelectFolderFailed(String),

    /// 获取文件夹元数据失败
    #[error("获取文件夹元数据失败: {0}")]
    FolderMetadataFailed(String),

    /// 搜索邮件失败
    #[error("搜索邮件失败: {0}")]
    SearchFailed(String),

    /// 获取邮件失败
    #[error("获取邮件失败: {0}")]
    FetchEmailFailed(String),

    /// 收集邮件数据失败
    #[error("收集邮件数据失败: {0}")]
    CollectEmailDataFailed(String),

    /// 批量获取邮件头失败
    #[error("批量获取邮件头失败: {0}")]
    BatchFetchHeadersFailed(String),

    /// 收集邮件头数据失败
    #[error("收集邮件头数据失败: {0}")]
    CollectHeadersDataFailed(String),

    /// 获取邮件头失败
    #[error("获取邮件头失败: {0}")]
    FetchHeaderFailed(String),

    /// 获取纯文本正文失败
    #[error("获取纯文本正文失败: {0}")]
    FetchBodyFailed(String),

    /// 收集纯文本正文数据失败
    #[error("收集纯文本正文数据失败: {0}")]
    CollectBodyDataFailed(String),

    /// 获取 HTML 正文失败
    #[error("获取HTML正文失败: {0}")]
    FetchHtmlBodyFailed(String),

    /// 收集 HTML 正文数据失败
    #[error("收集HTML正文数据失败: {0}")]
    CollectHtmlBodyDataFailed(String),

    /// 标记邮件失败
    #[error("标记邮件失败: {0}")]
    MarkEmailFailed(String),

    /// 收集标记结果失败
    #[error("收集标记结果失败: {0}")]
    CollectMarkResultFailed(String),

    /// 设置标志失败
    #[error("设置标志失败: {0}")]
    SetFlagFailed(String),

    /// 收集设置标志结果失败
    #[error("收集设置标志结果失败: {0}")]
    CollectSetFlagResultFailed(String),

    /// 标记删除失败
    #[error("标记删除失败: {0}")]
    MarkDeleteFailed(String),

    /// 收集标记删除结果失败
    #[error("收集标记删除结果失败: {0}")]
    CollectMarkDeleteResultFailed(String),

    /// 删除邮件失败
    #[error("删除邮件失败: {0}")]
    DeleteEmailFailed(String),

    /// 收集删除结果失败
    #[error("收集删除结果失败: {0}")]
    CollectDeleteResultFailed(String),

    /// 登出失败
    #[error("登出失败: {0}")]
    LogoutFailed(String),

    /// 获取服务器能力失败
    #[error("获取服务器能力失败: {0}")]
    CapabilityFailed(String),

    /// 搜索新邮件失败
    #[error("搜索新邮件失败: {0}")]
    SearchNewEmailFailed(String),

    /// 获取文件夹状态失败
    #[error("获取文件夹状态失败: {0}")]
    FolderStatusFailed(String),

    /// 无法获取邮件头
    #[error("无法获取邮件头")]
    CannotFetchEmailHeader,

    /// 解析邮件头编码失败
    #[error("解析邮件头编码失败: {0}")]
    ParseEmailHeaderEncodingFailed(String),

    /// 解码邮件编码失败
    #[error("解码邮件编码失败: {0}")]
    DecodeEmailEncodingFailed(String),

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
    #[error("IMAP 未连接")]
    NotConnected,

    /// 邮件不存在
    #[error("邮件 {0} 不存在")]
    EmailNotFound(u32),

    /// 邮件体为空
    #[error("邮件体为空")]
    EmailBodyEmpty,

    /// 邮件缺少 UID
    #[error("邮件缺少 UID")]
    EmailMissingUid,

    /// 邮件头为空
    #[error("邮件头为空")]
    EmailHeaderEmpty,

    /// 指定 UID 的邮件头为空
    #[error("邮件头为空 UID={0}")]
    EmailHeaderEmptyByUid(u32),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 网络 IO 错误
    #[error("网络 IO 错误: {0}")]
    NetworkIo(String),
}

/// OAuth 错误
#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Token 已过期")]
    TokenExpired,

    #[error("Token 刷新失败: {0}")]
    RefreshFailed(String),

    #[error("无效的授权码")]
    InvalidGrant,

    #[error("访问被拒绝")]
    AccessDenied,

    #[error("网络错误: {0}")]
    NetworkError(String),
}

impl OAuthError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            OAuthError::TokenExpired => ErrorSeverity::Medium,
            OAuthError::AccessDenied => ErrorSeverity::Critical,
            _ => ErrorSeverity::Medium,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            OAuthError::TokenExpired | OAuthError::RefreshFailed(_)
        )
    }

    pub fn retry_delay(&self) -> Duration {
        match self {
            OAuthError::TokenExpired => Duration::from_secs(0),
            _ => Duration::from_secs(5),
        }
    }
}

/// 存储错误
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("Keyring 错误: {0}")]
    Keyring(String),

    #[error("数据不存在: {0}")]
    NotFound(String),

    #[error("文件 IO 错误: {0}")]
    Io(String),
}

impl StorageError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            StorageError::NotFound(_) => ErrorSeverity::Medium,
            StorageError::Io(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Critical,
        }
    }

    pub fn is_retryable(&self) -> bool {
        false
    }

    pub fn retry_delay(&self) -> Duration {
        Duration::from_secs(0)
    }
}

// 实现 From trait 用于自动转换
impl From<sea_orm::DbErr> for MailError {
    fn from(err: sea_orm::DbErr) -> Self {
        MailError::Storage(StorageError::Database(err.to_string()))
    }
}

impl From<tokio::task::JoinError> for MailError {
    fn from(err: tokio::task::JoinError) -> Self {
        MailError::Internal(format!("任务执行错误: {}", err))
    }
}

impl MailError {
    /// 获取错误严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            MailError::Connection(e) => e.severity(),
            MailError::Authentication(e) => e.severity(),
            MailError::Sync(e) => e.severity(),
            MailError::OAuth(e) => e.severity(),
            MailError::Storage(e) => e.severity(),
            MailError::RateLimit { .. } => ErrorSeverity::Low,
            MailError::Internal(_) => ErrorSeverity::Critical,
            MailError::Imap(imap_error) => ErrorSeverity::High,
        }
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self.severity(), ErrorSeverity::Low | ErrorSeverity::Medium)
    }

    /// 获取建议的重试延迟（秒）
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            MailError::RateLimit { retry_after } => Some(Duration::from_secs(*retry_after)),
            MailError::Connection(e) => Some(e.retry_delay()),
            MailError::Authentication(AuthError::OAuthExpired) => Some(Duration::from_secs(0)),
            MailError::Sync(e) => Some(e.retry_delay()),
            MailError::OAuth(e) => Some(e.retry_delay()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_severity() {
        let err = ConnectionError::Timeout;
        assert_eq!(err.severity(), ErrorSeverity::Medium);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_auth_error_severity() {
        let err = AuthError::InvalidCredentials;
        assert_eq!(err.severity(), ErrorSeverity::High);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_oauth_error_retryable() {
        let err = AuthError::OAuthExpired;
        assert!(err.is_retryable());
    }

    #[test]
    fn test_sync_error_cancelled() {
        let err = SyncError::Cancelled;
        assert_eq!(err.severity(), ErrorSeverity::Low);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_mail_error_conversion() {
        let conn_err = MailError::from(ConnectionError::Timeout);
        assert_eq!(conn_err.severity(), ErrorSeverity::Medium);

        let auth_err = MailError::from(AuthError::InvalidCredentials);
        assert_eq!(auth_err.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_rate_limit_error() {
        let err = MailError::RateLimit { retry_after: 60 };
        assert_eq!(err.severity(), ErrorSeverity::Low);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(Duration::from_secs(60)));
    }
}
