use std::path::Path;

use crate::error::MailError;

pub const SCHEMA_SQL: &str = include_str!("../../../sql/schema.sql");

/// 异步 SQLite 数据库连接封装
///
/// 基于 `tokio_rusqlite`，将所有 SQL 操作调度到独立的专用线程执行，
/// 避免阻塞 async 运行时。内部启用外键约束、WAL 模式和忙等待超时。
/// 通过 `clone()` 可低成本地共享连接（底层是 `Arc`）。
#[derive(Clone)]
pub struct DbConn {
    inner: tokio_rusqlite::Connection,
}

impl DbConn {
    /// 打开或创建指定路径的数据库文件，并初始化表结构
    pub async fn open_or_create(db_path: &Path) -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open(db_path).await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// 创建内存数据库（仅用于测试）
    pub async fn open_in_memory_for_test() -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open_in_memory().await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// 初始化数据库 pragma 设置和表结构
    async fn initialize_schema(&self) -> Result<(), MailError> {
        self.call(|conn| {
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.pragma_update(None, "busy_timeout", 5000)?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.execute_batch(SCHEMA_SQL)?;
            Ok(())
        })
        .await
    }

    /// 在专用线程中执行单次数据库操作
    ///
    /// `f` 接收一个可变的 `rusqlite::Connection`，操作自动串行化。
    pub async fn call<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        Ok(self.inner.call(f).await?)
    }

    /// 在事务中执行数据库操作
    ///
    /// 操作成功时自动提交，失败时自动回滚。
    pub async fn transaction<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
        .await
    }
}

pub async fn init_database(data_dir: &Path) -> Result<DbConn, MailError> {
    let db_path = data_dir.join("postium.sqlite");
    std::fs::create_dir_all(data_dir).map_err(|err| MailError::DatabaseError(err.to_string()))?;

    tracing::info!("打开数据库: {}", db_path.display());
    DbConn::open_or_create(&db_path).await
}
