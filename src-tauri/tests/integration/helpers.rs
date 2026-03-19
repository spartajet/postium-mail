//! 集成测试辅助函数
//!
//! 提供数据库初始化、GreenMail 连接检查等测试辅助功能

use crate::integration::GreenMailConfig;
use std::time::Duration;
use tokio::net::TcpStream;

/// 创建测试用内存数据库
pub async fn create_test_db() -> sea_orm::DbConn {
    let db_url = "sqlite::memory:";

    sea_orm::Database::connect(db_url)
        .await
        .expect("Failed to connect to test database")
}

/// 初始化测试数据库表结构
pub async fn init_test_db(db: &sea_orm::DbConn) {
    postium_mail_lib::init_database(db)
        .await
        .expect("Failed to initialize test database");
}

/// 检查 GreenMail 是否运行
pub async fn check_greenmail_running(config: &GreenMailConfig) -> bool {
    TcpStream::connect(format!("{}:{}", config.host, config.imap_port))
        .await
        .is_ok()
}

/// 等待 GreenMail 就绪
pub async fn wait_for_greenmail(config: &GreenMailConfig, max_retries: u32) -> bool {
    for _ in 0..max_retries {
        if check_greenmail_running(config).await {
            return true;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    false
}

/// GreenMail 测试配置
impl Default for GreenMailConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_greenmail_config_default() {
        let config = GreenMailConfig::default();
        assert!(!config.host.is_empty());
        assert_eq!(config.imap_port, 3143);
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
    }

    #[tokio::test]
    async fn test_greenmail_config_imap_addr() {
        let config = GreenMailConfig {
            host: "localhost".to_string(),
            imap_port: 3143,
            smtp_port: 3025,
            username: "testuser",
            password: "testpass",
        };
        assert_eq!(config.imap_addr(), "localhost:3143");
        assert_eq!(config.smtp_addr(), "localhost:3025");
    }
}
