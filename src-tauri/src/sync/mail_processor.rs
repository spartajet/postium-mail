//! 邮件处理器
//!
//! 处理邮件的下载、解析和存储

use crate::error::{MailError, Result};
use crate::storage::models::email;
use crate::sync::change_detector::EmailFlags;
use sea_orm::{ActiveModelTrait, DbConn, EntityTrait, Set};
use std::sync::Arc;
use tracing::instrument;

/// 邮件处理结果
#[derive(Debug, Clone)]
pub struct MailProcessResult {
    /// 成功处理的邮件数量
    pub success_count: usize,
    /// 失败的邮件数量
    pub failed_count: usize,
    /// 跳过的邮件数量（已存在）
    pub skipped_count: usize,
    /// 总处理数量
    pub total_count: usize,
}

/// 邮件数据（从 IMAP 获取）
#[derive(Debug, Clone)]
pub struct MailData {
    /// UID
    pub uid: u32,
    /// 文件夹名称（标准化为小写）
    pub folder: String,
    /// Message-ID
    pub message_id: Option<String>,
    /// 主题
    pub subject: Option<String>,
    /// 发送者名称
    pub sender_name: Option<String>,
    /// 发送者邮箱
    pub sender_email: String,
    /// 收件人邮箱列表（JSON 数组）
    pub recipient_emails: String,
    /// 抄送邮箱列表（JSON 数组，可选）
    pub cc_emails: Option<String>,
    /// 密送邮箱列表（JSON 数组，可选）
    pub bcc_emails: Option<String>,
    /// 纯文本正文
    pub body_text: Option<String>,
    /// HTML 正文
    pub body_html: Option<String>,
    /// 发送时间
    pub sent_at: i64,
    /// 接收时间
    pub received_at: i64,
    /// 标志状态
    pub flags: EmailFlags,
    /// MODSEQ（CONDSTORE，可选）
    pub modseq: Option<i64>,
}

/// 邮件处理器
///
/// 负责处理邮件的下载、解析和存储
pub struct MailProcessor {
    db: Arc<DbConn>,
}

/// 从 AsyncImapClient::EmailData 转换为 MailData
///
/// # 参数
///
/// * `email_data` - IMAP 客户端获取的邮件数据
/// * `folder` - IMAP 文件夹名称（将被标准化为小写）
///
/// # 返回
///
/// 返回 MailData
pub fn from_imap_email(email_data: &crate::protocols::imap::EmailData, folder: &str) -> MailData {
    use crate::sync::change_detector::EmailFlags;

    // 标准化文件夹名称
    let normalized_folder = normalize_folder_name(folder);

    MailData {
        uid: email_data.uid,
        folder: normalized_folder,
        message_id: None, // 需要从邮件头中提取
        subject: Some(email_data.subject.clone()),
        sender_name: extract_name_from_address(&email_data.from),
        sender_email: extract_email_from_address(&email_data.from),
        recipient_emails: serialize_addresses(&email_data.to),
        cc_emails: if email_data.cc.is_empty() {
            None
        } else {
            Some(serialize_addresses(&email_data.cc))
        },
        bcc_emails: None, // EmailData 没有包含 bcc
        body_text: Some(email_data.body_text.clone()),
        body_html: Some(email_data.body_html.clone()),
        sent_at: email_data.date.timestamp(),
        received_at: chrono::Utc::now().timestamp(),
        flags: EmailFlags {
            seen: email_data.flags.seen,
            flagged: email_data.flags.flagged,
            answered: email_data.flags.answered,
            draft: false, // EmailData 没有包含 draft 标志
            deleted: email_data.flags.deleted,
            recent: false, // EmailData 没有包含 recent 标志
        },
        modseq: None, // TODO: 从 CONDSTORE 响应中提取 MODSEQ
    }
}

/// 从地址字符串中提取名称
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回名称部分（如果有）
fn extract_name_from_address(address: &str) -> Option<String> {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<') {
        if let Some(end) = address.find('>') {
            let name_part = &address[..start].trim();
            if !name_part.is_empty() {
                return Some(name_part.to_string());
            }
        }
    }
    None
}

