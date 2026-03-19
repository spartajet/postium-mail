use thiserror::Error;

/// IMAP 错误类型
#[derive(Error, Debug)]
pub enum ImapError {
    #[error("连接失败: {0}")]
    ConnectionFailed(String),

    #[error("认证失败: {0}")]
    AuthenticationFailed(String),

    #[error("IMAP 操作失败: {0}")]
    OperationFailed(String),

    #[error("解析失败: {0}")]
    ParseError(String),

    #[error("未连接")]
    NotConnected,

    #[error("其他错误: {0}")]
    Other(#[from] anyhow::Error),
}

/// IMAP Result 类型
pub type Result<T> = std::result::Result<T, ImapError>;
