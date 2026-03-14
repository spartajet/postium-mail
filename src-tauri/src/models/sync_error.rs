use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_errors")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub folder: Option<String>,       // 失败的文件夹 (可选)
    pub error_type: String,           // 错误类型 (connection, auth, parse, etc.)
    pub error_message: String,        // 错误消息
    pub uid: Option<i32>,             // 失败的邮件 UID (可选)
    pub stack_trace: Option<String>,  // 堆栈信息
    pub resolved: bool,               // 是否已解决
    pub created_at: i64,              // 发生时间
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            resolved: ActiveValue::Set(false),
            created_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            ..ActiveModelTrait::default()
        }
    }
}

// 前端传输用的 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncErrorDto {
    pub id: i32,
    pub account_id: i32,
    pub folder: Option<String>,
    pub error_type: String,
    pub error_message: String,
    pub uid: Option<i32>,
    pub resolved: bool,
    pub created_at: i64,
}

impl From<Model> for SyncErrorDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            folder: model.folder,
            error_type: model.error_type,
            error_message: model.error_message,
            uid: model.uid,
            resolved: model.resolved,
            created_at: model.created_at,
        }
    }
}

// 错误类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorType {
    Connection,
    Auth,
    Parse,
    Database,
    Timeout,
    Unknown,
}

impl ErrorType {
    pub fn as_str(&self) -> &str {
        match self {
            ErrorType::Connection => "connection",
            ErrorType::Auth => "auth",
            ErrorType::Parse => "parse",
            ErrorType::Database => "database",
            ErrorType::Timeout => "timeout",
            ErrorType::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "connection" => ErrorType::Connection,
            "auth" => ErrorType::Auth,
            "parse" => ErrorType::Parse,
            "database" => ErrorType::Database,
            "timeout" => ErrorType::Timeout,
            _ => ErrorType::Unknown,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, ErrorType::Connection | ErrorType::Database | ErrorType::Timeout)
    }
}