/// 标准化文件夹名称
///
/// 将 IMAP 文件夹名称映射为标准小写名称
///
/// # 参数
///
/// * `folder` - IMAP 文件夹名称（如 "INBOX", "Sent Messages", "已发送邮件" 等）
///
/// # 返回
///
/// 返回标准化的文件夹名称（如 "inbox", "sent", "drafts", "spam", "trash", "archive"）
fn normalize_folder_name(folder: &str) -> String {
    let folder_lower = folder.to_lowercase();

    // 常见文件夹名称映射
    if folder_lower == "inbox" {
        return "inbox".to_string();
    }

    // 已发送文件夹映射
    if folder_lower.contains("sent")
        || folder_lower.contains("已发送")
        || folder_lower.contains("发件箱")
        || folder_lower.contains("已发送邮件")
    {
        return "sent".to_string();
    }

    // 草稿文件夹映射
    if folder_lower.contains("draft")
        || folder_lower.contains("草稿")
        || folder_lower.contains("草稿箱")
    {
        return "drafts".to_string();
    }

    // 垃圾邮件/垃圾箱映射
    if folder_lower.contains("junk")
        || folder_lower.contains("spam")
        || folder_lower.contains("垃圾")
        || folder_lower.contains("垃圾邮件")
        || folder_lower.contains("垃圾箱")
    {
        return "spam".to_string();
    }

    // 已删除/废纸篓文件夹映射
    if folder_lower.contains("trash")
        || folder_lower.contains("deleted")
        || folder_lower.contains("已删除")
        || folder_lower.contains("废纸篓")
        || folder_lower.contains("删除")
    {
        return "trash".to_string();
    }

    // 归档文件夹映射
    if folder_lower.contains("archive")
        || folder_lower.contains("归档")
        || folder_lower.contains("存档")
    {
        return "archive".to_string();
    }

    // 默认返回原始名称的小写形式
    folder_lower
}

/// 从地址字符串中提取邮箱
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回邮箱地址
fn extract_email_from_address(address: &str) -> String {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<') {
        if let Some(end) = address.find('>') {
            return address[start + 1..end].to_string();
        }
    }
    address.to_string()
}

/// 序列化地址列表为 JSON 数组
///
/// # 参数
///
/// * `addresses` - 地址字符串（多个地址用逗号分隔）
///
/// # 返回
///
/// 返回 JSON 数组字符串
fn serialize_addresses(addresses: &str) -> String {
    if addresses.is_empty() {
        return "[]".to_string();
    }

    // 分割地址并提取邮箱部分
    let emails: Vec<String> = addresses
        .split(',')
        .map(|addr| extract_email_from_address(addr.trim()))
        .map(|email| format!("\"{}\"", email))
        .collect();

    format!("[{}]", emails.join(","))
}

