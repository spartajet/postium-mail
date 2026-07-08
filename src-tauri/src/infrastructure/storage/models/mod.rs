//! 数据模型模块
//!
//! 定义了应用中所有数据表的 Rust 数据模型，用于与数据库交互和数据序列化。
//!
//! # 模块列表
//!
//! - [`accounts`] - 账号数据模型，存储邮箱账号信息
//! - [`attachments`] - 附件数据模型，存储邮件附件元数据
//! - [`email_labels`] - 邮件-标签关联模型，存储多对多关联关系
//! - [`emails`] - 邮件数据模型，存储同步的邮件数据
//! - [`labels`] - 标签数据模型，存储用户自定义标签
//! - [`sync_errors`] - 同步错误模型，记录同步过程中的错误
//! - [`sync_state`] - 同步状态模型，跟踪 IMAP 同步进度

pub mod accounts;
pub mod attachments;
pub mod email_labels;
pub mod emails;
pub mod labels;
pub mod sync_errors;
pub mod sync_state;
