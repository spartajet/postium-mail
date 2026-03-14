use anyhow::{anyhow, Result};
use std::time::Instant;

/// IMAP 认证方法
pub enum ImapAuth {
    Password(String),
}

/// IMAP 客户端封装
pub struct ImapClient {
    session: Option<imap::Session<Box<dyn imap::ImapConnection>>>,
}

impl ImapClient {
    pub fn new() -> Self {
        Self { session: None }
    }

    /// 连接到 IMAP 服务器并登录
    pub fn connect(&mut self, host: &str, port: u16, email: &str, auth: ImapAuth) -> Result<()> {
        let start = Instant::now();

        let tls_connector = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| anyhow!("TLS 构建失败: {}", e))?;

        let client = imap::ClientBuilder::new(host, port)
            .connect()
            .map_err(|e| anyhow!("连接 IMAP 服务器失败: {}", e))?;

        let connect_time = start.elapsed().as_millis();

        let login_start = Instant::now();
        let session = match auth {
            ImapAuth::Password(password) => {
                client.login(email, &password)
                    .map_err(|(e, _)| anyhow!("IMAP 登录失败: {}", e))?
            }
        };

        let login_time = login_start.elapsed().as_millis();

        tracing::info!(
            "IMAP 连接成功: {} (连接: {}ms, 登录: {}ms)",
            email,
            connect_time,
            login_time
        );

        self.session = Some(session);
        Ok(())
    }

    pub fn session(&mut self) -> Result<&mut imap::Session<Box<dyn imap::ImapConnection>>> {
        self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))
    }

    pub fn select_folder(&mut self, folder: &str) -> Result<usize> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let count = session.search("ALL")
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?
            .len();

        Ok(count)
    }

    pub fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uids = session.search("ALL")
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        let mut uid_list: Vec<u32> = uids.iter().cloned().collect();
        uid_list.sort();
        uid_list.reverse();

        if uid_list.len() > limit {
            uid_list.truncate(limit);
        }

        Ok(uid_list)
    }

    pub fn fetch_email(&mut self, uid: u32, folder: &str) -> Result<EmailData> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();
        let messages = session.fetch(&uid_str, "(RFC822)")
            .map_err(|e| anyhow!("获取邮件失败: {}", e))?;

        let message = messages.iter().next()
            .ok_or_else(|| anyhow!("邮件不存在"))?;

        let body = message.body()
            .ok_or_else(|| anyhow!("邮件体为空"))?;

        let raw_email = std::str::from_utf8(body)
            .map_err(|e| anyhow!("解析邮件编码失败: {}", e))?;

        // 解析邮件头
        let (subject, from, to, date) = parse_email_headers(raw_email);

        // 提取正文（简化版本，在邮件头和正文之间查找分隔）
        let (body_text, body_html) = extract_body(raw_email);

        let flags = message.flags();

        Ok(EmailData {
            uid: uid as i32,
            subject,
            from,
            to,
            date,
            body_text,
            body_html,
            raw: raw_email.to_string(),
            flags: EmailFlags {
                seen: flags.contains(&imap::types::Flag::Seen),
                flagged: flags.contains(&imap::types::Flag::Flagged),
                answered: flags.contains(&imap::types::Flag::Answered),
                deleted: flags.contains(&imap::types::Flag::Deleted),
            },
        })
    }

    pub fn mark_as_read(&mut self, uid: u32, folder: &str, is_read: bool) -> Result<()> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();
        if is_read {
            session.store(&uid_str, "+FLAGS (\\Seen)")
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?;
        } else {
            session.store(&uid_str, "-FLAGS (\\Seen)")
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?;
        }
        Ok(())
    }

    pub fn set_flag(&mut self, uid: u32, folder: &str, flagged: bool) -> Result<()> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();
        if flagged {
            session.store(&uid_str, "+FLAGS (\\Flagged)")
                .map_err(|e| anyhow!("设置标志失败: {}", e))?;
        } else {
            session.store(&uid_str, "-FLAGS (\\Flagged)")
                .map_err(|e| anyhow!("设置标志失败: {}", e))?;
        }
        Ok(())
    }

    pub fn delete_email(&mut self, uid: u32, folder: &str) -> Result<()> {
        let session = self.session()?;
        session.select(folder)
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();
        session.store(&uid_str, "+FLAGS (\\Deleted)")
            .map_err(|e| anyhow!("标记删除失败: {}", e))?;

        session.expunge()
            .map_err(|e| anyhow!("删除邮件失败: {}", e))?;

        Ok(())
    }

    pub fn logout(&mut self) -> Result<()> {
        if let Some(mut session) = self.session.take() {
            session.logout()
                .map_err(|e| anyhow!("登出失败: {}", e))?;
        }
        Ok(())
    }
}

