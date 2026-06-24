/// 账号数据模型 — 对应数据库 accounts 表
///
/// 存储用户添加的邮箱账号信息，包括服务器连接配置、认证方式和同步状态。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 账号唯一 ID（自增主键）
    pub id: i32,
    /// 账号显示名称（用户自定义）
    pub name: String,
    /// 邮箱地址
    pub email: String,
    /// 发件人显示名称（显示在邮件的 From 字段）
    pub display_name: Option<String>,
    /// 服务商标识（如 "gmail"、"outlook"、"custom"）
    pub provider: String,
    /// IMAP 服务器主机地址
    pub imap_host: Option<String>,
    /// IMAP 服务器端口
    pub imap_port: Option<i32>,
    /// IMAP 是否启用 SSL
    pub imap_ssl: Option<bool>,
    /// IMAP SSL 模式（如 "implicit"、"starttls"）
    pub imap_ssl_mode: Option<String>,
    /// SMTP 服务器主机地址
    pub smtp_host: Option<String>,
    /// SMTP 服务器端口
    pub smtp_port: Option<i32>,
    /// SMTP 是否启用 SSL
    pub smtp_ssl: Option<bool>,
    /// SMTP SSL 模式（如 "implicit"、"starttls"）
    pub smtp_ssl_mode: Option<String>,
    /// 账号主题色（前端展示用，十六进制色值）
    pub color: Option<String>,
    /// 是否启用自动同步
    pub sync_enabled: Option<bool>,
    /// 上次成功同步的时间戳（Unix 毫秒）
    pub last_sync_at: Option<i64>,
    /// 认证方式（如 "password"、"oauth2"）
    pub auth_type: Option<String>,
    /// 账号类型（"personal" 或 "enterprise"）
    pub account_type: String,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: i64,
    /// 最后更新时间戳（Unix 毫秒）
    pub updated_at: i64,
}
