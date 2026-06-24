use crate::domain::providers::ImapServerConfig;
use crate::error::MailError;
use crate::infrastructure::protocols::types::FolderMetadata;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use specta::Type;
use utf7_imap::decode_utf7_imap;

// ─── 内部模块 ───
pub mod fetch;
pub mod parser;
pub mod search;
pub mod util;

pub use util::Xoauth2Authenticator;

// ─── DTO ───

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RawEmailHeader {
    pub uid: u32,
    pub flags: Vec<String>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MailboxInfo {
    pub exists: u32,
    pub recent: u32,
    pub unseen: Option<u32>,
    pub uid_validity: Option<u32>,
    pub uid_next: Option<u32>,
}

/// IMAP 客户端 — 封装 async-imap，支持 TLS
#[derive(Debug)]
pub struct ImapClient {
    session: async_imap::Session<tokio_native_tls::TlsStream<tokio::net::TcpStream>>,
}

impl ImapClient {
    // ─── 连接 ───

    /// 建立 IMAP 连接并登录
    pub async fn connect(
        config: &ImapServerConfig,
        email: &str,
        password: &str,
    ) -> Result<Self, MailError> {
        let host = &config.host;
        let port = config.port;

        tracing::info!(host, port, email, "IMAP: 正在连接");

        let tcp = Self::connect_tcp_stream(host, port).await?;

        tracing::debug!(host, "IMAP: TCP 连接成功，开始 TLS 握手");
        let tls = Self::upgrade_tls(tcp, host).await?;
        let mut client = async_imap::Client::new(tls);

        let _greeting = client.read_response().await.map_err(|e| {
            MailError::ImapConnectionFailed(format!("读取 IMAP greeting 失败: {e}"))
        })?;

        tracing::debug!(email, "IMAP: 正在登录");
        let session = client
            .login(email, password)
            .await
            .map_err(|(e, _)| MailError::AuthFailed(format!("登录失败: {e}")))?;

        tracing::info!(host, email, "IMAP: 连接并登录成功");
        Ok(Self { session })
    }

    /// 使用 XOAUTH2 登录 (Gmail 等)
    pub async fn connect_xoauth2(
        config: &ImapServerConfig,
        email: &str,
        access_token: &str,
    ) -> Result<Self, MailError> {
        tracing::info!(host = %config.host, port = config.port, email, "IMAP: 使用 XOAUTH2 连接");
        let tls = Self::connect_tls_stream(&config.host, config.port).await?;
        let mut client = async_imap::Client::new(tls);
        let _greeting = client.read_response().await.map_err(|e| {
            MailError::ImapConnectionFailed(format!("读取 IMAP greeting 失败: {e}"))
        })?;
        tracing::debug!(email, "IMAP: 正在进行 XOAUTH2 认证");
        let authenticator = util::Xoauth2Authenticator {
            user: email.to_string(),
            access_token: access_token.to_string(),
        };
        let session = client
            .authenticate("XOAUTH2", authenticator)
            .await
            .map_err(|(e, _)| MailError::AuthFailed(format!("XOAUTH2 认证失败: {e}")))?;
        tracing::info!(email, "IMAP: XOAUTH2 认证成功");
        Ok(Self { session })
    }

    // ─── 文件夹操作 ───

    /// 获取文件夹列表
    pub async fn list_folders(&mut self) -> Result<Vec<FolderInfo>, MailError> {
        tracing::debug!("IMAP: 列出文件夹");
        let mut list = self
            .session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("列文件夹失败: {e}")))?;

        let mut folders = Vec::new();
        while let Some(item) = list.next().await {
            let item = item
                .map_err(|e| MailError::ImapConnectionFailed(format!("解析文件夹项失败: {e}")))?;
            folders.push(FolderInfo {
                name: item.name().to_string(),
                delimiter: item.delimiter().map(|s: &str| s.to_string()),
                flags: item.attributes().iter().map(|f| format!("{f:?}")).collect(),
            });
        }
        tracing::debug!(count = folders.len(), "IMAP: 列出文件夹完成");
        Ok(folders)
    }

    /// 选择文件夹
    pub async fn select_folder(&mut self, folder: &str) -> Result<MailboxInfo, MailError> {
        tracing::debug!(folder, "IMAP: 选择文件夹");
        let mailbox =
            self.session.select(folder).await.map_err(|e| {
                MailError::FolderNotFound(format!("选择文件夹 '{folder}' 失败: {e}"))
            })?;

        let info = MailboxInfo {
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: Option::map(mailbox.unseen, |v| v),
            uid_validity: mailbox.uid_validity,
            uid_next: mailbox.uid_next,
        };
        tracing::debug!(folder, exists = info.exists, recent = info.recent, unseen = ?info.unseen, "IMAP: 文件夹已选择");
        Ok(info)
    }

    /// 获取文件夹 IMAP 元数据（UIDVALIDITY, UIDNEXT 等）
    /// 使用 EXAMINE 命令（只读模式）获取正确的 UIDNEXT 值
    pub async fn fetch_folder_metadata(
        &mut self,
        folder: &str,
    ) -> Result<FolderMetadata, MailError> {
        let mailbox = self
            .session
            .examine(folder)
            .await
            .map_err(|e| MailError::ImapFolderMetadataFailed(e.to_string()))?;

        tracing::info!(
            "EXAMINE 返回的 Mailbox 结构: folder={}, exists={}, recent={}, uid_validity={:?}, uid_next={:?}, highest_modseq={:?}",
            folder,
            mailbox.exists,
            mailbox.recent,
            mailbox.uid_validity,
            mailbox.uid_next,
            mailbox.highest_modseq
        );

        let uidvalidity = mailbox.uid_validity.map(|v| v as u64).unwrap_or(1);
        let uidnext = mailbox.uid_next.map(|v| v as u64).unwrap_or(1);

        tracing::info!(
            "文件夹元数据: folder={}, uidvalidity={}, uidnext={}, exists={}",
            folder,
            uidvalidity,
            uidnext,
            mailbox.exists
        );

        let nick_name = decode_utf7_imap(folder.to_string());

        Ok(FolderMetadata {
            uidvalidity,
            uidnext,
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: None,
            nick_name,
        })
    }

    // ─── 标志操作 ───

    /// 设置邮件标志
    pub async fn set_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        let uid_str = uid.to_string();
        let mut stream = self
            .session
            .uid_store(&uid_str, flags)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("设置标志失败: {e}")))?;
        while stream.next().await.is_some() {}
        Ok(())
    }

    /// 添加标志
    pub async fn add_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        self.set_flags(uid, &format!("+FLAGS ({flags})")).await
    }

    /// 移除标志
    pub async fn remove_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        self.set_flags(uid, &format!("-FLAGS ({flags})")).await
    }

    // ─── 连接管理 ───

    /// 登出
    pub async fn logout(mut self) -> Result<(), MailError> {
        self.session
            .logout()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("登出失败: {e}")))?;
        Ok(())
    }

    /// 获取能力列表
    pub async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        let caps = self
            .session
            .capabilities()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取能力列表失败: {e}")))?;
        Ok(caps.iter().map(|c| format!("{c:?}")).collect())
    }
}
