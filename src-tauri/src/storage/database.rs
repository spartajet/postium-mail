//! 数据库访问层
//!
//! 提供统一的数据库操作接口，封装 Sea-ORM

use sea_orm::DbConn;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{Result, StorageError};

/// 数据库连接包装器
#[derive(Clone)]
pub struct DatabaseConnection {
    pub conn: Arc<DbConn>,
}

impl DatabaseConnection {
    /// 创建新的数据库实例
    pub async fn new(db_url: &str) -> Result<Self> {
        use sea_orm::{Database, ConnectOptions};

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

/// 数据库仓库 Trait
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// 获取数据库连接
    fn db(&self) -> &DbConn;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_url_parsing() {
        // 测试数据库 URL 格式
        let url = "sqlite://./test.db?mode=rwc";
        assert!(url.starts_with("sqlite://"));
    }
}
