// 测试：通用 IMAP 功能测试
//
// 从 JSON 配置文件读取账号，支持任意邮件服务商
//
// 运行方式：
//   cargo test --test provider test_imap_emails -- --nocapture
//
// 指定测试账号（通过环境变量）：
//   TEST_ACCOUNT=163 cargo test --test provider test_imap_emails -- --nocapture
//   TEST_ACCOUNT=gmail cargo test --test provider test_imap_emails -- --nocapture
//   TEST_ACCOUNT=outlook cargo test --test provider test_imap_emails -- --nocapture

#[cfg(test)]
mod tests {
    use std::env;

    use crate::common::load_test_account;

    use postium_mail_lib::protocols::imap::{AsyncImapClient, ImapAuth};
    use tracing::{debug, info, warn};

    /// 解码 IMAP UTF-7 编码的文件夹名称
    /// IMAP 使用修改版 UTF-7 编码（RFC 3501）
    fn decode_imap_utf7(name: &str) -> Option<String> {
        // IMAP UTF-7 格式: &xxx- 其中 xxx 是 base64 编码的 UTF-16
        // 例如: &UXZO1mWHTvZZOQ- = "已发送邮件"

        // 检查是否包含 UTF-7 编码标记
        if !name.contains('&') {
            return None;
        }

        let mut result = String::new();
        let chars: Vec<char> = name.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '&' {
                // 查找结束标记 '-'
                let mut end = i + 1;
                while end < chars.len() && chars[end] != '-' {
                    end += 1;
                }

                if end < chars.len() {
                    // 提取编码部分
                    let encoded: String = chars[i + 1..end].iter().collect();

                    // &- 表示 & 字符本身
                    if encoded.is_empty() {
                        result.push('&');
                    } else {
                        // 尝试解码 UTF-7
                        if let Some(decoded) = decode_utf7_segment(&encoded) {
                            result.push_str(&decoded);
                        } else {
                            // 解码失败，保留原始字符串
                            result.push_str(&name[i..=end]);
                        }
                    }
                    i = end + 1;
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        }

        // 如果结果和原始字符串相同，说明没有实际解码
        if result == name {
            None
        } else {
            Some(result)
        }
    }

    /// 解码单个 UTF-7 段
    fn decode_utf7_segment(encoded: &str) -> Option<String> {
        // IMAP UTF-7 使用修改的 base64 字母表
        // 将 ',' 替换为 '/' 以使用标准 base64 解码
        let standard_base64 = encoded.replace(',', "/");

        // 添加填充
        let padding = (4 - standard_base64.len() % 4) % 4;
        let padded = format!("{}{}", standard_base64, "=".repeat(padding));

        // 解码 base64
        let bytes = base64_decode(&padded)?;

        // 将字节转换为 UTF-16BE，然后转换为 String
        if bytes.len() % 2 != 0 {
            return None;
        }

        let mut utf16_chars = Vec::new();
        for chunk in bytes.chunks(2) {
            let code_point = u16::from_be_bytes([chunk[0], chunk[1]]);
            utf16_chars.push(code_point);
        }

        String::from_utf16(&utf16_chars).ok()
    }

    /// 简单的 base64 解码（避免引入额外依赖）
    fn base64_decode(input: &str) -> Option<Vec<u8>> {
        const DECODE_TABLE: [i8; 128] = [
            -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62,
            -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1, -1, 0,
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, -1, -1, -1, -1, -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
            41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
        ];

        let input = input.trim_end_matches('=');
        let input_bytes = input.as_bytes();

        let mut result = Vec::with_capacity(input.len() * 3 / 4);

        let mut buffer: u32 = 0;
        let mut bits = 0;

        for &byte in input_bytes {
            if byte >= 128 {
                return None;
            }
            let val = DECODE_TABLE[byte as usize];
            if val < 0 {
                return None;
            }
            buffer = (buffer << 6) | (val as u32);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                result.push((buffer >> bits) as u8);
            }
        }

        Some(result)
    }

