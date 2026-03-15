use anyhow::{anyhow, Result};
use futures::{TryStreamExt};
use std::time::Instant;
use super::{types::{EmailData, FolderInfo, SpecialUse, EmailFlags}, ImapAuth};
use tokio::net::TcpStream;
use chrono::Datelike;

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
        let tls_stream = tls_connector.connect(host, tcp)
            .await
            .map_err(|e| anyhow!("TLS 握手失败: {}", e))?;

        // 创建 IMAP 客户端
        let client = async_imap::Client::new(tls_stream);

        // 异步登录
        let login_start = Instant::now();
        let session = client.login(email, match &auth {
            ImapAuth::Password(pwd) => pwd.as_str(),
        }).await
            .map_err(|(e, _)| anyhow!("IMAP 登录失败: {}", e))?;

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
    pub async fn list_folders_with_attributes(&mut self) -> Result<Vec<FolderInfo>> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // 使用 list 命令获取文件夹列表（返回流）
        let folders: Vec<async_imap::types::Name> = session.list(None, Some("*"))
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

    /// 解析 RFC 6154 Special-Use 属性
    /// async-imap 0.11 的属性处理方式不同，暂时使用名称匹配
    fn parse_special_use(_attrs: &[async_imap::types::NameAttribute]) -> Option<SpecialUse> {
        // 暂时返回 None，依赖 determine_standard_name 的名称推断
        // TODO: 研究异步 IMAP 库的属性 API
        None
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
            }.to_string();
        }

        // 根据名称推断
        let name_lower = name.to_lowercase();
        if name_lower.contains("inbox") || name_lower == "inbox" {
            "inbox".to_string()
        } else if name_lower.contains("sent") {
            "sent".to_string()
        } else if name_lower.contains("draft") {
            "drafts".to_string()
        } else if name_lower.contains("spam") || name_lower.contains("junk") {
            "spam".to_string()
        } else if name_lower.contains("trash") || name_lower.contains("deleted") {
            "trash".to_string()
        } else if name_lower.contains("archive") {
            "archive".to_string()
        } else {
            name.to_string()
        }
    }

    /// 异步获取 UID 列表
    pub async fn list_uids(&mut self, folder: &str, limit: usize) -> Result<Vec<u32>> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // SEARCH ALL 获取所有邮件 UID（返回 Result<HashSet<Seq>>）
        let uids = session.search("ALL")
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        let mut uid_list: Vec<u32> = uids.into_iter().map(|uid| uid.into()).collect();
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
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // SEARCH UID <min_uid:* 获取大于指定 UID 的邮件（返回 Result<HashSet<Seq>>）
        let search_cmd = format!("UID {}:*", min_uid + 1);
        let uids = session.search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        let mut uid_list: Vec<u32> = uids.into_iter().map(|uid| uid.into()).collect();
        uid_list.sort();
        uid_list.reverse();

        Ok(uid_list)
    }

    /// 获取指定时间范围内的邮件 UID 列表
    /// date_since: Unix 时间戳（秒）
    pub async fn list_uids_since_timestamp(&mut self, folder: &str, since_timestamp: i64) -> Result<Vec<u32>> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // 获取所有邮件 UID
        let uids = session.search("ALL")
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        // 转换为向量并排序（最新在前）
        let mut uid_list: Vec<u32> = uids.into_iter().map(|uid| uid.into()).collect();
        uid_list.sort();
        uid_list.reverse();

        Ok(uid_list)
    }

    /// 获取指定时间范围内的邮件 UID 列表（使用 IMAP SINCE 命令）
    /// date_since: IMAP 日期格式，如 "01-Jan-2025"
    pub async fn list_uids_since(&mut self, folder: &str, date_since: &str) -> Result<Vec<u32>> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // 使用 SINCE 命令搜索指定日期之后的邮件（返回 Result<HashSet<Seq>>）
        // 注意：SINCE 命令的日期格式是 "01-Jan-2025"（不需要双引号，根据 RFC 3501）
        let search_cmd = format!("SINCE {}", date_since);
        tracing::debug!("使用 IMAP 搜索命令: {}", search_cmd);

        let uids = session.search(&search_cmd)
            .await
            .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

        let mut uid_list: Vec<u32> = uids.into_iter().map(|uid| uid.into()).collect();
        uid_list.sort();
        uid_list.reverse();

        Ok(uid_list)
    }

    /// 异步获取邮件数据
    pub async fn fetch_email(&mut self, folder: &str, uid: u32) -> Result<EmailData> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        // SELECT 文件夹（返回 Result<Mailbox>）
        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        // FETCH 邮件内容（RFC822 和 FLAGS）（返回流）
        let uid_str = uid.to_string();
        let messages: Vec<async_imap::types::Fetch> = session.fetch(&uid_str, "(RFC822 FLAGS)")
            .await
            .map_err(|e| anyhow!("获取邮件失败: {}", e))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| anyhow!("收集邮件数据失败: {}", e))?;

        // 获取第一封邮件
        let message = messages.iter().next()
            .ok_or_else(|| anyhow!("邮件 {} 不存在", uid))?;

        // 解析邮件体
        let body = message.body()
            .ok_or_else(|| anyhow!("邮件体为空"))?;

        let raw_email = std::str::from_utf8(body)
            .map_err(|e| anyhow!("解析邮件编码失败: {}", e))?;

        // 使用 parser 模块解析邮件
        let email_data = super::parser::parse_email_with_mail_parser(raw_email, uid)?;

        // 解析标志（在返回前收集所有标志状态）
        let seen = message.flags().any(|f| f == async_imap::types::Flag::Seen);
        let flagged = message.flags().any(|f| f == async_imap::types::Flag::Flagged);
        let answered = message.flags().any(|f| f == async_imap::types::Flag::Answered);
        let deleted = message.flags().any(|f| f == async_imap::types::Flag::Deleted);

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

    /// 异步标记邮件为已读/未读
    pub async fn mark_as_read(&mut self, folder: &str, uid: u32, is_read: bool) -> Result<()> {
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        // 使用 STORE 命令设置/清除标志（返回流）
        // async-imap 0.11: flags 需要放在查询字符串中
        if is_read {
            session.store(&uid_str, "+FLAGS (\\Seen)")
                .await
                .map_err(|e| anyhow!("标记邮件失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集标记结果失败: {}", e))?;
        } else {
            session.store(&uid_str, "-FLAGS (\\Seen)")
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
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        if flagged {
            session.store(&uid_str, "+FLAGS (\\Flagged)")
                .await
                .map_err(|e| anyhow!("设置标志失败: {}", e))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| anyhow!("收集设置标志结果失败: {}", e))?;
        } else {
            session.store(&uid_str, "-FLAGS (\\Flagged)")
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
        let session = self.session.as_mut()
            .ok_or_else(|| anyhow!("IMAP 未连接"))?;

        session.select(folder)
            .await
            .map_err(|e| anyhow!("选择文件夹失败: {}", e))?;

        let uid_str = uid.to_string();

        // 标记为删除（返回流）
        session.store(&uid_str, "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| anyhow!("标记删除失败: {}", e))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| anyhow!("收集标记删除结果失败: {}", e))?;

        // 执行删除（返回流）
        session.expunge()
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
            session.logout()
                .await
                .map_err(|e| anyhow!("登出失败: {}", e))?;
        }
        Ok(())
    }
}

impl Default for AsyncImapClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算一年前的日期并返回 IMAP 格式 (dd-Mon-yyyy)
/// 例如: "01-Jan-2025"
pub fn one_year_ago_imap_format() -> String {
    let one_year_ago = chrono::Utc::now() - chrono::Duration::days(365);

    format!(
        "{:02}-{}-{:04}",
        one_year_ago.day(),
        month_abbr(one_year_ago.month()),
        one_year_ago.year()
    )
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
