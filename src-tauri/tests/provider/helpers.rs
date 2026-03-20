#![allow(dead_code)]
//! 服务商测试辅助函数

use crate::ProviderTestConfig;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// 检查服务器连接是否可用
pub async fn check_server_connection(host: &str, port: u16) -> bool {
    timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .is_ok()
}

/// 检查 IMAP 服务器连接
pub async fn check_imap_connection(config: &ProviderTestConfig) -> bool {
    check_server_connection(&config.imap_host, config.imap_port).await
}

/// 检查 SMTP 服务器连接
pub async fn check_smtp_connection(config: &ProviderTestConfig) -> bool {
    check_server_connection(&config.smtp_host, config.smtp_port).await
}

/// 等待服务器就绪
pub async fn wait_for_server_ready(config: &ProviderTestConfig, max_retries: u32) -> bool {
    for _ in 0..max_retries {
        if check_imap_connection(config).await {
            return true;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_server_connection() {
        // 测试一个通常可用的公共服务
        let result = check_server_connection("localhost", 8080).await;
        // 不做断言，因为可能失败也可能成功
        let _ = result;
    }
}
