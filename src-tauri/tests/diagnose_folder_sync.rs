// 诊断测试：检查 &XfJT0ZAB- 文件夹同步问题
//
// 运行：cargo test --test diagnose_folder_sync -- --nocapture

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
    async fn diagnose_sent_folder_sync() {
        use postium_mail_lib::services::imap::{ImapAuth, ImapClient};
        use tracing::info;

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        let account = load_test_account();

        info!("========================================");
        info!("诊断 &XfJT0ZAB- 文件夹同步问题");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = ImapClient::new();

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

                // 1. 获取所有文件夹
                match client.list_folders().await {
                    Ok(folders) => {
                        info!("========================================");
                        info!("步骤 1: 列出所有文件夹");
                        info!("========================================");

                        for folder in &folders {
                            if folder.contains("&XfJT0ZAB-")
                                || folder.contains("已发送")
                                || folder.contains("Sent")
                            {
                                info!("📁 找到已发送文件夹: {}", folder);
                            }
                        }

                        // 2. 选择 &XfJT0ZAB- 文件夹并获取 UID
                        info!("========================================");
                        info!("步骤 2: 选择 &XfJT0ZAB- 文件夹");
                        info!("========================================");

                        match client.select_folder("&XfJT0ZAB-").await {
                            Ok(count) => {
                                info!("✅ 成功选择 &XfJT0ZAB-, 邮件数量: {}", count);

                                // 获取 UID 列表
                                let uids =
                                    client.list_uids("&XfJT0ZAB-", 10).await.unwrap_or_default();
                                info!("✅ 获取到 {} 封邮件的 UID", uids.len());
                                info!("   UID 列表: {:?}", uids);

                                // 3. 尝试获取第一封邮件
                                if let Some(&uid) = uids.first() {
                                    info!("========================================");
                                    info!("步骤 3: 获取第一封邮件 (UID: {})", uid);
                                    info!("========================================");

                                    match client.fetch_email("&XfJT0ZAB-", uid).await {
                                        Ok(email) => {
                                            info!("✅ 成功获取邮件:");
                                            info!("   主题: {}", email.subject);
                                            info!("   发件人: {}", email.from);
                                            info!("   UID: {}", email.uid);

                                            // 关键诊断——检查 folder 参数
                                            info!("========================================");
                                            info!("关键诊断：folder 参数使用");
                                            info!("========================================");
                                            info!("当前实现:");
                                            info!("  - email_exists_by_uid 使用 folder: 'sent'");
                                            info!("  - save_email 使用 folder: 'sent'");
                                            info!("  - IMAP 请求使用 imap_folder: '&XfJT0ZAB-'");
                                            info!("");
                                            info!("问题分析:");
                                            info!("  如果数据库中已有相同 UID 的邮件，");
                                            info!("  但 folder 字段不是 'sent'，则检查会失败！");
                                            info!("");
                                            info!("  或者：邮件已经存在，folder='sent'，被跳过");
                                            info!("");
                                            info!("需要检查：");
                                            info!("  1. 数据库中是否已存在这些 UID 的邮件");
                                            info!("  2. 如果存在，folder 字段的值是什么");
                                        }
                                        Err(e) => {
                                            info!("❌ 获取邮件失败: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("❌ 选择 &XfJT0ZAB- 失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        info!("❌ 列出文件夹失败: {}", e);
                    }
                }

                let _ = client.logout().await;
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
            }
        }
    }
}
