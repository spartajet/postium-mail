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

    #[error("附件不存在: {0}")]
    AttachmentNotFound(i32),

    #[error("附件不可下载: {0}")]
    AttachmentUnavailable(String),

    #[error("附件下载失败: {0}")]
    AttachmentDownloadFailed(String),

    #[error("附件解码失败: {0}")]
    AttachmentDecodeFailed(String),

    #[error("文件系统错误: {0}")]
    FileSystemError(String),

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

impl From<rusqlite::Error> for MailError {
    fn from(err: rusqlite::Error) -> Self {
        MailError::DatabaseError(err.to_string())
    }
}

impl From<tokio_rusqlite::Error> for MailError {
    fn from(err: tokio_rusqlite::Error) -> Self {
        MailError::DatabaseError(err.to_string())
    }
}

// JSON 错误转换
impl From<serde_json::Error> for MailError {
    fn from(err: serde_json::Error) -> Self {
        MailError::InvalidParam(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mail_error_display() {
        let err = MailError::AccountNotFound(42);
        assert_eq!(err.to_string(), "账户不存在: 42");

        let err = MailError::AuthFailed("bad token".to_string());
        assert!(err.to_string().contains("bad token"));

        let err = MailError::InvalidParam("missing field".to_string());
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn test_from_db_error() {
        let db_err = rusqlite::Error::QueryReturnedNoRows;
        let mail_err: MailError = db_err.into();
        assert!(matches!(mail_err, MailError::DatabaseError(_)));
    }

    #[test]
    fn test_from_json_error() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let mail_err: MailError = json_err.into();
        assert!(matches!(mail_err, MailError::InvalidParam(_)));
    }
}
