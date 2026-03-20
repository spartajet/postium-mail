//! Outlook 服务商测试
//!
//! 测试 Outlook 邮箱的连接和基本功能

use crate::config::OutlookConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "需要真实 Outlook 账号"]
    async fn test_outlook_config_load() {
        let config = OutlookConfig::load();
        // 如果设置了环境变量，验证配置
        if let Some(cfg) = config {
            assert!(cfg.email.contains("@outlook"));
            assert!(!cfg.password.is_empty());
            assert_eq!(cfg.imap_host, "outlook.office365.com");
            assert_eq!(cfg.imap_port, 993);
        }
    }
}
