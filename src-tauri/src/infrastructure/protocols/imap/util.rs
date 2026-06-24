use crate::error::MailError;
use base64::Engine;
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, time::timeout};

use super::ImapClient;

// ─── TLS 辅助方法 ───

impl ImapClient {
    const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// 建立 IPv4 TCP 连接。
    pub(crate) async fn connect_tcp_stream(host: &str, port: u16) -> Result<TcpStream, MailError> {
        let addrs = tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("解析 {host}:{port} 失败: {e}")))?
            .filter(SocketAddr::is_ipv4)
            .collect::<Vec<SocketAddr>>();

        if addrs.is_empty() {
            return Err(MailError::ImapConnectionFailed(format!(
                "解析 {host}:{port} 未返回可用 IPv4 地址"
            )));
        }

        let mut last_error = None;
        for addr in addrs {
            match timeout(Self::TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                Ok(Ok(tcp)) => {
                    tracing::debug!(host, port, %addr, "IMAP: TCP 连接成功");
                    return Ok(tcp);
                }
                Ok(Err(error)) => {
                    tracing::debug!(host, port, %addr, %error, "IMAP: IPv4 TCP 连接失败，尝试下一个地址");
                    last_error = Some(error.to_string());
                }
                Err(_) => {
                    tracing::debug!(host, port, %addr, "IMAP: IPv4 TCP 连接超时，尝试下一个地址");
                    last_error = Some(format!(
                        "连接地址 {addr} 超时（{} 秒）",
                        Self::TCP_CONNECT_TIMEOUT.as_secs()
                    ));
                }
            }
        }

        Err(MailError::ImapConnectionFailed(format!(
            "连接 {host}:{port} 失败: {}",
            last_error.unwrap_or_else(|| "没有可用 IPv4 地址".to_string())
        )))
    }

    /// 建立 TCP+TLS 连接
    pub(crate) async fn connect_tls_stream(
        host: &str,
        port: u16,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let tcp = Self::connect_tcp_stream(host, port).await?;
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
