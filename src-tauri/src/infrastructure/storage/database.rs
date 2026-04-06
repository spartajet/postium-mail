use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::time::Duration;

pub type DbConn = DatabaseConnection;

/// 初始化数据库连接并运行迁移
pub async fn init_database(data_dir: &std::path::Path) -> Result<DbConn, sea_orm::DbErr> {
    let db_path = data_dir.join("postium.sqlite");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    tracing::info!("连接数据库: {}", db_path.display());

    let mut opt = ConnectOptions::new(&db_url);
    opt.max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;
    tracing::debug!("数据库连接成功");

    // 运行迁移
    postium_mail_migration::Migrator::up(&db, None).await?;
    tracing::info!("数据库迁移完成");

    Ok(db)
}
