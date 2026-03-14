use crate::config;
use anyhow::Result;
use sea_orm::{Database, DbConn, DbErr};

/// 数据库连接池类型
pub type DatabaseResult = Result<DbConn, DbErr>;

/// 建立数据库连接
/// 使用 SQLite 数据库，位置在 ~/.postium/postium.sqlite
pub async fn establish_connection() -> DatabaseResult {
    let db_path = config::get_db_path().map_err(|e| DbErr::Custom(e.to_string()))?;

    // SeaORM 需要 sqlite:// 协议前缀
    // Windows 路径需要将反斜杠转换为正斜杠
    let normalized_path = db_path.replace('\\', "/");
    let db_url = format!("sqlite://{}?mode=rwc", normalized_path);

    Database::connect(&db_url).await
}

/// 初始化数据库
/// 运行 migration/init.rs 中的初始化脚本
pub async fn init_database(db: &DbConn) -> Result<()> {
    crate::migration::init::initialize(db).await
}
