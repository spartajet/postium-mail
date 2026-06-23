#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    pub id: i32,
    pub email_id: i32,
    pub label_id: i32,
    pub created_at: i64,
}
