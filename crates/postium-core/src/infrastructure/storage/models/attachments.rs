/// 附件数据模型 — 对应数据库 attachments 表
///
/// 存储邮件附件的元数据，支持按 MIME section 路径惰性下载附件内容。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 附件唯一 ID（自增主键）
    pub id: i32,
    /// 所属邮件 ID（外键关联 emails 表）
    pub email_id: i32,
    /// 附件文件名（某些附件可能没有文件名）
    pub filename: Option<String>,
    /// MIME Content-Type，如 "application/pdf"、"image/png"
    pub content_type: Option<String>,
    /// 附件大小（字节）
    pub size: i64,
    /// MIME section 路径，如 "2"、"3.1"，用于 UID FETCH BODY.PEEK[<section>] 按需下载
    pub section_path: String,
    /// Content-Disposition，如 "attachment"、"inline"
    pub disposition: Option<String>,
    /// Content-ID，用于 HTML 邮件内嵌图片的 cid: 引用
    pub content_id: Option<String>,
    /// 本地存储路径（下载后才有值）
    pub path: Option<String>,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: i64,
}
