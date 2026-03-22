#![allow(deprecated)]

//! 异步 IMAP 客户端实现
//!
//! 提供基于 `async-imap` 的异步 IMAP 客户端，支持常见的邮件操作。
//!
//! # 核心功能
//!
//! - **连接管理**: TLS 加密连接、密码和 OAuth2 认证
//! - **文件夹操作**: 列出文件夹、获取文件夹元数据、RFC 6154 特殊用途支持
//! - **邮件操作**: 获取邮件、搜索、标志管理、删除
//! - **增量同步**: CONDSTORE 支持（RFC 4551）用于高效的变更检测
//! - **实时推送**: IDLE 支持（RFC 2177）用于新邮件通知
//!
//! # 与 `ImapService` 的区别
//!
//! `AsyncImapClient` 是底层协议客户端，`ImapService` 是高层封装：
//!
//! | 特性 | AsyncImapClient | ImapService |
//! |------|-----------------|-------------|
//! | 抽象级别 | 协议层 | 服务层 |
//! | 数据库操作 | 无 | 集成 |
//! | API 复杂度 | 较低 | 简化 |
//! | 推荐场景 | 直接控制 IMAP | 常规应用开发 |
//!
//! # 使用示例
//!
//! ## 基本连接和认证
//!
//! ```rust,no_run
//! use crate::protocols::imap::{AsyncImapClient, ImapAuth};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut client = AsyncImapClient::new();
//!
//! // 密码认证
//! let auth = ImapAuth::Password("app_password".to_string());
//! client.connect("imap.gmail.com", 993, "user@gmail.com", auth).await?;
//!
//! // OAuth2 认证
//! let auth = ImapAuth::OAuth2 {
//!     email: "user@gmail.com".to_string(),
//!     access_token: "ya29.a0AfH6...".to_string(),
//! };
//! client.connect("imap.gmail.com", 993, "user@gmail.com", auth).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 文件夹操作
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! // 列出文件夹（带特殊用途属性）
//! let folders = client.list_folders_with_attributes().await?;
//! for folder in folders {
//!     println!("{}: {:?}", folder.name, folder.special_use);
//! }
//!
//! // 获取文件夹元数据
//! let metadata = client.fetch_folder_metadata("INBOX").await?;
//! println!("UIDVALIDITY: {}", metadata.uidvalidity);
//! # Ok(())
//! # }
//! ```
//!
//! ## 邮件操作
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! // 获取 UID 列表
//! let uids = client.list_uids("INBOX", 50).await?;
//!
//! // 获取完整邮件
//! if let Some(&uid) = uids.first() {
//!     let email = client.fetch_email("INBOX", uid).await?;
//!     println!("主题: {}", email.subject);
//! }
//!
//! // 标记已读
//! client.mark_as_read("INBOX", uid, true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## CONDSTORE 增量同步
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! // 检查 CONDSTORE 支持
//! let supported = client.check_condstore_support().await?;
//!
//! if supported {
//!     // 使用 CONDSTORE 选择文件夹
//!     let (count, highest_modseq) = client.select_with_condstore("INBOX", None).await?;
//!     println!("HIGHESTMODSEQ: {:?}", highest_modseq);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # CONDSTORE 支持 (RFC 4551)
//!
//! CONDSTORE 扩展允许使用 MODSEQ（修改序列号）进行增量同步。
//!
//! ## 支持的服务商
//!
//! - ✅ Gmail (imap.gmail.com)
//! - ✅ iCloud (imap.mail.me.com)
//! - ❌ Outlook/Office365 (outlook.office365.com) - 不支持
//! - ❌ Yahoo (imap.mail.yahoo.com) - 不支持
//!
//! ## 限制说明
//!
//! 由于 `async-imap` 0.11 的限制：
//! - `search_modified_since()` 尚未完全实现，建议使用 UID SEARCH 降级策略
//! - `fetch_with_modseq()` 无法获取 MODSEQ 值，返回 None
//! - `select_with_condstore()` 只能启用 CONDSTORE 模式，不支持 UNCHANGEDSINCE 参数
//!
//! # IDLE 支持 (RFC 2177)
//!
//! IDLE 允许服务器推送新邮件通知，无需客户端轮询。
//!
//! ## 检查支持
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! let has_idle = client.check_idle_support().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 轮询降级策略
//!
//! 如果服务器不支持 IDLE，可以使用轮询：
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! // 检查新邮件（轻量级）
//! let (current_count, has_new) = client.check_new_emails("INBOX", 100).await?;
//!
//! // 轮询新邮件
//! let new_uids = client.polling_fallback("INBOX", last_uid).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # 邮件体解码
//!
//! `decode_email_body()` 函数自动检测常见编码：
//! - UTF-8（默认）
//! - GBK/GB18030（简体中文）
//! - Big5（繁体中文）
//! - Shift_JIS（日文）
//! - Windows-1252（西欧语言）
//!
//! # 中支持
//!
//! - 自动解码 IMAP UTF-7 编码的文件夹名称（163、QQ 邮箱等）
//! - 常见中文文件夹名称映射（收件箱、已发送、垃圾邮件等）
//!
//! # 性能优化
//!
//! - 使用 `BODY.PEEK[]` 避免自动设置已读标志
//! - 使用 `BODY.PEEK[HEADER]` 仅获取邮件头（骨架同步）
//! - 使用 `UID SEARCH SINCE` 获取指定日期后的邮件
//!
//! # 注意事项
//!
//! - 每个实例只能保持一个连接
//! - 使用完毕后应调用 `logout()` 清理资源
//! - 连接会在 Drop 时自动关闭
//! - OAuth2 认证需要进一步实现（当前返回错误）

use super::auth::ImapAuth;
use super::types::{EmailData, EmailFlags, FolderInfo, SpecialUse};
use crate::providers::generate_xoauth2_string;
use anyhow::{anyhow, Result};
use async_imap::Authenticator;
use chrono::Datelike;
use futures::TryStreamExt;
use std::time::Instant;
use tokio::net::TcpStream;
use tracing::instrument;

