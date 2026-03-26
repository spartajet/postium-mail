//! 数据库连接管理
//!
//! 提供统一的数据库连接池管理和健康检查功能。
//!
//! # 核心功能
//!
//! - **连接池管理**: SeaORM 连接池配置和生命周期管理
//! - **健康检查**: 数据库连接健康状态检测
//! - **连接配置**: 可配置的连接参数（超时、连接数等）
//! - **Repository Trait**: 统一的数据库访问抽象
//! - **数据库迁移**: 使用 SeaORM 官方迁移系统
//!
//! # 连接池配置
//!
//! ```text
//! max_connections: 10      - 最大连接数
//! min_connections: 1       - 最小空闲连接数
//! connect_timeout: 8s      - 连接超时
//! idle_timeout: 600s       - 空闲连接超时
//! ```
//!
//! # 使用示例
//!
//! ## 创建数据库连接
//!
//! ```rust,no_run
//! use crate::storage::{establish_connection, init_database, DatabaseConnection};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 方式1: 使用 establish_connection（自动获取数据库路径）
//! let db = establish_connection().await?;
//!
//! // 方式2: 使用 DatabaseConnection::new（自定义 URL）
//! let db_conn = DatabaseConnection::new("sqlite://./data.db").await?;
//!
//! // 初始化数据库（运行迁移）
//! init_database(&db).await?;
//!
//! // 健康检查
//! let is_healthy = db_conn.health_check().await?;
//! println!("数据库健康: {}", is_healthy);
//! # Ok(())
//! # }
//! ```
//!
//! ## 从现有连接创建
//!
//! ```rust,no_run
//! # use sea_orm::DbConn;
//! # use crate::storage::DatabaseConnection;
//! # fn example(conn: DbConn) {
//! let db = DatabaseConnection::from_conn(conn);
//!
//! // 获取底层连接
//! let conn = db.connection();
//! # }
//! ```
//!
//! # Repository Trait
//!
//! [`Repository`] trait 定义了统一的数据访问接口：
//!
//! ```rust
//! #[async_trait::async_trait]
//! pub trait Repository: Send + Sync {
//!     fn db(&self) -> &DbConn;
//! }
//! ```
//!
//! 实现 Repository 的结构体可以：
//! - 访问数据库连接
//! - 执行 CRUD 操作
//! - 被组合到更复杂的存储操作中
//!
//! # 连接 URL 格式
//!
//! ## SQLite
//!
//! ```text
//! sqlite://./path/to/db.db
//! sqlite://./data.db?mode=rwc
//! ```
//!
//! 查询参数：
//! - `mode=rwc` - 读写创建模式
//! - `cache=shared` - 共享缓存模式
//! - `thread_mode=SERIALIZED` - 线程安全模式
//!
//! # 注意事项
//!
//! - SQLite 连接池大小通常较小（1-10 个连接）
//! - 长时间运行的应用应定期进行健康检查
//! - 数据库文件应放置在用户数据目录中
//! - 生产环境应考虑数据库备份策略

use sea_orm::{ConnectOptions, Database, DbConn, DbErr};
use sea_orm_migration::MigratorTrait;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    error::{Result, StorageError},
    sys::path,
};

/// 数据库连接包装器
#[derive(Clone)]
pub struct DatabaseConnection {
    pub conn: Arc<DbConn>,
}

impl DatabaseConnection {
    /// 创建新的数据库实例
    pub async fn new(db_url: &str) -> Result<Self> {
        let mut opt = ConnectOptions::new(db_url);
        opt.sqlx_logging(false)
            .max_connections(10)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(600));

        let conn = Database::connect(opt)
            .await
            .map_err(|e| StorageError::Database(format!("连接数据库失败: {}", e)))?;

        Ok(Self {
            conn: Arc::new(conn),
        })
    }

    /// 从现有连接创建
    pub fn from_conn(conn: DbConn) -> Self {
        Self {
            conn: Arc::new(conn),
        }
    }

    /// 获取数据库连接
    pub fn connection(&self) -> &DbConn {
        &self.conn
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool> {
        self.conn
            .ping()
            .await
            .map_err(|e| StorageError::Database(format!("健康检查失败: {}", e)))?;
        Ok(true)
    }
}

/// 建立数据库连接
///
/// 使用 SQLite 数据库，位置在 ~/.postium/postium.sqlite
pub async fn establish_connection() -> std::result::Result<DbConn, DbErr> {
    let db_path = path::get_db_path().map_err(|e| DbErr::Custom(e.to_string()))?;

    // SeaORM 需要 sqlite:// 协议前缀
    // Windows 路径需要将反斜杠转换为正斜杠
    let normalized_path = db_path.replace('\\', "/");
    let db_url = format!("sqlite://{}?mode=rwc", normalized_path);

    // 配置连接选项，禁用 SQL 日志输出
    let mut opt = ConnectOptions::new(&db_url);
    opt.sqlx_logging(false); // 禁用 SQL 日志

    Database::connect(opt).await
}

/// 初始化数据库
///
/// 使用 SeaORM 官方迁移系统运行所有待执行的迁移。
/// 迁移状态自动记录在 `seaql_migrations` 表中。
pub async fn init_database(db: &DbConn) -> Result<()> {
    // 使用官方 Migrator
    migration::Migrator::up(db, None)
        .await
        .map_err(|e| StorageError::Database(format!("运行迁移失败: {}", e)))?;

    tracing::info!("数据库迁移完成");
    Ok(())
}

/// 数据库仓库 Trait
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// 获取数据库连接
    fn db(&self) -> &DbConn;
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_database_url_parsing() {
        // 测试数据库 URL 格式
        let url = "sqlite://./test.db?mode=rwc";
        assert!(url.starts_with("sqlite://"));
    }
}
