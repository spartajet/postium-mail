//! 服务类型层
//!
//! 定义业务相关的数据传输对象（DTO）和请求/响应类型。
//!
//! # 职责
//!
//! - **Request 类型**: 创建/更新资源的请求参数
//! - **Response 类型**: 返回给前端的数据结构
//! - **DTO 类型**: 数据传输对象，用于层间通信
//! - **转换逻辑**: Model ↔ DTO 的相互转换
//!
//! # 模块说明
//!
//! ## [`account`]
//!
//! 账号相关的服务类型：
//! - `CreateAccountRequest` - 创建账号请求
//! - `UpdateAccountRequest` - 更新账号请求
//! - `AccountDto` - 账号数据传输对象
//!
//! ## [`email`]
//!
//! 邮件相关的服务类型：
//! - `EmailListResponse` - 邮件列表响应
//! - `EmailDto` - 邮件数据传输对象
//!
//! ## [`folder`]
//!
//! 文件夹相关的服务类型：
//! - `FolderSyncStateDto` - 文件夹同步状态传输对象
//! - `StandardFolder` - 标准文件夹枚举
//!
//! # 设计原则
//!
//! - **单一职责**: 每个类型只负责一项职责
//! - **序列化友好**: 所有类型都实现了 `Serialize` 和 `Deserialize`
//! - **转换便利**: 提供 `From<Model>` 实现，简化 Model ↔ DTO 转换
//! - **验证内嵌**: 包含业务验证规则（如果有）

pub mod account;
pub mod email;
pub mod folder;

// 重新导出常用类型
pub use account::{AccountDto, AccountRepository, CreateAccountRequest, UpdateAccountRequest};
pub use email::{AttachmentInfo, EmailAddress, EmailDetail, EmailListResponse, EmailListItem, EmailRepository, EmailSearchParams, SendEmailRequest};
pub use folder::{FolderSyncStateDto, StandardFolder};
