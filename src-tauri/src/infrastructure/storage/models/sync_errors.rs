/// 同步错误数据模型 — 对应数据库 sync_errors 表
///
/// 记录邮件同步过程中发生的错误，用于错误追踪和重试。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 错误记录唯一 ID（自增主键）
    pub id: i32,
    /// 发生错误的账号 ID（外键关联 accounts 表）
    pub account_id: i32,
    /// 发生错误的文件夹（如 "INBOX"）
    pub folder: Option<String>,
    /// 错误类型标识（如 "imap_connection"、"parse_failed"）
    pub error_type: String,
    /// 错误描述信息
    pub error_message: String,
    /// 发生错误时的邮件 UID（如果与特定邮件相关）
    pub uid: Option<u32>,
    /// 错误堆栈跟踪（用于调试）
    pub stack_trace: Option<String>,
    /// 是否已解决（标记为已处理后不再重试）
    pub resolved: Option<bool>,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: i64,
}
