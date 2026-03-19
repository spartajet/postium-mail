//! SMTP 客户端实现
//!
//! 提供邮件发送功能，支持密码和 OAuth2 认证

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
pub struct SmtpClient {
    mailer: Arc<Mutex<Option<SmtpTransport>>>,
    from_address: Arc<Mutex<String>>,
}

impl SmtpClient {
    /// 创建新的 SMTP 客户端
    pub fn new() -> Self {
        Self {
            mailer: Arc::new(Mutex::new(None)),
            from_address: Arc::new(Mutex::new(String::new())),
        }
    }

    /// 连接到 SMTP 服务器
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
            .ok_or_else(|| SmtpError::NotConnected)?
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
            .ok_or_else(|| SmtpError::NotConnected)?
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
