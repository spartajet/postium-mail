// 测试：检查原始邮件数据
//
// 运行：cargo test --test test_raw_email -- --nocapture

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

        if let Some((key_value, value)) = line.split_once('=') {
            let key = key_value.trim();
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
    use postium_mail_lib::protocols::imap::{AsyncImapClient, ImapAuth};
    use tracing::info;

    #[tokio::test]
    async fn test_raw_email_headers() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试：检查原始邮件头");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = AsyncImapClient::new();

        match client
            .connect(
                &account.imap_server,
                account.imap_port,
                &account.account,
                ImapAuth::Password(account.password.clone()),
            )
            .await
        {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                let folder = "INBOX";
                let target_uid = 2408;

                info!("========================================");
                info!("尝试获取原始邮件 UID: {}", target_uid);
                info!("========================================");

                match client.fetch_raw_header(folder, target_uid).await {
                    Ok(raw_header) => {
                        info!("✅ 成功获取邮件头!");
                        info!("========================================");
                        info!("原始邮件头:");
                        info!("========================================");
                        info!("{}", raw_header);

                        // 提取Subject字段
                        for line in raw_header.lines() {
                            if line.starts_with("Subject:") {
                                info!("========================================");
                                info!("Subject 字段原始值:");
                                info!("========================================");
                                info!("{}", line);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        info!("❌ 获取邮件头失败: {}", e);
                    }
                }
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
            }
        }
    }
}
