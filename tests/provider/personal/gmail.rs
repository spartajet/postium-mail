//! Gmail 服务商测试
//!
//! 测试 Gmail 邮箱的连接和基本功能

use crate::provider::config::GmailConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "需要真实 Gmail 账号"]
    async fn test_gmail_config_load() {
        let config = GmailConfig::load();
        // 如果设置了环境变量，验证配置
        if let Some(cfg) = config {
            assert!(cfg.email.contains("@gmail.com"));
            assert!(!cfg.password.is_empty());
            assert_eq!(cfg.imap_host, "imap.gmail.com");
            assert_eq!(cfg.imap_port, 993);
            assert_eq!(cfg.smtp_host, "smtp.gmail.com");
            assert_eq!(cfg.smtp_port, 465);
        }
    }

    #[tokio::test]
    #[ignore = "需要真实 Gmail 账号"]
    async fn test_gmail_imap_connection() {
        let config = GmailConfig::load();
        if let Some(cfg) = config {
            // 这里可以添加实际的 IMAP 连接测试
            // 使用 async-imap 连接到 Gmail
            assert!(cfg.imap_port == 993); // IMAPS 端口
        }
    }
}
