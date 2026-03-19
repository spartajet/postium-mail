//! 邮件服务商测试模块
//!
//! 使用真实账号进行服务商兼容性测试
//!
//! ## 测试账号配置
//!
//! 测试账号从环境变量读取：
//!
//! ```bash
//! # Gmail
//! export GMAIL_EMAIL="test@gmail.com"
//! export GMAIL_APP_PASSWORD="app_password"
//!
//! # Outlook
//! export OUTLOOK_EMAIL="test@outlook.com"
//! export OUTLOOK_APP_PASSWORD="app_password"
//!
//! # 163
//! export EMAIL_163_ADDR="test@163.com"
//! export EMAIL_163_PASS="password"
//! ```
//!
//! ## 运行方式
//!
//! ```bash
//! # 运行所有服务商测试
//! cargo test --test provider
//!
//! # 运行特定服务商测试
//! cargo test --test provider test_gmail --
//!
//! # 列出可用测试
//! cargo test --test provider -- --list
//! ```

pub mod config;
pub mod helpers;

// 服务商兼容性测试
pub mod smtp;           // SMTP 发送测试
pub mod folder_sync;    // 文件夹同步诊断
pub mod folder_attrs;   // 文件夹属性测试
pub mod rfc6154;        // RFC 6154 特殊文件夹测试
pub mod uid_tests;      // UID 相关测试
pub mod sync_features;  // 同步功能测试
pub mod email_parsing;  // 邮件解析测试

pub mod encoding;       // 编码问题测试
pub mod personal;       // 个人邮箱服务商（Gmail, Outlook, 163等）
pub mod enterprise;     // 企业邮箱服务商（Microsoft 365, Google Workspace）

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
