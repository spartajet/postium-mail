//! 存储层模块
//!
//! 提供数据库和缓存功能

pub mod cache;
pub mod database;

// 重新导出常用类型
pub use cache::{
    CacheManager, CounterCache, EmailContentCache, FolderListCache, MemoryCache,
};
pub use database::{DatabaseConnection, Repository};
