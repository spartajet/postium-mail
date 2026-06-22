use crate::error::MailError;
use base64::Engine;

use super::ImapClient;

// ─── TLS 辅助方法 ───

impl ImapClient {
    /// 建立 TCP+TLS 连接
    pub(crate) async fn connect_tls_stream(
        host: &str,
        port: u16,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let tcp = tokio::net::TcpStream::connect((host, port))
            .await
            .map_err(|e| {
                MailError::ImapConnectionFailed(format!("连接 {host}:{port} 失败: {e}"))
            })?;
        tracing::debug!(host, port, "IMAP连接成功: 正在进行 TLS 握手");
        Self::upgrade_tls(tcp, host).await
    }

    /// TCP → TLS 升级
    pub(crate) async fn upgrade_tls(
        tcp: tokio::net::TcpStream,
        host: &str,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let connector = tokio_native_tls::native_tls::TlsConnector::new()
            .map_err(|e| MailError::ImapConnectionFailed(format!("TLS 构建失败: {e}")))?;
        let connector = tokio_native_tls::TlsConnector::from(connector);
        connector
            .connect(host, tcp)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("TLS 握手失败: {e}")))
    }
}

// ─── XOAUTH2 Authenticator ───

#[derive(Debug)]
pub struct Xoauth2Authenticator {
    pub user: String,
    pub access_token: String,
}

impl async_imap::Authenticator for Xoauth2Authenticator {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        let s = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        );
        s.into_bytes()
    }
}

impl Xoauth2Authenticator {
    #[allow(dead_code)]
    pub fn generate_xoauth2_string(&self, user: &str, access_token: &str) -> String {
        let s = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
        let s_bytes = s.into_bytes();
        base64::engine::general_purpose::STANDARD.encode(s_bytes)
    }
}
