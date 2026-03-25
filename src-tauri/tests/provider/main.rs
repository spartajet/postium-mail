//! 服务商测试入口
//!
//! 运行方式：
//! ```bash
//! # 运行所有服务商测试
//! cargo test --test provider
//!
//! # 运行特定服务商测试（需要配置环境变量）
//! export GMAIL_EMAIL="test@gmail.com"
//! export GMAIL_APP_PASSWORD="app_password"
//! cargo test --test provider test_gmail --
//! ```

use std::env;

// 引入测试辅助模块
mod test_macros;
mod test_tracing;

// 引入服务商测试模块
mod common;
mod config;
mod email_parsing;
mod encoding;
mod enterprise;
mod folder_attrs;
mod folder_sync;
mod helpers;
mod personal;
mod smtp;
mod sync_features;
mod uid_tests;

/// 测试账号配置
#[derive(Debug, Clone)]
pub struct ProviderTestConfig {
    pub email: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

impl ProviderTestConfig {
    /// 创建新的测试配置
    pub fn new(
        email: String,
        password: String,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
    ) -> Self {
        Self {
            email,
            password,
            imap_host: imap_host.to_string(),
            imap_port,
            smtp_host: smtp_host.to_string(),
            smtp_port,
        }
    }

    /// 获取 IMAP 服务器地址
    pub fn imap_addr(&self) -> String {
        format!("{}:{}", self.imap_host, self.imap_port)
    }

    /// 获取 SMTP 服务器地址
    pub fn smtp_addr(&self) -> String {
        format!("{}:{}", self.smtp_host, self.smtp_port)
    }
}

/// Gmail 配置加载器
pub struct GmailConfig;

impl GmailConfig {
    /// 从环境变量加载 Gmail 配置
    pub fn load() -> Option<ProviderTestConfig> {
        let email = env::var("GMAIL_EMAIL").ok()?;
        let password = env::var("GMAIL_APP_PASSWORD").ok()?;
        Some(ProviderTestConfig::new(
            email,
            password,
            "imap.gmail.com",
            993,
            "smtp.gmail.com",
            465,
        ))
    }
}

/// Outlook 配置加载器
pub struct OutlookConfig;

impl OutlookConfig {
    /// 从环境变量加载 Outlook 配置
    pub fn load() -> Option<ProviderTestConfig> {
        let email = env::var("OUTLOOK_EMAIL").ok()?;
        let password = env::var("OUTLOOK_APP_PASSWORD").ok()?;
        Some(ProviderTestConfig::new(
            email,
            password,
            "outlook.office365.com",
            993,
            "smtp-mail.outlook.com",
            587,
        ))
    }
}

/// 163 邮箱配置加载器
pub struct Email163Config;

impl Email163Config {
    /// 从环境变量加载 163 邮箱配置
    pub fn load() -> Option<ProviderTestConfig> {
        let email = env::var("EMAIL_163_ADDR").ok()?;
        let password = env::var("EMAIL_163_PASS").ok()?;
        Some(ProviderTestConfig::new(
            email,
            password,
            "imap.163.com",
            993,
            "smtp.163.com",
            465,
        ))
    }
}

fn main() {
    // 初始化测试环境 tracing
    test_tracing::init_test_tracing();

    test_section!("Postium Mail 服务商测试");
    tracing::info!("");
    test_section!("需要配置环境变量");
    tracing::info!("");
    tracing::info!("个人邮箱:");
    tracing::info!("  GMAIL_EMAIL / GMAIL_APP_PASSWORD");
    tracing::info!("  OUTLOOK_EMAIL / OUTLOOK_APP_PASSWORD");
    tracing::info!("  EMAIL_163_ADDR / EMAIL_163_PASS");
    tracing::info!("  EMAIL_QQ_ADDR / EMAIL_QQ_PASS");
    tracing::info!("");
    tracing::info!("企业邮箱:");
    tracing::info!("  MICROSOFT_365_EMAIL / MICROSOFT_365_PASSWORD");
    tracing::info!("  GOOGLE_WORKSPACE_EMAIL / GOOGLE_WORKSPACE_PASSWORD");
    tracing::info!("");

    // 检查是否配置了环境变量
    let has_config = env::vars().any(|(k, _)| {
        k.contains("EMAIL")
            || k.contains("GMAIL")
            || k.contains("OUTLOOK")
            || k.contains("PASSWORD")
            || k.contains("PASS")
    });

    if has_config {
        test_success!("检测到环境变量配置");
    } else {
        test_warn!("未检测到环境变量配置");
        tracing::info!("  部分测试将被跳过");
    }
}

// ========== 测试模块 ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load_returns_none_when_not_set() {
        // 清除环境变量
        unsafe { env::remove_var("GMAIL_EMAIL") };
        unsafe { env::remove_var("GMAIL_APP_PASSWORD") };

        assert!(GmailConfig::load().is_none());
    }

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

    #[tokio::test]
    #[ignore = "需要真实 163 账号"]
    async fn test_163_config_load() {
        let config = Email163Config::load();
        // 如果设置了环境变量，验证配置
        if let Some(cfg) = config {
            assert!(cfg.email.contains("@163.com"));
            assert!(!cfg.password.is_empty());
            assert_eq!(cfg.imap_host, "imap.163.com");
            assert_eq!(cfg.imap_port, 993);
        }
    }

    #[tokio::test]
    #[ignore = "需要真实账号"]
    async fn test_provider_test_config_imap_addr() {
        let config = ProviderTestConfig::new(
            "test@example.com".to_string(),
            "password".to_string(),
            "imap.example.com",
            993,
            "smtp.example.com",
            587,
        );
        assert_eq!(config.imap_addr(), "imap.example.com:993");
        assert_eq!(config.smtp_addr(), "smtp.example.com:587");
    }
}
