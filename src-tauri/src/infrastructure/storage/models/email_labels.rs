/// 邮件-标签关联数据模型 — 对应数据库 email_labels 表
///
/// 存储邮件与标签的多对多关联关系（一封邮件可打多个标签，一个标签可关联多封邮件）。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 关联记录唯一 ID（自增主键）
    pub id: i32,
    /// 邮件 ID（外键关联 emails 表）
    pub email_id: i32,
    /// 标签 ID（外键关联 labels 表）
    pub label_id: i32,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: i64,
}
