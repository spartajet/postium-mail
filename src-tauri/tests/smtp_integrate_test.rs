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
    imap_ssl: bool,
    smtp_server: String,
    smtp_port: u16,
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
    use postium_mail_lib::services::imap_service::{ImapAuth, ImapClient};
    use postium_mail_lib::services::smtp_service::{SmtpAuth, SmtpClient};
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
        );

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
        let count = client.select_folder("INBOX").expect("选择收件箱失败");
        info!("   选择收件箱成功");
        info!("   邮件数量: {}", count);

        // 获取邮件 UID 列表（限制 10 封）
        let uids = client.list_uids("INBOX", 10).expect("获取 UID 列表失败");
        info!("   获取 UID 列表成功");
        debug!("   最新 {} 封邮件 UID: {:?}", uids.len(), uids);

        // 获取第一封邮件（如果有）
        if let Some(&uid) = uids.first() {
            info!("尝试获取第一封邮件 (UID: {})...", uid);
            let email = client.fetch_email(uid, "INBOX").expect("获取邮件失败");
            info!("✅ 获取邮件成功!");
            info!("   主题: {}", email.subject);
            info!("   发件人: {}", email.from);
            info!("   日期: {}", email.date.format("%Y-%m-%d %H:%M:%S"));
            debug!("   正文长度: {} 字符", email.body_text.len());
            debug!("   HTML 长度: {} 字符", email.body_html.len());
        }

        // 登出
        let _ = client.logout();
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

        let mut client = SmtpClient::new();

        // 连接
        let result = client.connect(
            &account.smtp_server,
            account.smtp_port,
            &account.account,
            SmtpAuth::Password(account.password.clone()),
        );

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

        let message_id = client
            .send_email(
                vec![account.account.clone()],
                &test_subject,
                &test_body,
                Some("Postium Mail 集成测试 - 纯文本备用"),
            )
            .expect("邮件发送失败");

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
        ) {
            Ok(_) => {
                info!("   ✅ IMAP 连接成功");

                // 选择收件箱
                match imap_client.select_folder("INBOX") {
                    Ok(count) => {
                        info!("   ✅ 收件箱: {} 封邮件", count);

                        // 获取最新 5 封邮件
                        match imap_client.list_uids("INBOX", 5) {
                            Ok(uids) => {
                                info!("   ✅ 获取到 {} 封最新邮件", uids.len());

                                // 获取第一封邮件详情
                                if let Some(uid) = uids.first() {
                                    match imap_client.fetch_email(*uid, "INBOX") {
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

                let _ = imap_client.logout();
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
            let mut smtp_client = SmtpClient::new();

            match smtp_client.connect(
                &account.smtp_server,
                account.smtp_port,
                &account.account,
                SmtpAuth::Password(account.password.clone()),
            ) {
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

                    match smtp_client.send_email(
                        vec![account.account.clone()],
                        &test_subject,
                        &test_body,
                        Some("纯文本备用"),
                    ) {
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
}