    #[tokio::test]
    async fn test_imap_emails() {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .with_line_number(true)
            .try_init();

        // 从环境变量获取要测试的账号 key，默认为 "163"
        let account_key = env::var("TEST_ACCOUNT").unwrap_or_else(|_| "163".to_string());

        info!("========================================");
        info!("通用 IMAP 功能测试");
        info!("========================================");
        info!("测试账号: {}", account_key);
        info!("========================================");
        info!("");

        // 加载测试账号
        let account = match load_test_account(&account_key) {
            Some(acc) => acc,
            None => {
                info!("❌ 未找到账号配置: {}", account_key);
                info!("提示：");
                info!("  1. 请确保 .test_mail_accounts.json 文件存在");
                info!("  2. 请确保配置文件中包含 '{}' 账号", account_key);
                info!("  3. 或者设置相应的环境变量");
                info!("");
                info!("示例配置文件：.test_mail_accounts.json.example");
                return;
            }
        };

        info!("========================================");
        info!("账号信息");
        info!("========================================");
        info!("邮箱: {}", account.email);
        info!("IMAP 服务器: {}:{}", account.imap.host, account.imap.port);
        info!("IMAP SSL: {}", account.imap.ssl);
        info!("SMTP 服务器: {}:{}", account.smtp.host, account.smtp.port);
        info!("SMTP SSL: {}", account.smtp.ssl);
        info!("========================================");
        info!("");

        let mut client = AsyncImapClient::new();

        // 步骤 1: 连接并登录
        info!("========================================");
        info!("步骤 1: 连接并登录 IMAP 服务器");
        info!("========================================");

        match client
            .connect(
                &account.imap.host,
                account.imap.port,
                &account.email,
                ImapAuth::Password(account.password.clone()),
            )
            .await
        {
            Ok(_) => {
                info!("✅ IMAP 连接并登录成功!");
            }
            Err(e) => {
                info!("❌ IMAP 连接失败: {}", e);
                return;
            }
        }
        info!("");

        // 步骤 2: 获取邮箱支持的 IMAP 指令 (CAPABILITIES)
        info!("========================================");
        info!("步骤 2: 获取 IMAP CAPABILITIES");
        info!("========================================");

        // 使用 check_condstore_support 内部获取 capabilities 的方式
        match client.check_condstore_support().await {
            Ok(has_condstore) => {
                debug!("CONDSTORE 支持: {}", has_condstore);
                if has_condstore {
                    info!("✅ CONDSTORE 支持: 是");
                } else {
                    info!("⚠️  CONDSTORE 支持: 否");
                }
            }
            Err(e) => {
                warn!("获取 CONDSTORE 支持状态失败: {}", e);
            }
        }

        match client.check_idle_support().await {
            Ok(has_idle) => {
                debug!("IDLE 支持: {}", has_idle);
                if has_idle {
                    info!("✅ IDLE 支持: 是");
                } else {
                    info!("⚠️  IDLE 支持: 否");
                }
            }
            Err(e) => {
                warn!("获取 IDLE 支持状态失败: {}", e);
            }
        }
        info!("");

        // 步骤 3: 获取邮箱的所有文件夹
        info!("========================================");
        info!("步骤 3: 获取邮箱文件夹列表");
        info!("========================================");

        let folders = match client.list_folders_with_attributes().await {
            Ok(folders) => folders,
            Err(e) => {
                info!("❌ 获取文件夹列表失败: {}", e);
                let _ = client.logout().await;
                return;
            }
        };

        info!("📊 共找到 {} 个文件夹", folders.len());
        info!("");

        // 步骤 4: 获取每个文件夹的邮件数量
        info!("========================================");
        info!("步骤 4: 获取每个文件夹的邮件数量");
        info!("========================================");
        info!("");

        let mut total_emails = 0;

        for (index, folder) in folders.iter().enumerate() {
            info!("[{}/{}] 文件夹: {}", index + 1, folders.len(), folder.name);

            // 尝试解码 UTF-7 编码的文件夹名称
            if let Some(decoded) = decode_imap_utf7(&folder.name) {
                info!("  UTF-7 解码: {} -> {}", folder.name, decoded);
            }

            // 显示特殊用途属性
            if let Some(ref special_use) = folder.special_use {
                debug!("  特殊用途: {:?}", special_use);
                info!("  📌 特殊用途: {:?}", special_use);
            }

            // 获取邮件数量
            match client.select_folder(&folder.name).await {
                Ok(count) => {
                    info!("  📧 邮件数量: {}", count);
                    total_emails += count;
                }
                Err(e) => {
                    warn!("  ❌ 无法访问文件夹: {}", e);
                }
            }

            info!("");
        }

        info!("========================================");
        info!("测试完成");
        info!("========================================");
        info!("账号: {} ({})", account.email, account_key);
        info!("总文件夹数: {}", folders.len());
        info!("总邮件数: {}", total_emails);
        info!("========================================");

        // 登出
        let _ = client.logout().await;
    }

    /// 测试所有可用账号
    ///
    /// 这个测试会列出所有配置的账号，但不会实际连接
    #[test]
    fn test_list_available_accounts() {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .try_init();

        use crate::common::list_available_accounts;

        info!("========================================");
        info!("列出所有可用的测试账号");
        info!("========================================");

        let accounts = list_available_accounts();

        if accounts.is_empty() {
            info!("⚠️  没有找到任何配置的测试账号");
            info!("");
            info!("请创建 .test_mail_accounts.json 文件或设置环境变量");
            info!("参考 .test_mail_accounts.json.example 文件");
        } else {
            info!("✅ 找到 {} 个可用账号:", accounts.len());
            info!("");
            for (index, account_key) in accounts.iter().enumerate() {
                info!("  {}. {}", index + 1, account_key);

                // 尝试加载账号信息
                if let Some(account) = load_test_account(account_key) {
                    info!("     邮箱: {}", account.email);
                    info!("     IMAP: {}:{}", account.imap.host, account.imap.port);
                }
                info!("");
            }
        }

        info!("========================================");
        info!("使用方法");
        info!("========================================");
        info!("测试特定账号:");
        info!("  TEST_ACCOUNT=163 cargo test --test provider test_imap_emails -- --nocapture");
        info!("  TEST_ACCOUNT=gmail cargo test --test provider test_imap_emails -- --nocapture");
        info!("========================================");
    }

    /// 测试特定账号的配置加载
    #[test]
    fn test_account_config_load() {
        let account_key = env::var("TEST_ACCOUNT").unwrap_or_else(|_| "163".to_string());

        match load_test_account(&account_key) {
            Some(account) => {
                println!("✅ 成功加载账号配置: {}", account_key);
                println!("   邮箱: {}", account.email);
                println!("   IMAP: {}:{}", account.imap.host, account.imap.port);
                println!("   SMTP: {}:{}", account.smtp.host, account.smtp.port);
            }
            None => {
                println!("❌ 未找到账号配置: {}", account_key);
            }
        }
    }
}
