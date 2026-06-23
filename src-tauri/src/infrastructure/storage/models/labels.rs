#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}
