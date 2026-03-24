//! IMAP 服务层
//!
//! 提供对 IMAP 客户端的高级封装，简化常见操作。
//!
//! # 功能
//!
//! - 连接管理
//! - 文件夹操作（列表、元数据获取）
//! - 邮件操作（获取、搜索、保存）
//! - 邮件标志操作（已读、星标、删除）
//!
//! # 与 AsyncImapClient 的区别
//!
//! `ImapService` 是 `AsyncImapClient` 的高级封装：
//! - 提供更简单的 API
//! - 集成数据库操作
//! - 处理连接状态管理
//! - 提供 IDLE 支持
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use postium_mail_lib::protocols::imap::service::ImapService;
//! use postium_mail_lib::protocols::imap::ImapAuth;
//!
//! let mut service = ImapService::new();
//!
//! // 连接
//! service.connect("imap.gmail.com", 993, "user@gmail.com", auth).await?;
//!
//! // 列出文件夹
//! let folders = service.list_folders_with_attributes().await?;
//!
//! // 获取邮件
//! let email = service.fetch_email("INBOX", 123).await?;
//!
//! // 断开连接
//! service.logout().await?;
//! ```

use super::{AsyncImapClient, EmailData, FolderInfo, FolderMetadata, ImapAuth};
use anyhow::{anyhow, Result};
use sea_orm::DbConn;
use crate::storage;

/// IMAP 服务
///
/// 高级 IMAP 客户端封装，提供简化的 API 和额外的功能。
///
/// # 设计目标
///
/// - 简化常见 IMAP 操作
/// - 自动管理连接状态
/// - 集成数据库操作
/// - 提供更好的错误处理
///
/// # 使用模式
///
/// ```rust,no_run
/// use postium_mail_lib::protocols::imap::service::ImapService;
///
/// let mut service = ImapService::new();
///
/// // 1. 连接
/// service.connect(host, port, email, auth).await?;
///
/// // 2. 执行操作
/// let folders = service.list_folders_with_attributes().await?;
///
/// // 3. 断开（可选）
/// service.logout().await?;
/// ```
///
/// # 注意
///
/// - 服务内部持有客户端连接，需要注意生命周期
/// - 建议在使用完毕后调用 `logout()` 清理资源
/// - 如果服务被销毁，连接会自动关闭
pub struct ImapService {
    /// 内部 IMAP 客户端
    client: Option<AsyncImapClient>,
}

impl ImapService {
    /// 创建新的 IMAP 服务实例
    ///
    /// # 返回
    ///
    /// 返回一个未连接的服务实例。
    /// 需要调用 `connect()` 方法建立连接后才能执行操作。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use postium_mail_lib::protocols::imap::service::ImapService;
    ///
    /// let service = ImapService::new();
    /// service.connect(...).await?;
    /// ```
    pub fn new() -> Self {
        Self { client: None }
    }

    /// 异步连接到 IMAP 服务器
    ///
    /// 建立到 IMAP 服务器的连接并执行认证。
    ///
    /// # 参数
    ///
    /// - `host`: IMAP 服务器地址（如 "imap.gmail.com"）
    /// - `port`: IMAP 服务器端口（通常 993 for SSL/TLS）
    /// - `email`: 邮箱地址
    /// - `auth`: 认证信息（密码或 OAuth 令牌）
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
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use postium_mail_lib::protocols::imap::service::ImapService;
    /// use postium_mail_lib::protocols::imap::ImapAuth;
    ///
    /// let auth = ImapAuth::Password("app_password".to_string());
    /// service.connect("imap.gmail.com", 993, "user@gmail.com", auth).await?;
    /// ```
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