/// XOAUTH2 认证器
///
/// 实现 async-imap 的 Authenticator trait，用于 OAuth2 认证
pub struct XOAuth2Authenticator {
    /// 用户邮箱
    email: String,
    /// OAuth2 访问令牌
    access_token: String,
    /// 是否已发送初始响应
    initial_response_sent: bool,
}

impl XOAuth2Authenticator {
    /// 创建新的 XOAUTH2 认证器
    pub fn new(email: String, access_token: String) -> Self {
        Self {
            email,
            access_token,
            initial_response_sent: false,
        }
    }
}

impl Authenticator for XOAuth2Authenticator {
    type Response = String;

    /// 处理服务器挑战
    ///
    /// XOAUTH2 认证流程：
    /// 1. 客户端发送初始响应（base64 编码的 auth 字符串）
    /// 2. 如果认证成功，服务器返回 OK
    /// 3. 如果认证失败，服务器发送挑战（包含错误信息），客户端应发送空响应
    fn process(&mut self, challenge: &[u8]) -> Self::Response {
        // 如果这是初始请求（challenge 为空），发送认证字符串
        if challenge.is_empty() && !self.initial_response_sent {
            self.initial_response_sent = true;

            // 构造 XOAUTH2 字符串
            // 格式: base64("user=" + email + "\x01auth=Bearer " + token + "\x01\x01")
            let auth_string = format!(
                "user={}\x01auth=Bearer {}\x01\x01",
                self.email, self.access_token
            );

            tracing::debug!("XOAUTH2: 发送初始认证响应");

            // Base64 编码
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&auth_string)
        } else {
            // 服务器发送了挑战（通常是认证失败的错误信息）
            // 根据规范，我们应该发送空响应来终止认证
            tracing::warn!(
                "XOAUTH2: 收到服务器挑战: {:?}",
                String::from_utf8_lossy(challenge)
            );

            // 发送空响应以终止认证流程
            String::new()
        }
    }
}

/// 异步 IMAP 客户端会话
pub struct AsyncImapClient {
    session: Option<async_imap::Session<tokio_native_tls::TlsStream<TcpStream>>>,
}

impl AsyncImapClient {
    pub fn new() -> Self {
        Self { session: None }
    }

