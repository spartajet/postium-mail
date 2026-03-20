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
    crate::storage::migration::m001_20250314_init::initialize(db).await?;

    // 然后运行迁移（为现有数据库添加新字段）
    crate::storage::migration::m002_20250314_add_oauth_fields::run_migrations(db).await?;

    // 添加同步相关表
    crate::storage::migration::m003_20250315_add_sync_tables::add_sync_tables(db).await?;

    // 删除敏感字段（密码和 token 迁移到 Stronghold）
    crate::storage::migration::m004_20250315_remove_sensitive_fields::run_migrations(db).await?;

    // 添加 IMAP 元数据字段
    crate::storage::migration::m005_20250315_add_imap_metadata::add_imap_metadata(db).await?;

    // 添加离线操作和同步元数据表
    crate::storage::migration::m006_20250315_add_sync_operations::add_sync_operations(db).await?;

    // 添加账号类型支持（个人/企业）
    crate::storage::migration::m007_20250317_add_account_types::migrate(db).await?;

    // 添加 MODSEQ 支持（CONDSTORE 扩展）
    crate::storage::migration::m008_20250317_add_modseq_support::migrate(db).await?;

    // 创建 folder_sync_states 表（分离同步状态）
    crate::storage::migration::m009_20250321_create_folder_sync_states::migrate(db).await?;

    Ok(())
}
