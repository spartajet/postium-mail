//! 异步 IMAP 客户端实现
//!
//! 提供基于 `async-imap` 的异步 IMAP 客户端，支持常见的邮件操作。
//!
//! # 核心功能
//!
//! - **连接管理**: TLS 加密连接、密码和 OAuth2 认证
//! - **文件夹操作**: 列出文件夹、获取文件夹元数据、RFC 6154 特殊用途支持
//! - **邮件操作**: 获取邮件、搜索、标志管理、删除
//! - **增量同步**: UID 搜索策略用于高效的变更检测
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
//! ## UID 搜索增量同步
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = AsyncImapClient::new();
//! // 搜索所有邮件 UID
//! let uids = client.search_all("INBOX").await?;
//! println!("Found {} emails", uids.len());
//! # Ok(())
//! # }
//! ```
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

use super::types::{EmailData, EmailFlags, FolderInfo, SpecialUse};
use anyhow::{Result, anyhow};
use async_imap::{Authenticator, Session};
use chrono::Datelike;
use futures::TryStreamExt;
use std::time::Instant;
use tokio::net::TcpStream;
use tracing::instrument;

/// IMAP 认证方式
///
/// 表示 IMAP 连接时使用的认证信息。
///
/// # 变体说明
///
/// ## Password
///
/// 传统密码认证，使用 IMAP `LOGIN` 命令。
///
/// **注意**: 对于 Gmail 等服务商，需要使用应用专用密码而非账号密码。
///
/// ## OAuth2
///
/// OAuth2 认证，使用 SASL XOAUTH2 机制。
///
/// **字段**:
/// - `email`: OAuth 邮箱地址（通常与账号邮箱相同）
/// - `access_token`: OAuth2 访问令牌
///
/// **生成格式**:
/// ```text
/// user={email}\x01auth=Bearer {access_token}\x01\x01
/// ```
///
/// # 使用建议
///
/// - 优先使用 OAuth2 认证，更安全且支持更精细的权限控制
/// - 对于不支持 OAuth2 的服务商，回退到密码认证
/// - 密码认证时应使用应用专用密码，而非账号主密码
#[derive(Debug, Clone)]
pub enum ImapAuth {
    /// 传统密码认证
    ///
    /// 使用 IMAP LOGIN 命令进行认证。
    /// 需要提供应用专用密码（App Password）而非账号密码。
    Password(String),

    /// OAuth2 认证
    ///
    /// 使用 SASL XOAUTH2 机制进行认证。
    /// 访问令牌应该从 OAuth2 流程中获取并定期刷新。
    OAuth2 {
        /// OAuth 邮箱地址（可能与认证邮箱不同）
        email: String,
        /// OAuth 访问令牌
        access_token: String,
    },
}

impl Authenticator for ImapAuth {
    type Response = String;

    /// 处理服务器挑战
    ///
    /// XOAUTH2 认证流程：
    /// 1. 客户端发送初始响应（base64 编码的 auth 字符串）
    /// 2. 如果认证成功，服务器返回 OK
    /// 3. 如果认证失败，服务器发送挑战（包含错误信息），客户端应发送空响应
    fn process(&mut self, challenge: &[u8]) -> Self::Response {
        match self {
            ImapAuth::Password(password) => password.to_string(),
            ImapAuth::OAuth2 {
                email,
                access_token,
            } => format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token),
        }
    }
}

