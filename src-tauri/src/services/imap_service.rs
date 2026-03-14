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

    /// 列出服务器上的所有文件夹
    pub fn list_folders(&mut self) -> Result<Vec<String>> {
        let session = self.session()?;

        let folders = session.list(None, Some(""))
            .map_err(|e| anyhow!("列出文件夹失败: {}", e))?;

        let mut folder_names = Vec::new();
        for folder in folders.iter() {
            // 获取文件夹名称（name() 直接返回 &str）
            let name_str = folder.name().to_string();
            // 过滤掉系统文件夹，只保留用户文件夹
            if !name_str.starts_with('.') && !name_str.contains('/') {
                folder_names.push(name_str);
            }
        }

        Ok(folder_names)
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

/// 解析邮件头（使用 mail_parser 库解码 MIME 编码）
fn parse_email_headers(raw: &str) -> (String, String, String, chrono::DateTime<chrono::Utc>) {
    use mail_parser::MessageParser;

    let message = MessageParser::default().parse(raw.as_bytes());

    // 使用 mail_parser 解码主题（支持 MIME 编码）
    let subject = message
        .as_ref()
        .and_then(|m| m.subject())
        .unwrap_or("无主题")
        .to_string();

    // From 和 To 使用简单的字符串解析（mail_parser 的地址解析可能返回 None）
    let mut from = "".to_string();
    let mut to = "".to_string();
    let mut date = chrono::Utc::now();

    // 回退到简单解析
    for line in raw.lines().take(100) {
        if line.starts_with("From:") && from.is_empty() {
            from = extract_email(line[5..].trim());
        } else if line.starts_with("To:") && to.is_empty() {
            to = extract_emails(line[3..].trim());
        } else if line.starts_with("Date:") {
            let date_str = line[5..].trim();
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
                date = dt.with_timezone(&chrono::Utc);
                break; // 找到日期后就退出
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

/// 提取邮件正文（使用 mail_parser 库）
fn extract_body(raw: &str) -> (String, String) {
    use mail_parser::MessageParser;

    // 使用 mail-parser 解析邮件
    let message = MessageParser::default().parse(raw.as_bytes());

    if let Some(message) = message {
        // 获取纯文本正文（索引 0 表示第一个正文部分）
        let body_text = message
            .body_text(0)
            .map(|s| s.to_string())
            .unwrap_or_default();

        // 获取 HTML 正文
        let body_html = message
            .body_html(0)
            .map(|s| s.to_string())
            .unwrap_or_default();

        (body_text, body_html)
    } else {
        // 解析失败，返回空字符串
        tracing::warn!("邮件解析失败，使用空正文");
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

    /// 带进度回调的同步方法
    pub async fn sync_folder_with_progress(
        &mut self,
        account_id: i32,
        db: &sea_orm::DbConn,
        folder: &str,
        progress_callback: Box<dyn Fn(usize, usize, String) + Send + Sync>,
    ) -> Result<usize> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        let uids = client.list_uids(folder, 100)?;
        let total = uids.len();
        let mut synced_count = 0;

        // 发送初始进度
        progress_callback(0, total, format!("准备同步 {} 封邮件...", total));

        for (i, uid) in uids.iter().enumerate() {
            let exists = crate::services::email_service::email_exists_by_uid(db, account_id, *uid as i32, folder).await;

            if !exists {
                match client.fetch_email(*uid, folder) {
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

            // 发送进度更新
            let current = i + 1;
            progress_callback(current, total, format!("已同步 {}/{} 封邮件", current, total));
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

    /// 将 IMAP 文件夹名称映射到应用分类
    fn map_folder_name(imap_folder: &str) -> String {
        match imap_folder.to_uppercase().as_str() {
            "INBOX" => "INBOX".to_string(),
            "SENT" | "SENT ITEMS" | "SENT MAIL" | "SENT-MESSAGES" | "SENDEN" => "sent".to_string(),
            "DRAFT" | "DRAFTS" | "ENTWURFE" => "drafts".to_string(),
            "TRASH" | "DELETED" | "DELETED ITEMS" | "GELÖSCHTE" | "PAPER" => "trash".to_string(),
            "SPAM" | "JUNK" | "JUNK E-MAIL" | "POSTINI" => "spam".to_string(),
            "ARCHIVE" | "ARCHIVES" => "archive".to_string(),
            // 其他文件夹直接使用原名称
            _ => imap_folder.to_string(),
        }
    }

    /// 克隆进度回调
    fn clone_progress_callback<'a>(
        callback: &'a Box<dyn Fn(usize, usize, String) + Send + Sync>,
    ) -> Box<dyn Fn(usize, usize, String) + Send + Sync + 'a> {
        // 这里我们无法直接克隆 Fn，所以创建一个新的回调
        Box::new(move |current: usize, total: usize, message: String| {
            callback(current, total, message)
        })
    }

    /// 智能查找目标文件夹的 IMAP 名称
    fn find_folder_name(server_folders: &[String], target: &str) -> Option<String> {
        let target_upper = target.to_uppercase();

        for folder in server_folders {
            let folder_upper = folder.to_uppercase();

            match target {
                "sent" => {
                    if folder_upper.contains("SENT") || folder_upper.contains("SENDEN") {
                        return Some(folder.clone());
                    }
                }
                "drafts" => {
                    if folder_upper.contains("DRAFT") {
                        return Some(folder.clone());
                    }
                }
                "spam" | "trash" => {
                    if folder_upper.contains(target)
                        || folder_upper.contains("JUNK") && target == "spam"
                        || (folder_upper.contains("DELETED") || folder_upper.contains("TRASH") || folder_upper.contains("GELÖSCHTE")) && target == "trash" {
                        return Some(folder.clone());
                    }
                }
                _ => {
                    if folder_upper == target_upper {
                        return Some(folder.clone());
                    }
                }
            }
        }

        None
    }

    /// 同步多个文件夹
    pub async fn sync_multiple_folders(
        &mut self,
        account_id: i32,
        db: &sea_orm::DbConn,
        progress_callback: Box<dyn Fn(usize, usize, String) + Send + Sync>,
    ) -> Result<usize> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 先列出服务器上的所有文件夹
        let server_folders = client.list_folders()?;
        tracing::debug!("服务器上的文件夹: {:?}", server_folders);

        let mut total_synced = 0;

        // INBOX 特殊处理（几乎所有邮箱都有）
        progress_callback(0, 100, "正在同步 INBOX...".to_string());
        match client.select_folder("INBOX") {
            Ok(count) => {
                if count > 0 {
                    let uids = client.list_uids("INBOX", 100)?;
                    // 使用统一的小写文件夹名称 "inbox"
                    let synced = sync_uids(client, account_id, db, "inbox", "INBOX", &uids, &progress_callback, total_synced).await?;
                    total_synced += synced;
                    tracing::info!("INBOX 同步完成，同步了 {} 封邮件", synced);
                }
            }
            Err(e) => {
                tracing::warn!("无法访问 INBOX: {}", e);
            }
        }

        // 其他常用文件夹
        let other_targets = vec!["sent", "drafts", "spam", "trash"];

        for target in other_targets {
            if let Some(folder_name) = Self::find_folder_name(&server_folders, target) {
                match client.select_folder(&folder_name) {
                    Ok(count) => {
                        if count > 0 {
                            progress_callback(
                                total_synced,
                                999, // 未知总数
                                format!("正在同步 {}...", target),
                            );

                            let uids = client.list_uids(&folder_name, 50)?;
                            // 使用统一的小写文件夹名称 (target) 而不是 IMAP 文件夹名称
                            let synced = sync_uids(client, account_id, db, target, &folder_name, &uids, &progress_callback, total_synced).await?;
                            total_synced += synced;
                            tracing::info!("{} ({}) 同步完成，同步了 {} 封邮件", target, folder_name, synced);
                        }
                    }
                    Err(e) => {
                        tracing::debug!("跳过文件夹 {}: {}", folder_name, e);
                    }
                }
            } else {
                tracing::debug!("未找到文件夹: {}", target);
            }
        }

        progress_callback(total_synced, total_synced, "同步完成".to_string());
        Ok(total_synced)
    }
}

/// 同步 UID 列表的辅助函数
/// db_folder: 保存到数据库的统一文件夹名称（如 "inbox", "sent" 等）
/// imap_folder: IMAP 服务器上的实际文件夹名称（如 "INBOX", "Sent Items" 等）
async fn sync_uids(
    client: &mut ImapClient,
    account_id: i32,
    db: &sea_orm::DbConn,
    db_folder: &str,
    imap_folder: &str,
    uids: &[u32],
    progress_callback: &Box<dyn Fn(usize, usize, String) + Send + Sync>,
    base_count: usize,
) -> Result<usize> {
    let mut synced_count = 0;

    for (i, uid) in uids.iter().enumerate() {
        // 使用 db_folder 检查邮件是否存在
        let exists = crate::services::email_service::email_exists_by_uid(db, account_id, *uid as i32, db_folder).await;

        if !exists {
            // 使用 imap_folder 从 IMAP 获取邮件
            match client.fetch_email(*uid, imap_folder) {
                Ok(email_data) => {
                    // 使用 db_folder 保存到数据库
                    if let Err(e) = crate::services::email_service::save_email_from_imap(
                        db,
                        account_id,
                        &email_data,
                        db_folder,
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

        // 发送进度更新
        let current = base_count + synced_count + i + 1;
        progress_callback(current, 999, format!("已同步 {} 封邮件", current));
    }

    Ok(synced_count)
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
