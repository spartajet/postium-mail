// 测试：检查 163 IMAP 服务器是否限制返回的邮件数量
//
// 运行：cargo test --test test_163_imap_limit -- --nocapture

use chrono::Datelike;
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
    async fn test_163_imap_limit() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试 163 IMAP 邮件数量限制");
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

                info!("========================================");
                info!("测试 1: SEARCH ALL 命令");
                info!("========================================");

                // 获取 session
                let session = client.get_session_mut();

                // SELECT 文件夹
                match session.select(folder).await {
                    Ok(mailbox) => {
                        info!("✅ 成功选择文件夹");
                        info!("   邮箱存在性: {}", mailbox.exists);
                        info!("   最近邮件数: {}", mailbox.recent);
                        info!("   未读邮件数: {}", mailbox.unseen);
                        info!("   第一个未读 UID: {:?}", mailbox.uid_next);
                        info!("   UID 有效性: {:?}", mailbox.uid_validity);
                        info!("   标志: {:?}", mailbox.flags);

                        // 尝试不同的搜索命令
                        info!("========================================");
                        info!("测试 2: 不同的搜索命令");
                        info!("========================================");

                        // 测试 1: SEARCH ALL
                        match session.search("ALL").await {
                            Ok(uids) => {
                                info!("📊 SEARCH ALL: {} 封邮件", uids.len());
                                if !uids.is_empty() {
                                    let min_uid = uids.iter().min().unwrap();
                                    let max_uid = uids.iter().max().unwrap();
                                    info!("   UID 范围: {} ~ {}", min_uid, max_uid);
                                }
                            }
                            Err(e) => {
                                info!("❌ SEARCH ALL 失败: {}", e);
                            }
                        }

                        // 测试 2: SEARCH UID 1:*
                        match session.search("UID 1:*").await {
                            Ok(uids) => {
                                info!("📊 SEARCH UID 1:*: {} 封邮件", uids.len());
                            }
                            Err(e) => {
                                info!("❌ SEARCH UID 1:* 失败: {}", e);
                            }
                        }

                        // 测试 3: SEARCH ANSWERED
                        match session.search("ANSWERED").await {
                            Ok(uids) => {
                                info!("📊 SEARCH ANSWERED: {} 封已回复邮件", uids.len());
                            }
                            Err(e) => {
                                info!("❌ SEARCH ANSWERED 失败: {}", e);
                            }
                        }

                        // 测试 4: 使用不同的 SINCE 日期
                        info!("========================================");
                        info!("测试 3: SINCE 命令边界测试");
                        info!("========================================");

                        // 测试很早的日期（应该返回所有邮件）
                        match session.search("SINCE 01-Jan-2020").await {
                            Ok(uids) => {
                                info!("📊 SINCE 01-Jan-2020: {} 封邮件", uids.len());
                            }
                            Err(e) => {
                                info!("❌ SINCE 01-Jan-2020 失败: {}", e);
                            }
                        }

                        // 测试未来的日期（应该返回 0 封邮件）
                        match session.search("SINCE 01-Jan-2030").await {
                            Ok(uids) => {
                                info!("📊 SINCE 01-Jan-2030 (未来): {} 封邮件", uids.len());
                            }
                            Err(e) => {
                                info!("❌ SINCE 01-Jan-2030 失败: {}", e);
                            }
                        }

                        // 测试很早的日期（1970年）
                        match session.search("SINCE 01-Jan-1970").await {
                            Ok(uids) => {
                                info!("📊 SINCE 01-Jan-1970: {} 封邮件", uids.len());
                            }
                            Err(e) => {
                                info!("❌ SINCE 01-Jan-1970 失败: {}", e);
                            }
                        }

                        // 测试 FETCH 来获取邮件头
                        info!("========================================");
                        info!("测试 4: 检查实际可获取的邮件");
                        info!("========================================");

                        match session.search("ALL").await {
                            Ok(uids) => {
                                info!("📊 总共 {} 封邮件", uids.len());

                                if uids.len() > 0 {
                                    // 尝试获取第一封邮件
                                    let first_uid = *uids.iter().max().unwrap();
                                    info!("尝试获取最新邮件 UID: {}", first_uid);

                                    match session.fetch(first_uid, "(RFC822.HEADER)").await {
                                        Ok(_) => {
                                            info!("✅ 成功获取最新邮件头");
                                        }
                                        Err(e) => {
                                            info!("❌ 获取邮件头失败: {}", e);
                                        }
                                    }
                                }

                                if uids.len() > 10 {
                                    // 尝试获取第 10 封邮件
                                    let uid_10 = uids.iter().nth(9).unwrap();
                                    info!("尝试获取第 10 封邮件 UID: {}", uid_10);

                                    match session.fetch(*uid_10, "(RFC822.HEADER)").await {
                                        Ok(_) => {
                                            info!("✅ 成功获取第 10 封邮件头");
                                        }
                                        Err(e) => {
                                            info!("❌ 获取邮件头失败: {}", e);
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
                        info!("❌ 选择文件夹失败: {}", e);
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
