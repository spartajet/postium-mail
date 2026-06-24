/// 标签数据模型 — 对应数据库 labels 表
///
/// 存储用户自定义的邮件标签（分类标记），每个标签属于特定账号。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    /// 标签唯一 ID（自增主键）
    pub id: i32,
    /// 所属账号 ID（外键关联 accounts 表）
    pub account_id: i32,
    /// 标签名称
    pub name: String,
    /// 标签颜色（十六进制色值，前端展示用）
    pub color: String,
    /// 创建时间戳（Unix 毫秒）
    pub created_at: i64,
}