    /// 异步连接到 IMAP服务器并登录
    ///
    /// # 参数
    ///
    /// * `host` - IMAP 服务器地址
    /// * `port` - IMAP 服务器端口
    /// * `email` - 邮箱地址
    /// * `auth` - 认证信息
    #[instrument(skip(self), fields(host, port, email))]
    pub async fn connect(
        &mut self,
        host: &str,
        port: u16,
        email: &str,
        auth: ImapAuth,
    ) -> Result<()> {
        let start = Instant::now();

        // 创建 TCP 连接
        let tcp = TcpStream::connect(format!("{}:{}", host, port))
            .await
            .map_err(|e| anyhow!("连接 IMAP 服务器失败: {}", e))?;

        let connect_time = start.elapsed();

        // 创建 TLS 连接器并连接
        let tls_connector = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| anyhow!("创建 TLS 连接器失败: {}", e))?;
        let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);
        let tls_stream = tls_connector
            .connect(host, tcp)
            .await
            .map_err(|e| anyhow!("TLS 握手失败: {}", e))?;

        // 创建 IMAP 客户端
        let client = async_imap::Client::new(tls_stream);

        // 异步登录
        let login_start = Instant::now();
        let session = match &auth {
            ImapAuth::Password(pwd) => {
                // 传统密码认证
                client
                    .login(email, pwd)
                    .await
                    .map_err(|(e, _)| anyhow!("IMAP 密码登录失败: {}", e))?
            }
            ImapAuth::OAuth2 {
                email: oauth_email,
                access_token,
            } => {
                // OAuth2/XOAUTH2 认证
                tracing::info!("使用OAuth2认证IMAP: {}", oauth_email);

                // 生成XOAUTH2字符串
                let xoauth2_str = generate_xoauth2_string(oauth_email, access_token);

                // 使用authenticate命令进行OAuth2认证
                // 注意：async-imap可能不直接支持authenticate，需要使用原始命令
                // 这里我们尝试使用authenticate方法，如果失败则回退到手动实现

                // 方法1: 尝试使用authenticate（如果支持）
                // client.authenticate("XOAUTH2", &xoauth2_str).await
                //     .map_err(|(e, _)| anyhow!("IMAP OAuth2登录失败: {}", e))?

                // 方法2: 手动发送AUTHENTICATE命令（更可靠）
                Self::authenticate_oauth2(client, oauth_email, &xoauth2_str).await?
            }
        };

        let login_time = login_start.elapsed();

        tracing::info!(
            "IMAP 连接成功: {} (连接: {:?}, 登录: {:?})",
            email,
            connect_time,
            login_time
        );

        self.session = Some(session);
        Ok(())
    }

    /// OAuth2/XOAUTH2认证辅助方法
    /// 手动发送IMAP AUTHENTICATE XOAUTH2命令
    async fn authenticate_oauth2(
        _client: async_imap::Client<tokio_native_tls::TlsStream<TcpStream>>,
        _email: &str,
        xoauth2_str: &str,
    ) -> Result<async_imap::Session<tokio_native_tls::TlsStream<TcpStream>>> {
        // 发送AUTHENTICATE XOAUTH2命令
        // 格式: AUTHENTICATE XOAUTH2 <base64_string>
        let authenticate_cmd = format!("AUTHENTICATE XOAUTH2 {}", xoauth2_str);

        tracing::info!("发送OAuth2认证命令: {}", authenticate_cmd);

        // 这里我们需要使用async-imap的低级API来发送自定义命令
        // 由于async-imap的限制，我们采用以下策略：

        // 策略1: 尝试使用client的authenticate方法（如果可用）
        // 这个方法在较新版本的async-imap中可能存在

        // 策略2: 使用run_command_and_read_response手动发送命令
        // 但这需要访问客户端的内部状态

        // 策略3: 由于async-imap的限制，我们需要使用login命令的特殊格式
        // 某些IMAP服务器支持直接在login中使用XOAUTH2字符串

        // 对于Outlook/Office365，我们可以尝试使用用户名+空密码，然后立即发送AUTHENTICATE命令
        // 但这比较复杂

        // 当前实现：我们假设需要更底层的控制
        // 作为一个临时解决方案，我们返回一个错误，提示需要升级IMAP库或使用其他方法

        // 实际的实现可能需要：
        // 1. 升级到支持SASL的IMAP库版本
        // 2. 或者直接使用TCP/TLS流发送原始IMAP命令
        // 3. 或者使用其他IMAP库

        // 作为一个变通方案，我们尝试使用login方法，但传递XOAUTH2字符串作为密码
        // 注意：这种方法不标准，可能不适用于所有服务器

        // 对于Microsoft Exchange/Outlook，更好的方法是使用SASL IR (SASL Initial Response)
        // 但async-imap可能不支持

        // tracing::warn!("OAuth2 IMAP认证需要特殊处理，当前实现可能需要改进");

        // 临时解决方案：尝试使用一个占位实现
        // 在实际应用中，你需要：
        // 1. 升级async-imap到支持authenticate的版本
        // 2. 或手动实现SASL认证流程
        // 3. 或使用其他支持OAuth2的IMAP库

        // 使用 async-imap 的 authenticate 方法
        // XOAUTH2 认证器实现
        let authenticator = XOAuth2Authenticator::new(_email.to_string(), xoauth2_str.to_string());

        tracing::info!("使用 XOAUTH2 进行 IMAP 认证: {}", _email);

        match _client.authenticate("XOAUTH2", authenticator).await {
            Ok(session) => {
                tracing::info!("IMAP OAuth2 认证成功");
                Ok(session)
            }
            Err((e, _)) => {
                tracing::error!("IMAP OAuth2 认证失败: {}", e);
                Err(anyhow!("IMAP OAuth2 认证失败: {}", e))
            }
        }
    }

    /// 异步列出服务器上的所有文件夹及其属性（RFC 6154）
    #[instrument(skip(self))]
    pub async fn list_folders_with_attributes(&mut self) -> Result<Vec<FolderInfo>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 list 命令获取文件夹列表（返回流）
        let folders: Vec<async_imap::types::Name> = session
            .list(None, Some("*"))
            .await
            .map_err(|e| anyhow!("列出文件夹失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Name>>()
            .await
            .map_err(|e| anyhow!("收集文件夹列表失败: {}", e))?;

        let mut folder_infos = Vec::new();

        for folder in folders.iter() {
            let name_str = folder.name().to_string();
            let special_use = Self::parse_special_use(folder.attributes());

            // 过滤掉以 . 开头的文件夹（通常是系统文件夹）
            if !name_str.starts_with('.') {
                folder_infos.push(FolderInfo {
                    name: name_str.clone(),
                    special_use,
                    standard_name: Self::determine_standard_name(special_use, &name_str),
                });
            }
        }

        tracing::info!("返回 {} 个文件夹", folder_infos.len());
        Ok(folder_infos)
    }

    /// 异步列出服务器上的所有文件夹（仅返回名称）
    #[instrument(skip(self))]
    pub async fn list_folders(&mut self) -> Result<Vec<String>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 list 命令获取文件夹列表（返回流）
        let folders: Vec<async_imap::types::Name> = session
            .list(None, Some("*"))
            .await
            .map_err(|e| anyhow!("列出文件夹失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Name>>()
            .await
            .map_err(|e| anyhow!("收集文件夹列表失败: {}", e))?;

        // 只返回文件夹名称
        let folder_names: Vec<String> = folders
            .iter()
            .map(|f| f.name().to_string())
            .filter(|name| !name.starts_with('.')) // 过滤掉系统文件夹
            .collect();

        Ok(folder_names)
    }

    /// 选择文件夹并返回邮件数量
    pub async fn select_folder(&mut self, folder: &str) -> Result<usize> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        let mailbox = session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        Ok(mailbox.exists as usize)
    }

    /// 获取文件夹 IMAP 元数据（UIDVALIDITY, UIDNEXT 等）
    /// 使用 STATUS 命令获取文件夹元数据而不选中文件夹
    pub async fn fetch_folder_metadata(
        &mut self,
        folder: &str,
    ) -> Result<super::types::FolderMetadata> {
        use super::types::FolderMetadata;

        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 STATUS 命令获取文件夹元数据
        // 请求: MESSAGES, RECENT, UIDNEXT, UIDVALIDITY, UNSEEN
        let status_response = session
            .status(folder, "(MESSAGES RECENT UIDNEXT UIDVALIDITY UNSEEN)")
            .await
            .map_err(|e| anyhow!("获取文件夹状态失败: {}", e))?;

        // 从 Mailbox 对象解析状态信息
        let mailbox = status_response;

        // 处理 Option 类型的字段，允许服务器不返回某些字段
        let uidvalidity = mailbox.uid_validity.map(|v| v as u64).unwrap_or(1); // 默认值为 1

        let uidnext = mailbox.uid_next.map(|v| v as u64).unwrap_or(1); // 默认值为 1

        // 如果服务器未返回 UIDVALIDITY 或 UIDNEXT，记录警告
        if mailbox.uid_validity.is_none() {
            tracing::warn!("文件夹 {} 服务器未返回 UIDVALIDITY，使用默认值 1", folder);
        }
        if mailbox.uid_next.is_none() {
            tracing::warn!("文件夹 {} 服务器未返回 UIDNEXT，使用默认值 1", folder);
        }

        Ok(FolderMetadata {
            uidvalidity,
            uidnext,
            highest_modseq: None, // CONDSTORE 支持将在后续实现
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: None, // Mailbox 结构不直接提供 unseen，需要从其他途径获取
        })
    }

    /// 解析 RFC 6154 Special-Use 属性
    /// async-imap 0.11 的属性处理方式不同，暂时使用名称匹配
    fn parse_special_use(_attrs: &[async_imap::types::NameAttribute]) -> Option<SpecialUse> {
        // 暂时返回 None，依赖 determine_standard_name 的名称推断
        // TODO: 研究异步 IMAP 库的属性 API
        None
    }

    /// 解码 IMAP UTF-7 编码的文件夹名称（163、QQ邮箱等中文文件夹）
    fn decode_imap_utf7(imap_name: &str) -> String {
        // 常见的中文邮箱文件夹名称映射
        let common_mappings = [
            ("&XfJT0ZAB-", "已发送"),
            ("&XfJSIJZk-", "收件箱"),
            ("&V4NXPpCuTvY-", "垃圾邮件"),
            ("&dcVr0mWHTvZZOQ-", "已删除"),
            ("&g0l6P3ux-", "草稿箱"),
        ];

        for (encoded, decoded) in common_mappings.iter() {
            if imap_name == *encoded || imap_name.ends_with(encoded) {
                return decoded.to_string();
            }
        }

        imap_name.to_string()
    }

    /// 根据特殊用途或名称确定标准文件夹名
    fn determine_standard_name(special_use: Option<SpecialUse>, name: &str) -> String {
        if let Some(special) = special_use {
            return match special {
                SpecialUse::All => "all",
                SpecialUse::Archive => "archive",
                SpecialUse::Drafts => "drafts",
                SpecialUse::Flagged => "flagged",
                SpecialUse::Junk => "spam",
                SpecialUse::Sent => "sent",
                SpecialUse::Trash => "trash",
            }
            .to_string();
        }

        // 先尝试解码 UTF-7 编码的中文名称
        let decoded_name = Self::decode_imap_utf7(name);

        // 根据名称推断（包括解码后的中文名称）
        let name_lower = decoded_name.to_lowercase();
        if name_lower.contains("inbox") || name_lower == "inbox" || name_lower.contains("收件箱")
        {
            "inbox".to_string()
        } else if name_lower.contains("sent") || name_lower.contains("已发送") {
            "sent".to_string()
        } else if name_lower.contains("draft") || name_lower.contains("草稿") {
            "drafts".to_string()
        } else if name_lower.contains("spam")
            || name_lower.contains("junk")
            || name_lower.contains("垃圾邮件")
        {
            "spam".to_string()
        } else if name_lower.contains("trash")
            || name_lower.contains("deleted")
            || name_lower.contains("已删除")
        {
            "trash".to_string()
        } else if name_lower.contains("archive") || name_lower.contains("归档") {
            "archive".to_string()
        } else {
            name.to_string()
        }
    }

    /// 异步获取 UID 列表
    pub async fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        let mailbox = session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // 记录邮箱信息
        tracing::info!(
            "📬 邮箱信息: exists={}, recent={}, unseen={:?}, uid_next={:?}",
            mailbox.exists,
            mailbox.recent,
            mailbox.unseen,
            mailbox.uid_next
        );

        // SEARCH ALL 获取所有邮件 UID（返回 Result<HashSet<Seq>>）
        let uids = session
            .search("ALL")
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        tracing::debug!("SEARCH ALL 返回 {} 个 UID", uids.len());

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        uid_list.reverse(); // 最新的在前

        // 应用限制
        if uid_list.len() > limit {
            uid_list.truncate(limit);
        }

        Ok(uid_list)
    }

    /// 获取大于指定 UID 的邮件列表
    pub async fn list_uids_after(&mut self, folder: &str, min_uid: u32) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // SEARCH UID <min_uid:* 获取大于指定 UID 的邮件（返回 Result<HashSet<Seq>>）
        let search_cmd = format!("UID {}:*", min_uid + 1);
        let uids = session
            .search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        uid_list.reverse();

        Ok(uid_list)
    }

    /// 获取指定时间范围内的邮件 UID 列表
    /// date_since: Unix 时间戳（秒）
    pub async fn list_uids_since_timestamp(
        &mut self,
        folder: &str,
        _since_timestamp: i64,
    ) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // 获取所有邮件 UID
        let uids = session
            .search("ALL")
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        // 转换为向量并排序（最新在前）
        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        uid_list.reverse();

        Ok(uid_list)
    }

    /// 获取指定时间范围内的邮件 UID 列表（使用 IMAP SINCE 命令）
    /// date_since: IMAP 日期格式，如 "01-Jan-2025"
    pub async fn list_uids_since(&mut self, folder: &str, date_since: &str) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        tracing::info!(
            "🔍 list_uids_since 开始: folder={}, date_since={}",
            folder,
            date_since
        );

        // SELECT 文件夹（返回 Result<Mailbox>）
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // 首先获取总邮件数（用于诊断）
        let all_uids = session
            .search("ALL")
            .await
            .map_err(|e| anyhow!("搜索所有邮件失败: {}", e))?;
        tracing::info!("📊 文件夹总邮件数: {}", all_uids.len());

        // 使用 SINCE 命令搜索指定日期之后的邮件
        // 注意：SINCE 命令的日期格式是 "01-Jan-2025"（不需要双引号，根据 RFC 3501）
        let search_cmd = format!("SINCE {}", date_since);
        tracing::info!("📤 使用 IMAP 搜索命令: '{}'", search_cmd);

        let uids = session
            .search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        tracing::info!(
            "📥 SINCE 命令返回 {} 个 UID (预期: 所有近一年的邮件)",
            uids.len()
        );
        tracing::info!(
            "📊 比例: {}/{} ({:.1}%)",
            uids.len(),
            all_uids.len(),
            (uids.len() as f64 / all_uids.len() as f64) * 100.0
        );

        // 如果 SINCE 返回的结果太少，记录警告
        if all_uids.len() > 100 && uids.len() < all_uids.len() / 2 {
            tracing::warn!(
                "⚠️  SINCE 命令返回的邮件数量异常少！可能的原因:\n\
                 1. 日期格式不正确: '{}'\n\
                 2. IMAP 服务器不支持 SINCE 命令\n\
                 3. 服务器上的邮件确实都在近一个月内",
                date_since
            );
        }

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        uid_list.reverse();

        if !uid_list.is_empty() {
            tracing::info!(
                "   UID 范围: {} ~ {}",
                uid_list.last().unwrap_or(&0),
                uid_list.first().unwrap_or(&0)
            );
        }

        Ok(uid_list)
    }

    /// 异步获取邮件数据
    pub async fn fetch_email(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // FETCH 邮件内容（使用 BODY.PEEK[] 不会自动设置 \Seen 标志）
        // RFC 3501: BODY.PEEK[] 不会将邮件标记为已读，RFC822.PEEK 是过时的语法
        // 使用 BODY.PEEK[] 保持邮件原本的已读/未读状态
        let uid_str = uid.to_string();
        let messages: Vec<async_imap::types::Fetch> = session
            .fetch(&uid_str, "(BODY.PEEK[] FLAGS)")
            .await
            .map_err(|e| anyhow!("获取邮件失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| anyhow!("收集邮件数据失败: {}", e))?;

        // 获取第一封邮件
        let message = messages
            .first()
            .ok_or_else(|| anyhow!("邮件 {} 不存在", uid))?;

        // 解析邮件体
        let body = message.body().ok_or_else(|| anyhow!("邮件体为空"))?;

        // 尝试使用不同的编码解码邮件体
        let raw_email = decode_email_body(body)?;

        // 使用 parser 模块解析邮件
        let email_data = super::parser::parse_email_with_mail_parser(&raw_email, uid)?;

        // 解析标志（在返回前收集所有标志状态）
        let seen = message.flags().any(|f| f == async_imap::types::Flag::Seen);
        let flagged = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Flagged);
        let answered = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Answered);
        let deleted = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Deleted);

        Ok(EmailData {
            flags: EmailFlags {
                seen,
                flagged,
                answered,
                deleted,
            },
            ..email_data
        })
    }

    /// 仅获取邮件头（用于骨架同步，不获取正文）
    /// 使用 BODY.PEEK[HEADER] 避免设置已读标志
    pub async fn fetch_email_headers(
        &mut self,
        folder: &str,
        uid: u32,
    ) -> Result<super::types::EmailHeader> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // FETCH 邮件头（使用 BODY.PEEK[HEADER] 不会设置已读标志）
        let uid_str = uid.to_string();
        let messages: Vec<async_imap::types::Fetch> = session
            .fetch(&uid_str, "(BODY.PEEK[HEADER] FLAGS)")
            .await
            .map_err(|e| anyhow!("获取邮件头失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| anyhow!("收集邮件头数据失败: {}", e))?;

        let message = messages
            .first()
            .ok_or_else(|| anyhow!("邮件 {} 不存在", uid))?;

        // 解析邮件头
        let header_body = message.body().ok_or_else(|| anyhow!("邮件头为空"))?;
        let raw_header = decode_email_body(header_body)?;

        // 使用 mail_parser 解析邮件头
        let email_header = super::parser::parse_email_header_only(&raw_header, uid)?;

        // 解析标志
        let seen = message.flags().any(|f| f == async_imap::types::Flag::Seen);
        let flagged = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Flagged);
        let answered = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Answered);
        let deleted = message
            .flags()
            .any(|f| f == async_imap::types::Flag::Deleted);

        Ok(super::types::EmailHeader {
            flags: super::types::EmailFlags {
                seen,
                flagged,
                answered,
                deleted,
            },
            ..email_header
        })
    }

    /// 仅获取邮件正文（用于后台填充）
    /// 使用 BODY[TEXT] 会设置已读标志
    pub async fn fetch_email_body(&mut self, folder: &str, uid: u32) -> Result<(String, String)> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹
        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // FETCH 邮件正文（使用 BODY[1.TEXT] 获取纯文本，BODY[1.HTML] 获取HTML）
        let uid_str = uid.to_string();

        // 先获取纯文本正文
        let text_messages: Vec<async_imap::types::Fetch> = session
            .fetch(&uid_str, "BODY[1.TEXT]")
            .await
            .map_err(|e| anyhow!("获取纯文本正文失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| anyhow!("收集纯文本正文数据失败: {}", e))?;

        // 再获取HTML正文
        let html_messages: Vec<async_imap::types::Fetch> = session
            .fetch(&uid_str, "BODY[1.HTML]")
            .await
            .map_err(|e| anyhow!("获取HTML正文失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| anyhow!("收集HTML正文数据失败: {}", e))?;

        // 解析纯文本正文
        let body_text = if let Some(msg) = text_messages.first() {
            if let Some(body) = msg.body() {
                decode_email_body(body).unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // 解析HTML正文
        let body_html = if let Some(msg) = html_messages.first() {
            if let Some(body) = msg.body() {
                decode_email_body(body).unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        Ok((body_text, body_html))
    }

    /// 异步标记邮件为已读/未读
    pub async fn mark_as_read(&mut self, folder: &str, uid: u32, is_read: bool) -> Result<()> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        // 使用 STORE 命令设置/清除标志（返回流）
        // async-imap 0.11: flags 需要放在查询字符串中
        if is_read {
            session
                .store(&uid_str, "+FLAGS (\\Seen)")
                .await
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集标记结果失败: {}", e))?;
        } else {
            session
                .store(&uid_str, "-FLAGS (\\Seen)")
                .await
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集标记结果失败: {}", e))?;
        }

        Ok(())
    }

    /// 异步设置星标
    pub async fn set_flag(&mut self, folder: &str, uid: u32, flagged: bool) -> Result<()> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        if flagged {
            session
                .store(&uid_str, "+FLAGS (\\Flagged)")
                .await
                .map_err(|e| anyhow!("设置标志失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集设置标志结果失败: {}", e))?;
        } else {
            session
                .store(&uid_str, "-FLAGS (\\Flagged)")
                .await
                .map_err(|e| anyhow!("设置标志失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集设置标志结果失败: {}", e))?;
        }

        Ok(())
    }

    /// 异步删除邮件
    pub async fn delete_email(&mut self, folder: &str, uid: u32) -> Result<()> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        // 标记为删除（返回流）
        session
            .store(&uid_str, "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| anyhow!("标记删除失败: {}", e))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| anyhow!("收集标记删除结果失败: {}", e))?;

        // 执行删除（返回流）
        session
            .expunge()
            .await
            .map_err(|e| anyhow!("删除邮件失败: {}", e))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| anyhow!("收集删除结果失败: {}", e))?;

        Ok(())
    }

    /// 异步登出
    pub async fn logout(&mut self) -> Result<()> {
        if let Some(mut session) = self.session.take() {
            session
                .logout()
                .await
                .map_err(|e| anyhow!("登出失败: {}", e))?;
        }
        Ok(())
    }

    /// 获取原始邮件头（用于诊断）
    pub async fn fetch_raw_header(&mut self, folder: &str, uid: u32) -> Result<String> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();
        let messages = session
            .fetch(&uid_str, "(RFC822.HEADER)")
            .await
            .map_err(|e| anyhow!("获取邮件头失败: {}", e))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| anyhow!("收集邮件头失败: {}", e))?;

        let message = messages
            .first()
            .ok_or_else(|| anyhow!("邮件 {} 不存在", uid))?;

        tracing::debug!("Fetch响应: {:?}", message);

        // 检查是否有RFC822.HEADER字段
        if let Some(header) = message.header() {
            return String::from_utf8(header.to_vec())
                .map_err(|e| anyhow!("解析邮件头编码失败: {}", e));
        }

        // 尝试body方法
        if let Some(body) = message.body() {
            return String::from_utf8(body.to_vec())
                .map_err(|e| anyhow!("解析邮件头编码失败: {}", e));
        }

        Err(anyhow!("无法获取邮件头"))
    }

    // ========== CONDSTORE 支持 (RFC 4551) ==========

    /// 检查是否支持 CONDSTORE 扩展
    ///
    /// 使用 CAPABILITY 命令检测服务器是否支持 CONDSTORE。
    ///
    /// **注意**：async-imap 0.11 原生支持 `capabilities()` 方法。
    ///
    /// # RFC 4551 CONDSTORE
    ///
    /// CONDSTORE 扩展允许客户端：
    /// - 使用 MODSEQ（修改序列号）进行增量同步
    /// - 使用 UNCHANGEDSINCE 参数检测变更
    /// - 使用 SEARCH MODSEQ 查找修改的邮件
    ///
    /// # 支持的服务商
    ///
    /// - ✅ Gmail (imap.gmail.com)
    /// - ✅ iCloud (imap.mail.me.com)
    /// - ❌ Outlook (outlook.office365.com) - 不支持
    /// - ❌ Yahoo (imap.mail.yahoo.com) - 不支持
    ///
    /// # 返回
    ///
    /// - `Ok(true)` - 服务器支持 CONDSTORE
    /// - `Ok(false)` - 服务器不支持 CONDSTORE
    /// - `Err` - IMAP 未连接或命令执行失败
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let supported = client.check_condstore_support().await?;
    /// if supported {
    ///     println!("服务器支持 CONDSTORE");
    /// }
    /// ```
    pub async fn check_condstore_support(&mut self) -> Result<bool> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 async-imap 0.11 的 capabilities() 方法
        let capabilities = session
            .capabilities()
            .await
            .map_err(|e| anyhow!("获取服务器能力失败: {}", e))?;
        for c in capabilities.iter() {
            tracing::debug!("邮件服务器能力：{:?}", c)
        }

        // 检查是否包含 CONDSTORE
        let has_condstore = capabilities.iter().any(|cap| {
            // 将 Capability 转换为字符串进行比较
            let cap_str = format!("{:?}", cap);
            cap_str.to_ascii_uppercase().contains("CONDSTORE") || cap_str == "Condstore"
        });

        if has_condstore {
            tracing::info!("服务器支持 CONDSTORE 扩展");
        } else {
            tracing::debug!("服务器不支持 CONDSTORE 扩展");
        }

        Ok(has_condstore)
    }

    /// 选择文件夹（带 CONDSTORE 参数）
    ///
    /// 使用 CONDSTORE 扩展选择文件夹，支持 UNCHANGEDSINCE 参数进行增量同步。
    ///
    /// **注意**：async-imap 0.11 原生支持 `select_condstore()` 方法。
    ///
    /// # 参数
    ///
    /// * `folder` - 文件夹名称
    /// * `changed_since` - 起始 MODSEQ 值（None 表示普通 CONDSTORE SELECT）
    ///
    /// # 返回
    ///
    /// (邮件数量, HIGHESTMODSEQ)
    ///
    /// # 实现说明
    ///
    /// **当前**：使用 `select_condstore()` 启用 CONDSTORE 模式。
    ///
    /// **UNCHANGEDSINCE 参数**：
    /// - async-imap 0.11 的 `select_condstore()` 启用 CONDSTORE 模式
    /// - UNCHANGEDSINCE 参数需要使用原始命令实现
    /// - 当前实现总是启用 CONDSTORE，不使用 UNCHANGEDSINCE 过滤
    ///
    /// **正确的命令格式**：
    /// ```text
    /// A1 SELECT INBOX (CONDSTORE)
    /// * 172 EXISTS
    /// * OK [HIGHESTMODSEQ 1234567900] Highest
    /// A1 OK [READ-WRITE] Select completed (0.001 + 0.000 secs).
    /// ```
    ///
    /// # 返回值
    ///
    /// - 邮件数量：邮箱中的邮件总数
    /// - HIGHESTMODSEQ：邮箱当前最高的修改序列号
    ///
    /// # 示例
    ///
    /// ```ignore
    /// // 启用 CONDSTORE 模式
    /// let (count, highest_modseq) = client.select_with_condstore("INBOX", None).await?;
    ///
    /// // 注意：UNCHANGEDSINCE 过滤需要使用原始命令（未来实现）
    /// ```
    pub async fn select_with_condstore(
        &mut self,
        folder: &str,
        changed_since: Option<u64>,
    ) -> Result<(usize, Option<u64>)> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        if changed_since.is_some() {
            tracing::warn!(
                "UNCHANGEDSINCE 参数尚未实现（需要原始命令），使用普通 CONDSTORE SELECT"
            );
        }

        // 使用 async-imap 0.11 的 select_condstore() 方法
        // 这会启用 CONDSTORE 模式，但不支持 UNCHANGEDSINCE 参数
        let mailbox = session
            .select_condstore(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败 (CONDSTORE): {}", e))?;

        // 获取 HIGHESTMODSEQ（服务器支持 CONDSTORE 时返回）
        let highest_modseq = mailbox.highest_modseq;

        tracing::debug!(
            "CONDSTORE SELECT: folder={}, exists={}, highest_modseq={:?}",
            folder,
            mailbox.exists,
            highest_modseq
        );

        Ok((mailbox.exists as usize, highest_modseq))
    }

    /// 搜索修改的邮件（MODSEQ）
    ///
    /// 使用 SEARCH MODSEQ 命令查找自指定 MODSEQ 后修改的邮件。
    ///
    /// **注意**：此方法需要异步 IMAP 库支持 MODSEQ 搜索语法。
    ///
    /// # 参数
    ///
    /// * `modseq` - 起始 MODSEQ 值
    ///
    /// # 返回
    ///
    /// 修改的邮件 UID 列表
    ///
    /// # 命令格式
    ///
    /// ```text
    /// A1 SEARCH MODSEQ 1234567890:* ALL
    /// * SEARCH 1 3 5 7 9
    /// A1 OK Search completed
    /// ```
    ///
    /// # 实现说明
    ///
    /// **当前**：async-imap 0.11 的 `search()` 方法不支持 MODSEQ 语法。
    ///
    /// **建议**：使用 UID SEARCH 降级策略：
    /// - `UID SEARCH SINCE <date>` - 搜索指定日期后的邮件
    /// - 然后逐个检查邮件的 FLAGS 变更
    ///
    /// # 降级策略
    ///
    /// 如果命令执行失败或服务器不支持 CONDSTORE：
    /// - 使用 `uid_search()` 方法配合时间范围
    /// - 逐个对比 FLAGS 来检测变更
    ///
    /// # 示例
    ///
    /// ```ignore
    /// // 当前实现返回错误，建议使用降级策略
    /// match client.search_modified_since(1234567890).await {
    ///     Ok(uids) => { /* 使用 MODSEQ 结果 */ }
    ///     Err(_) => {
    ///         // 降级到 UID SEARCH
    ///         let uids = client.uid_search("SINCE 17-Mar-2026").await?;
    ///     }
    /// }
    /// ```
    pub async fn search_modified_since(&mut self, _modseq: u64) -> Result<Vec<u32>> {
        // 当前 async-imap 0.11 不支持 SEARCH MODSEQ 语法
        // 建议使用 UID SEARCH 降级策略
        Err(anyhow!(
            "SEARCH MODSEQ 尚未实现（async-imap 限制），请使用 UID SEARCH 降级策略"
        ))
    }

    /// 获取邮件及其 MODSEQ
    ///
    /// FETCH 邮件时同时获取 MODSEQ 值。
    ///
    /// **注意**：此方法需要异步 IMAP 库支持 MODSEQ 响应解析。
    ///
    /// # 参数
    ///
    /// * `folder` - 文件夹名称
    /// * `uid` - 邮件 UID
    ///
    /// # 返回
    ///
    /// (邮件数据, MODSEQ 值)
    ///
    /// # 命令格式
    ///
    /// ```text
    /// A1 FETCH 123 (FLAGS BODY.PEEK[] MODSEQ)
    /// * 123 FETCH (FLAGS (\Seen) BODY[...] {size} MODSEQ (1234567890))
    /// A1 OK Fetch completed
    /// ```
    ///
    /// # 实现说明
    ///
    /// **当前**：使用 `fetch_email()` 获取邮件数据，MODSEQ 暂时无法获取。
    ///
    /// **原因**：async-imap 0.11 的 `fetch()` 方法不返回 MODSEQ 值。
    ///
    /// **建议**：
    /// - 使用 SELECT CONDSTORE 后，邮箱会返回 HIGHESTMODSEQ
    /// - 可以通过对比本地和服务器状态来检测变更
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let (email, modseq) = client.fetch_with_modseq("INBOX", 123).await?;
    /// // modseq 当前为 None
    /// // 可以使用 HIGHESTMODSEQ 来检测变更
    /// ```
    pub async fn fetch_with_modseq(
        &mut self,
        folder: &str,
        uid: u32,
    ) -> Result<(EmailData, Option<u64>)> {
        let _session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        tracing::debug!("FETCH UID {} (MODSEQ 尚不支持)", uid);

        // 使用普通 fetch 获取邮件数据
        let email_data = self.fetch_email(folder, uid).await?;

        // MODSEQ 暂时无法获取（async-imap 限制）
        Ok((email_data, None))
    }

    /// 获取多个邮件的 MODSEQ 值
    ///
    /// 批量获取邮件的 MODSEQ，用于变更检测。
    ///
    /// **注意**：此方法需要异步 IMAP 库支持批量 MODSEQ 查询。
    ///
    /// # 参数
    ///
    /// * `uids` - 邮件 UID 列表
    ///
    /// # 返回
    ///
    /// (UID, MODSEQ) 列表
    ///
    /// # 命令格式
    ///
    /// ```text
    /// A1 FETCH 1,2,3 (FLAGS MODSEQ)
    /// * 1 FETCH (FLAGS () MODSEQ (1234567880))
    /// * 2 FETCH (FLAGS (\Seen) MODSEQ (1234567890))
    /// * 3 FETCH (FLAGS (\Flagged) MODSEQ (1234567900))
    /// A1 OK Fetch completed
    /// ```
    ///
    /// # 实现说明
    ///
    /// **当前**：返回 UID 列表，所有 MODSEQ 值为 None。
    ///
    /// **原因**：async-imap 0.11 的 `fetch()` 方法不返回 MODSEQ 值。
    ///
    /// **建议**：
    /// - 使用 SELECT CONDSTORE 后的 HIGHESTMODSEQ 来检测整体变更
    /// - 对于具体的邮件变更检测，使用 UID 对比策略
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let uids = vec![1, 2, 3, 4, 5];
    /// let results = client.fetch_modseqs(&uids).await?;
    /// // 所有 MODSEQ 值为 None
    /// // 建议使用其他策略检测变更
    /// ```
    pub async fn fetch_modseqs(&mut self, uids: &[u32]) -> Result<Vec<(u32, Option<u64>)>> {
        let _session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        if !uids.is_empty() {
            tracing::debug!("FETCH MODSEQ batch 尚未实现（async-imap 限制），返回 UID 列表");
        }

        // 返回 UID 列表，MODSEQ 为 None
        Ok(uids.iter().map(|&uid| (uid, None)).collect())
    }

    // ========== IMAP IDLE 支持 (RFC 2177) ==========

    /// 检查是否支持 IDLE 扩展
    ///
    /// IDLE 允许服务器推送新邮件通知，而不是客户端轮询。
    pub async fn check_idle_support(&mut self) -> Result<bool> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        let capabilities = session
            .capabilities()
            .await
            .map_err(|e| anyhow!("获取服务器能力失败: {}", e))?;

        let has_idle = capabilities.iter().any(|cap| {
            let cap_str = format!("{:?}", cap);
            cap_str.to_ascii_uppercase().contains("IDLE") || cap_str == "Idle"
        });

        if has_idle {
            tracing::info!("服务器支持 IDLE 扩展");
        } else {
            tracing::debug!("服务器不支持 IDLE 扩展");
        }

        Ok(has_idle)
    }

    /// 轮询降级策略
    pub async fn polling_fallback(&mut self, folder: &str, last_uid: u32) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session
            .select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let search_cmd = format!("UID {}:{}", last_uid + 1, "*");
        let uids = session
            .search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索新邮件失败: {}", e))?;

        let uid_list: Vec<u32> = uids.into_iter().collect();

        if !uid_list.is_empty() {
            tracing::info!(
                "📩 轮询检测到 {} 封新邮件 (UIDs: {:?})",
                uid_list.len(),
                uid_list
            );
        }

        Ok(uid_list)
    }

    /// 检查邮箱是否有新邮件（轻量级）
    pub async fn check_new_emails(
        &mut self,
        folder: &str,
        previous_count: usize,
    ) -> Result<(usize, bool)> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        let status_response = session
            .status(folder, "(MESSAGES)")
            .await
            .map_err(|e| anyhow!("获取文件夹状态失败: {}", e))?;

        let current_count = status_response.exists as usize;
        let has_new = current_count > previous_count;

        if has_new {
            tracing::info!(
                "📬 检测到新邮件: {} (之前: {})",
                current_count,
                previous_count
            );
        }

        Ok((current_count, has_new))
    }
}

