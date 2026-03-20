//! 邮件实体模型
//!
//! 定义邮件的数据库实体和相关 DTO。
//!
//! # 实体字段
//!
//! | 字段 | 类型 | 说明 | 默认值 |
//! |------|------|------|--------|
//! | id | i32 | 主键 | 自动生成 |
//! | account_id | i32 | 所属账号 ID | - |
//! | folder | String | 文件夹名称 | - |
//! | uid | Option\<i32\> | IMAP UID | - |
//! | message_id | Option\<String\> | RFC 5322 Message-ID | - |
//! | subject | Option\<String\> | 邮件主题 | - |
//! | sender_name | Option\<String\> | 发件人名称 | - |
//! | sender_email | String | 发件人邮箱 | - |
//! | recipient_emails | String | 收件人列表（JSON） | - |
//! | cc_emails | Option\<String\> | 抄送列表（JSON） | - |
//! | bcc_emails | Option\<String\> | 密送列表（JSON） | - |
//! | body_text | Option\<String\> | 纯文本正文 | - |
//! | body_html | Option\<String\> | HTML 正文 | - |
//! | is_read | bool | 是否已读 | false |
//! | is_starred | bool | 是否星标 | false |
//! | is_draft | bool | 是否草稿 | false |
//! | sent_at | i64 | 发送时间 | - |
//! | received_at | i64 | 接收时间 | - |
//!
//! # 关联关系
//!
//! ```text
//! Email
//!   ├─ N:1 ─ Account (所属账号)
//!   ├─ N:1 ─ Folder (所属文件夹，级联删除)
//!   └─ 1:N ─ Attachment (附件列表)
//! ```
//!
//! # 数据传输对象
//!
//! ## EmailDetail
//!
//! 完整的邮件详情，包含：
//! - 邮件基本字段
//! - 收件人、抄送、密送列表
//! - 正文内容
//! - 附件列表
//!
//! ## EmailListItem
//!
//! 邮件列表项，用于列表显示：
//! - 不包含完整正文
//! - 包含 snippet（摘要）
//! - 包含附件数量
//!
//! ## EmailAddress
//!
//! 邮件地址结构：
//! ```rust
//! pub struct EmailAddress {
//!     pub email: String,    // 邮箱地址
//!     pub name: Option<String>,  // 显示名称
//! }
//! ```

use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "emails")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: Option<i32>,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub recipient_emails: String,            // JSON 数组
    pub cc_emails: Option<String>,           // JSON 数组
    pub bcc_emails: Option<String>,          // JSON 数组
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub sent_at: i64,
    pub received_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
    #[sea_orm(has_many = "super::attachment::Entity")]
    Attachments,
    #[sea_orm(
        belongs_to = "super::folder::Entity",
        from = "Column::Folder",
        to = "super::folder::Column::Name",
        on_delete = "Cascade"
    )]
    Folder,
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl Related<super::attachment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Attachments.def()
    }
}

impl Related<super::folder::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Folder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            created_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            updated_at: ActiveValue::Set(chrono::Utc::now().timestamp()),
            is_read: ActiveValue::Set(false),
            is_starred: ActiveValue::Set(false),
            is_draft: ActiveValue::Set(false),
            ..ActiveModelTrait::default()
        }
    }
}

// 邮件地址结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub email: String,
    pub name: Option<String>,
}

// 邮件详情 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDetail {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: Option<i32>,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub recipients: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub sent_at: i64,
    pub received_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: i32,
    pub filename: String,
    pub content_type: Option<String>,
    pub size: i64,
    pub path: Option<String>,
}

// 邮件列表项 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailListItem {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub snippet: Option<String>,           // 正文摘要
    pub has_attachment: bool,
    pub attachment_count: i32,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub sent_at: i64,
    pub received_at: i64,
}

// 发送邮件请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub subject: String,
    pub body_html: String,
    pub body_text: Option<String>,
    pub attachments: Vec<String>,           // 附件文件路径
    pub in_reply_to: Option<String>,        // 回复的邮件 ID
}

// 邮件搜索参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSearchParams {
    pub query: String,
    pub account_id: Option<i32>,
    pub folder: Option<String>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
