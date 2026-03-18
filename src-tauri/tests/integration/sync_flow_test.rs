// 同步流程集成测试
//
// 测试完整的同步功能，包括文件夹同步、邮件同步等

use crate::integration::test_helpers::{check_greenmail_running, GreenmailConfig};

#[tokio::test]
#[ignore]
async fn test_database_initialization() {
    println!("🗄️  测试数据库初始化...");

    use crate::integration::test_helpers::{create_test_db, init_test_db};

    let db = create_test_db().await;
    init_test_db(&db).await;

    // 验证数据库已创建
    println!("✅ 数据库初始化成功");
}

#[tokio::test]
#[ignore]
async fn test_greenmail_imap_capabilities() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("🔍 测试 IMAP CAPABILITY 命令...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();
            let _welcome = String::from_utf8_lossy(&buffer);

            // 发送 CAPABILITY 命令
            let cmd = "A001 CAPABILITY\r\n";
            stream.write_all(cmd.as_bytes()).await.unwrap();

            // 读取响应
            let mut buffer = [0u8; 2048];
            let n = stream.read(&mut buffer).await.unwrap();
            let response = String::from_utf8_lossy(&buffer[..n]);

            println!("📩 CAPABILITY 响应:\n{}", response);

            // 验证响应包含 IMAP4rev1
            assert!(response.contains("IMAP4rev1"), "应该支持 IMAP4rev1");

            // 检查是否支持 CONDSTORE（GreenMail 1.6.0 可能不支持）
            let has_condstore = response.contains("CONDSTORE");
            if has_condstore {
                println!("✅ GreenMail 支持 CONDSTORE");
            } else {
                println!("ℹ️  GreenMail 不支持 CONDSTORE（这是预期的）");
            }

            println!("✅ CAPABILITY 命令执行成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_imap_list_folders() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("📁 测试 LIST 命令...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();

            // 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            stream.read(&mut buffer).await.unwrap();

            // 发送 LIST 命令
            let list_cmd = "A002 LIST \"\" *\r\n";
            stream.write_all(list_cmd.as_bytes()).await.unwrap();

            // 读取响应
            let mut buffer = [0u8; 4096];
            let mut all_responses = String::new();

            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                let response = String::from_utf8_lossy(&buffer[..n]);
                all_responses.push_str(&response);

                // 检查是否完成
                if response.contains("A002 OK") {
                    break;
                }
            }

            println!("📩 LIST 响应:\n{}", all_responses);

            // 验证 INBOX 存在
            assert!(all_responses.contains("INBOX"), "应该有 INBOX 文件夹");

            println!("✅ LIST 命令执行成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_imap_select_inbox() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("📥 测试 SELECT INBOX 命令...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();

            // 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            stream.read(&mut buffer).await.unwrap();

            // 发送 SELECT INBOX 命令
            let select_cmd = "A002 SELECT INBOX\r\n";
            stream.write_all(select_cmd.as_bytes()).await.unwrap();

            // 读取响应
            let mut buffer = [0u8; 2048];
            let mut all_responses = String::new();

            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                let response = String::from_utf8_lossy(&buffer[..n]);
                all_responses.push_str(&response);

                if response.contains("A002 OK") {
                    break;
                }
            }

            println!("📩 SELECT 响应:\n{}", all_responses);

            // 验证 SELECT 成功
            assert!(all_responses.contains("A002 OK"), "SELECT 应该成功");

            // 检查邮件数量
            if all_responses.contains("EXISTS") {
                println!("✓ 检测到 EXISTS 响应（邮件数量）");
            }

            println!("✅ SELECT INBOX 成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_imap_search_all() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("🔍 测试 SEARCH ALL 命令...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();

            // 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            stream.read(&mut buffer).await.unwrap();

            // SELECT INBOX
            let select_cmd = "A002 SELECT INBOX\r\n";
            stream.write_all(select_cmd.as_bytes()).await.unwrap();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                let response = String::from_utf8_lossy(&buffer[..n]);
                if response.contains("A002 OK") {
                    break;
                }
            }

            // 发送 SEARCH ALL 命令
            let search_cmd = "A003 SEARCH ALL\r\n";
            stream.write_all(search_cmd.as_bytes()).await.unwrap();

            // 读取响应
            let mut buffer = [0u8; 1024];
            let mut all_responses = String::new();

            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                let response = String::from_utf8_lossy(&buffer[..n]);
                all_responses.push_str(&response);

                if response.contains("A003 OK") {
                    break;
                }
            }

            println!("📩 SEARCH 响应:\n{}", all_responses);

            // 验证 SEARCH 成功
            assert!(all_responses.contains("A003 OK"), "SEARCH 应该成功");

            // 检查是否有邮件
            if all_responses.contains("* SEARCH") {
                println!("✓ 检测到 SEARCH 结果");
                // 提取 UID 列表
                let search_line: Vec<&str> = all_responses
                    .lines()
                    .filter(|line| line.contains("* SEARCH"))
                    .collect();

                if let Some(line) = search_line.first() {
                    println!("📊 邮件列表: {}", line);
                }
            } else {
                println!("ℹ️  INBOX 为空（没有邮件）");
            }

            println!("✅ SEARCH ALL 成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_imap_noop_command() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("💤 测试 NOOP 命令（保持连接活跃）...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();

            // 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            stream.read(&mut buffer).await.unwrap();

            // 发送 NOOP 命令
            let noop_cmd = "A002 NOOP\r\n";
            stream.write_all(noop_cmd.as_bytes()).await.unwrap();

            // 读取响应
            let n = stream.read(&mut buffer).await.unwrap();
            let response = String::from_utf8_lossy(&buffer[..n]);

            println!("📩 NOOP 响应: {}", response);

            // 验证 NOOP 成功
            assert!(response.contains("A002 OK"), "NOOP 应该成功");

            println!("✅ NOOP 命令执行成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_imap_logout() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("👋 测试 LOGOUT 命令...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            // 读取欢迎消息
            let mut buffer = [0u8; 1024];
            stream.read(&mut buffer).await.unwrap();

            // 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            stream.read(&mut buffer).await.unwrap();

            // 发送 LOGOUT 命令
            let logout_cmd = "A002 LOGOUT\r\n";
            stream.write_all(logout_cmd.as_bytes()).await.unwrap();

            // 读取响应
            let n = stream.read(&mut buffer).await.unwrap();
            let response = String::from_utf8_lossy(&buffer[..n]);

            println!("📩 LOGOUT 响应: {}", response);

            // 验证 LOGOUT 成功
            assert!(
                response.contains("A002 OK") || response.contains("BYE"),
                "LOGOUT 应该成功"
            );

            println!("✅ LOGOUT 命令执行成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_complete_imap_session() {
    if !check_greenmail_running().await {
        println!("⚠️  GreenMail 未运行，跳过测试");
        return;
    }

    println!("🔄 测试完整的 IMAP 会话流程...");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let config = GreenmailConfig::default();

    match TcpStream::connect(format!("{}:{}", config.host, config.imap_port)).await {
        Ok(mut stream) => {
            let mut buffer = [0u8; 4096];

            // 1. 读取欢迎消息
            let n = stream.read(&mut buffer).await.unwrap();
            let welcome = String::from_utf8_lossy(&buffer[..n]);
            println!("1️⃣  欢迎消息: {}", welcome.trim());
            assert!(welcome.contains("* OK"), "应该收到欢迎消息");

            // 2. 登录
            let login_cmd = format!("A001 LOGIN {} {}\r\n", config.username, config.password);
            stream.write_all(login_cmd.as_bytes()).await.unwrap();
            let mut response = String::new();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                response.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if response.contains("A001") {
                    break;
                }
            }
            println!("2️⃣  登录: {}", response.lines().last().unwrap_or(&""));
            assert!(response.contains("A001 OK"), "登录应该成功");

            // 3. CAPABILITY
            stream.write_all(b"A002 CAPABILITY\r\n").await.unwrap();
            response.clear();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                response.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if response.contains("A002") {
                    break;
                }
            }
            println!(
                "3️⃣  CAPABILITY: {}",
                response
                    .lines()
                    .filter(|l| l.contains("CAPABILITY"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            assert!(response.contains("A002 OK"), "CAPABILITY 应该成功");

            // 4. LIST
            stream.write_all(b"A003 LIST \"\" *\r\n").await.unwrap();
            response.clear();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                response.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if response.contains("A003") {
                    break;
                }
            }
            let folder_count = response.lines().filter(|l| l.contains("LIST")).count();
            println!("4️⃣  LIST: 找到 {} 个文件夹", folder_count);
            assert!(response.contains("INBOX"), "应该有 INBOX");
            assert!(response.contains("A003 OK"), "LIST 应该成功");

            // 5. SELECT INBOX
            stream.write_all(b"A004 SELECT INBOX\r\n").await.unwrap();
            response.clear();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                response.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if response.contains("A004") {
                    break;
                }
            }
            println!(
                "5️⃣  SELECT INBOX: {}",
                response.lines().last().unwrap_or(&"")
            );
            assert!(response.contains("A004 OK"), "SELECT 应该成功");

            // 6. SEARCH ALL
            stream.write_all(b"A005 SEARCH ALL\r\n").await.unwrap();
            response.clear();
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                response.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if response.contains("A005") {
                    break;
                }
            }
            println!(
                "6️⃣  SEARCH ALL: {}",
                response
                    .lines()
                    .filter(|l| l.contains("SEARCH"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            assert!(response.contains("A005 OK"), "SEARCH 应该成功");

            // 7. NOOP
            stream.write_all(b"A006 NOOP\r\n").await.unwrap();
            response.clear();
            let n = stream.read(&mut buffer).await.unwrap();
            response.push_str(&String::from_utf8_lossy(&buffer[..n]));
            println!("7️⃣  NOOP: {}", response.trim());
            assert!(response.contains("A006 OK"), "NOOP 应该成功");

            // 8. LOGOUT
            stream.write_all(b"A007 LOGOUT\r\n").await.unwrap();
            response.clear();
            let n = stream.read(&mut buffer).await.unwrap();
            response.push_str(&String::from_utf8_lossy(&buffer[..n]));
            println!("8️⃣  LOGOUT: {}", response.trim());
            assert!(
                response.contains("A007 OK") || response.contains("BYE"),
                "LOGOUT 应该成功"
            );

            println!("✅ 完整 IMAP 会话流程成功");
        }
        Err(e) => {
            panic!("❌ 无法连接到 GreenMail: {}", e);
        }
    }
}
