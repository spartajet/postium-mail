//! 测试账号配置
//!
//! 从环境变量读取真实账号配置

use crate::ProviderTestConfig;
use std::env;

/// Gmail 配置加载器
pub struct GmailConfig;

impl GmailConfig {
    /// 从环境变量加载 Gmail 配置
    ///
    /// 环境变量：
    /// - `GMAIL_EMAIL`: Gmail 邮箱地址
    /// - `GMAIL_APP_PASSWORD`: Gmail 应用专用密码
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
    ///
    /// 环境变量：
    /// - `OUTLOOK_EMAIL`: Outlook 邮箱地址
    /// - `OUTLOOK_APP_PASSWORD`: Outlook 应用密码
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
    ///
    /// 环境变量：
    /// - `EMAIL_163_ADDR`: 163 邮箱地址
    /// - `EMAIL_163_PASS`: 163 邮箱密码
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

/// QQ 邮箱配置加载器
pub struct EmailQQConfig;

impl EmailQQConfig {
    /// 从环境变量加载 QQ 邮箱配置
    ///
    /// 环境变量：
    /// - `EMAIL_QQ_ADDR`: QQ 邮箱地址
    /// - `EMAIL_QQ_PASS`: QQ 邮箱密码
    pub fn load() -> Option<ProviderTestConfig> {
        let email = env::var("EMAIL_QQ_ADDR").ok()?;
        let password = env::var("EMAIL_QQ_PASS").ok()?;
        Some(ProviderTestConfig::new(
            email,
            password,
            "imap.qq.com",
            993,
            "smtp.qq.com",
            465,
        ))
    }
}

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
}