impl MailProcessor {
    /// 创建新的邮件处理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 批量处理邮件
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `mails` - 邮件数据列表（注意：已跳过，避免记录大量数据）
    ///
    /// # 返回
    ///
    /// 返回处理结果
    #[instrument(
        skip(self, mails),
        fields(
            account_id,
            folder,
            mail_count = mails.len()
        )
    )]
    pub async fn process_mails(
        &self,
        account_id: i32,
        folder: &str,
        mails: Vec<MailData>,
    ) -> Result<MailProcessResult> {
        // 将 folder 转换为小写，确保与前端一致
        let folder = folder.to_lowercase();

        tracing::info!(
            "开始处理邮件: account_id={}, folder={}, count={}",
            account_id,
            folder,
            mails.len()
        );

        let mut success_count = 0;
        let mut failed_count = 0;
        let mut skipped_count = 0;

        for mail_data in mails {
            match self
                .process_single_mail(account_id, &folder, &mail_data)
                .await
            {
                Ok(true) => success_count += 1,
                Ok(false) => skipped_count += 1,
                Err(e) => {
                    tracing::error!("处理邮件失败: uid={}, error={}", mail_data.uid, e);
                    failed_count += 1;
                }
            }
        }

        let total_count = success_count + failed_count + skipped_count;

        tracing::info!(
            "邮件处理完成: account_id={}, folder={}, success={}, failed={}, skipped={}, total={}",
            account_id,
            folder,
            success_count,
            failed_count,
            skipped_count,
            total_count
        );

        Ok(MailProcessResult {
            success_count,
            failed_count,
            skipped_count,
            total_count,
        })
    }

    /// 处理单个邮件
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `mail_data` - 邮件数据
    ///
    /// # 返回
    ///
    /// 返回 `Ok(true)` 表示已插入/更新，`Ok(false)` 表示已存在（跳过）
    async fn process_single_mail(
        &self,
        account_id: i32,
        folder: &str,
        mail_data: &MailData,
    ) -> Result<bool> {
        tracing::debug!(
            "处理邮件: account_id={}, folder={}, uid={}, subject={:?}",
            account_id,
            folder,
            mail_data.uid,
            mail_data.subject
        );

        // 存储邮件到数据库（或更新已存在的邮件）
        self.save_mail_to_db(account_id, folder, mail_data).await
    }

    /// 存储邮件到数据库
    async fn save_mail_to_db(
        &self,
        account_id: i32,
        folder: &str,
        mail_data: &MailData,
    ) -> Result<bool> {
        use crate::storage::models::email::ActiveModel;

        // 检查邮件是否已存在
        let existing = self
            .check_mail_exists(account_id, folder, mail_data.uid)
            .await?;

        if existing {
            // 邮件已存在，更新标志和 MODSEQ
            self.update_mail_flags(account_id, folder, mail_data)
                .await?;
            Ok(false) // 返回 false 表示已存在（跳过插入）
        } else {
            // 插入新邮件
            let mail_active = ActiveModel {
                account_id: Set(account_id),
                folder: Set(folder.to_string()),
                uid: Set(Some(mail_data.uid as i32)),
                message_id: Set(mail_data.message_id.clone()),
                subject: Set(mail_data.subject.clone()),
                sender_name: Set(mail_data.sender_name.clone()),
                sender_email: Set(mail_data.sender_email.clone()),
                recipient_emails: Set(mail_data.recipient_emails.clone()),
                cc_emails: Set(mail_data.cc_emails.clone()),
                bcc_emails: Set(mail_data.bcc_emails.clone()),
                body_text: Set(mail_data.body_text.clone()),
                body_html: Set(mail_data.body_html.clone()),
                is_read: Set(mail_data.flags.seen),
                is_starred: Set(mail_data.flags.flagged),
                is_draft: Set(mail_data.flags.draft),
                sent_at: Set(mail_data.sent_at),
                received_at: Set(mail_data.received_at),
                ..Default::default()
            };

            mail_active
                .insert(self.db.as_ref())
                .await
                .map_err(|e| MailError::Internal(format!("插入邮件失败: {}", e)))?;

            tracing::debug!(
                "插入邮件: account_id={}, folder={}, uid={}",
                account_id,
                folder,
                mail_data.uid
            );

            Ok(true) // 返回 true 表示已插入
        }
    }

    /// 检查邮件是否已存在
    async fn check_mail_exists(&self, account_id: i32, folder: &str, uid: u32) -> Result<bool> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let mails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.eq(uid as i32))
            .all(self.db.as_ref())
            .await?;

        Ok(!mails.is_empty())
    }

    /// 更新邮件标志
    async fn update_mail_flags(
        &self,
        account_id: i32,
        folder: &str,
        mail_data: &MailData,
    ) -> Result<()> {
        use crate::storage::models::email::ActiveModel;
        use sea_orm::{ColumnTrait, QueryFilter};

        let mails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.eq(mail_data.uid as i32))
            .all(self.db.as_ref())
            .await?;

        if let Some(mail) = mails.first() {
            let mut mail_active: ActiveModel = mail.clone().into();
            mail_active.is_read = Set(mail_data.flags.seen);
            mail_active.is_starred = Set(mail_data.flags.flagged);
            mail_active.is_draft = Set(mail_data.flags.draft);
            mail_active.updated_at = Set(chrono::Utc::now().timestamp());

            mail_active
                .update(self.db.as_ref())
                .await
                .map_err(|e| MailError::Internal(format!("更新邮件标志失败: {}", e)))?;

            tracing::debug!(
                "更新邮件标志: account_id={}, folder={}, uid={}, seen={}, starred={}",
                account_id,
                folder,
                mail_data.uid,
                mail_data.flags.seen,
                mail_data.flags.flagged
            );
        }

        Ok(())
    }

    /// 删除邮件
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `uids` - 要删除的 UID 列表
    pub async fn delete_mails(&self, account_id: i32, folder: &str, uids: &[u32]) -> Result<usize> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let uid_i32: Vec<i32> = uids.iter().map(|&uid| uid as i32).collect();

        let result = email::Entity::delete_many()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.is_in(uid_i32))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("删除邮件失败: {}", e)))?;

        let deleted_count = result.rows_affected;

        tracing::info!(
            "删除邮件: account_id={}, folder={}, count={}",
            account_id,
            folder,
            deleted_count
        );

        Ok(deleted_count as usize)
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

    #[test]
    fn test_mail_process_result_default() {
        let result = MailProcessResult {
            success_count: 10,
            failed_count: 2,
            skipped_count: 5,
            total_count: 17,
        };

        assert_eq!(result.success_count, 10);
        assert_eq!(result.failed_count, 2);
        assert_eq!(result.skipped_count, 5);
        assert_eq!(result.total_count, 17);
    }

    #[test]
    fn test_mail_data_creation() {
        let flags = EmailFlags {
            seen: true,
            flagged: false,
            answered: false,
            draft: false,
            deleted: false,
            recent: false,
        };

        let mail_data = MailData {
            uid: 12345,
            folder: "inbox".to_string(),
            message_id: Some("<test@example.com>".to_string()),
            subject: Some("Test Email".to_string()),
            sender_name: Some("John Doe".to_string()),
            sender_email: "john@example.com".to_string(),
            recipient_emails: "[]".to_string(),
            cc_emails: None,
            bcc_emails: None,
            body_text: Some("Test body".to_string()),
            body_html: None,
            sent_at: chrono::Utc::now().timestamp(),
            received_at: chrono::Utc::now().timestamp(),
            flags,
            modseq: None,
        };

        assert_eq!(mail_data.uid, 12345);
        assert_eq!(mail_data.subject, Some("Test Email".to_string()));
        assert!(mail_data.flags.seen);
    }
}
