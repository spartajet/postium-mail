//! SMTP 客户端实现
//!
//! 提供邮件发送功能，基于 `lettre` 库实现。
//!
//! # 核心功能
//!
//! - **TLS 支持**: STARTTLS (端口 587) 和隐式 SSL/TLS (端口 465)
//! - **认证方式**: 密码认证和 OAuth2/XOAUTH2
//! - **邮件构建**: 支持纯文本、HTML 和多部分邮件
//! - **附件支持**: MIME 多部分附件（待完整实现）
//! - **异步接口**: 使用 tokio spawn_blocking 包装 lettre 的同步 API
//!
//! # 与 lettre 库的关系
//!
//! 本模块是对 `lettre` 库的封装：
//!
//! | 功能 | lettre | SmtpClient |
//! |------|--------|------------|
//! | 核心协议 | ✅ | ✅ (使用 lettre) |
//! | 异步接口 | ❌ (同步) | ✅ (spawn_blocking) |
//! | 状态管理 | 手动 | 自动 |
//! | 错误处理 | lettre::error | SmtpError |
//! | 类型定义 | lettre | 自定义类型 |
//!
//! # 连接类型
//!
//! ## 端口 465 - SSL/TLS
//!
//! 直接使用 SSL/TLS 加密连接。
//!
//! ```rust,no_run
//! # use crate::protocols::smtp::{SmtpClient, SmtpAuth};
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = SmtpClient::new();
//! client.connect("smtp.gmail.com", 465, "user@gmail.com", SmtpAuth::Password("...".to_string())).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 端口 587 - STARTTLS
//!
//! 先使用普通连接，然后升级到 TLS。
//!
//! ```rust,no_run
//! # use crate::protocols::smtp::{SmtpClient, SmtpAuth};
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = SmtpClient::new();
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", SmtpAuth::Password("...".to_string())).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 其他端口
//!
//! 使用普通连接（不推荐，不安全）。
//!
//! # 使用示例
//!
//! ## 基本发送流程
//!
//! ```rust,no_run
//! use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut client = SmtpClient::new();
//!
//! // 1. 连接
//! let auth = SmtpAuth::Password("app_password".to_string());
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//!
//! // 2. 构建请求
//! let request = SendEmailRequest {
//!     from: "sender@example.com".to_string(),
//!     to: vec!["recipient@example.com".to_string()],
//!     cc: None,
//!     bcc: None,
//!     subject: "测试邮件".to_string(),
//!     html_body: "<h1>测试内容</h1>".to_string(),
//!     text_body: Some("测试内容".to_string()),
//!     attachments: vec![],
//! };
//!
//! // 3. 发送
//! let result = client.send_email(request).await?;
//! println!("邮件已发送: {}", result.message_id);
//! # Ok(())
//! # }
//! ```
//!
//! ## OAuth2 认证
//!
//! ```rust,no_run
//! # use crate::protocols::smtp::{SmtpClient, SmtpAuth};
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = SmtpClient::new();
//! let auth = SmtpAuth::OAuth2("ya29.a0AfH6...".to_string());
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # MIME 邮件结构
//!
//! ## 多部分邮件
//!
//! ```text
//! mixed (根容器)
//!   ├── alternative (正文选择)
//!   │   ├── text/plain (纯文本)
//!   │   └── text/html (HTML)
//!   └── attachment (附件 1)
//! ```
//!
//! - `alternative`: 客户端选择显示纯文本或 HTML
//! - `mixed`: 混合正文和附件
//!
//! # 常见问题
//!
//! ## 认证失败
//!
//! - 检查用户名和密码是否正确
//! - Gmail/Outlook 需要应用专用密码
//! - OAuth2 令牌可能已过期
//!
//! ## 连接超时
//!
//! - 检查网络连接
//! - 检查防火墙设置
//! - 验证服务器地址和端口
//!
//! ## 邮件被拒
//!
//! - 检查发件人地址是否有效
//! - 检查收件人地址是否有效
//! - 检查邮件内容是否触发垃圾邮件过滤
//!
//! # 限制说明
//!
//! - lettre 是同步库，使用 spawn_blocking 包装为异步
//! - 附件功能尚未完整实现（当前仅占位）
//! - 暂不支持自定义邮件头
//!
//! # 参考资料
//!
//! - [lettre 文档](https://docs.rs/lettre/)
//! - [RFC 5321 - SMTP](https://datatracker.ietf.org/doc/html/rfc5321)
//! - [RFC 6409 - Submission Port](https://datatracker.ietf.org/doc/html/rfc6409)

use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::auth::SmtpAuth;
use super::error::{SmtpError, SmtpResult};
use super::types::{SendEmailRequest, SendEmailResult};

