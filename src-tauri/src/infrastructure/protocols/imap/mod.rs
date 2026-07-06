//! IMAP 协议实现模块
//!
//! 本模块提供 IMAP 协议的完整实现，包括：
//! - 连接管理（TLS、XOAUTH2 认证）
//! - 文件夹操作（LIST、SELECT、EXAMINE）
//! - 邮件获取（FETCH 命令）
//! - 邮件搜索（SEARCH 命令）
//! - 邮件标志操作（STORE、MOVE）
//!
//! 主要类型：
//! - `ImapClient`：IMAP 客户端封装
//! - `FolderInfo`：文件夹信息
//! - `RawEmailHeader`：邮件头部摘要
//! - `MailboxInfo`：邮箱状态信息

use crate::domain::folders::SpecialUseFlag;
use crate::domain::providers::ImapServerConfig;
use crate::error::MailError;
use crate::infrastructure::protocols::types::FolderMetadata;
use async_imap::imap_proto::NameAttribute;
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

/// IMAP 文件夹信息（LIST 命令返回）
///
/// 包含文件夹的名称、分隔符、属性和特殊用途标记。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderInfo {
    /// 文件夹名称（可能包含 UTF-7 编码）
    pub name: String,
    /// 层级分隔符（如 "/" 或 "."）
    pub delimiter: Option<String>,
    /// 文件夹属性标志列表（如 \HasChildren）
    pub flags: Vec<String>,
    /// RFC 6154 SPECIAL-USE 标记（从 attributes 解析）
    #[serde(default)]
    pub special_use: Vec<SpecialUseFlag>,
    /// 是否 \NoSelect/\NonExistent（不可选文件夹）
    #[serde(default)]
    pub no_select: bool,
}

/// 邮件头部摘要（UID FETCH 返回的轻量信息）
///
/// 包含邮件的基本信息，用于列表展示和快速预览。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RawEmailHeader {
    /// IMAP UID
    pub uid: u32,
    /// 邮件标志列表
    pub flags: Vec<String>,
    /// 邮件主题
    pub subject: Option<String>,
    /// 发件人
    pub from: Option<String>,
    /// 收件人
    pub to: Option<String>,
    /// 邮件日期
    pub date: Option<String>,
    /// RFC 2822 Message-ID
    pub message_id: Option<String>,
}

/// 选中文件夹后的邮箱状态信息（SELECT/EXAMINE 命令返回）
///
/// 包含文件夹的邮件计数和 UID 相关信息。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MailboxInfo {
    /// 文件夹中的邮件总数
    pub exists: u32,
    /// 最近到达的邮件数
    pub recent: u32,
    /// 未读邮件数（服务器支持时）
    pub unseen: Option<u32>,
    /// UIDVALIDITY 值
    pub uid_validity: Option<u32>,
    /// 预期的下一个 UID
    pub uid_next: Option<u32>,
}

/// IMAP 客户端
///
/// 封装 async-imap 库，提供 TLS 连接和各种 IMAP 操作。
/// 支持普通密码登录和 XOAUTH2 认证。
#[derive(Debug)]
pub struct ImapClient {
    session: async_imap::Session<tokio_native_tls::TlsStream<tokio::net::TcpStream>>,
}

impl ImapClient {
    // ─── 连接管理 ───

    /// 建立 IMAP 连接并使用密码登录
    ///
    /// # 参数
    /// - `config`: IMAP 服务器配置
    /// - `email`: 邮箱地址
    /// - `password`: 密码
    ///
    /// # 返回
    /// 成功时返回已认证的 ImapClient 实例
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

