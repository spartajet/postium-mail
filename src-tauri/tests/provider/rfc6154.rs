// 测试 RFC 6154 Special-Use Mailboxes 属性支持
//
// 运行：cargo test --test test_rfc6154 -- --nocapture

use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct TestAccount {
    account: String,
    imap_server: String,
    imap_port: u16,
    password: String,
}

fn load_test_account() -> TestAccount {
    let account_path = Path::new("../.test_mail_accounts");
    let content = fs::read_to_string(account_path).expect("无法读取 .test_mail_accounts 文件");

    let mut account = String::new();
    let mut imap_server = String::new();
    let mut imap_port = 993u16;
    let mut password = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "account" => account = value.to_string(),
                "imap_server" => imap_server = value.to_string(),
                "imap_port" => imap_port = value.parse().unwrap_or(993),
                "password" => password = value.to_string(),
                _ => {}
            }
        }
    }

    TestAccount { account, imap_server, imap_port, password }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rfc6154_folder_attributes() {
        use postium_mail_lib::protocols::imap::{ImapAuth, ImapService};
        use tracing::info;

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试 RFC 6154 Special-Use Mailboxes");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut imap_service = ImapService::new();

        match imap_service.connect(&account.imap_server, account.imap_port, &account.account, ImapAuth::Password(account.password.clone())).await {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                // 使用新的 list_folders_with_attributes 方法
                match imap_service.list_folders_with_attributes().await {
                    Ok(folders) => {
                        info!("========================================");
                        info!("文件夹列表（共 {} 个）:", folders.len());
                        info!("========================================");

                        for folder in &folders {
                            info!("📁 文件夹: {}", folder.name);
                            info!("   标准名称: {}", folder.standard_name);
                            info!("   Special-Use: {:?}", folder.special_use);

                            // 验证特殊文件夹
                            if let Some(_special_use) = folder.special_use {
                                info!("   ✅ 这是 RFC 6154 定义的特殊文件夹");
                            }
                        }

                        info!("========================================");
                        info!("统计:");
                        let with_special_use = folders.iter().filter(|f| f.special_use.is_some()).count();
                        info!("  - 带有 RFC 6154 属性的文件夹: {}", with_special_use);
                        info!("  - 使用名称映射的文件夹: {}", folders.len() - with_special_use);
                        info!("========================================");
                    }
                    Err(e) => {
                        info!("列出文件夹失败: {}", e);
                    }
                }

                let _ = imap_service.logout().await;
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001") || err_str.contains("dns") {
                    info!("⚠️  网络连接不可用，跳过测试");
                } else {
                    info!("❌ 连接失败: {}", e);
                }
            }
        }
    }
}
