//! 数据仓库模块（Repository）
//!
//! 提供对数据库表的访问层封装，包含各个领域的仓库实现。
//! 每个仓库负责对应表的 CRUD 操作和业务查询。
//!
//! # 子模块
//!
//! - [`account_repo`] - 账号数据访问
//! - [`attachment_repo`] - 附件数据访问
//! - [`email_repo`] - 邮件数据访问
//! - [`label_repo`] - 标签数据访问
//! - [`sync_repo`] - 同步状态数据访问

pub mod account_repo;
pub mod attachment_repo;
pub mod email_repo;
pub mod label_repo;
pub mod sync_repo;
