use super::{AsyncImapClient, EmailData, FolderInfo, FolderMetadata, ImapAuth};
use anyhow::{anyhow, Result};
use sea_orm::DbConn;
use crate::storage;

/// IMAP 服务
pub struct ImapService {
    client: Option<AsyncImapClient>,
}

impl ImapService {
    pub fn new() -> Self {
        Self { client: None }
    }

    /// 异步连接
    pub async fn connect(
        &mut self,
        host: &str,
        port: u16,
        email: &str,
        auth: ImapAuth,
    ) -> Result<()> {
        let mut client = AsyncImapClient::new();
        client.connect(host, port, email, auth).await?;
        self.client = Some(client);
        Ok(())
    }

    /// 异步列出文件夹
    pub async fn list_folders_with_attributes(&mut self) -> Result<Vec<FolderInfo>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_folders_with_attributes().await
    }

    /// 获取文件夹 IMAP 元数据（UIDVALIDITY, UIDNEXT 等）
    pub async fn fetch_folder_metadata(&mut self, folder: &str) -> Result<FolderMetadata> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.fetch_folder_metadata(folder).await
    }

    /// 异步获取 UID 列表
    pub async fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids(folder, limit).await
    }

    /// 获取大于指定 UID 的邮件列表（用于增量同步）
    pub async fn list_uids_after(&mut self, folder: &str, min_uid: u32) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids_after(folder, min_uid).await
    }

    /// 获取指定时间范围内的邮件列表（用于同步近一年的邮件）
    /// date_since: IMAP 日期格式，如 "01-Jan-2025"
    pub async fn list_uids_since(&mut self, folder: &str, date_since: &str) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids_since(folder, date_since).await
    }

    /// 异步获取邮件
    pub async fn fetch_email(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;

        client.fetch_email(folder, uid).await
    }

    /// 异步通过 UID 获取邮件
    pub async fn fetch_email_by_uid(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.fetch_email(folder, uid).await
    }

    /// 保存邮件到数据库
    pub async fn save_email(
        &mut self,
        db: &DbConn,
        account_id: i32,
        folder: &str,
        email_data: &EmailData,
    ) -> Result<i32> {
        use crate::models::email;

        // 解析收件人列表
        let recipients: Vec<email::EmailAddress> = email_data.to
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(email::EmailAddress {
                        name: None,
                        email: s.to_string(),
                    })
                }
            })
            .collect();

        let recipient_emails = serde_json::to_string(&recipients)
            .unwrap_or_default();

        // 解析发件人
        let sender_name = None;
        let sender_email = email_data.from.clone();

        // 时间戳转换
        let timestamp = email_data.date.timestamp();

        storage::EmailRepository::save_email_from_imap(
            db, account_id, folder, email_data.uid as i32,
            Some(email_data.subject.clone()),
            sender_name,
            sender_email,
            recipient_emails,
            Some(email_data.body_text.clone()),
            Some(email_data.body_html.clone()),
            timestamp,
            timestamp,
        )
        .await
        .map_err(|e| anyhow!("保存邮件失败: {}", e))
    }

    /// 检查邮件是否已存在（通过 UID）
    pub async fn email_exists_by_uid(
        &mut self,
        db: &DbConn,
        account_id: i32,
        uid: i32,
        folder: &str,
    ) -> bool {
        storage::EmailRepository::email_exists_by_uid(db, account_id, folder, uid)
            .await
            .unwrap_or(false)
    }

    /// 标记邮件为已读/未读
    pub async fn mark_as_read(&mut self, folder: &str, uid: u32, is_read: bool) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.mark_as_read(folder, uid, is_read).await
    }

    /// 设置星标
    pub async fn set_flag(&mut self, folder: &str, uid: u32, flagged: bool) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.set_flag(folder, uid, flagged).await
    }

    /// 删除邮件
    pub async fn delete_email(&mut self, folder: &str, uid: u32) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.delete_email(folder, uid).await
    }

    /// 登出
    pub async fn logout(&mut self) -> Result<()> {
        if let Some(client) = self.client.as_mut() {
            client.logout().await?;
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