/// SMTP 客户端
///
/// 基于 lettre 库的异步 SMTP 客户端封装。
///
/// # 设计目标
///
/// - 简化邮件发送流程
/// - 自动管理连接状态
/// - 支持常见的认证方式
/// - 提供异步接口
///
/// # 使用模式
///
/// ```text
/// 创建实例 → 连接服务器 → 发送邮件 → 断开连接
/// ```
///
/// # 注意事项
///
/// - 客户端内部持有连接，需要注意生命周期
/// - 连接可以复用发送多封邮件
/// - 建议在使用完毕后调用 `disconnect()` 清理资源
///
/// # 线程安全
///
/// 使用 Arc<Mutex<>> 包装内部状态，支持跨线程访问。
pub struct SmtpClient {
    mailer: Arc<Mutex<Option<SmtpTransport>>>,
    from_address: Arc<Mutex<String>>,
}

impl SmtpClient {
    /// 创建新的 SMTP 客户端
    ///
    /// # 返回
    ///
    /// 返回一个未连接的客户端实例。
    /// 需要调用 `connect()` 方法建立连接后才能发送邮件。
    pub fn new() -> Self {
        Self {
            mailer: Arc::new(Mutex::new(None)),
            from_address: Arc::new(Mutex::new(String::new())),
        }
    }

