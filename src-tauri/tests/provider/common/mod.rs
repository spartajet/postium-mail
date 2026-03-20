//! 测试账号加载模块
//!
//! 支持从 JSON 文件加载多个邮件服务商的测试账号配置
//!
//! ## 配置文件格式
//!
//! 配置文件位于项目根目录的 `.test_mail_accounts.json`，格式如下：
//!
//! ```json
//! {
//!   "accounts": {
//!     "163": {
//!       "email": "test@163.com",
//!       "password": "password",
//!       "imap": {
//!         "host": "imap.163.com",
//!         "port": 993,
//!         "ssl": true
//!       },
//!       "smtp": {
//!         "host": "smtp.163.com",
//!         "port": 465,
//!         "ssl": true
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! ## 使用方式
//!
//! ```rust
//! use tests::test_accounts::load_test_account;
//!
//! // 加载 163 邮箱配置
//! if let Some(account) = load_test_account("163") {
//!     println!("Email: {}", account.email);
//!     println!("IMAP: {}:{}", account.imap.host, account.imap.port);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// IMAP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub ssl: bool,
}

/// SMTP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub ssl: bool,
}

/// 测试账号配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAccount {
    pub email: String,
    pub password: String,
    pub imap: ImapConfig,
    pub smtp: SmtpConfig,
}

impl TestAccount {
    /// 获取 IMAP 服务器地址
    pub fn imap_addr(&self) -> String {
        format!("{}:{}", self.imap.host, self.imap.port)
    }

    /// 获取 SMTP 服务器地址
    pub fn smtp_addr(&self) -> String {
        format!("{}:{}", self.smtp.host, self.smtp.port)
    }
}

/// 整个配置文件的结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAccountsConfig {
    pub accounts: HashMap<String, TestAccount>,
}

/// 全局配置缓存
static CONFIG: OnceLock<TestAccountsConfig> = OnceLock::new();

/// 加载配置文件
///
/// 从项目根目录的 `.test_mail_accounts.json` 文件加载配置
/// 配置会被缓存，只会加载一次
fn load_config() -> &'static TestAccountsConfig {
    CONFIG.get_or_init(|| {
        // 尝试多个可能的配置文件路径
        let config_paths = vec![
            PathBuf::from(".test_mail_accounts.json"),
            PathBuf::from("../.test_mail_accounts.json"),
            PathBuf::from("../../.test_mail_accounts.json"),
        ];

        for config_path in config_paths {
            if config_path.exists() {
                match fs::read_to_string(&config_path) {
                    Ok(content) => match serde_json::from_str::<TestAccountsConfig>(&content) {
                        Ok(config) => {
                            eprintln!(
                                "[test_accounts] ✓ 成功从 {:?} 加载配置，共 {} 个账号",
                                config_path,
                                config.accounts.len()
                            );
                            return config;
                        }
                        Err(e) => {
                            eprintln!(
                                "[test_accounts] ✗ 配置文件 {:?} 格式错误: {}",
                                config_path, e
                            );
                        }
                    },
                    Err(e) => {
                        eprintln!(
                            "[test_accounts] ✗ 无法读取配置文件 {:?}: {}",
                            config_path, e
                        );
                    }
                }
            }
        }

        eprintln!("[test_accounts] ⚠ 未找到配置文件，将尝试从环境变量加载");
        TestAccountsConfig {
            accounts: HashMap::new(),
        }
    })
}