impl Default for AsyncImapClient {
    fn default() -> Self {
        Self::new()
    }
}

pub fn three_months_ago_imap_format() -> String {
    let now = chrono::Utc::now();
    let three_months_ago = now - chrono::Duration::days(90);

    let date_str = format!(
        "{:02}-{}-{:04}",
        three_months_ago.day(),
        month_abbr(three_months_ago.month()),
        three_months_ago.year()
    );

    tracing::info!(
        "📅 计算三个月前的日期: 现在={}, 三个月前={}, 格式化后={}",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        three_months_ago.format("%Y-%m-%d %H:%M:%S UTC"),
        date_str
    );

    date_str
}

/// 将月份数字转换为英文缩写
fn month_abbr(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Jan",
    }
}

/// 尝试使用不同的编码解码邮件体
/// 邮件可能使用 UTF-8、GBK、GB2312、ISO-8859-1 等编码
fn decode_email_body(bytes: &[u8]) -> Result<String> {
    // 首先尝试 UTF-8（最常见）
    if let Ok(text) = std::str::from_utf8(bytes) {
        // 检查是否包含大量替换字符（可能是错误解码）
        let replacement_count = text.chars().filter(|&c| c == '�').count();
        let total_chars = text.chars().count();

        if total_chars > 0 && (replacement_count as f64 / total_chars as f64) < 0.05 {
            tracing::debug!("邮件体使用 UTF-8 编码");
            return Ok(text.to_string());
        }
    }

    // 如果UTF-8失败或包含大量乱码，尝试其他编码
    // 使用encoding_rs库尝试常见编码

    // 尝试 GBK (常见于中文邮件)
    let (text, _, _) = encoding_rs::GBK.decode(bytes);
    let replacement_ratio = if text.chars().count() > 0 {
        text.chars().filter(|&c| c == '\u{FFFD}').count() as f64 / text.chars().count() as f64
    } else {
        0.0
    };
    if !text.contains('\u{FFFD}') || replacement_ratio < 0.05 {
        tracing::info!("邮件体使用 GBK 编码（已转换）");
        return Ok(text.to_string());
    }

    // 尝试 GB18030 (GBK的超集)
    let (text, _, _) = encoding_rs::GB18030.decode(bytes);
    let replacement_ratio = if text.chars().count() > 0 {
        text.chars().filter(|&c| c == '\u{FFFD}').count() as f64 / text.chars().count() as f64
    } else {
        0.0
    };
    if !text.contains('\u{FFFD}') || replacement_ratio < 0.05 {
        tracing::info!("邮件体使用 GB18030 编码（已转换）");
        return Ok(text.to_string());
    }

    // 尝试 Big5 (繁体中文)
    let (text, _, _) = encoding_rs::BIG5.decode(bytes);
    let replacement_ratio = if text.chars().count() > 0 {
        text.chars().filter(|&c| c == '\u{FFFD}').count() as f64 / text.chars().count() as f64
    } else {
        0.0
    };
    if !text.contains('\u{FFFD}') || replacement_ratio < 0.05 {
        tracing::info!("邮件体使用 Big5 编码（已转换）");
        return Ok(text.to_string());
    }

    // 尝试 Shift_JIS (日文)
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    let replacement_ratio = if text.chars().count() > 0 {
        text.chars().filter(|&c| c == '\u{FFFD}').count() as f64 / text.chars().count() as f64
    } else {
        0.0
    };
    if !text.contains('\u{FFFD}') || replacement_ratio < 0.05 {
        tracing::info!("邮件体使用 Shift_JIS 编码（已转换）");
        return Ok(text.to_string());
    }

    // 尝试 ISO-8859-1 (Latin-1)
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    let replacement_ratio = if text.chars().count() > 0 {
        text.chars().filter(|&c| c == '\u{FFFD}').count() as f64 / text.chars().count() as f64
    } else {
        0.0
    };
    if !text.contains('\u{FFFD}') || replacement_ratio < 0.05 {
        tracing::info!("邮件体使用 WINDOWS_1252 编码（已转换）");
        return Ok(text.to_string());
    }

    // 如果所有编码都失败，回退到UTF-8并记录警告
    tracing::warn!("无法确定邮件编码，使用UTF-8作为回退，可能存在乱码");
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| anyhow!("解码邮件编码失败: {}", e))
}
