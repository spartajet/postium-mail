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
use async_imap::imap_proto::{AttributeValue, NameAttribute, Response, Status};
use async_imap::types::Flag;
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
    /// 最高 MODSEQ（RFC 7162 CONDSTORE/QRESYNC；服务器不支持时为 None）
    pub highest_modseq: Option<u64>,
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
            tracing::warn!(host, error = %e, "IMAP: 读取 greeting 失败");
            MailError::ImapConnectionFailed(format!("读取 IMAP greeting 失败: {e}"))
        })?;

        // RFC 2971 ID：网易（163/126/yeah）要求 LOGIN 前自报客户端身份，
        // 否则以 "Unsafe Login" 拒绝登录；对其他服务器无害，失败可忽略。
        // （async-imap 未认证阶段的 Client 未暴露 id()，借 Connection 的裸命令接口发送）
        let id_command = format!(
            "ID (\"name\" \"Postium Mail\" \"version\" \"{}\")",
            env!("CARGO_PKG_VERSION")
        );
        if let Err(e) = client.run_command_and_check_ok(id_command.as_str(), None).await {
            tracing::debug!(host, error = %e, "IMAP: ID 命令失败（服务器可能不支持，忽略）");
        }

        tracing::debug!(email, "IMAP: 正在登录");
        let session = client
            .login(email, password)
            .await
            .map_err(|(e, _)| {
                let msg = login_error_message(host, &e.to_string());
                tracing::warn!(host, email, error = %msg, "IMAP: 登录失败");
                MailError::AuthFailed(msg)
            })?;
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
            .map_err(|(e, _)| {
                tracing::warn!(email, error = %e, "IMAP: XOAUTH2 认证失败");
                MailError::AuthFailed(format!("XOAUTH2 认证失败: {e}"))
            })?;
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
            highest_modseq: mailbox.highest_modseq,
        };
        tracing::debug!(folder, exists = info.exists, recent = info.recent, unseen = ?info.unseen, "IMAP: 文件夹已选择");
        Ok(info)
    }

    /// EXAMINE（只读）选择文件夹。
    ///
    /// 与 [`Self::select_folder`] 的区别：只读模式下对邮件标志的修改会被
    /// 服务器拒绝，适合增量同步等不允许副作用的场景。
    pub async fn examine_folder(&mut self, folder: &str) -> Result<MailboxInfo, MailError> {
        tracing::debug!(folder, "IMAP: EXAMINE 文件夹");
        let mailbox = self
            .session
            .examine(folder)
            .await
            .map_err(|e| MailError::FolderNotFound(format!("EXAMINE '{folder}' 失败: {e}")))?;
        let info = MailboxInfo {
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: mailbox.unseen,
            uid_validity: mailbox.uid_validity,
            uid_next: mailbox.uid_next,
            highest_modseq: mailbox.highest_modseq,
        };
        tracing::debug!(
            folder,
            exists = info.exists,
            highest_modseq = ?info.highest_modseq,
            "IMAP: 文件夹已只读选择"
        );
        Ok(info)
    }

    /// QRESYNC 参数化 SELECT（RFC 7162）。
    ///
    /// 需先 ENABLE QRESYNC/CONDSTORE。SELECT 响应中的 flag 变更（FETCH）与
    /// VANISHED 数据由后续解析方法提取——本方法只负责发送命令并确认 tagged OK。
    pub async fn select_qresync(
        &mut self,
        folder: &str,
        uidvalidity: u32,
        modseq: u64,
    ) -> Result<(), MailError> {
        tracing::debug!(folder, uidvalidity, modseq, "IMAP: QRESYNC SELECT");
        // async-imap 0.11 未暴露 QRESYNC 参数化 SELECT，走裸命令；
        // 文件夹名按 IMAP quoted-string 转义（同 async-imap 内部 quote! 宏规则）
        let quoted = format!("\"{}\"", folder.replace('\\', "\\\\").replace('"', "\\\""));
        let cmd = format!("SELECT {quoted} (QRESYNC ({uidvalidity} {modseq}))");
        self.run_command_and_check_ok(&cmd).await
    }

    /// CHANGEDSINCE flag 拉取（RFC 7162 CONDSTORE 档）：
    /// `UID FETCH uid_set (UID FLAGS) (CHANGEDSINCE modseq)`。
    ///
    /// 返回自 `modseq` 以来标志发生变化的 (uid, flags) 列表。
    ///
    /// 实现说明：`run_command_and_check_ok` 会把 untagged 响应路由到
    /// unsolicited 通道后丢弃，因此这里用 `run_command` + 手动循环
    /// `read_response`，直接解析 untagged FETCH 的 UID/FLAGS，
    /// 直到本命令的 tagged OK。
    pub async fn fetch_flags_changed_since(
        &mut self,
        folder: &str,
        uid_set: &str,
        modseq: u64,
    ) -> Result<Vec<(u32, Vec<String>)>, MailError> {
        tracing::debug!(folder, uid_set, modseq, "IMAP: CHANGEDSINCE 拉取 flag 变更");
        self.select_folder(folder).await?;

        let cmd = format!("UID FETCH {uid_set} (UID FLAGS) (CHANGEDSINCE {modseq})");
        let request_id = self
            .session
            .run_command(cmd.as_str())
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("命令 {cmd} 发送失败: {e}")))?;

        let mut results = Vec::new();
        loop {
            let Some(response) = self
                .session
                .read_response()
                .await
                .map_err(|e| MailError::ImapConnectionFailed(format!("读取 FETCH 响应失败: {e}")))?
            else {
                return Err(MailError::ImapConnectionFailed(
                    "连接中断：未收到 CHANGEDSINCE FETCH 的 tagged 响应".to_string(),
                ));
            };
            match response.parsed() {
                Response::Fetch(_seq, attrs) => {
                    let mut uid = None;
                    let mut flags = Vec::new();
                    for attr in attrs {
                        match attr {
                            AttributeValue::Uid(id) => uid = Some(*id),
                            AttributeValue::Flags(raw_flags) => {
                                // 与 fetch_flags 一致：用 Flag 的 Debug 形式
                                // （如 "Seen"、"Custom(\"...\")"）
                                flags = raw_flags
                                    .iter()
                                    .map(|f| format!("{:?}", Flag::from(f.as_ref())))
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                    // 缺 UID 的 untagged FETCH（服务器单边推送）无法定位邮件，跳过
                    if let Some(uid) = uid {
                        results.push((uid, flags));
                    }
                }
                Response::Done {
                    tag,
                    status,
                    code,
                    information,
                } => {
                    if *tag != request_id {
                        // 理论上不会混入其他命令的 tagged 响应，防御性跳过
                        continue;
                    }
                    match status {
                        Status::Ok => {
                            tracing::debug!(count = results.len(), "IMAP: CHANGEDSINCE 拉取完成");
                            return Ok(results);
                        }
                        other => {
                            return Err(MailError::ImapConnectionFailed(format!(
                                "CHANGEDSINCE FETCH 失败: {other:?} {code:?} {information:?}"
                            )));
                        }
                    }
                }
                // EXISTS/RECENT/EXPUNGE 等服务器单边推送：跳过
                _ => {}
            }
        }
    }

    /// UID SEARCH ALL：文件夹内全部 UID（vanished 对比/增量同步用）。
    ///
    /// 转发 [`Self::list_all_uids`]（自带 SELECT + 排序）。
    pub async fn uid_search_all(&mut self, folder: &str) -> Result<Vec<u32>, MailError> {
        tracing::debug!(folder, "IMAP: UID SEARCH ALL");
        self.list_all_uids(folder).await
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

    /// EXPUNGE 当前选中文件夹：物理清除所有带 \Deleted 标志的邮件。
    ///
    /// 注意：EXPUNGE 作用于整个文件夹——其他客户端标记 \Deleted 的邮件
    /// 也会被一并清除（async-imap 0.11 未暴露 UID EXPUNGE）。
    pub async fn expunge(&mut self) -> Result<(), MailError> {
        let expunged = self
            .session
            .expunge()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("EXPUNGE 失败: {e}")))?;
        futures::pin_mut!(expunged);
        while expunged.next().await.is_some() {}
        Ok(())
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

    /// 转入 IDLE 模式（消费会话——IDLE 连接是专属连接）。
    ///
    /// 返回的 `Handle` 需先 `init()` 发出 IDLE 命令，再 `wait_with_timeout()`
    /// 等待服务器推送。退出 IDLE：drop `StopSource` 只会中断等待
    /// （wait 返回 `ManualInterrupt`，**不发 DONE**）；显式发 DONE 并取回
    /// 会话须调 `Handle::done()`（消费 Handle，返回 `Session`）。
    pub fn into_idle(
        self,
    ) -> async_imap::extensions::idle::Handle<tokio_native_tls::TlsStream<tokio::net::TcpStream>>
    {
        self.session.idle()
    }

    /// 从 `Handle::done()` 返回的会话重建客户端（IDLE 专属连接跨轮复用）。
    pub(crate) fn from_idle_session(
        session: async_imap::Session<tokio_native_tls::TlsStream<tokio::net::TcpStream>>,
    ) -> Self {
        Self { session }
    }

    /// 发送任意命令并检查 OK（引擎 ENABLE 用）。
    pub async fn run_command_and_check_ok(&mut self, cmd: &str) -> Result<(), MailError> {
        self.session
            .run_command_and_check_ok(cmd)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("命令 {cmd} 失败: {e}")))
    }

    /// 获取服务器能力列表
    ///
    /// # 返回
    /// 能力字符串列表（如 "IMAP4rev1"、"UIDPLUS"、"MOVE" 等）
    pub async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        let caps = self
            .session
            .capabilities()
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取能力列表失败: {e}")))?;
        Ok(caps.iter().map(|c| format!("{c:?}")).collect())
    }
}

