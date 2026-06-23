#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    pub id: i32,
    pub account_id: i32,
    pub folder: Option<String>,
    pub error_type: String,
    pub error_message: String,
    pub uid: Option<u32>,
    pub stack_trace: Option<String>,
    pub resolved: Option<bool>,
    pub created_at: i64,
}
