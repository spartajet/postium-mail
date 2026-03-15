// 诊断测试：检查 IMAP SINCE 命令是否正确获取近一年的邮件
//
// 运行：cargo test --test test_sync_since -- --nocapture

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

        // 支持两种格式：
        // 1. "key = value"（带空格）
        // 2. "key=value"（不带空格）
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

/// 将月份数字转换为英文缩写
fn month_abbr(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

/// 计算一年前的日期（IMAP 格式）
fn one_year_ago_imap_format() -> String {
    let now = chrono::Utc::now();
    let one_year_ago = now - chrono::Duration::days(365);

    let date_str = format!(
        "{:02}-{}-{:04}",
        one_year_ago.day(),
        month_abbr(one_year_ago.month()),
        one_year_ago.year()
    );

    tracing::info!(
        "📅 计算一年前的日期: 现在={}, 一年前={}, 格式化后={}",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        one_year_ago.format("%Y-%m-%d %H:%M:%S UTC"),
        date_str
    );

    date_str
}

#[cfg(test)]
mod tests {
    use super::*;
    use postium_mail_lib::services::imap::{AsyncImapClient, ImapAuth};
    use tracing::info;

    #[tokio::test]
    async fn test_imap_since_command() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .pretty()
            .try_init();

        let account = load_test_account();

        info!("========================================");
        info!("测试 IMAP SINCE 命令 - 获取近一年邮件");
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

                // 测试收件箱文件夹
                let folder = "INBOX";

                info!("========================================");
                info!("步骤 1: 获取所有邮件 (SEARCH ALL)");
                info!("========================================");

                // 使用 list_uids 获取所有邮件
                match client.list_uids(folder, 999999).await {
                    Ok(all_uids) => {
                        info!("📊 文件夹 '{}' 总邮件数: {}", folder, all_uids.len());

                        if !all_uids.is_empty() {
                            info!(
                                "   UID 范围: {} ~ {}",
                                all_uids.last().unwrap_or(&0),
                                all_uids.first().unwrap_or(&0)
                            );
                        }

                        // 获取第一封和最后一封邮件的日期
                        if all_uids.len() >= 2 {
                            let first_uid = all_uids[0];
                            let last_uid = all_uids[all_uids.len() - 1];

                            info!("========================================");
                            info!("步骤 2: 获取最早和最新邮件的日期");
                            info!("========================================");

                            if let Ok(first_email) = client.fetch_email(folder, first_uid).await {
                                info!("最新邮件 (UID {}):", first_uid);
                                info!("  日期: {}", first_email.date.format("%Y-%m-%d %H:%M:%S"));
                                info!("  主题: {}", first_email.subject);
                            }

                            if let Ok(last_email) = client.fetch_email(folder, last_uid).await {
                                info!("最旧邮件 (UID {}):", last_uid);
                                info!("  日期: {}", last_email.date.format("%Y-%m-%d %H:%M:%S"));
                                info!("  主题: {}", last_email.subject);
                            }
                        }

                        info!("========================================");
                        info!("步骤 3: 使用 SINCE 命令获取近一年邮件");
                        info!("========================================");

                        let date_since = one_year_ago_imap_format();
                        info!("日期参数: {}", date_since);

                        // 使用 list_uids_since
                        match client.list_uids_since(folder, &date_since).await {
                            Ok(since_uids) => {
                                info!("📥 SINCE 命令返回邮件数: {}", since_uids.len());

                                if !since_uids.is_empty() {
                                    info!(
                                        "   UID 范围: {} ~ {}",
                                        since_uids.last().unwrap_or(&0),
                                        since_uids.first().unwrap_or(&0)
                                    );

                                    // 获取 SINCE 返回的最旧邮件
                                    if let Some(oldest_uid) = since_uids.last() {
                                        if let Ok(email) =
                                            client.fetch_email(folder, *oldest_uid).await
                                        {
                                            info!("SINCE 返回的最旧邮件 (UID {}):", oldest_uid);
                                            info!(
                                                "  日期: {}",
                                                email.date.format("%Y-%m-%d %H:%M:%S")
                                            );
                                            info!("  主题: {}", email.subject);
                                        }
                                    }
                                }

                                info!("========================================");
                                info!("步骤 4: 数据分析");
                                info!("========================================");

                                let percentage = if all_uids.is_empty() {
                                    0.0
                                } else {
                                    (since_uids.len() as f64 / all_uids.len() as f64) * 100.0
                                };

                                info!("总邮件数: {}", all_uids.len());
                                info!("SINCE 返回: {}", since_uids.len());
                                info!("比例: {:.1}%", percentage);

                                // 诊断结果
                                info!("========================================");
                                info!("诊断结果");
                                info!("========================================");

                                if all_uids.is_empty() {
                                    info!("⚠️  文件夹为空，无法进行测试");
                                } else if since_uids.len() == all_uids.len() {
                                    info!("✅ SINCE 命令返回了所有邮件");
                                    info!("   说明：所有邮件都在近一年内");
                                } else if since_uids.len() < all_uids.len() / 2 {
                                    info!("❌ SINCE 命令返回的邮件数量异常少！");
                                    info!("   可能原因：");
                                    info!("   1. IMAP 服务器不支持 SINCE 命令");
                                    info!("   2. 日期格式不正确");
                                    info!(
                                        "   3. SINCE 命令使用的是邮件 Date: header，而非接收时间"
                                    );
                                } else {
                                    info!("✅ SINCE 命令正常工作");
                                    info!("   返回了近 {}% 的邮件", percentage as i32);
                                }

                                // 测试不同的日期格式
                                info!("========================================");
                                info!("步骤 5: 测试不同日期格式");
                                info!("========================================");

                                // 测试 6 个月前
                                let six_months_ago =
                                    chrono::Utc::now() - chrono::Duration::days(180);
                                let six_months_str = format!(
                                    "{:02}-{}-{:04}",
                                    six_months_ago.day(),
                                    month_abbr(six_months_ago.month()),
                                    six_months_ago.year()
                                );

                                if let Ok(uids) =
                                    client.list_uids_since(folder, &six_months_str).await
                                {
                                    info!("6个月前 ({}): {} 封邮件", six_months_str, uids.len());
                                }

                                // 测试 3 个月前
                                let three_months_ago =
                                    chrono::Utc::now() - chrono::Duration::days(90);
                                let three_months_str = format!(
                                    "{:02}-{}-{:04}",
                                    three_months_ago.day(),
                                    month_abbr(three_months_ago.month()),
                                    three_months_ago.year()
                                );

                                if let Ok(uids) =
                                    client.list_uids_since(folder, &three_months_str).await
                                {
                                    info!("3个月前 ({}): {} 封邮件", three_months_str, uids.len());
                                }

                                // 测试 1 个月前
                                let one_month_ago = chrono::Utc::now() - chrono::Duration::days(30);
                                let one_month_str = format!(
                                    "{:02}-{}-{:04}",
                                    one_month_ago.day(),
                                    month_abbr(one_month_ago.month()),
                                    one_month_ago.year()
                                );

                                if let Ok(uids) =
                                    client.list_uids_since(folder, &one_month_str).await
                                {
                                    info!("1个月前 ({}): {} 封邮件", one_month_str, uids.len());
                                }
                            }
                            Err(e) => {
                                info!("❌ SINCE 命令失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        info!("❌ 获取所有邮件失败: {}", e);
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
