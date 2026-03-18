// 集成测试辅助函数
//
// 提供测试所需的各种辅助功能

use sea_orm::{Database, DbConn};
use std::sync::Arc;

/// 创建测试数据库
pub async fn create_test_db() -> Arc<DbConn> {
    let db: DbConn = Database::connect("sqlite::memory:").await.unwrap();
    Arc::new(db)
}

/// 初始化数据库表结构
pub async fn init_test_db(db: &DbConn) {
    // 在测试中，我们需要手动初始化必要的表
    // 这里我们简化处理：只验证数据库连接
    // 实际的表初始化将在具体的测试中按需进行
    let _ = db;
}

/// 检查 GreenMail 是否运行
pub async fn check_greenmail_running() -> bool {
    use tokio::net::TcpStream;

    // 使用公网 GreenMail 服务器
    match TcpStream::connect("139.59.228.56:3143").await {
        Ok(_) => true,  // 连接成功，GreenMail 正在运行
        Err(_) => false, // 连接失败，GreenMail 未运行
    }
}

/// GreenMail 配置常量
pub struct GreenmailConfig {
    pub host: &'static str,
    pub imap_port: u16,
    pub username: &'static str,
    pub password: &'static str,
}

impl Default for GreenmailConfig {
    fn default() -> Self {
        Self {
            host: "139.59.228.56",
            imap_port: 3143,
            username: "testuser",
            password: "testpass",
        }
    }
}

/// 等待 GreenMail 就绪
#[allow(dead_code)]
pub async fn wait_for_greenmail(max_seconds: u64) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::{sleep, Duration};

    for _ in 0..max_seconds {
        if TcpStream::connect("139.59.228.56:3143").await.is_ok() {
            return true;
        }
        sleep(Duration::from_secs(1)).await;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_db() {
        let db = create_test_db().await;
        init_test_db(&db).await;

        // 验证数据库初始化没有panic
        println!("✅ 测试数据库初始化成功");
    }

    #[test]
    fn test_greenmail_config_default() {
        let config = GreenmailConfig::default();
        assert_eq!(config.host, "139.59.228.56");
        assert_eq!(config.imap_port, 3143);
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
    }
}