/// 从环境变量加载测试账号
///
/// 支持的环境变量：
/// - Gmail: GMAIL_EMAIL, GMAIL_APP_PASSWORD
/// - Outlook: OUTLOOK_EMAIL, OUTLOOK_APP_PASSWORD
/// - 163: EMAIL_163_ADDR, EMAIL_163_PASS
/// - QQ: EMAIL_QQ_ADDR, EMAIL_QQ_PASS
fn load_from_env(key: &str) -> Option<TestAccount> {
    match key {
        "163" => {
            let email = env::var("EMAIL_163_ADDR").ok()?;
            let password = env::var("EMAIL_163_PASS").ok()?;
            Some(TestAccount {
                email,
                password,
                imap: ImapConfig {
                    host: "imap.163.com".to_string(),
                    port: 993,
                    ssl: true,
                },
                smtp: SmtpConfig {
                    host: "smtp.163.com".to_string(),
                    port: 465,
                    ssl: true,
                },
            })
        }
        "qq" => {
            let email = env::var("EMAIL_QQ_ADDR").ok()?;
            let password = env::var("EMAIL_QQ_PASS").ok()?;
            Some(TestAccount {
                email,
                password,
                imap: ImapConfig {
                    host: "imap.qq.com".to_string(),
                    port: 993,
                    ssl: true,
                },
                smtp: SmtpConfig {
                    host: "smtp.qq.com".to_string(),
                    port: 465,
                    ssl: true,
                },
            })
        }
        "gmail" => {
            let email = env::var("GMAIL_EMAIL").ok()?;
            let password = env::var("GMAIL_APP_PASSWORD").ok()?;
            Some(TestAccount {
                email,
                password,
                imap: ImapConfig {
                    host: "imap.gmail.com".to_string(),
                    port: 993,
                    ssl: true,
                },
                smtp: SmtpConfig {
                    host: "smtp.gmail.com".to_string(),
                    port: 465,
                    ssl: true,
                },
            })
        }
        "outlook" => {
            let email = env::var("OUTLOOK_EMAIL").ok()?;
            let password = env::var("OUTLOOK_APP_PASSWORD").ok()?;
            Some(TestAccount {
                email,
                password,
                imap: ImapConfig {
                    host: "outlook.office365.com".to_string(),
                    port: 993,
                    ssl: true,
                },
                smtp: SmtpConfig {
                    host: "smtp-mail.outlook.com".to_string(),
                    port: 587,
                    ssl: true,
                },
            })
        }
        _ => None,
    }
}

/// 加载测试账号配置
///
/// 优先从 JSON 配置文件加载，如果失败则尝试从环境变量加载
///
/// # 参数
///
/// * `key` - 账号标识，如 "163", "qq", "gmail", "outlook" 等
///
/// # 返回
///
/// 返回对应的测试账号配置，如果找不到则返回 None
///
/// # 示例
///
/// ```rust
/// use tests::test_accounts::load_test_account;
///
/// if let Some(account) = load_test_account("163") {
///     println!("Email: {}", account.email);
///     println!("IMAP: {}", account.imap_addr());
///     println!("SMTP: {}", account.smtp_addr());
/// }
/// ```
pub fn load_test_account(key: &str) -> Option<TestAccount> {
    // 首先尝试从配置文件加载
    let config = load_config();

    if let Some(account) = config.accounts.get(key) {
        eprintln!("[test_accounts] ✓ 从配置文件加载账号: {}", key);
        return Some(account.clone());
    }

    // 如果配置文件中没有，尝试从环境变量加载
    if let Some(account) = load_from_env(key) {
        eprintln!("[test_accounts] ✓ 从环境变量加载账号: {}", key);
        return Some(account);
    }

    eprintln!(
        "[test_accounts] ✗ 未找到账号配置: {} (请检查配置文件或环境变量)",
        key
    );
    None
}

/// 列出所有可用的测试账号
///
/// 返回所有已配置的账号标识列表
pub fn list_available_accounts() -> Vec<String> {
    let config = load_config();
    let mut accounts: Vec<String> = config.accounts.keys().cloned().collect();

    // 添加环境变量支持的账号
    let env_keys = vec!["163", "qq", "gmail", "outlook"];
    for key in env_keys {
        if !accounts.contains(&key.to_string()) && load_from_env(key).is_some() {
            accounts.push(key.to_string());
        }
    }

    accounts.sort();
    accounts
}

/// 检查指定账号是否可用
///
/// # 参数
///
/// * `key` - 账号标识
///
/// # 返回
///
/// 如果账号可用返回 true，否则返回 false
pub fn is_account_available(key: &str) -> bool {
    load_test_account(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let config = load_config();
        // 配置文件可能不存在，所以我们只检查不会 panic
        println!("Config loaded: {:?}", config);
    }

    #[test]
    fn test_list_available_accounts() {
        let accounts = list_available_accounts();
        println!("Available accounts: {:?}", accounts);
        // 至少应该能列出环境变量支持的账号（如果设置了的话）
    }

    #[test]
    fn test_is_account_available() {
        // 测试不存在的账号
        assert!(!is_account_available("non_existent_account"));
    }

    #[test]
    fn test_load_test_account_non_existent() {
        let account = load_test_account("non_existent_account");
        assert!(account.is_none());
    }

    #[test]
    fn test_imap_addr() {
        let account = TestAccount {
            email: "test@example.com".to_string(),
            password: "password".to_string(),
            imap: ImapConfig {
                host: "imap.example.com".to_string(),
                port: 993,
                ssl: true,
            },
            smtp: SmtpConfig {
                host: "smtp.example.com".to_string(),
                port: 465,
                ssl: true,
            },
        };

        assert_eq!(account.imap_addr(), "imap.example.com:993");
        assert_eq!(account.smtp_addr(), "smtp.example.com:465");
    }
}
