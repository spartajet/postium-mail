// SMTP 集成测试 - 使用真实账号测试 SMTP 发送功能
//
// 运行测试：cargo test --test smtp_integrate_test -- --nocapture
//
// 注意：需要真实的网络连接和测试账号

use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// 测试账号配置
#[derive(Debug, Clone)]
struct TestAccount {
    account: String,
    imap_server: String,
    imap_port: u16,
    #[allow(dead_code)]
    imap_ssl: bool,
    smtp_server: String,
    smtp_port: u16,
    #[allow(dead_code)]
    smtp_ssl: bool,
    password: String,
}

/// 从文件解析测试账号配置
fn load_test_account() -> TestAccount {
    // 测试在 src-tauri 目录运行，需要回到项目根目录
    let account_path = Path::new("../.test_mail_accounts");

    let content = fs::read_to_string(account_path).expect("无法读取 .test_mail_accounts 文件");

    let mut account = String::new();
    let mut imap_server = String::new();
    let mut imap_port = 993u16;
    let mut imap_ssl = true;
    let mut smtp_server = String::new();
    let mut smtp_port = 465u16;
    let mut smtp_ssl = true;
    let mut password = String::new();

    // 解析键值对
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
                "imap_ssl" => imap_ssl = value.parse().unwrap_or(true),
                "smtp_server" => smtp_server = value.to_string(),
                "smtp_port" => smtp_port = value.parse().unwrap_or(465),
                "smtp_ssl" => smtp_ssl = value.parse().unwrap_or(true),
                "password" => password = value.to_string(),
                _ => {}
            }
        }
    }

    TestAccount {
        account,
        imap_server,
        imap_port,
        imap_ssl,
        smtp_server,
        smtp_port,
        smtp_ssl,
        password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use postium_mail_lib::protocols::imap::{ImapAuth, ImapClient};
    use postium_mail_lib::protocols::smtp::{SendEmailRequest, SmtpAuth, SmtpClient};
    use std::time::Instant;

    // 初始化 tracing 日志（仅测试时）
    fn init_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    }

    /// 测试 IMAP 连接
    #[tokio::test]
    async fn test_imap_connection() {
        init_tracing();

        let account = load_test_account();

        info!("========================================");
        info!("测试 IMAP 连接");
        info!("账号: {}", account.account);
        info!("服务器: {}:{}", account.imap_server, account.imap_port);
        info!("========================================");

        let start = Instant::now();

        let mut client = ImapClient::new();

        // 连接并登录
        let result = client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ).await;

        let connect_time = start.elapsed();

        // 如果是 DNS 错误，跳过测试（网络问题）
        if let Err(e) = &result {
            let err_str = e.to_string();
            error!(err_str);
            if err_str.contains("11001")
                || err_str.contains("dns")
                || err_str.contains("不知道这样的主机")
            {
                warn!("⚠️  网络连接不可用 (DNS 错误)，跳过测试");
                warn!("   错误: {}", e);
                info!("提示: 请检查网络连接后重新运行测试");
                return; // 测试跳过，不算失败
            }
        }

        // 断言连接成功
        assert!(result.is_ok(), "IMAP 连接失败: {:?}", result.unwrap_err());
        info!("✅ IMAP 连接成功!");
        info!("   连接耗时: {:?}", connect_time);

        // 尝试选择收件箱
        let count = client.select_folder("INBOX").await.expect("选择收件箱失败");
        info!("   选择收件箱成功");
        info!("   邮件数量: {}", count);

        // 获取邮件 UID 列表（限制 10 封）
        let uids = client.list_uids("INBOX", 10).await.expect("获取 UID 列表失败");
        info!("   获取 UID 列表成功");
        debug!("   最新 {} 封邮件 UID: {:?}", uids.len(), uids);

        // 获取第一封邮件（如果有）
        if let Some(&uid) = uids.first() {
            info!("尝试获取第一封邮件 (UID: {})...", uid);
            let email = client.fetch_email("INBOX", uid).await.expect("获取邮件失败");
            info!("✅ 获取邮件成功!");
            info!("   主题: {}", email.subject);
            info!("   发件人: {}", email.from);
            info!("   日期: {}", email.date.format("%Y-%m-%d %H:%M:%S"));
            debug!("   正文长度: {} 字符", email.body_text.len());
            debug!("   HTML 长度: {} 字符", email.body_html.len());
        }

        // 登出
        let _ = client.logout().await;
        info!("✅ IMAP 测试完成!");
    }

    /// 测试 SMTP 连接和发送
    #[tokio::test]
    async fn test_smtp_connection() {
        init_tracing();

        let account = load_test_account();

        info!("========================================");
        info!("测试 SMTP 连接");
        info!("账号: {}", account.account);
        info!("服务器: {}:{}", account.smtp_server, account.smtp_port);
        info!("========================================");

        let start = Instant::now();

        let client = SmtpClient::new();

        // 连接
        let result = client.connect(
            &account.smtp_server,
            account.smtp_port,
            &account.account,
            SmtpAuth::Password(account.password.clone()),
        ).await;

        let connect_time = start.elapsed();

        // 如果是 DNS 错误，跳过测试（网络问题）
        if let Err(e) = &result {
            let err_str = e.to_string();
            if err_str.contains("11001")
                || err_str.contains("dns")
                || err_str.contains("不知道这样的主机")
            {
                warn!("⚠️  网络连接不可用 (DNS 错误)，跳过测试");
                warn!("   错误: {}", e);
                info!("提示: 请检查网络连接后重新运行测试");
                return; // 测试跳过，不算失败
            }
        }

        // 断言连接成功
        assert!(result.is_ok(), "SMTP 连接失败: {:?}", result.unwrap_err());
        info!("✅ SMTP 连接成功!");
        info!("   连接耗时: {:?}", connect_time);

        // 发送测试邮件
        let test_subject = format!(
            "Postium Mail 测试邮件 - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        );
        let test_body = r#"
<!DOCTYPE html>
<html>
<body>
    <h2>这是一封测试邮件</h2>
    <p>如果你收到这封邮件，说明 SMTP 发送功能正常工作。</p>
    <p><strong>测试时间:</strong> {timestamp}</p>
    <hr>
    <p style="color: #666; font-size: 12px;">此邮件由 Postium Mail 集成测试自动发送</p>
</body>
</html>
"#
        .replace(
            "{timestamp}",
            &chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        );

        info!("发送测试邮件到 {}...", account.account);

        let request = SendEmailRequest {
            from: account.account.clone(),
            to: vec![account.account.clone()],
            cc: None,
            bcc: None,
            subject: test_subject.clone(),
            html_body: test_body.clone(),
            text_body: Some("Postium Mail 集成测试 - 纯文本备用".to_string()),
            attachments: vec![],
        };

        let result = client.send_email(request).await;

        assert!(result.is_ok(), "邮件发送失败: {:?}", result.unwrap_err());
        let message_id = result.unwrap().message_id;

        info!("✅ 邮件发送成功!");
        info!("   Message-ID: {}", message_id);
        info!("✅ SMTP 测试完成!");
    }

    /// 完整的集成测试 - IMAP + SMTP
    #[tokio::test]
    async fn test_full_integration() {
        init_tracing();

        info!("╔══════════════════════════════════════════════╗");
        info!("║       Postium Mail 集成测试                    ║");
        info!("╚══════════════════════════════════════════════╝");

        let account = load_test_account();

        info!("测试账号: {}", account.account);
        info!("IMAP: {}:{}", account.imap_server, account.imap_port);
        info!("SMTP: {}:{}", account.smtp_server, account.smtp_port);

        let mut has_network = true;

        // 测试 IMAP
        info!("▶️  测试 IMAP 功能...");
        let mut imap_client = ImapClient::new();

        match imap_client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ).await {
            Ok(_) => {
                info!("   ✅ IMAP 连接成功");

                // 选择收件箱
                match imap_client.select_folder("INBOX").await {
                    Ok(count) => {
                        info!("   ✅ 收件箱: {} 封邮件", count);

                        // 获取最新 5 封邮件
                        match imap_client.list_uids("INBOX", 5).await {
                            Ok(uids) => {
                                info!("   ✅ 获取到 {} 封最新邮件", uids.len());

                                // 获取第一封邮件详情
                                if let Some(uid) = uids.first() {
                                    match imap_client.fetch_email("INBOX", *uid).await {
                                        Ok(email) => info!("   ✅ 最新邮件: {}", email.subject),
                                        Err(e) => warn!("   ⚠️  获取邮件详情失败: {}", e),
                                    }
                                }
                            }
                            Err(e) => warn!("   ⚠️  获取邮件列表失败: {}", e),
                        }
                    }
                    Err(e) => warn!("   ⚠️  选择收件箱失败: {}", e),
                }

                let _ = imap_client.logout().await;
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001")
                    || err_str.contains("dns")
                    || err_str.contains("不知道这样的主机")
                {
                    warn!("   ⚠️  网络连接不可用 (DNS 错误)");
                    has_network = false;
                } else {
                    error!("   ❌ IMAP 失败: {}", e);
                }
            }
        }

        // 测试 SMTP
        if has_network {
            info!("▶️  测试 SMTP 功能...");
            let smtp_client = SmtpClient::new();

            match smtp_client.connect(
                &account.smtp_server,
                account.smtp_port,
                &account.account,
                SmtpAuth::Password(account.password.clone()),
            ).await {
                Ok(_) => {
                    info!("   ✅ SMTP 连接成功");

                    // 发送测试邮件
                    let test_subject = format!(
                        "Postium 集成测试 - {}",
                        chrono::Utc::now().format("%H:%M:%S")
                    );
                    let test_body = format!(
                        "<p>集成测试时间: {}</p>",
                        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
                    );

                    let request = SendEmailRequest {
                        from: account.account.clone(),
                        to: vec![account.account.clone()],
                        cc: None,
                        bcc: None,
                        subject: test_subject,
                        html_body: test_body,
                        text_body: Some("纯文本备用".to_string()),
                        attachments: vec![],
                    };

                    match smtp_client.send_email(request).await {
                        Ok(_) => info!("   ✅ 邮件发送成功"),
                        Err(e) => error!("   ❌ 发送失败: {}", e),
                    }
                }
                Err(e) => error!("   ❌ SMTP 失败: {}", e),
            }
        } else {
            info!("▶️  跳过 SMTP 测试 (网络不可用)");
        }

        info!("╚══════════════════════════════════════════════╝");
        info!("║              集成测试完成                      ║");
        info!("╚══════════════════════════════════════════════╝");
    }

    /// 测试列出所有 IMAP 文件夹
    #[tokio::test]
    async fn test_list_folders() {
        init_tracing();

        let account = load_test_account();

        info!("========================================");
        info!("测试列出所有 IMAP 文件夹");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = ImapClient::new();

        // 连接并登录
        match client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ).await {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                // 列出所有文件夹
                match client.list_folders().await {
                    Ok(folders) => {
                        info!("✅ 获取到 {} 个文件夹:", folders.len());
                        for folder in &folders {
                            info!("   - {} (flags: {:?})", folder, folder.as_bytes());
                        }

                        // 检查常见的已发送邮件文件夹名称
                        let sent_variants = vec![
                            "Sent", "SENT", "Sent Items", "Sent Mail", "Sent Messages",
                            "已发送", "已发送邮件", "发送",
                            "INBOX.Sent", "INBOX.Sent Items",
                        ];

                        info!("\n检查可能的已发送文件夹名称:");
                        for test_name in &sent_variants {
                            let exists = folders.iter().any(|f| f.contains(test_name));
                            if exists {
                                info!("   ✅ 找到包含 '{}' 的文件夹", test_name);
                            } else {
                                debug!("   - '{}' 不存在", test_name);
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 列出文件夹失败: {}", e);
                    }
                }

                let _ = client.logout().await;
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001") || err_str.contains("dns") || err_str.contains("不知道这样的主机") {
                    warn!("⚠️  网络连接不可用 (DNS 错误)，跳过测试");
                } else {
                    error!("❌ IMAP 连接失败: {}", e);
                }
            }
        }
    }

    /// 测试从 Sent 文件夹获取邮件
    #[tokio::test]
    async fn test_fetch_sent_emails() {
        init_tracing();

        let account = load_test_account();

        info!("========================================");
        info!("测试从 Sent 文件夹获取邮件");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = ImapClient::new();

        // 连接并登录
        match client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ).await {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                // 先列出所有文件夹
                match client.list_folders().await {
                    Ok(folders) => {
                        info!("✅ 可用文件夹:");
                        for folder in &folders {
                            info!("   - {}", folder);
                        }

                        // 尝试查找可能的 Sent 文件夹
                        let sent_folder_names = vec![
                            "Sent", "SENT", "Sent Items", "Sent Mail", "Sent Messages",
                            "已发送", "已发送邮件", "发送",
                        ];

                        for folder_name in &sent_folder_names {
                            info!("\n尝试选择文件夹: '{}'...", folder_name);

                            match client.select_folder(folder_name).await {
                                Ok(count) => {
                                    info!("✅ 成功选择 '{}', 邮件数量: {}", folder_name, count);

                                    if count > 0 {
                                        // 获取最新 5 封邮件
                                        match client.list_uids(folder_name, 5).await {
                                            Ok(uids) => {
                                                info!("✅ 获取到 {} 封最新邮件", uids.len());

                                                // 获取第一封邮件详情
                                                if let Some(uid) = uids.first() {
                                                    match client.fetch_email(folder_name, *uid).await {
                                                        Ok(email) => {
                                                            info!("✅ 最新邮件详情:");
                                                            info!("   主题: {}", email.subject);
                                                            info!("   发件人: {}", email.from);
                                                            info!("   收件人: {:?}", email.to);
                                                            info!("   日期: {}", email.date.format("%Y-%m-%d %H:%M:%S"));
                                                        }
                                                        Err(e) => {
                                                            warn!("⚠️  获取邮件详情失败: {}", e);
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("⚠️  获取 UID 列表失败: {}", e);
                                            }
                                        }
                                    } else {
                                        info!("⚠️  文件夹 '{}' 为空", folder_name);
                                    }

                                    // 找到一个有效的 Sent 文件夹就退出
                                    break;
                                }
                                Err(e) => {
                                    debug!("文件夹 '{}' 不可用: {}", folder_name, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ 列出文件夹失败: {}", e);
                    }
                }

                let _ = client.logout().await;
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001") || err_str.contains("dns") || err_str.contains("不知道这样的主机") {
                    warn!("⚠️  网络连接不可用 (DNS 错误)，跳过测试");
                } else {
                    error!("❌ IMAP 连接失败: {}", e);
                }
            }
        }
    }

    /// 测试检查所有文件夹的邮件数量
    #[tokio::test]
    async fn test_check_all_folders_email_count() {
        init_tracing();

        let account = load_test_account();

        info!("========================================");
        info!("测试检查所有文件夹的邮件数量");
        info!("账号: {}", account.account);
        info!("========================================");

        let mut client = ImapClient::new();

        // 连接并登录
        match client.connect(
            &account.imap_server,
            account.imap_port,
            &account.account,
            ImapAuth::Password(account.password.clone()),
        ).await {
            Ok(_) => {
                info!("✅ IMAP 连接成功!");

                // 列出所有文件夹
                match client.list_folders().await {
                    Ok(folders) => {
                        info!("✅ 获取到 {} 个文件夹", folders.len());
                        info!("========================================");

                        for folder in &folders {
                            match client.select_folder(folder).await {
                                Ok(count) => {
                                    info!("📁 {:30} - {:4} 封邮件", folder, count);

                                    // 如果有邮件，获取第一封的主题
                                    if count > 0 {
                                        if let Ok(uids) = client.list_uids(folder, 1).await {
                                            if let Some(uid) = uids.first() {
                                                if let Ok(email) = client.fetch_email(folder, *uid).await {
                                                    info!("   └─ 最新: {}", email.subject);
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    debug!("⚠️  无法选择 '{}': {}", folder, e);
                                }
                            }
                        }

                        info!("========================================");
                        info!("提示: 这些名称可能包含已发送邮件:");
                        info!("  - Sent / Sent Items (英文)");
                        info!("  - 已发送 / 发送 (中文)");
                        info!("  - & 开头的编码名称 (UTF-7)");
                    }
                    Err(e) => {
                        error!("❌ 列出文件夹失败: {}", e);
                    }
                }

                let _ = client.logout().await;
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("11001") || err_str.contains("dns") || err_str.contains("不知道这样的主机") {
                    warn!("⚠️  网络连接不可用 (DNS 错误)，跳过测试");
                } else {
                    error!("❌ IMAP 连接失败: {}", e);
                }
            }
        }
    }
}
