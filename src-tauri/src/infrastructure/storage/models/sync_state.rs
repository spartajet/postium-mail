/// 同步状态数据模型 — 对应数据库 sync_state 表
///
/// 跟踪每个账号下各文件夹的 IMAP 同步进度，用于增量同步和断点续传。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 同步状态唯一 ID（自增主键）
    pub id: i32,
    /// 所属账号 ID（外键关联 accounts 表）
    pub account_id: i32,
    /// 文件夹路径（如 "INBOX"、"Sent"）
    pub folder: String,
    /// 标准分类（如 "inbox"、"sent"），由 FolderRegistry 写入
    pub folder_category: Option<String>,
    /// 文件夹别名（IMAP 服务器返回的显示名称）
    pub folder_nick_name: Option<String>,
    /// IMAP UIDVALIDITY 值（文件夹重建时变化，用于检测 UID 失效）
    pub uidvalidity: Option<u32>,
    /// IMAP UIDNEXT 值（下一个分配的 UID，用于判断是否有新邮件）
    pub uidnext: Option<u32>,
    /// 上次同步完成时间戳（Unix 毫秒）
    pub synced_at: Option<i64>,
    /// 已同步的最后一个邮件 UID（增量同步的断点）
    pub last_sync_uid: Option<u32>,
    /// 历史邮件已经同步到的最早时间戳（Unix 秒）
    pub history_synced_since: Option<i64>,
    /// 下一次历史回填应查询的 UID 上界（查询 UID 小于该值的邮件）
    pub history_before_uid: Option<u32>,
    /// 是否已确认没有更早的历史邮件
    pub history_exhausted: Option<bool>,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: Option<i64>,
    /// 最后更新时间戳（Unix 毫秒）
    pub updated_at: Option<i64>,
}
