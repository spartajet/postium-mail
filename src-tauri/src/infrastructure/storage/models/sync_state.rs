#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Model {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub folder_nick_name: Option<String>,
    pub uidvalidity: Option<u32>,
    pub uidnext: Option<u32>,
    pub synced_at: Option<i64>,
    pub last_sync_uid: Option<u32>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}
