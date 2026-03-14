use crate::config;
use anyhow::Result;
use sea_orm::{Database, DbConn, DbErr, ConnectOptions};

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

    // 配置连接选项，禁用 SQL 日志输出
    let mut opt = ConnectOptions::new(&db_url);
    opt.sqlx_logging(false); // 禁用 SQL 日志

    Database::connect(opt).await
}

/// 初始化数据库
/// 运行所有迁移脚本
pub async fn init_database(db: &DbConn) -> Result<()> {
    // 先运行初始化脚本
    crate::migration::m001_20250314_init::initialize(db).await?;

    // 然后运行迁移（为现有数据库添加新字段）
    crate::migration::m002_20250314_add_oauth_fields::run_migrations(db).await?;

    Ok(())
}