/// 构建 IMAP 登录失败的错误消息
///
/// 网易系（163.com / 126.com / yeah.net）服务器追加授权码提示：
/// 其 IMAP/SMTP 不接受网页版登录密码，必须使用「客户端授权码」。
fn login_error_message(host: &str, server_error: &str) -> String {
    let mut msg = format!("登录失败: {server_error}");
    if is_netease_host(host) {
        msg.push_str(
            "。网易邮箱需在网页版 设置→POP3/SMTP/IMAP 开启服务，并使用客户端授权码登录（而非登录密码）",
        );
    }
    msg
}

/// 判断 IMAP 主机是否属于网易系（163.com / 126.com / yeah.net）域名
fn is_netease_host(host: &str) -> bool {
    const NETEASE_DOMAINS: [&str; 3] = ["163.com", "126.com", "yeah.net"];
    NETEASE_DOMAINS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netease_hosts_should_be_recognized() {
        for host in [
            "imap.163.com",
            "imap.126.com",
            "imap.yeah.net",
            "163.com",
            "126.com",
            "yeah.net",
        ] {
            assert!(is_netease_host(host), "应识别 {host}");
        }
    }

    #[test]
    fn non_netease_hosts_should_not_be_recognized() {
        for host in [
            "imap.gmail.com",
            "outlook.office365.com",
            "imap.qq.com",
            "x163.com.example.org",
            "imap.163.com.evil.example",
        ] {
            assert!(!is_netease_host(host), "不应识别 {host}");
        }
    }

    #[test]
    fn login_error_should_append_hint_for_netease() {
        let msg = login_error_message("imap.163.com", "NO LOGIN Unsafe Login");
        assert!(msg.contains("登录失败"));
        assert!(msg.contains("授权码"), "网易登录失败应附带授权码提示: {msg}");
    }

    #[test]
    fn login_error_should_not_append_hint_for_others() {
        let msg = login_error_message("imap.gmail.com", "AUTHENTICATIONFAILED");
        assert!(msg.starts_with("登录失败"));
        assert!(!msg.contains("授权码"));
    }
}
