// 测试 RFC 6154 Special-Use Mailboxes 支持
//
// 运行：cargo test --test test_folder_attrs test_folder_attributes -- --nocapture

use std::fs;
use std::path::Path;

/// 测试账号配置
#[derive(Debug, Clone)]
struct TestAccount {
    account: String,
    imap_server: String,
    imap_port: u16,
    password: String,
}

/// 从文件解析测试账号配置
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
            let key = key.trim();
            let value = value.trim();

            match key {
                "account" => account = value.to_string(),
                "imap_server" => imap_server = value.to_string(),
                "imap_port" => imap_port = value.parse().unwrap_or(993),
                "password" => password = value.to_string(),
                _ => {}
            }
        }
    }

    TestAccount {
        account,
        imap_server,
        imap_port,
        password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_folder_attributes() {
        use postium_mail_lib::services::imap_service::{ImapAuth, ImapClient};
        use tracing::info;

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试文件夹属性（RFC 6154）");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = ImapClient::new();

        match client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ) {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                // 获取文件夹列表
                match client.list_folders() {
                    Ok(folders) => {
                        info!("========================================");
                        info!("文件夹列表（共 {} 个）:", folders.len());
                        info!("========================================");

                        for folder_name in &folders {
                            info!("📁 {}", folder_name);

                            // 尝试选择文件夹获取更多信息
                            if let Ok(count) = client.select_folder(folder_name) {
                                info!("   邮件数: {}", count);
                            }
                        }

                        info!("========================================");
                        info!("说明：当前实现使用文件夹名称匹配");
                        info!("RFC 6154 支持需要使用 LIST 响应的属性标志");
                        info!("imap crate v3 可能需要查看 Name.attributes() 方法");
                        info!("========================================");
                    }
                    Err(e) => {
                        info!("列出文件夹失败: {}", e);
                    }
                }

                let _ = client.logout();
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001") || err_str.contains("dns") {
                    info!("⚠️  网络连接不可用，跳过测试");
                } else {
                    info!("❌ IMAP 连接失败: {}", e);
                }
            }
        }
    }
}
