use super::{
    types::{EmailData, EmailFlags, FolderInfo, SpecialUse},
    ImapAuth,
};
use anyhow::{anyhow, Result};
use chrono::Datelike;
use futures::TryStreamExt;
use std::time::Instant;
use tokio::net::TcpStream;
use crate::services::oauth_service::OAuthService;

/// 异步 IMAP 客户端会话
pub struct AsyncImapClient {
    session: Option<async_imap::Session<tokio_native_tls::TlsStream<TcpStream>>>,
}

impl AsyncImapClient {
    pub fn new() -> Self {
        Self { session: None }
    }

    /// 异步连接到 IMAP服务器并登录
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
                let oauth_service = OAuthService::default();
                let xoauth2_str = oauth_service.generate_xoauth2_string(oauth_email, access_token);

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

        tracing::warn!("OAuth2 IMAP认证需要特殊处理，当前实现可能需要改进");

        // 临时解决方案：尝试使用一个占位实现
        // 在实际应用中，你需要：
        // 1. 升级async-imap到支持authenticate的版本
        // 2. 或手动实现SASL认证流程
        // 3. 或使用其他支持OAuth2的IMAP库

        // 这里我们返回一个错误，指示需要实现OAuth2认证
        return Err(anyhow!("OAuth2 IMAP认证需要进一步实现。请考虑：\n\
            1. 升级async-imap到支持SASL的版本\n\
            2. 手动实现IMAP AUTHENTICATE命令\n\
            3. 使用支持OAuth2的其他IMAP库"));

        // 如果async-imap支持authenticate方法，正确的实现应该是：
        // client.authenticate("XOAUTH2", xoauth2_str).await
        //     .map_err(|(e, _)| anyhow!("IMAP OAuth2登录失败: {}", e))
    }

    /// 异步列出服务器上的所有文件夹及其属性（RFC 6154）
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
    pub async fn fetch_folder_metadata(&mut self, folder: &str) -> Result<super::types::FolderMetadata> {
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
        let uidvalidity = mailbox
            .uid_validity
            .map(|v| v as u64)
            .unwrap_or(1); // 默认值为 1

        let uidnext = mailbox
            .uid_next
            .map(|v| v as u64)
            .unwrap_or(1); // 默认值为 1

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
        if name_lower.contains("inbox") || name_lower == "inbox" || name_lower.contains("收件箱") {
            "inbox".to_string()
        } else if name_lower.contains("sent") || name_lower.contains("已发送") {
            "sent".to_string()
        } else if name_lower.contains("draft") || name_lower.contains("草稿") {
            "drafts".to_string()
        } else if name_lower.contains("spam") || name_lower.contains("junk") || name_lower.contains("垃圾邮件") {
            "spam".to_string()
        } else if name_lower.contains("trash") || name_lower.contains("deleted") || name_lower.contains("已删除") {
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

        tracing::info!("🔍 list_uids_since 开始: folder={}, date_since={}", folder, date_since);

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

        tracing::info!("📥 SINCE 命令返回 {} 个 UID (预期: 所有近一年的邮件)", uids.len());
        tracing::info!("📊 比例: {}/{} ({:.1}%)", uids.len(), all_uids.len(),
            (uids.len() as f64 / all_uids.len() as f64) * 100.0);

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
    pub async fn fetch_email_headers(&mut self, folder: &str, uid: u32) -> Result<super::types::EmailHeader> {
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
        let flagged = message.flags().any(|f| f == async_imap::types::Flag::Flagged);
        let answered = message.flags().any(|f| f == async_imap::types::Flag::Answered);
        let deleted = message.flags().any(|f| f == async_imap::types::Flag::Deleted);

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
