//! IMAP 工具函数模块
//!
//! 本模块提供 IMAP 连接和认证的辅助功能，包括：
//! - TCP/TLS 连接建立
//! - XOAUTH2 认证器实现
//!
//! 主要类型：
//! - `Xoauth2Authenticator`：XOAUTH2 认证器

use crate::error::MailError;
use base64::Engine;
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, time::timeout};

use super::ImapClient;

// ─── TLS 连接辅助方法 ───

impl ImapClient {
    // TCP 连接超时时间（秒）
    const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// 建立 IPv4 TCP 连接
    ///
    /// # 参数
    /// - `host`: 主机名
    /// - `port`: 端口号
    ///
    /// # 返回
    /// 成功时返回 TCP 流
    ///
    /// # 功能
    /// - 解析主机名为 IPv4 地址
    /// - 遍历所有 IPv4 地址尝试连接
    /// - 支持连接超时控制
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
    ///
    /// # 参数
    /// - `host`: 主机名
    /// - `port`: 端口号
    ///
    /// # 返回
    /// 成功时返回 TLS 流
    pub(crate) async fn connect_tls_stream(
        host: &str,
        port: u16,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let tcp = Self::connect_tcp_stream(host, port).await?;
        tracing::debug!(host, port, "IMAP连接成功: 正在进行 TLS 握手");
        Self::upgrade_tls(tcp, host).await
    }

    /// TCP 连接升级为 TLS
    ///
    /// # 参数
    /// - `tcp`: TCP 流
    /// - `host`: 主机名（用于 SNI）
    ///
    /// # 返回
    /// 成功时返回 TLS 流
    ///
    /// # 功能
    /// - 创建 TLS 连接器
    /// - 执行 TLS 握手
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

// ─── XOAUTH2 认证器 ───

/// XOAUTH2 认证器
///
/// 实现 async-imap 的 Authenticator trait，用于 OAuth2 认证。
///
/// # 格式
/// ```text
/// user={user}\x01auth=Bearer {access_token}\x01\x01
/// ```
#[derive(Debug)]
pub struct Xoauth2Authenticator {
    /// 用户邮箱地址
    pub user: String,
    /// OAuth2 访问令牌
    pub access_token: String,
}

impl async_imap::Authenticator for Xoauth2Authenticator {
    type Response = Vec<u8>;

    /// 生成 XOAUTH2 认证字符串
    ///
    /// # 参数
    /// - `_challenge`: 服务器挑战（XOAUTH2 不使用）
    ///
    /// # 返回
    /// 返回编码后的认证字符串字节
    ///
    /// # 格式
    /// `user={email}\x01auth=Bearer {token}\x01\x01`
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        let s = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        );
        s.into_bytes()
    }
}

impl Xoauth2Authenticator {
    /// 生成 Base64 编码的 XOAUTH2 字符串
    ///
    /// # 参数
    /// - `user`: 用户邮箱地址
    /// - `access_token`: OAuth2 访问令牌
    ///
    /// # 返回
    /// Base64 编码的 XOAUTH2 字符串
    ///
    /// # 注意
    /// 此方法标记为 `#[allow(dead_code)]`，实际认证时使用 `process` 方法
    #[allow(dead_code)]
    pub fn generate_xoauth2_string(&self, user: &str, access_token: &str) -> String {
        let s = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
        let s_bytes = s.into_bytes();
        base64::engine::general_purpose::STANDARD.encode(s_bytes)
    }
}
