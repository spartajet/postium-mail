use super::{AsyncImapClient, EmailData, FolderInfo, ImapAuth};
use anyhow::{anyhow, Result};
use sea_orm::DbConn;

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
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_folders_with_attributes().await
    }

    /// 异步获取 UID 列表
    pub async fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids(folder, limit).await
    }

    /// 获取大于指定 UID 的邮件列表（用于增量同步）
    pub async fn list_uids_after(&mut self, folder: &str, min_uid: u32) -> Result<Vec<u32>> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids_after(folder, min_uid).await
    }

    /// 异步获取邮件
    pub async fn fetch_email(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.fetch_email(folder, uid).await
    }

    /// 保存邮件到数据库
    pub async fn save_email(
        &mut self,
        db: &DbConn,
        account_id: i32,
        folder: &str,
        _uid: u32,  // UID 已包含在 email_data 中
        email_data: &EmailData,
    ) -> Result<i32> {
        crate::services::email_service::save_email_from_imap(
            db,
            account_id,
            email_data,
            folder,
        ).await
    }

    /// 检查邮件是否已存在（通过 UID）
    pub async fn email_exists_by_uid(
        &mut self,
        db: &DbConn,
        account_id: i32,
        uid: i32,
        folder: &str,
    ) -> bool {
        crate::services::email_service::email_exists_by_uid(db, account_id, uid, folder).await
    }

    /// 标记邮件为已读/未读
    pub async fn mark_as_read(&mut self, folder: &str, uid: u32, is_read: bool) -> Result<()> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.mark_as_read(folder, uid, is_read).await
    }

    /// 设置星标
    pub async fn set_flag(&mut self, folder: &str, uid: u32, flagged: bool) -> Result<()> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.set_flag(folder, uid, flagged).await
    }

    /// 删除邮件
    pub async fn delete_email(&mut self, folder: &str, uid: u32) -> Result<()> {
        let client = self.client.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;
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
