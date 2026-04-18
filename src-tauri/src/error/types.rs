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

    #[error("无效服务商: {0}")]
    InvalidProvider(String),

    #[error("OAuth2 流程错误: {0}")]
    OAuth2Error(String),

    #[error("未实现: {0}")]
    NotImplemented(String),

    #[error("标签不存在: {0}")]
    LabelNotFound(i32),

    #[error("IMAP 文件夹元数据获取失败: {0}")]
    ImapFolderMetadataFailed(String),

    #[error("IMAP 搜索失败: {0}")]
    ImapSearchFailed(String),

    #[error("批量获取邮件头失败: {0}")]
    BatchFetchHeadersFailed(String),

    #[error("IMAP 错误: {0}")]
    ImapError(String),

    #[error("邮件缺少 UID: {0}")]
    EmailMissingUid(String),
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
