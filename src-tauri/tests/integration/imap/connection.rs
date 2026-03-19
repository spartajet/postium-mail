//! IMAP 连接和认证测试
//!
//! 测试与 GreenMail 服务器的连接和认证

use crate::integration::{GreenMailConfig, helpers};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_greenmail_config() {
        let config = GreenMailConfig::default();
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
        assert_eq!(config.imap_port, 3143);
    }

    #[tokio::test]
    async fn test_greenmail_config_from_env() {
        let config = GreenMailConfig::from_env();
        assert!(!config.host.is_empty());
        assert!(config.imap_port > 0);
    }

    #[tokio::test]
    #[ignore = "需要 GreenMail 服务器"]
    async fn test_greenmail_running() {
        let config = GreenMailConfig::default();
        let running = helpers::check_greenmail_running(&config).await;
        assert!(running, "GreenMail 服务器未运行");
    }

    #[tokio::test]
    #[ignore = "需要 GreenMail 服务器"]
    async fn test_greenmail_imap_addr() {
        let config = GreenMailConfig::default();
        let addr = config.imap_addr();
        assert!(addr.contains(':'));
    }
}
