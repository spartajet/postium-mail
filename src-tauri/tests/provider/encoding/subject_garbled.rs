// 测试：诊断邮件标题乱码问题
//
// 运行：cargo test --test test_garbled_subject -- --nocapture

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
    async fn test_garbled_subject_diagnosis() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试：诊断邮件标题乱码问题");
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

                // 获取最近几封邮件
                match client.list_uids(folder, 10).await {
                    Ok(uids) => {
                        info!("📊 获取到 {} 封邮件", uids.len());

                        // 检查用户提到的特定 UID 2408
                        let target_uid = 2408;
                        let uids_vec: Vec<u32> = uids.to_vec();

                        if uids_vec.contains(&target_uid) {
                            info!("========================================");
                            info!("检查用户提到的邮件 UID: {}", target_uid);
                            info!("========================================");

                            match client.fetch_email(folder, target_uid).await {
                                Ok(email) => {
                                    info!("主题: {}", email.subject);
                                    info!("发件人: {}", email.from);

                                    // 检测乱码
                                    let has_replacement_chars = email.subject.contains('�');
                                    let has_invalid_utf8 =
                                        email.subject.bytes().any(|b| b == 0xFF || b == 0xFE);

                                    if has_replacement_chars || has_invalid_utf8 {
                                        info!("⚠️  仍然检测到乱码!");
                                        info!("   包含替换字符: {}", has_replacement_chars);
                                        info!("   包含无效字节: {}", has_invalid_utf8);
                                    } else {
                                        info!("✅ 主题显示正常!");
                                    }
                                }
                                Err(e) => {
                                    info!("❌ 获取邮件失败: {}", e);
                                }
                            }
                        } else {
                            info!("⚠️  UID {} 不在最近的邮件中", target_uid);
                        }

                        for uid in uids.iter().take(5) {
                            info!("========================================");
                            info!("检查邮件 UID: {}", uid);
                            info!("========================================");

                            match client.fetch_email(folder, *uid).await {
                                Ok(email) => {
                                    info!("主题: {}", email.subject);
                                    info!("发件人: {}", email.from);

                                    // 检测乱码
                                    let has_replacement_chars = email.subject.contains('�');
                                    let has_invalid_utf8 =
                                        email.subject.bytes().any(|b| b == 0xFF || b == 0xFE);

                                    if has_replacement_chars || has_invalid_utf8 {
                                        info!("⚠️  检测到可能的乱码!");
                                        info!("   包含替换字符: {}", has_replacement_chars);
                                        info!("   包含无效字节: {}", has_invalid_utf8);

                                        // 打印原始字节
                                        info!("   主题字节: {:?}", email.subject.as_bytes());
                                    }
                                }
                                Err(e) => {
                                    info!("❌ 获取邮件失败: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        info!("❌ 获取邮件列表失败: {}", e);
                    }
                }
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
            }
        }
    }
}
