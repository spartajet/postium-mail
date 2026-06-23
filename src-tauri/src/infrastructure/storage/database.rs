use std::path::{Path, PathBuf};

use crate::error::MailError;

pub const SCHEMA_SQL: &str = include_str!("../../../sql/schema.sql");

#[derive(Clone)]
pub struct DbConn {
    inner: tokio_rusqlite::Connection,
}

impl DbConn {
    pub async fn open_reset(db_path: &Path) -> Result<Self, MailError> {
        reset_database_files(db_path)?;
        let conn = tokio_rusqlite::Connection::open(db_path).await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    pub async fn open_in_memory_for_test() -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open_in_memory().await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

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

    pub async fn call<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        Ok(self.inner.call(f).await?)
    }

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

    tracing::info!("重建数据库: {}", db_path.display());
    DbConn::open_reset(&db_path).await
}

fn reset_database_files(db_path: &Path) -> Result<(), MailError> {
    for path in database_files(db_path) {
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!("已删除旧数据库文件: {}", path.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(MailError::DatabaseError(err.to_string())),
        }
    }
    Ok(())
}

fn database_files(db_path: &Path) -> [PathBuf; 3] {
    [
        db_path.to_path_buf(),
        PathBuf::from(format!("{}-wal", db_path.display())),
        PathBuf::from(format!("{}-shm", db_path.display())),
    ]
}
