//! 原生邮箱服务商测试
//!
//! 测试 163、QQ、iCloud 等原生邮箱服务商

use crate::provider::config::{Email163Config, EmailQQConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "需要真实账号"]
    async fn test_163_config_load() {
        let config = Email163Config::load();
        // 如果设置了环境变量，验证配置
        if let Some(cfg) = config {
            assert!(!cfg.email.is_empty());
            assert!(!cfg.password.is_empty());
            assert_eq!(cfg.imap_host, "imap.163.com");
            assert_eq!(cfg.imap_port, 993);
        }
    }

    #[tokio::test]
    #[ignore = "需要真实账号"]
    async fn test_qq_config_load() {
        let config = EmailQQConfig::load();
        // 如果设置了环境变量，验证配置
        if let Some(cfg) = config {
            assert!(!cfg.email.is_empty());
            assert!(!cfg.password.is_empty());
            assert_eq!(cfg.imap_host, "imap.qq.com");
            assert_eq!(cfg.imap_port, 993);
        }
    }
}
