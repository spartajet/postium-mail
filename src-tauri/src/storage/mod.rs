//! 存储层模块
//!
//! 提供数据库和缓存功能

pub mod accounts;
pub mod cache;
pub mod database;
pub mod emails;
pub mod folders;

// 重新导出常用类型
pub use accounts::{AccountRepository, CreateAccountRequest, UpdateAccountRequest};
pub use cache::{
    CacheManager, CounterCache, EmailContentCache, FolderListCache, MemoryCache,
};
pub use database::{DatabaseConnection, Repository};
pub use emails::{EmailListResponse, EmailRepository};
pub use folders::FolderRepository;
