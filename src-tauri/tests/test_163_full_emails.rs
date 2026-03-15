// 测试：尝试不同方法获取 163 邮箱的所有邮件
//
// 运行：cargo test --test test_163_full_emails -- --nocapture

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
    use postium_mail_lib::services::imap::{AsyncImapClient, ImapAuth};
    use tracing::info;

    #[tokio::test]
    async fn test_163_full_emails() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试：尝试获取 163 邮箱的所有邮件");
        info!("账号: {}", account.account);
        info!("========================================");
        info!("");
        info!("说明：");
        info!("163 邮箱可能限制了 IMAP 访问的邮件数量。");
        info!("本测试尝试不同的方法来获取更多邮件。");
        info!("========================================");
        info!("");

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
                info!("");

                let folder = "INBOX";

                // 方法 1: 使用 list_uids 获取所有邮件
                info!("========================================");
                info!("方法 1: list_uids(limit=999999)");
                info!("========================================");
                match client.list_uids(folder, 999999).await {
                    Ok(uids) => {
                        info!("📊 返回 {} 封邮件", uids.len());
                        if !uids.is_empty() {
                            info!("   UID 范围: {} ~ {}", uids.last().unwrap_or(&0), uids.first().unwrap_or(&0));
                        }
                    }
                    Err(e) => {
                        info!("❌ 失败: {}", e);
                    }
                }

                // 方法 2: 使用 list_uids_since 获取很久以前的邮件
                info!("");
                info!("========================================");
                info!("方法 2: list_uids_since(很久以前)");
                info!("========================================");
                match client.list_uids_since(folder, "01-Jan-2000").await {
                    Ok(uids) => {
                        info!("📊 返回 {} 封邮件", uids.len());
                        if !uids.is_empty() {
                            info!("   UID 范围: {} ~ {}", uids.last().unwrap_or(&0), uids.first().unwrap_or(&0));
                        }
                    }
                    Err(e) => {
                        info!("❌ 失败: {}", e);
                    }
                }

                // 方法 3: 获取最早和最新的邮件日期
                info!("");
                info!("========================================");
                info!("方法 3: 获取邮件日期范围");
                info!("========================================");
                match client.list_uids(folder, 999999).await {
                    Ok(uids) => {
                        if uids.len() >= 2 {
                            let first_uid = uids[0];
                            let last_uid = uids[uids.len() - 1];

                            // 获取最新邮件
                            if let Ok(first_email) = client.fetch_email(folder, first_uid).await {
                                info!("最新邮件 (UID {}):", first_uid);
                                info!("  日期: {}", first_email.date.format("%Y-%m-%d %H:%M:%S"));
                                info!("  主题: {}", first_email.subject);
                            }

                            // 获取最旧邮件
                            if let Ok(last_email) = client.fetch_email(folder, last_uid).await {
                                info!("最旧邮件 (UID {}):", last_uid);
                                info!("  日期: {}", last_email.date.format("%Y-%m-%d %H:%M:%S"));
                                info!("  主题: {}", last_email.subject);
                            }

                            // 计算日期跨度
                            if let (Ok(first_email), Ok(last_email)) = (
                                client.fetch_email(folder, first_uid).await,
                                client.fetch_email(folder, last_uid).await,
                            ) {
                                let duration = first_email.date.signed_duration_since(last_email.date);
                                info!("日期跨度: {} 天", duration.num_days());
                            }
                        }
                    }
                    Err(e) => {
                        info!("❌ 失败: {}", e);
                    }
                }

                info!("");
                info!("========================================");
                info!("结论与建议");
                info!("========================================");
                info!("");
                info!("如果只能获取到 {} 封邮件，而网页邮箱有更多邮件：", 47);
                info!("");
                info!("1️⃣  检查 163 邮箱设置：");
                info!("   - 登录 mail.163.com");
                info!("   - 进入「设置」→「POP3/SMTP/IMAP」");
                info!("   - 查看 IMAP 是否开启了「完整同步」或类似选项");
                info!("");
                info!("2️⃣  163 免费邮箱限制：");
                info!("   - 免费账户可能只能通过 IMAP 访问最近的 50-100 封邮件");
                info!("   - 需要升级到 VIP 邮箱才能完整访问");
                info!("");
                info!("3️⃣  邮件激活机制：");
                info!("   - 某些邮箱只在邮件被「查看」后才会通过 IMAP 可见");
                info!("   - 尝试在网页邮箱中打开旧邮件，然后重新同步");
                info!("");
                info!("========================================");

                let _ = client.logout().await;
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
            }
        }
    }
}
