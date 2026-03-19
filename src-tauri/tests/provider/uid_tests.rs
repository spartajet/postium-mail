// 测试：检查特定UID 2408的编码问题
//
// 运行：cargo test --test test_uid_2408 -- --nocapture

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
    async fn test_uid_2408_encoding() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试：UID 2408 编码问题");
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
                info!("尝试获取邮件 UID: {}", target_uid);
                info!("========================================");

                match client.fetch_email(folder, target_uid).await {
                    Ok(email) => {
                        info!("✅ 邮件获取成功!");
                        info!("主题: {}", email.subject);
                        info!("发件人: {}", email.from);
                        info!("日期: {}", email.date.format("%Y-%m-%d %H:%M:%S"));

                        // 检测乱码
                        let has_replacement_chars = email.subject.contains('�');
                        let replacement_count = email.subject.chars().filter(|&c| c == '�').count();
                        let total_chars = email.subject.chars().count();

                        info!("========================================");
                        info!("编码分析");
                        info!("========================================");
                        info!("主题长度: {} 字符", total_chars);
                        info!("替换字符数量: {}", replacement_count);

                        if has_replacement_chars {
                            info!("⚠️  检测到乱码!");
                            info!("   替换字符占比: {:.1}%",
                                (replacement_count as f64 / total_chars as f64) * 100.0);

                            // 打印主题的字节表示
                            info!("   主题字节: {:?}", email.subject.as_bytes());

                            // 检查是否是特定的乱码模式
                            if email.subject.contains("ƶ") || email.subject.contains("Ʊ") {
                                info!("   检测到GBK编码错误的典型模式");
                            }
                        } else {
                            info!("✅ 主题显示正常，没有检测到乱码!");
                        }

                        // 检查主题是否是预期的内容
                        if email.subject.contains("移动") || email.subject.contains("发票") {
                            info!("✅ 主题包含预期的中文关键词!");
                        } else if email.subject.len() > 50 {
                            // 如果主题很长但无法识别，可能是乱码
                            info!("⚠️  主题长度异常（{} 字符），可能是乱码", email.subject.len());
                        }
                    }
                    Err(e) => {
                        info!("❌ 获取邮件失败: {}", e);
                        info!("可能原因:");
                        info!("  1. UID {} 不存在", target_uid);
                        info!("  2. 邮件已被删除");
                        info!("  3. IMAP 权限问题");
                    }
                }
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
            }
        }
    }
}