    /// 连接到 SMTP 服务器
    ///
    /// 建立到 SMTP 服务器的连接并执行认证。
    ///
    /// # 参数
    ///
    /// - `host`: SMTP 服务器地址（如 "smtp.gmail.com"）
    /// - `port`: SMTP 服务器端口
    ///   - `465`: SSL/TLS 加密连接
    ///   - `587`: STARTTLS（推荐）
    ///   - 其他: 普通连接（不推荐）
    /// - `username`: 用户名（通常是邮箱地址）
    /// - `auth`: 认证信息
    ///
    /// # 返回
    ///
    /// 成功时返回空值，失败时返回错误。
    ///
    /// # 错误
    ///
    /// - 网络连接失败
    /// - TLS 握手失败
    /// - 认证失败
    /// - 配置错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use crate::protocols::smtp::{SmtpClient, SmtpAuth};
    /// # async fn example() -> anyhow::Result<()> {
    /// # let mut client = SmtpClient::new();
    /// let auth = SmtpAuth::Password("app_password".to_string());
    /// client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(
        &self,
        host: &str,
        port: u16,
        username: &str,
        auth: SmtpAuth,
    ) -> SmtpResult<()> {
        let host = host.to_string();
        let username_clone = username.to_string();
        let auth = auth.clone();

        // 保留用于日志的克隆
        let host_for_log = host.clone();
        let username_for_log = username_clone.clone();

        // 在 spawn_blocking 中执行同步的 lettre 操作
        let transport = tokio::task::spawn_blocking(move || {
            // 构建邮件服务器配置
            let creds = match &auth {
                SmtpAuth::Password(password) => {
                    Some(Credentials::new(username_clone.clone(), password.clone()))
                }
                SmtpAuth::OAuth2(token) => {
                    // XOAUTH2 认证
                    use crate::providers::generate_xoauth2_string;
                    let xoauth2_str = generate_xoauth2_string(&username_clone, token);
                    Some(Credentials::new(username_clone, xoauth2_str))
                }
            };

            // 根据端口确定连接类型
            let transport = if port == 465 {
                // SSL/TLS 连接
                if let Some(creds) = creds {
                    SmtpTransport::relay(&host)
                        .map_err(|e| SmtpError::Connection(format!("构建 SMTP 传输失败: {}", e)))?
                        .credentials(creds)
                        .build()
                } else {
                    SmtpTransport::builder_dangerous(&host)
                        .port(465)
                        .build()
                }
            } else if port == 587 {
                // STARTTLS 连接
                if let Some(creds) = creds {
                    SmtpTransport::starttls_relay(&host)
                        .map_err(|e| SmtpError::Connection(format!("构建 SMTP 传输失败: {}", e)))?
                        .credentials(creds)
                        .build()
                } else {
                    return Err(SmtpError::Config("SMTP 587 端口需要认证".to_string()));
                }
            } else {
                // 普通连接（不推荐）
                if let Some(creds) = creds {
                    SmtpTransport::builder_dangerous(&host)
                        .port(port)
                        .credentials(creds)
                        .build()
                } else {
                    SmtpTransport::builder_dangerous(&host)
                        .port(port)
                        .build()
                }
            };

            // 测试连接
            transport
                .test_connection()
                .map_err(|e| SmtpError::Connection(format!("SMTP 连接测试失败: {}", e)))?;

            Ok::<SmtpTransport, SmtpError>(transport)
        })
        .await
        .map_err(|e| SmtpError::Connection(format!("连接任务失败: {}", e)))??;

        // 保存连接和发件人地址
        *self.mailer.lock().await = Some(transport);
        *self.from_address.lock().await = username_for_log;

        tracing::info!("SMTP 连接成功: {} (端口: {})", host_for_log, port);

        Ok(())
    }

    /// 发送邮件
    pub async fn send_email(&self, request: SendEmailRequest) -> SmtpResult<SendEmailResult> {
        // 解析发件人地址
        let from_mailbox: Mailbox = request
            .from
            .parse()
            .map_err(|e| SmtpError::BuildFailed(format!("发件人地址格式错误: {}", e)))?;

        // 更新发件人地址
        *self.from_address.lock().await = request.from.clone();

        // 构建邮件
        let mut email_builder = Message::builder()
            .from(from_mailbox)
            .subject(request.subject);

        // 添加收件人
        for to_addr in &request.to {
            let to_mailbox: Mailbox = to_addr
                .parse()
                .map_err(|e| SmtpError::BuildFailed(format!("收件人地址格式错误: {}", e)))?;
            email_builder = email_builder.to(to_mailbox);
        }

        // 添加抄送
        if let Some(cc) = &request.cc {
            for cc_addr in cc {
                if let Ok(cc_mailbox) = cc_addr.parse() {
                    email_builder = email_builder.cc(cc_mailbox);
                }
            }
        }

        // 添加密送
        if let Some(bcc) = &request.bcc {
            for bcc_addr in bcc {
                if let Ok(bcc_mailbox) = bcc_addr.parse() {
                    email_builder = email_builder.bcc(bcc_mailbox);
                }
            }
        }

        // 设置邮件内容
        let email = if let Some(text) = &request.text_body {
            email_builder
                .multipart(
                    lettre::message::MultiPart::mixed()
                        .multipart(
                            lettre::message::MultiPart::alternative()
                                .singlepart(
                                    lettre::message::SinglePart::builder()
                                        .header(ContentType::TEXT_PLAIN)
                                        .body(text.clone())
                                )
                                .singlepart(
                                    lettre::message::SinglePart::builder()
                                        .header(ContentType::TEXT_HTML)
                                        .body(request.html_body.clone())
                                )
                        )
                        .singlepart(
                            lettre::message::SinglePart::builder()
                                .header(ContentType::TEXT_PLAIN)
                                .body("Attachments not yet supported".to_string())
                        )
                )
                .map_err(|e| SmtpError::BuildFailed(format!("构建邮件失败: {}", e)))?
        } else {
            email_builder
                .singlepart(
                    lettre::message::SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(request.html_body)
                )
                .map_err(|e| SmtpError::BuildFailed(format!("构建邮件失败: {}", e)))?
        };

        // 获取 mailer 的克隆用于发送
        let mailer = self
            .mailer
            .lock()
            .await
            .as_ref()
            .ok_or(SmtpError::NotConnected)?
            .clone();

        // 在 spawn_blocking 中执行同步的 lettre 操作
        let response = tokio::task::spawn_blocking(move || {
            mailer
                .send(&email)
                .map_err(|e| SmtpError::SendFailed(format!("发送邮件失败: {}", e)))
        })
        .await
        .map_err(|e| SmtpError::SendFailed(format!("发送任务失败: {}", e)))??;

        // 生成 message-id
        let message_id = format!("<{}@postium.smtp>", chrono::Utc::now().timestamp_millis());

        tracing::info!("邮件发送成功: {:?}", response);

        Ok(SendEmailResult {
            message_id,
            sent_at: chrono::Utc::now(),
        })
    }

    /// 测试连接
    pub async fn test_connection(&self) -> SmtpResult<()> {
        let mailer = self
            .mailer
            .lock()
            .await
            .as_ref()
            .ok_or(SmtpError::NotConnected)?
            .clone();

        // 在 spawn_blocking 中执行同步的 lettre 操作
        tokio::task::spawn_blocking(move || {
            mailer
                .test_connection()
                .map_err(|e| SmtpError::Connection(format!("SMTP 测试连接失败: {}", e)))
        })
        .await
        .map_err(|e| SmtpError::Connection(format!("测试连接任务失败: {}", e)))??;

        Ok(())
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        self.mailer.lock().await.is_some()
    }

    /// 断开连接
    pub async fn disconnect(&self) {
        *self.mailer.lock().await = None;
        tracing::info!("SMTP 连接已断开");
    }
}

impl Default for SmtpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    #[tokio::test]
    async fn test_smtp_client_creation() {
        let client = SmtpClient::new();
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_smtp_client_not_connected() {
        let client = SmtpClient::new();
        let result = client.test_connection().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SmtpError::NotConnected));
    }
}
