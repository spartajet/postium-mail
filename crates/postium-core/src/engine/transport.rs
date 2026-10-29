//! 传输层抽象：引擎面向 `ImapSession` trait 编写，mock 会话可注入。

use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use async_trait::async_trait;

/// 已认证的 IMAP 会话（引擎视角的最小接口，按阶段扩展）。
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

    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        self.client.logout().await
    }
}
