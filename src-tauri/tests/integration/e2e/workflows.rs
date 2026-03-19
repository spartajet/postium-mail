//! 端到端工作流测试
//!
//! 测试完整的同步和引擎工作流

use crate::integration::{GreenMailConfig, helpers};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_greenmail_end_to_end_config() {
        let config = GreenMailConfig::default();
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
        assert_eq!(config.imap_addr(), format!("{}:3143", config.host));
    }

    #[tokio::test]
    #[ignore = "需要 GreenMail 服务器"]
    async fn test_database_workflow() {
        let db = helpers::create_test_db().await;
        helpers::init_test_db(&db).await;
        // 成功创建并初始化数据库
        drop(db);
    }
}