    /// 使用 XOAUTH2 认证连接 IMAP 服务器
    ///
    /// # 参数
    /// - `config`: IMAP 服务器配置
    /// - `email`: 邮箱地址
    /// - `access_token`: OAuth2 访问令牌
    ///
    /// # 返回
    /// 成功时返回已认证的 ImapClient 实例
    ///
    /// # 适用场景
    /// Gmail、Outlook 等 OAuth2 认证的服务
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
    ///
    /// # 返回
    /// 成功时返回文件夹信息列表
    ///
    /// # 功能
    /// - 使用 IMAP LIST 命令获取所有文件夹
    /// - 解析 RFC 6154 SPECIAL-USE 标记（\All、\Archive、\Drafts、\Flagged、\Junk、\Sent、\Trash、\Important）
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
            folders.push({
                let attrs = item.attributes();
                let special_use = SpecialUseFlag::from_attributes(attrs);
                let no_select = attrs.iter().any(|a| matches!(a, NameAttribute::NoSelect));
                FolderInfo {
                    name: item.name().to_string(),
                    delimiter: item.delimiter().map(|s: &str| s.to_string()),
                    flags: attrs.iter().map(|f| format!("{f:?}")).collect(),
                    special_use,
                    no_select,
                }
            });
        }
        tracing::debug!(count = folders.len(), "IMAP: 列出文件夹完成");
        Ok(folders)
    }

    /// 选择文件夹（可写模式）
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    /// 成功时返回邮箱状态信息
    ///
    /// # 功能
    /// - 使用 IMAP SELECT 命令
    /// - 使文件夹处于可操作状态（可修改标志）
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

    /// 获取文件夹 IMAP 元数据
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    /// 成功时返回文件夹元数据
    ///
    /// # 功能
    /// - 使用 IMAP EXAMINE 命令（只读模式）获取正确的 UIDNEXT 值
    /// - 返回 UIDVALIDITY、UIDNEXT、邮件数量等信息
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

    // ─── 邮件标志操作 ───

    /// 设置邮件标志
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    /// - `flags`: 标志设置字符串（如 "FLAGS (\Seen)"）
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

    /// 添加邮件标志
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    /// - `flags`: 要添加的标志（如 "\Seen"）
    pub async fn add_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        self.set_flags(uid, &format!("+FLAGS ({flags})")).await
    }

    /// 移除邮件标志
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    /// - `flags`: 要移除的标志（如 "\Seen"）
    pub async fn remove_flags(&mut self, uid: u32, flags: &str) -> Result<(), MailError> {
        self.set_flags(uid, &format!("-FLAGS ({flags})")).await
    }

    /// 移动邮件到目标文件夹
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    /// - `target_folder`: 目标文件夹名称
    ///
    /// # 返回
    /// 成功时返回 ()
    ///
    /// # 功能
    /// - 优先使用 RFC 6851 UID MOVE 命令
    /// - 服务器不支持时降级为 UID COPY + \Deleted 标志
    /// - 不执行 EXPUNGE，由调用者决定何时清理
    pub async fn move_uid_to_folder(
        &mut self,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let uid_str = uid.to_string();

        match self.session.uid_mv(&uid_str, target_folder).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(
                    uid,
                    target_folder,
                    error = %error,
                    "IMAP: UID MOVE 失败，降级为 UID COPY + \\Deleted"
                );
            }
        }

        self.copy_uid_to_folder(uid, target_folder).await?;
        self.mark_uid_deleted(uid).await?;
        Ok(())
    }

    /// 复制邮件到目标文件夹
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    /// - `target_folder`: 目标文件夹名称
    async fn copy_uid_to_folder(&mut self, uid: u32, target_folder: &str) -> Result<(), MailError> {
        let uid_str = uid.to_string();
        self.session
            .uid_copy(&uid_str, target_folder)
            .await
            .map_err(|e| {
                MailError::ImapConnectionFailed(format!("复制邮件到目标文件夹失败: {e}"))
            })?;
        Ok(())
    }

    /// 标记邮件为已删除
    ///
    /// # 参数
    /// - `uid`: 邮件 UID
    async fn mark_uid_deleted(&mut self, uid: u32) -> Result<(), MailError> {
        self.set_flags(uid, "+FLAGS.SILENT (\\Deleted)").await
    }

    /// 标记指定 UID 邮件为已删除。
    pub async fn delete_uid(&mut self, uid: u32) -> Result<(), MailError> {
        self.mark_uid_deleted(uid).await
    }

    /// 将一封完整 RFC822 邮件追加到目标文件夹，并可显式传入 IMAP flags。
    pub async fn append_email_with_flags(
        &mut self,
        folder: &str,
        flags: Option<&str>,
        raw: &[u8],
    ) -> Result<(), MailError> {
        self.session
            .append(folder, flags, None, raw)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("追加邮件失败: {e}")))?;
        Ok(())
    }

    /// 将一封完整 RFC822 邮件追加到目标文件夹。
    ///
    /// 发送成功后的 Sent 远端归档使用 IMAP APPEND，并标记为已读。
    pub async fn append_email(&mut self, folder: &str, raw: &[u8]) -> Result<(), MailError> {
        self.append_email_with_flags(folder, Some("(\\Seen)"), raw)
            .await
    }

    // ─── 连接管理 ───

    /// 登出 IMAP 服务器
    ///
    /// # 返回
    /// 成功时返回 ()
    pub async fn logout(mut self) -> Result<(), MailError> {
        self.session
            .logout()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("登出失败: {e}")))?;
        Ok(())
    }

    /// 获取服务器能力列表
    ///
    /// # 返回
    /// 成功时返回能力字符串列表（如 "IMAP4rev1"、"UIDPLUS"、"MOVE" 等）
    pub async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        let caps = self
            .session
            .capabilities()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取能力列表失败: {e}")))?;
        Ok(caps.iter().map(|c| format!("{c:?}")).collect())
    }
}
