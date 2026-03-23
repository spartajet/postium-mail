//! 数据库迁移 CLI 工具

#[cfg(feature = "cli")]
mod cli {
    use clap::{Parser, Subcommand};
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use std::path::PathBuf;

    /// 数据库迁移工具
    #[derive(Parser)]
    #[command(name = "migration")]
    #[command(about = "Postium Mail 数据库迁移工具", long_about = None)]
    struct Cli {
        /// 数据库文件路径
        #[arg(short, long, default_value = "~/.postium/postium.sqlite")]
        database: String,

        #[command(subcommand)]
        command: Commands,
    }

    #[derive(Subcommand)]
    enum Commands {
        /// 应用所有待执行的迁移
        Up,
        /// 回滚最后一次迁移
        Down,
        /// 回滚所有迁移
        Reset,
        /// 刷新数据库（回滚后重新应用）
        Refresh,
        /// 显示迁移状态
        Status,
    }

    pub async fn run() -> anyhow::Result<()> {
        let cli = Cli::parse();

        // 展开路径中的 ~/
        let db_path = shellexpand::tilde(&cli.database);
        let normalized_path = db_path.replace('\\', "/");
        let db_url = format!("sqlite://{}?mode=rwc", normalized_path);

        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();

        let db = Database::connect(&db_url).await?;

        match cli.command {
            Commands::Up => {
                println!("应用迁移...");
                migration::Migrator::up(&db, None).await?;
                println!("迁移完成！");
            }
            Commands::Down => {
                println!("回滚最后一次迁移...");
                migration::Migrator::down(&db, None).await?;
                println!("回滚完成！");
            }
            Commands::Reset => {
                println!("回滚所有迁移...");
                migration::Migrator::reset(&db).await?;
                println!("重置完成！");
            }
            Commands::Refresh => {
                println!("刷新数据库...");
                migration::Migrator::refresh(&db).await?;
                println!("刷新完成！");
            }
            Commands::Status => {
                println!("检查迁移状态...");
                let applied = migration::Migrator::get_applied_migrations(&db).await?;
                println!("已应用的迁移数量: {}", applied.len());
                for m in applied {
                    println!("  - {}", m.name);
                }
            }
        }

        Ok(())
    }
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}

#[cfg(not(feature = "cli"))]
fn main() {
    println!("CLI 功能未启用。请使用 --features cli 编译。");
}