    /// 异步列出文件夹及属性
    ///
    /// 获取账号中的所有文件夹，包括 RFC 6154 特殊用途属性。
    ///
    /// # 返回
    ///
    /// 返回文件夹信息列表，每项包含：
    /// - 文件夹名称
    /// - 特殊用途属性（如 \Sent, \Trash）
    /// - 标准化名称
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use postium_mail_lib::protocols::imap::service::ImapService;
    ///
    /// let folders = service.list_folders_with_attributes().await?;
    /// for folder in folders {
    ///     println!("{}: {}", folder.name, folder.standard_name);
    /// }
    /// ```
    pub async fn list_folders_with_attributes(&mut self) -> Result<Vec<FolderInfo>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_folders_with_attributes().await
    }

    /// 获取文件夹 IMAP 元数据
    ///
    /// 获取文件夹的 IMAP 协议元数据，用于增量同步。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    ///
    /// 返回文件夹元数据，包括：
    /// - `uidvalidity`: UIDVALIDITY 值
    /// - `uidnext`: 下一个 UID
    /// - `exists`: 邮件数量
    /// - `recent`: 最近邮件数量
    /// - `unseen`: 未读邮件数量
    ///
    /// # 使用场景
    ///
    /// - 增量同步前获取当前状态
    /// - 检测文件夹是否发生变化
    /// - 验证 UID 有效性
    pub async fn fetch_folder_metadata(&mut self, folder: &str) -> Result<FolderMetadata> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.fetch_folder_metadata(folder).await
    }

    /// 异步获取 UID 列表
    ///
    /// 获取文件夹中的邮件 UID 列表，按 UID 倒序排列。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `limit`: 返回的最大 UID 数量
    ///
    /// # 返回
    ///
    /// 返回 UID 列表，最大的 UID 在前面。
    ///
    /// # 使用场景
    ///
    /// - 获取最近的邮件
    /// - 分页加载邮件列表
    pub async fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids(folder, limit).await
    }

    /// 获取大于指定 UID 的邮件列表
    ///
    /// 用于增量同步，获取自上次同步以来的新邮件。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `min_uid`: 最小 UID（不包含）
    ///
    /// # 返回
    ///
    /// 返回 UID 列表，所有 UID 都大于 `min_uid`。
    ///
    /// # 使用场景
    ///
    /// - 增量同步新邮件
    /// - 基于 UID 的变更检测
    pub async fn list_uids_after(&mut self, folder: &str, min_uid: u32) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids_after(folder, min_uid).await
    }

    /// 获取指定时间之后的邮件列表
    ///
    /// 使用 IMAP SEARCH SINCE 命令获取指定时间后的邮件。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `date_since`: IMAP 日期格式（如 "01-Jan-2025"）
    ///
    /// # 返回
    ///
    /// 返回符合条件的邮件 UID 列表。
    ///
    /// # 日期格式
    ///
    /// IMAP 使用特定的日期格式：`DD-Mon-YYYY`
    /// - `01-Jan-2025`
    /// - `15-Feb-2024`
    ///
    /// # 使用场景
    ///
    /// - 同步近期邮件（如最近一年）
    /// - 避免同步所有历史邮件
    pub async fn list_uids_since(&mut self, folder: &str, date_since: &str) -> Result<Vec<u32>> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.list_uids_since(folder, date_since).await
    }

    /// 异步获取邮件
    ///
    /// 从指定文件夹获取完整的邮件数据。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `uid`: 邮件 UID
    ///
    /// # 返回
    ///
    /// 返回完整的邮件数据，包括：
    /// - 邮件头信息
    /// - 邮件正文（纯文本和 HTML）
    /// - 邮件标志
    /// - 附件列表
    /// - 原始邮件源码
    ///
    /// # 性能注意
    ///
    /// 此操作会下载完整的邮件内容，可能数据量较大。
    /// 如果只需要邮件头，考虑使用其他方法。
    pub async fn fetch_email(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;

        client.fetch_email(folder, uid).await
    }

    /// 异步通过 UID 获取邮件
    ///
    /// 此方法是 `fetch_email` 的别名，提供更明确的命名。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `uid`: 邮件 UID
    ///
    /// # 返回
    ///
    /// 返回完整的邮件数据。
    pub async fn fetch_email_by_uid(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.fetch_email(folder, uid).await
    }

    /// 保存邮件到数据库
    ///
    /// 将 IMAP 邮件数据解析并保存到数据库。
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `account_id`: 账号 ID
    /// - `folder`: 文件夹名称
    /// - `email_data`: IMAP 邮件数据
    ///
    /// # 返回
    ///
    /// 返回保存后的邮件数据库 ID。
    ///
    /// # 处理逻辑
    ///
    /// 1. 解析收件人列表
    /// 2. 解析发件人信息
    /// 3. 转换时间戳
    /// 4. 调用 `EmailRepository::save_email_from_imap` 保存
    ///
    /// # 注意
    ///
    /// - 收件人列表以 JSON 格式存储
    /// - 发件人名称和邮箱分开存储
    /// - 时间戳转换为 Unix 时间戳
    pub async fn save_email(
        &mut self,
        db: &DbConn,
        account_id: i32,
        folder: &str,
        email_data: &EmailData,
    ) -> Result<i32> {
        use crate::storage::service::email::EmailAddress;

        // 解析收件人列表
        let recipients: Vec<EmailAddress> = email_data.to
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(EmailAddress {
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
    ///
    /// 查询数据库中是否已存在指定 UID 的邮件。
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `account_id`: 账号 ID
    /// - `uid`: 邮件 UID
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    ///
    /// - `true`: 邮件已存在
    /// - `false`: 邮件不存在
    ///
    /// # 使用场景
    ///
    /// - 避免重复保存邮件
    /// - 检测新邮件
    /// - 增量同步
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
    ///
    /// 修改邮件的 `\Seen` 标志。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `uid`: 邮件 UID
    /// - `is_read`: true 设为已读，false 设为未读
    ///
    /// # 返回
    ///
    /// 成功时返回空值，失败时返回错误。
    ///
    /// # IMAP 命令
    ///
    /// 此方法执行 IMAP STORE 命令：
    /// ```text
    /// UID STORE uid +FLAGS (\Seen)    # 标记为未读
    /// UID STORE uid -FLAGS (\Seen)    # 标记为已读
    /// ```
    ///
    /// # 注意
    ///
    /// - 操作会同步到服务器
    /// - 多设备同步时会看到状态变更
    pub async fn mark_as_read(&mut self, folder: &str, uid: u32, is_read: bool) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.mark_as_read(folder, uid, is_read).await
    }

    /// 设置星标
    ///
    /// 修改邮件的 `\Flagged` 标志。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `uid`: 邮件 UID
    /// - `flagged`: true 添加星标，false 移除星标
    ///
    /// # 返回
    ///
    /// 成功时返回空值，失败时返回错误。
    ///
    /// # IMAP 命令
    ///
    /// 此方法执行 IMAP STORE 命令：
    /// ```text
    /// UID STORE uid +FLAGS (\Flagged)  # 添加星标
    /// UID STORE uid -FLAGS (\Flagged)  # 移除星标
    /// ```
    pub async fn set_flag(&mut self, folder: &str, uid: u32, flagged: bool) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.set_flag(folder, uid, flagged).await
    }

    /// 删除邮件
    ///
    /// 将邮件标记为删除（添加 `\Deleted` 标志）。
    ///
    /// # 参数
    ///
    /// - `folder`: 文件夹名称
    /// - `uid`: 邮件 UID
    ///
    /// # 返回
    ///
    /// 成功时返回空值，失败时返回错误。
    ///
    /// # 注意
    ///
    /// - 此操作只添加 `\Deleted` 标志，不会立即删除邮件
    /// - 需要执行 EXPUNGE 命令才会真正删除邮件
    /// - 许多邮件客户端在关闭连接时自动执行 EXPUNGE
    pub async fn delete_email(&mut self, folder: &str, uid: u32) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| anyhow!("IMAP 未连接"))?;
        client.delete_email(folder, uid).await
    }

    /// 登出
    ///
    /// 向 IMAP 服务器发送 LOGOUT 命令并关闭连接。
    ///
    /// # 返回
    ///
    /// 成功时返回空值，失败时返回错误。
    ///
    /// # 清理操作
    ///
    /// 1. 发送 IMAP LOGOUT 命令
    /// 2. 服务器关闭连接
    /// 3. 清理客户端状态
    ///
    /// # 注意
    ///
    /// - 如果连接已断开，此操作不会报错
    /// - 登出后需要重新连接才能执行其他操作
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