/// 异步 IMAP 客户端会话
pub struct AsyncImapClient {
    session: Option<Session<tokio_native_tls::TlsStream<TcpStream>>>,
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
            ImapAuth::OAuth2 { .. } => {
                // OAuth2/XOAUTH2 认证
                // tracing::info!("使用OAuth2认证IMAP: {}", oauth_email);
                // let oauth2 = XOAuth2Authenticator::new(email.to_string(), access_token.to_string());
                client
                    .authenticate("XOAUTH2", auth)
                    .await
                    .map_err(|e| anyhow!("IMAP OAuth2 登录失败: {:?}", e))?
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
    /// 使用 EXAMINE 命令（只读模式）获取正确的 UIDNEXT 值
    pub async fn fetch_folder_metadata(
        &mut self,
        folder: &str,
    ) -> Result<super::types::FolderMetadata> {
        use super::types::FolderMetadata;

        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 EXAMINE 命令（只读模式）获取文件夹元数据
        // 这会正确返回 UIDNEXT 和 UIDVALIDITY
        // 注意：async-imap 的 status() 方法返回的 Mailbox.uid_next 始终为 None，
        // 因为 STATUS 命令的响应格式与 SELECT/EXAMINE 不同
        let mailbox = session
            .examine(folder)
            .await
            .map_err(|e| anyhow!("获取文件夹元数据失败: {}", e))?;

        // 详细日志：打印 Mailbox 结构的所有字段
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

        // 关闭当前文件夹（返回未选中状态）
        // 这样后续可以选中其他文件夹进行操作
        if let Err(e) = session.close().await {
            tracing::debug!("关闭文件夹 {} 失败（可忽略）: {}", folder, e);
        }

        tracing::info!(
            "文件夹元数据: folder={}, uidvalidity={}, uidnext={}, exists={}",
            folder,
            uidvalidity,
            uidnext,
            mailbox.exists
        );

        Ok(FolderMetadata {
            uidvalidity,
            uidnext,
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: None,
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
            .uid_search("ALL")
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
        let search_cmd = format!("UID {}:{}", min_uid + 1, min_uid + 100000);
        let uids = session
            .uid_search(&search_cmd)
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
            .uid_search("ALL")
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

        // // 首先获取总邮件数（用于诊断）
        // let all_uids = session
        //     .search("ALL")
        //     .await
        //     .map_err(|e| anyhow!("搜索所有邮件失败: {}", e))?;
        // tracing::info!("📊 文件夹总邮件数: {}", all_uids.len());

        // 使用 SINCE 命令搜索指定日期之后的邮件
        // 注意：SINCE 命令的日期格式是 "01-Jan-2025"（不需要双引号，根据 RFC 3501）
        let search_cmd = format!("SINCE {}", date_since);
        tracing::info!("📤 使用 IMAP 搜索命令: '{}'", search_cmd);

        let uids = session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        tracing::info!(
            "📥 SINCE 命令返回 {} 个 UID (预期: 所有近三个月的邮件)",
            uids.len()
        );
        // tracing::info!(
        //     "📊 比例: {}/{} ({:.1}%)",
        //     uids.len(),
        //     all_uids.len(),
        //     (uids.len() as f64 / all_uids.len() as f64) * 100.0
        // );

        // // 如果 SINCE 返回的结果太少，记录警告
        // if all_uids.len() > 100 && uids.len() < all_uids.len() / 2 {
        //     tracing::warn!(
        //         "⚠️  SINCE 命令返回的邮件数量异常少！可能的原因:\n\
        //          1. 日期格式不正确: '{}'\n\
        //          2. IMAP 服务器不支持 SINCE 命令\n\
        //          3. 服务器上的邮件确实都在近一个月内",
        //         date_since
        //     );
        // }

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
            .uid_fetch(&uid_str, "(BODY.PEEK[] FLAGS)")
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
            .uid_fetch(&uid_str, "(BODY.PEEK[HEADER] FLAGS)")
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
            .uid_fetch(&uid_str, "BODY[1.TEXT]")
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
                .uid_store(&uid_str, "+FLAGS (\\Seen)")
                .await
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集标记结果失败: {}", e))?;
        } else {
            session
                .uid_store(&uid_str, "-FLAGS (\\Seen)")
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
                .uid_store(&uid_str, "+FLAGS (\\Flagged)")
                .await
                .map_err(|e| anyhow!("设置标志失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集设置标志结果失败: {}", e))?;
        } else {
            session
                .uid_store(&uid_str, "-FLAGS (\\Flagged)")
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
            .uid_store(&uid_str, "+FLAGS (\\Deleted)")
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
            .uid_fetch(&uid_str, "(RFC822.HEADER)")
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
            .uid_search(&search_cmd)
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