impl Default for ImapClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析邮件头
fn parse_email_headers(raw: &str) -> (String, String, String, chrono::DateTime<chrono::Utc>) {
    let mut subject = "无主题".to_string();
    let mut from = "".to_string();
    let mut to = "".to_string();
    let mut date = chrono::Utc::now();

    for line in raw.lines().take(100) {
        if line.starts_with("Subject:") {
            subject = line[8..].trim().to_string();
        } else if line.starts_with("From:") {
            from = extract_email(line[5..].trim());
        } else if line.starts_with("To:") {
            to = extract_emails(line[3..].trim());
        } else if line.starts_with("Date:") {
            let date_str = line[5..].trim();
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
                date = dt.with_timezone(&chrono::Utc);
            }
        }
    }

    (subject, from, to, date)
}

/// 提取单个邮箱地址
fn extract_email(s: &str) -> String {
    // 查找 <...> 中的邮箱
    if let Some(start) = s.find('<') {
        if let Some(end) = s.find('>') {
            return s[start + 1..end].to_string();
        }
    }
    s.to_string()
}

/// 提取多个邮箱地址
fn extract_emails(s: &str) -> String {
    s.split(',')
        .map(|email| extract_email(email.trim()))
        .filter(|e| !e.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 提取邮件正文（简化版）
fn extract_body(raw: &str) -> (String, String) {
    // 查找空行分隔邮件头和正文
    if let Some(pos) = raw.find("\n\n") {
        let body = &raw[pos + 2..];

        // 简单检测是否包含 HTML
        if body.contains("<html") || body.contains("<HTML") || body.contains("<body") {
            (String::new(), body.to_string())
        } else {
            (body.to_string(), String::new())
        }
    } else {
        (String::new(), String::new())
    }
}

/// 邮件数据
#[derive(Debug, Clone)]
pub struct EmailData {
    pub uid: i32,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub body_text: String,
    pub body_html: String,
    pub raw: String,
    pub flags: EmailFlags,
}

#[derive(Debug, Clone)]
pub struct EmailFlags {
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub deleted: bool,
}

/// IMAP 服务
pub struct ImapService {
    client: Option<ImapClient>,
}

impl ImapService {
    pub fn new() -> Self {
        Self { client: None }
    }

    pub fn connect(&mut self, host: &str, port: u16, email: &str, auth: ImapAuth) -> Result<()> {
        let mut client = ImapClient::new();
        client.connect(host, port, email, auth)?;
        self.client = Some(client);
        Ok(())
    }

    pub async fn sync_folder(
        &mut self,
        account_id: i32,
        db: &sea_orm::DbConn,
        folder: &str,
    ) -> Result<usize> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        let uids = client.list_uids(folder, 100)?;
        let mut synced_count = 0;

        for uid in uids {
            let exists = crate::services::email_service::email_exists_by_uid(db, account_id, uid as i32, folder).await;

            if !exists {
                match client.fetch_email(uid, folder) {
                    Ok(email_data) => {
                        if let Err(e) = crate::services::email_service::save_email_from_imap(
                            db,
                            account_id,
                            &email_data,
                            folder,
                        ).await {
                            tracing::warn!("保存邮件失败 (UID {}): {}", uid, e);
                        } else {
                            synced_count += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("获取邮件失败 (UID {}): {}", uid, e);
                    }
                }
            }
        }

        Ok(synced_count)
    }

    pub fn logout(&mut self) -> Result<()> {
        if let Some(client) = self.client.as_mut() {
            client.logout()?;
        }
        self.client = None;
        Ok(())
    }
}

impl Default for ImapService {
    fn default() -> Self {
        Self::new()
    }
}

/// 连接测试结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub connect_time: u64,
    pub login_time: u64,
    pub email_count: usize,
    pub error: Option<String>,
}

pub fn test_connection(
    host: &str,
    port: u16,
    email: &str,
    auth: ImapAuth,
) -> Result<ConnectionTestResult> {
    let start = Instant::now();
    let mut client = ImapClient::new();

    match client.connect(host, port, email, auth) {
        Ok(_) => {
            let total_time = start.elapsed();

            let email_count = match client.select_folder("INBOX") {
                Ok(count) => count,
                Err(e) => {
                    tracing::warn!("获取邮件数量失败: {}", e);
                    0
                }
            };

            let _ = client.logout();

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
