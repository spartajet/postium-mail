//! 传输层抽象：引擎面向 `ImapSession` trait 编写，mock 会话可注入。

use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::protocols::types::{FetchedBodySection, WholeEmailDto};
use async_trait::async_trait;

/// 已认证的 IMAP 会话（引擎视角的最小接口，按阶段扩展）。
///
/// 除 `capabilities`/`enable`/`logout` 外，操作类方法均自带"先选文件夹"语义
/// （folder 参数），命令失败即整连接可疑 → 由连接池重建重试。
#[async_trait]
pub trait ImapSession: Send {
    /// 服务器能力列表（原始响应行）。
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError>;
    /// 发送 ENABLE 命令（服务器不支持时忽略错误）。
    async fn enable(&mut self, caps: &[&str]) -> Result<(), MailError>;
    /// 选择文件夹（可写模式），返回消息数等状态。
    async fn select_folder(
        &mut self,
        folder: &str,
    ) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError>;
    /// 获取邮件正文原文（BODY.PEEK[]）。
    async fn fetch_body(&mut self, folder: &str, uid: u32) -> Result<String, MailError>;
    /// 移动邮件到目标文件夹（服务器支持 MOVE 扩展时用 MOVE，否则 COPY+\Deleted）。
    async fn move_uid(&mut self, folder: &str, uid: u32, to_folder: &str) -> Result<(), MailError>;
    /// 设置 flags（替换语义：最终 flags = flags）。
    async fn set_flags(
        &mut self,
        folder: &str,
        uid: u32,
        flags: Vec<String>,
    ) -> Result<(), MailError>;
    /// 添加单条 flag（STORE +FLAGS，不动其他标志）。
    async fn add_flags(&mut self, folder: &str, uid: u32, flag: &str) -> Result<(), MailError>;
    /// 移除单条 flag（STORE -FLAGS，不动其他标志）。
    async fn remove_flags(&mut self, folder: &str, uid: u32, flag: &str) -> Result<(), MailError>;
    /// 追加邮件到文件夹。
    ///
    /// `message_id` 传入时追加后按 Message-ID SEARCH 确认新 UID（找不到报
    /// `ImapSearchFailed`，与旧草稿路径行为一致）；未传时返回 0（APPEND
    /// 响应不回带 UID，UIDPLUS 的 APPENDUID 在 async-imap 0.11 不可得）。
    async fn append(
        &mut self,
        folder: &str,
        email: &[u8],
        flags: &[String],
        message_id: Option<&str>,
    ) -> Result<u32, MailError>;
    /// 删除邮件（\Deleted + EXPUNGE；EXPUNGE 会一并清除该文件夹内所有
    /// \Deleted 邮件——async-imap 0.11 无 UID EXPUNGE）。
    async fn delete(&mut self, folder: &str, uid: u32) -> Result<(), MailError>;
    /// 按 UID 拉取一封完整邮件（含正文与附件元信息），邮件不存在返回 None。
    async fn fetch_email_by_uid(
        &mut self,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError>;
    /// 下载指定 MIME section 的附件原始字节，邮件/section 不存在返回 None。
    async fn fetch_body_section(
        &mut self,
        folder: &str,
        uid: u32,
        section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError>;
    /// 登出并关闭会话。
    async fn logout(self: Box<Self>) -> Result<(), MailError>;
}

/// 真实会话：包装现有 `ImapClient`。
pub struct RealSession {
    client: ImapClient,
}

impl RealSession {
    pub fn new(client: ImapClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ImapSession for RealSession {
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        self.client.capabilities().await
    }

    async fn enable(&mut self, caps: &[&str]) -> Result<(), MailError> {
        if caps.is_empty() {
            return Ok(());
        }
        // async-imap 通过 run_command_and_check_ok 转发；失败仅记录
        // （服务器不支持 ENABLE 不影响降级路径）。
        let cmd = format!("ENABLE {}", caps.join(" "));
        let _ = self.client.run_command_and_check_ok(cmd.as_str()).await;
        Ok(())
    }

    async fn select_folder(
        &mut self,
        folder: &str,
    ) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError> {
        self.client.select_folder(folder).await
    }

    async fn fetch_body(&mut self, folder: &str, uid: u32) -> Result<String, MailError> {
        self.client.fetch_body_raw(folder, uid).await
    }

    async fn move_uid(&mut self, folder: &str, uid: u32, to_folder: &str) -> Result<(), MailError> {
        self.client.select_folder(folder).await?;
        self.client.move_uid_to_folder(uid, to_folder).await
    }

    async fn set_flags(
        &mut self,
        folder: &str,
        uid: u32,
        flags: Vec<String>,
    ) -> Result<(), MailError> {
        self.client.select_folder(folder).await?;
        self.client
            .set_flags(uid, &format!("FLAGS ({})", flags.join(" ")))
            .await
    }

    async fn add_flags(&mut self, folder: &str, uid: u32, flag: &str) -> Result<(), MailError> {
        self.client.select_folder(folder).await?;
        self.client.add_flags(uid, flag).await
    }

    async fn remove_flags(&mut self, folder: &str, uid: u32, flag: &str) -> Result<(), MailError> {
        self.client.select_folder(folder).await?;
        self.client.remove_flags(uid, flag).await
    }

    async fn append(
        &mut self,
        folder: &str,
        email: &[u8],
        flags: &[String],
        message_id: Option<&str>,
    ) -> Result<u32, MailError> {
        let flags_arg = if flags.is_empty() {
            None
        } else {
            Some(format!("({})", flags.join(" ")))
        };
        self.client
            .append_email_with_flags(folder, flags_arg.as_deref(), email)
            .await?;
        match message_id {
            Some(mid) => self
                .client
                .find_uid_by_message_id(folder, mid)
                .await?
                .ok_or_else(|| {
                    MailError::ImapSearchFailed(format!("追加邮件后无法确认远端 UID: {mid}"))
                }),
            None => Ok(0),
        }
    }

    async fn delete(&mut self, folder: &str, uid: u32) -> Result<(), MailError> {
        self.client.select_folder(folder).await?;
        self.client.delete_uid(uid).await?;
        self.client.expunge().await
    }

    async fn fetch_email_by_uid(
        &mut self,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        self.client.fetch_email_by_uid(folder, uid).await
    }

    async fn fetch_body_section(
        &mut self,
        folder: &str,
        uid: u32,
        section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError> {
        self.client
            .fetch_body_section_with_mime(folder, uid, section_path)
            .await
    }

    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        self.client.logout().await
    }
}
