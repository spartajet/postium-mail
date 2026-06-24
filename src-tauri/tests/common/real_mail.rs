use postium_mail_lib::domain::providers::{ImapServerConfig, SslMode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct TruthConfigFile {
    accounts: BTreeMap<String, TruthAccountFile>,
}

#[derive(Debug, Deserialize)]
struct TruthAccountFile {
    email: String,
    password: String,
    imap: TruthServerConfig,
    smtp: TruthServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TruthServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: bool,
}

#[derive(Debug, Clone)]
pub struct TruthAccount {
    pub key: String,
    pub email: String,
    pub password: String,
    pub imap: TruthServerConfig,
}

pub fn load_truth_accounts() -> Vec<TruthAccount> {
    assert_truth_enabled();
    let path = truth_config_path();
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("读取 truth 配置失败 {}: {err}", path.display()));
    let parsed: TruthConfigFile = serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("解析 truth 配置失败 {}: {err}", path.display()));

    let accounts: Vec<TruthAccount> = parsed
        .accounts
        .into_iter()
        .map(|(key, account)| {
            validate_account(&key, &account);
            TruthAccount {
                key,
                email: account.email,
                password: account.password,
                imap: account.imap,
            }
        })
        .collect();

    assert!(
        !accounts.is_empty(),
        "truth 配置必须包含至少一个 accounts 条目"
    );
    accounts
}

pub fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_string();
    };
    let first = local.chars().next().unwrap_or('*');
    format!("{first}***@{domain}")
}

pub fn imap_server_config(account: &TruthAccount) -> ImapServerConfig {
    ImapServerConfig {
        host: account.imap.host.clone(),
        port: account.imap.port,
        ssl: ssl_mode_from_bool(account.imap.ssl),
    }
}

fn assert_truth_enabled() {
    assert_eq!(
        std::env::var("POSTIUM_REAL_MAIL").ok().as_deref(),
        Some("1"),
        "truth 测试必须通过 POSTIUM_REAL_MAIL=1 显式启用"
    );
}

fn truth_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("POSTIUM_REAL_MAIL_CONFIG") {
        return PathBuf::from(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri 应有仓库根目录")
        .join(".test_mail_accounts.json")
}

fn ssl_mode_from_bool(enabled: bool) -> SslMode {
    if enabled {
        SslMode::Implicit
    } else {
        SslMode::None
    }
}

fn validate_account(key: &str, account: &TruthAccountFile) {
    assert!(!key.trim().is_empty(), "truth account key 不能为空");
    assert!(
        !account.email.trim().is_empty(),
        "truth account {key} 缺少 email"
    );
    assert!(
        !account.password.is_empty(),
        "truth account {key} 缺少 password"
    );
    validate_server(key, "imap", &account.imap);
    validate_server(key, "smtp", &account.smtp);
}

fn validate_server(key: &str, kind: &str, server: &TruthServerConfig) {
    assert!(
        !server.host.trim().is_empty(),
        "truth account {key} 缺少 {kind}.host"
    );
    assert!(server.port > 0, "truth account {key} 的 {kind}.port 无效");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_email_should_keep_domain_and_hide_local_part() {
        assert_eq!(mask_email("tester@example.com"), "t***@example.com");
    }
}
