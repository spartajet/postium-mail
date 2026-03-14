use anyhow::{anyhow, Result};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};

/// SMTP 认证方法
pub enum SmtpAuth {
    Password(String),
    OAuth2(String),
}

/// SMTP 客户端封装
pub struct SmtpClient {
    mailer: Option<SmtpTransport>,
    from_address: String,
}

impl SmtpClient {
    /// 创建新的 SMTP 客户端
    pub fn new() -> Self {
        Self {
            mailer: None,
            from_address: String::new(),
        }
    }

    /// 连接到 SMTP 服务器
    pub fn connect(&mut self, host: &str, port: u16, username: &str, auth: SmtpAuth) -> Result<()> {
        // 构建邮件服务器配置
        let creds = match auth {
            SmtpAuth::Password(password) => {
                Some(Credentials::new(username.to_string(), password))
            }
            SmtpAuth::OAuth2(_token) => {
                // OAuth2 认证需要特殊处理，暂时使用占位符
                tracing::warn!("SMTP OAuth2 认证尚未完全实现");
                return Err(anyhow!("SMTP OAuth2 认证暂未支持"));
            }
        };

        // 根据端口确定连接类型
        let transport = if port == 465 {
            // SSL/TLS 连接
            if let Some(creds) = creds {
                SmtpTransport::relay(host)
                    .map_err(|e| anyhow!("构建 SMTP 传输失败: {}", e))?
                    .credentials(creds)
                    .build()
            } else {
                SmtpTransport::builder_dangerous(host)
                    .port(465)
                    .build()
            }
        } else if port == 587 {
            // STARTTLS 连接
            if let Some(creds) = creds {
                SmtpTransport::starttls_relay(host)
                    .map_err(|e| anyhow!("构建 SMTP 传输失败: {}", e))?
                    .credentials(creds)
                    .build()
            } else {
                return Err(anyhow!("SMTP 587 端口需要认证"));
            }
        } else {
            // 普通连接（不推荐）
            if let Some(creds) = creds {
                SmtpTransport::builder_dangerous(host)
                    .port(port)
                    .credentials(creds)
                    .build()
            } else {
                SmtpTransport::builder_dangerous(host)
                    .port(port)
                    .build()
            }
        };

        // 测试连接
        transport.test_connection()
            .map_err(|e| anyhow!("SMTP 连接测试失败: {}", e))?;

        self.mailer = Some(transport);
        self.from_address = username.to_string();

        tracing::info!("SMTP 连接成功: {} (端口: {})", host, port);

        Ok(())
    }

    /// 发送邮件
    pub fn send_email(
        &mut self,
        to: Vec<String>,
        subject: &str,
        html_body: &str,
        text_body: Option<&str>,
    ) -> Result<String> {
        let mailer = self.mailer.as_ref()
            .ok_or_else(|| anyhow!("SMTP 未连接"))?;

        // 解析发件人地址
        let from_mailbox: Mailbox = self.from_address.parse()
            .map_err(|e| anyhow!("发件人地址格式错误: {}", e))?;

        // 构建邮件
        let mut email_builder = Message::builder()
            .from(from_mailbox)
            .subject(subject.to_string());

        // 添加收件人
        for to_addr in &to {
            let to_mailbox: Mailbox = to_addr.parse()
                .map_err(|e| anyhow!("收件人地址格式错误: {}", e))?;
            email_builder = email_builder.to(to_mailbox);
        }

        // 设置邮件内容 - lettre 需要 String 而不是 &str
        let email = if let Some(text) = text_body {
            email_builder
                .multipart(
                    lettre::message::MultiPart::mixed()
                        .multipart(
                            lettre::message::MultiPart::alternative()
                                .singlepart(
                                    lettre::message::SinglePart::builder()
                                        .header(ContentType::TEXT_PLAIN)
                                        .body(text.to_string())
                                )
                                .singlepart(
                                    lettre::message::SinglePart::builder()
                                        .header(ContentType::TEXT_HTML)
                                        .body(html_body.to_string())
                                )
                        )
                )
                .map_err(|e| anyhow!("构建邮件失败: {}", e))?
        } else {
            email_builder
                .singlepart(
                    lettre::message::SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body.to_string())
                )
                .map_err(|e| anyhow!("构建邮件失败: {}", e))?
        };

        // 发送邮件
        let response = mailer.send(&email)
            .map_err(|e| anyhow!("发送邮件失败: {}", e))?;

        // 生成 message-id（lettre 不直接返回）
        let message_id = format!("<{}@postium.smtp>", chrono::Utc::now().timestamp_millis());

        tracing::info!("邮件发送成功: {:?}", response);

        Ok(message_id)
    }

    /// 测试连接
    pub fn test_connection(&self) -> Result<()> {
        let mailer = self.mailer.as_ref()
            .ok_or_else(|| anyhow!("SMTP 未连接"))?;

        mailer.test_connection()
            .map_err(|e| anyhow!("SMTP 测试连接失败: {}", e))?;

        Ok(())
    }
}

impl Default for SmtpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// SMTP 服务（高层封装）
pub struct SmtpService {
    client: Option<SmtpClient>,
}

impl SmtpService {
    pub fn new() -> Self {
        Self { client: None }
    }

    /// 连接到 SMTP 服务器
    pub fn connect(&mut self, host: &str, port: u16, username: &str, auth: SmtpAuth) -> Result<()> {
        let mut client = SmtpClient::new();
        client.connect(host, port, username, auth)?;
        self.client = Some(client);
        Ok(())
    }

    /// 发送邮件
    pub fn send_email(
        &mut self,
        from: &str,
        to: Vec<String>,
        subject: &str,
        html_body: &str,
        text_body: Option<&str>,
    ) -> Result<String> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("SMTP 未连接"))?;

        // 更新发件人地址
        client.from_address = from.to_string();

        client.send_email(to, subject, html_body, text_body)
    }
}

impl Default for SmtpService {
    fn default() -> Self {
        Self::new()
    }
}
