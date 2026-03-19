use super::{AsyncImapClient, ImapAuth};
use anyhow::Result;
use std::time::Instant;

/// 连接测试结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub connect_time: u64,
    pub login_time: u64,
    pub email_count: usize,
    pub error: Option<String>,
}

/// 测试 IMAP 连接
pub async fn test_connection(
    host: &str,
    port: u16,
    email: &str,
    auth: ImapAuth,
) -> Result<ConnectionTestResult> {
    let start = Instant::now();
    let mut client = AsyncImapClient::new();

    match client.connect(host, port, email, auth).await {
        Ok(_) => {
            let total_time = start.elapsed();

            // 尝试获取 INBOX 邮件数量
            let email_count = match client.list_uids("INBOX", 1).await {
                Ok(uids) => uids.len(),
                Err(e) => {
                    tracing::warn!("获取邮件数量失败: {}", e);
                    0
                }
            };

            let _ = client.logout().await;

            Ok(ConnectionTestResult {
                success: true,
                connect_time: total_time.as_millis() as u64,
                login_time: 0,
                email_count,
                error: None,
            })
        }
        Err(e) => {
            Ok(ConnectionTestResult {
                success: false,
                connect_time: start.elapsed().as_millis() as u64,
                login_time: 0,
                email_count: 0,
                error: Some(e.to_string()),
            })
        }
    }
}
