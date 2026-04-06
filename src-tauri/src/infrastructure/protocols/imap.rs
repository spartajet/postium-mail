use crate::error::MailError;
use crate::infrastructure::protocols::types::{
    AttachmentInfo, EmailHeader, MailEnvelope, WholeEmailDto,
};
use crate::infrastructure::protocols::utils::{
    extract_email_from_address, extract_name_from_address, format_address_list,
};
use crate::{
    domain::providers::ImapServerConfig, infrastructure::protocols::types::FolderMetadata,
};
use async_imap::imap_proto::{self, Envelope};
use base64::{Engine, prelude::BASE64_STANDARD};
use futures::{StreamExt, TryStreamExt};
use mail_parser::MessageParser;
use serde::{Deserialize, Serialize};
use specta::Type;
use utf7_imap::decode_utf7_imap;

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

/// 辅助：从 Cow<[u8]> 转为可读 String
fn cow_bytes_to_string(cow: &[u8]) -> String {
    String::from_utf8_lossy(cow).into_owned()
}

/// 辅助：从 Address 提取显示名
fn address_to_string(addr: &imap_proto::Address<'_>) -> Option<String> {
    addr.name
        .as_ref()
        .map(|n| cow_bytes_to_string(n))
        .or_else(|| addr.adl.as_ref().map(|m| cow_bytes_to_string(m)))
        .or_else(|| {
            // 拼成 user@host
            let mailbox = addr.mailbox.as_ref().map(|m| cow_bytes_to_string(m));
            let host = addr.host.as_ref().map(|h| cow_bytes_to_string(h));
            match (mailbox, host) {
                (Some(m), Some(h)) => Some(format!("{m}@{h}")),
                (Some(m), None) => Some(m),
                _ => None,
            }
        })
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

        let tcp = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| {
                MailError::ImapConnectionFailed(format!("连接 {host}:{port} 失败: {e}"))
            })?;

        // 所有模式均使用 TLS 连接（StartTLS/None 暂走隐式 TLS 路径）
        tracing::debug!(host, "IMAP: TCP 连接成功，开始 TLS 握手");
        let tls = Self::upgrade_tls(tcp, host).await?;
        let mut client = async_imap::Client::new(tls);

        // 读取服务器 greeting
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
        let authenticator = Xoauth2Authenticator {
            user: email.to_string(),
            access_token: access_token.to_string(),
        };
        tracing::debug!("IMAP: XOAUTH2 认证中: {:?}", authenticator);
        // let base64_string = authenticator.generate_xoauth2_string(email, access_token);
        // tracing::info!(base64_string, "Base64 字符串已经生成");
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

    // ─── 邮件获取 ───

    /// 获取指定时间范围内的邮件 UID 列表（使用 IMAP SINCE 命令）
    /// date_since: IMAP 日期格式，如 "01-Jan-2025"
    pub async fn list_uids_since(
        &mut self,
        folder: &str,
        date_since: &str,
    ) -> Result<Vec<u32>, MailError> {
        tracing::info!(
            "🔍 list_uids_since 开始: folder={}, date_since={}",
            folder,
            date_since
        );

        // SELECT 文件夹（返回 Result<Mailbox>）
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let search_cmd = format!("SINCE {}", date_since);
        tracing::info!("📤 使用 IMAP 搜索命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        tracing::info!(
            "📥 SINCE 命令返回 {} 个 UID (预期: 所有近三个月的邮件)",
            uids.len()
        );

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.reverse();

        if !uid_list.is_empty() {
            uid_list.sort();
            tracing::info!(
                "   UID 范围: {} ~ {}",
                uid_list.first().unwrap_or(&0),
                uid_list.last().unwrap_or(&0),
            );
        }

        Ok(uid_list)
    }

    pub async fn list_uids_since_uid(
        &mut self,
        folder: &str,
        uid_since: u32,
    ) -> Result<Vec<u32>, MailError> {
        // tracing::info!("📤 使用 IMAP 搜索命令: 'UID {}:*'", uid_since);
        // SELECT 文件夹（返回 Result<Mailbox>）
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;
        let search_cmd = format!("UID {}:{}", uid_since, uid_since + 100);
        tracing::info!("📤 使用 IMAP 搜索命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();

        if !uid_list.is_empty() {
            uid_list.sort();
            tracing::info!(
                "   UID 范围: {} ~ {}",
                uid_list.first().unwrap_or(&0),
                uid_list.last().unwrap_or(&0),
            );
        }

        Ok(uid_list)
    }

    /// 批量获取邮件头（用于骨架同步，不获取正文）
    ///
    /// 使用 UID FETCH 命令批量获取多个邮件的头信息，避免设置已读标志。
    ///
    /// # 参数
    ///
    /// * `folder` - 文件夹名称
    /// * `start_uid` - 起始 UID
    /// * `end_uid` - 结束 UID
    ///
    /// # 返回
    ///
    /// 返回邮件头列表
    ///
    /// # IMAP 命令示例
    ///
    /// ```text
    /// UID FETCH 101:110 (FLAGS INTERNALDATE RFC822.SIZE ENVELOPE BODYSTRUCTURE UID)
    /// ```
    ///
    /// # 性能优化
    ///
    /// - 使用 UID 范围（如 101:110）批量获取
    /// - 使用 BODY.PEEK[HEADER] 避免设置已读标志
    /// - 单次网络往返获取多个邮件头
    pub async fn batch_fetch_email_headers(
        &mut self,
        folder: &str,
        start_uid: u32,
        end_uid: u32,
    ) -> Result<Vec<EmailHeader>, MailError> {
        // SELECT 文件夹
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        // 构建 UID 范围（如 "101:110"）
        let uid_range = format!("{}:{}", start_uid, end_uid);

        tracing::trace!("批量获取邮件头: folder={}, uid_range={}", folder, uid_range);

        // 批量 FETCH 邮件头
        // 使用 BODY.PEEK[HEADER] 不会设置已读标志
        let fetches = self
            .session
            .uid_fetch(
                &uid_range,
                "(FLAGS INTERNALDATE RFC822.SIZE ENVELOPE BODYSTRUCTURE UID)",
            )
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        tracing::debug!("获取邮件头message count: {}", fetches.len());

        let mut headers = Vec::new();

        for fetch in fetches {
            if fetch.uid.is_none() {
                tracing::warn!("Fetch missing UID, skipping");
                continue;
            }
            let uid = fetch.uid.unwrap();

            let seen = fetch.flags().any(|f| f == async_imap::types::Flag::Seen);
            let flagged = fetch.flags().any(|f| f == async_imap::types::Flag::Flagged);
            let answered = fetch
                .flags()
                .any(|f| f == async_imap::types::Flag::Answered);
            let deleted = fetch.flags().any(|f| f == async_imap::types::Flag::Deleted);
            let draft = fetch.flags().any(|f| f == async_imap::types::Flag::Draft);
            let recent = fetch.flags().any(|f| f == async_imap::types::Flag::Recent);

            // 解析 ENVELOPE 获取邮件头信息
            let mail_envelope = fetch
                .envelope()
                .and_then(|enve| self.parse_envelope(enve).ok());

            if let Some(enve) = mail_envelope {
                // 从 BODYSTRUCTURE 解析附件元信息
                let attachments = fetch
                    .bodystructure()
                    .map(|bs| Self::extract_attachments(bs, ""))
                    .unwrap_or_default();

                tracing::trace!(uid, attachment_count = attachments.len(), "解析附件完成");

                headers.push(EmailHeader {
                    uid,
                    subject: enve.subject,
                    from: enve.from,
                    to: enve.to,
                    cc: enve.cc,
                    date: enve.date,
                    flags: super::types::EmailFlags {
                        seen,
                        flagged,
                        answered,
                        deleted,
                        draft,
                        recent,
                    },
                    attachments,
                });
            }
        }

        tracing::debug!(
            "批量获取邮件头完成: folder={}, count={}",
            folder,
            headers.len()
        );

        Ok(headers)
    }

    /// 按 UID 范围获取邮件头
    pub async fn fetch_uids(
        &mut self,
        start: u32,
        end: u32,
    ) -> Result<Vec<RawEmailHeader>, MailError> {
        let range = if start == 0 && end == 0 {
            "1:*".to_string()
        } else if end == 0 {
            format!("{start}:*")
        } else {
            format!("{start}:{end}")
        };
        tracing::debug!(range = %range, "IMAP: 获取邮件头");

        let mut fetches = self
            .session
            .uid_fetch(&range, "(ENVELOPE FLAGS UID)")
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取邮件头失败: {e}")))?;

        let mut headers = Vec::new();
        while let Some(fetch_result) = fetches.next().await {
            let fetch = fetch_result
                .map_err(|e| MailError::ImapConnectionFailed(format!("解析邮件头失败: {e}")))?;
            let uid = fetch.uid.unwrap_or(0);

            let envelope = match fetch.envelope() {
                Some(e) => e,
                None => {
                    headers.push(RawEmailHeader {
                        uid,
                        flags: fetch.flags().map(|f| format!("{f:?}")).collect(),
                        subject: None,
                        from: None,
                        to: None,
                        date: None,
                        message_id: None,
                    });
                    continue;
                }
            };

            let subject = envelope
                .subject
                .as_ref()
                .map(|s| cow_bytes_to_string(s.as_ref()));
            let from = envelope
                .from
                .as_ref()
                .and_then(|addrs| addrs.first().and_then(|a| address_to_string(a)));
            let to = envelope
                .to
                .as_ref()
                .and_then(|addrs| addrs.first().and_then(|a| address_to_string(a)));
            let date = envelope
                .date
                .as_ref()
                .map(|s| cow_bytes_to_string(s.as_ref()));
            let message_id = envelope
                .message_id
                .as_ref()
                .map(|s| cow_bytes_to_string(s.as_ref()));

            let flags: Vec<String> = fetch.flags().map(|f| format!("{f:?}")).collect();

            headers.push(RawEmailHeader {
                uid,
                flags,
                subject,
                from,
                to,
                date,
                message_id,
            });
        }
        tracing::debug!(count = headers.len(), "IMAP: 鎷取邮件头完成");
        Ok(headers)
    }

    /// 获取邮件完整内容 (RFC822)
    pub async fn fetch_body(&mut self, uid: u32) -> Result<(String, String), MailError> {
        tracing::debug!(uid, "IMAP: 获取邮件体");
        let mut fetches = self
            .session
            .uid_fetch(format!("{}", uid), "BODY.PEEK[]")
            .await
            .map_err(|e| MailError::ImapError(format!("获取邮件体失败: {e}")))?;

        let body_result = if let Some(fetch) = fetches.next().await
            && let Ok(fetch_content) =
                fetch.map_err(|e| MailError::ImapError(format!("解析邮件体失败: {e}")))
            && let Ok(fetch_body_raw) = fetch_content
                .body()
                .ok_or(|| MailError::ImapError("邮件体为空".to_string()))
            && let Ok(email_message) = MessageParser::default()
                .parse(fetch_body_raw)
                .ok_or(|| MailError::ImapError("解析邮件失败".to_string()))
            && let Ok(body_text) = email_message
                .body_text(0)
                .ok_or(|| MailError::ImapError("获取邮件内容失败".to_string()))
            && let Ok(body_html) = email_message
                .body_html(0)
                .ok_or(|| MailError::ImapError("获取邮件内容失败".to_string()))
        {
            (body_text.to_string(), body_html.to_string())
        } else {
            return Err(MailError::ImapError("获取邮件内容失败".to_string()));
        };

        Ok(body_result)
    }

    /// 获取邮件的 FLAGS 和 UID（用于增量同步）
    pub async fn fetch_flags(
        &mut self,
        uid_set: &str,
    ) -> Result<Vec<(u32, Vec<String>)>, MailError> {
        tracing::debug!(uid_set = %uid_set, "IMAP: 获取邮件标志");
        let mut fetches = self
            .session
            .uid_fetch(uid_set, "(FLAGS UID)")
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取标志失败: {e}")))?;

        let mut results = Vec::new();
        while let Some(fetch_result) = fetches.next().await {
            let fetch = fetch_result
                .map_err(|e| MailError::ImapConnectionFailed(format!("解析标志失败: {e}")))?;
            let uid = fetch.uid.unwrap_or(0);
            let flags = fetch.flags().map(|f| format!("{f:?}")).collect();
            results.push((uid, flags));
        }
        Ok(results)
    }

    /// 按 UID 范围批量获取完整邮件（包含正文）
    ///
    /// 与 `batch_fetch_email_headers` 类似，但额外获取 BODY.PEEK[] 并用
    /// `mail_parser` 解析出纯文本/HTML 正文、预览文本等。
    ///
    /// # 注意
    ///
    /// `id`、`account_id`、`created_at` 为数据库侧字段，此处设为默认值（0 / 当前时间），
    /// 由调用者在持久化后填充。
    pub async fn batch_fetch_emails(
        &mut self,
        folder: &str,
        start_uid: u32,
        end_uid: u32,
    ) -> Result<Vec<WholeEmailDto>, MailError> {
        // SELECT 文件夹
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        let uid_range = format!("{}:{}", start_uid, end_uid);
        tracing::trace!(
            "批量获取完整邮件: folder={}, uid_range={}",
            folder,
            uid_range
        );

        // 批量 FETCH：ENVELOPE + FLAGS + BODYSTRUCTURE + BODY.PEEK[] + INTERNALDATE + UID
        let fetches = self
            .session
            .uid_fetch(
                &uid_range,
                "(FLAGS INTERNALDATE ENVELOPE BODYSTRUCTURE BODY.PEEK[1] BODY.PEEK[2] UID)",
            )
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        tracing::debug!("获取完整邮件数量: {}", fetches.len());

        let now = chrono::Utc::now().timestamp();
        let mut emails = Vec::new();

        for fetch in fetches {
            let uid = match fetch.uid {
                Some(uid) => uid,
                None => {
                    tracing::warn!("Fetch missing UID, skipping");
                    continue;
                }
            };

            // ─── FLAGS ───
            let seen = fetch.flags().any(|f| f == async_imap::types::Flag::Seen);
            let flagged = fetch.flags().any(|f| f == async_imap::types::Flag::Flagged);
            let answered = fetch
                .flags()
                .any(|f| f == async_imap::types::Flag::Answered);
            let deleted = fetch.flags().any(|f| f == async_imap::types::Flag::Deleted);
            let draft = fetch.flags().any(|f| f == async_imap::types::Flag::Draft);

            // ─── INTERNALDATE → received_at ───
            let received_at = fetch.internal_date().map(|d| d.timestamp()).unwrap_or(now);

            // ─── ENVELOPE ───
            let mail_envelope = fetch
                .envelope()
                .and_then(|enve| self.parse_envelope(enve).ok());

            // ─── BODY.PEEK[] → 解析正文 ───
            let mut body_text: Option<String> = None;
            let mut body_html: Option<String> = None;
            let mut preview: Option<String> = None;
            let mut sender_name: Option<String> = None;
            let mut sender_email = String::new();
            let mut recipient_emails = String::new();
            let mut message_id: Option<String> = None;
            let mut sent_at = now;

            if let Some(raw) = fetch.body()
                && let Some(msg) = mail_parser::MessageParser::default().parse(raw)
            {
                body_text = msg.body_text(0).map(|t| t.to_string());
                body_html = msg.body_html(0).map(|t| t.to_string());
                preview = body_text.as_ref().map(|t| t.chars().take(200).collect());

                sender_name = msg
                    .from()
                    .and_then(|a| a.first().and_then(|p| p.name().map(|n| n.to_string())));
                sender_email = msg
                    .from()
                    .and_then(|a| a.first().and_then(|p| p.address().map(|a| a.to_string())))
                    .unwrap_or_default();
                recipient_emails = msg
                    .to()
                    .map(|addr| {
                        addr.iter()
                            .filter_map(|a| a.address().map(|a| a.to_string()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                message_id = msg.message_id().map(|s| s.to_string());
                sent_at = msg.date().map(|d| d.to_timestamp()).unwrap_or(now);
            }

            // ENVELOPE 兜底：当 body 解析失败时从 ENVELOPE 取地址信息
            if let Some(enve) = &mail_envelope {
                if sender_email.is_empty() {
                    sender_email = extract_email_from_address(&enve.from);
                }
                if sender_name.is_none() {
                    sender_name = extract_name_from_address(&enve.from);
                }
                if recipient_emails.is_empty() {
                    recipient_emails = enve.to.clone();
                }
                if message_id.is_none() {
                    message_id = fetch.envelope().and_then(|e| {
                        e.message_id
                            .as_ref()
                            .map(|m| String::from_utf8_lossy(m.as_ref()).into_owned())
                    });
                }
                if sent_at == now {
                    sent_at = enve.date.timestamp();
                }
            }

            // ─── BODYSTRUCTURE → 附件 ───
            let attachments = fetch
                .bodystructure()
                .map(|bs| Self::extract_attachments(bs, ""))
                .unwrap_or_default();

            tracing::trace!(uid, attachment_count = attachments.len(), "解析附件完成");

            // ─── ENVELOPE 中的 cc / bcc ───
            let cc_emails = mail_envelope.as_ref().and_then(|e| {
                if e.cc.is_empty() {
                    None
                } else {
                    Some(e.cc.clone())
                }
            });
            let bcc_emails = mail_envelope.as_ref().and_then(|e| {
                if e.bcc.is_empty() {
                    None
                } else {
                    Some(e.bcc.clone())
                }
            });

            emails.push(WholeEmailDto {
                // 数据库侧字段，暂用默认值，由调用者持久化后填充
                id: 0,
                account_id: 0,
                folder: folder.to_string(),
                uid,
                message_id,
                sender_name,
                sender_email,
                recipient_emails,
                cc_emails,
                bcc_emails,
                subject: mail_envelope
                    .as_ref()
                    .map(|e| e.subject.clone())
                    .filter(|s| !s.is_empty()),
                preview,
                body_text,
                body_html,
                attachments,
                is_read: seen,
                is_starred: flagged,
                is_draft: draft,
                is_answered: answered,
                is_deleted: deleted,
                sent_at,
                received_at,
                created_at: now,
            });
        }

        tracing::debug!(
            "批量获取完整邮件完成: folder={}, count={}",
            folder,
            emails.len()
        );

        Ok(emails)
    }

    /// 获取文件夹 IMAP 元数据（UIDVALIDITY, UIDNEXT 等）
    /// 使用 EXAMINE 命令（只读模式）获取正确的 UIDNEXT 值
    pub async fn fetch_folder_metadata(
        &mut self,
        folder: &str,
    ) -> Result<FolderMetadata, MailError> {
        use super::types::FolderMetadata;

        // 使用 EXAMINE 命令（只读模式）获取文件夹元数据
        // 这会正确返回 UIDNEXT 和 UIDVALIDITY
        // 注意：async-imap 的 status() 方法返回的 Mailbox.uid_next 始终为 None，
        // 因为 STATUS 命令的响应格式与 SELECT/EXAMINE 不同
        let mailbox = self
            .session
            .examine(folder)
            .await
            .map_err(|e| MailError::ImapFolderMetadataFailed(e.to_string()))?;

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

        tracing::info!(
            "文件夹元数据: folder={}, uidvalidity={}, uidnext={}, exists={}",
            folder,
            uidvalidity,
            uidnext,
            mailbox.exists
        );

        // 解码 IMAP UTF-7 编码的文件夹名称作为昵称
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
        // 消费 stream 以确保命令完成
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

    // ─── 内部方法 ───

    /// 建立 TCP+TLS 连接
    async fn connect_tls_stream(
        host: &str,
        port: u16,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let tcp = tokio::net::TcpStream::connect((host, port))
            .await
            .map_err(|e| {
                MailError::ImapConnectionFailed(format!("连接 {host}:{port} 失败: {e}"))
            })?;
        tracing::debug!(host, port, "IMAP连接成功: 正在进行 TLS 握手");
        Self::upgrade_tls(tcp, host).await
    }

    /// TCP → TLS 升级
    async fn upgrade_tls(
        tcp: tokio::net::TcpStream,
        host: &str,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>, MailError> {
        let connector = tokio_native_tls::native_tls::TlsConnector::new()
            .map_err(|e| MailError::ImapConnectionFailed(format!("TLS 构建失败: {e}")))?;
        let connector = tokio_native_tls::TlsConnector::from(connector);
        connector
            .connect(host, tcp)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("TLS 握手失败: {e}")))
    }

    /// 解析 IMAP ENVELOPE 响应
    ///
    /// 从服务器返回的 ENVELOPE 数据中提取邮件头信息，包括：
    /// - Subject: 邮件主题（使用自定义 RFC 2047 解码器）
    /// - From: 发件人地址列表
    /// - To: 收件人地址列表
    /// - Cc: 抄送地址列表
    /// - Bcc: 密送地址列表
    ///
    /// # 参数
    ///
    /// * `envelope` - IMAP ENVELOPE 响应
    ///
    /// # 返回
    ///
    /// 返回包含解码后邮件头信息的 `MailEnvelope`
    pub fn parse_envelope(&self, envelope: &Envelope) -> Result<MailEnvelope, MailError> {
        // 解析 Subject 字段
        // subject 是 Option<Cow<'_, [u8]>>
        let subject = envelope
            .subject
            .as_ref()
            .map(|subject| {
                // Cow<'_, [u8]> -> String
                let subject_str = String::from_utf8_lossy(subject.as_ref());
                // 使用自定义的 RFC 2047 解码器
                match rfc2047_decoder::decode(subject_str.as_bytes()) {
                    Ok(decoded) => decoded,
                    Err(_) => subject_str.into_owned(),
                }
            })
            .unwrap_or_default();

        // 解析 From 地址列表
        let from = envelope
            .from
            .as_ref()
            .map(|addrs| format_address_list(addrs.as_slice()))
            .unwrap_or_default();

        // 解析 To 地址列表
        let to = envelope
            .to
            .as_ref()
            .map(|addrs| format_address_list(addrs.as_slice()))
            .unwrap_or_default();

        // 解析 Cc 地址列表
        let cc = envelope
            .cc
            .as_ref()
            .map(|addrs| format_address_list(addrs.as_slice()))
            .unwrap_or_default();

        // 解析 Bcc 地址列表
        let bcc = envelope
            .bcc
            .as_ref()
            .map(|addrs| format_address_list(addrs.as_slice()))
            .unwrap_or_default();
        // 解析日期
        // date 是 Option<Cow<'_, [u8]>>，RFC 2822 格式
        let date = envelope
            .date
            .as_ref()
            .and_then(|date_bytes| {
                // 将 Cow<[u8]> 转换为字符串
                let date_str = String::from_utf8_lossy(date_bytes.as_ref());
                // 解析 RFC 2822 日期格式
                chrono::DateTime::parse_from_rfc2822(&date_str)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            })
            .unwrap_or_else(|| {
                tracing::warn!("IMAP ENVELOPE 日期解析失败，使用当前时间");
                chrono::Utc::now()
            });

        Ok(MailEnvelope {
            subject,
            from,
            to,
            cc,
            bcc,
            date,
        })
    }

    // ─── BODYSTRUCTURE 附件解析 ───

    /// 从 BODYSTRUCTURE 中递归提取附件信息
    ///
    /// 遍历 MIME 树，找到所有被判定为附件的叶子节点。
    /// `section` 参数用于跟踪当前节点在 MIME 树中的路径（如 "2"、"3.1"），
    /// 该路径后续用于 `UID FETCH BODY.PEEK[<section>]` 按需下载附件内容。
    fn extract_attachments(
        body: &imap_proto::BodyStructure<'_>,
        section: &str,
    ) -> Vec<AttachmentInfo> {
        let mut attachments = Vec::new();

        match body {
            imap_proto::BodyStructure::Basic { common, other, .. } => {
                if Self::is_attachment(common) {
                    attachments.push(Self::build_attachment_info(common, other, section));
                }
            }
            imap_proto::BodyStructure::Text { common, other, .. } => {
                if Self::is_attachment(common) {
                    attachments.push(Self::build_attachment_info(common, other, section));
                }
            }
            imap_proto::BodyStructure::Message {
                common,
                other,
                body: inner_body,
                ..
            } => {
                if Self::is_attachment(common) {
                    attachments.push(Self::build_attachment_info(common, other, section));
                }
                // 递归解析 message/rfc822 内部
                let inner_section = if section.is_empty() {
                    "1".to_string()
                } else {
                    format!("{}.1", section)
                };
                attachments.extend(Self::extract_attachments(inner_body, &inner_section));
            }
            imap_proto::BodyStructure::Multipart { bodies, .. } => {
                for (i, sub_body) in bodies.iter().enumerate() {
                    let sub_section = if section.is_empty() {
                        format!("{}", i + 1)
                    } else {
                        format!("{}.{}", section, i + 1)
                    };
                    attachments.extend(Self::extract_attachments(sub_body, &sub_section));
                }
            }
        }

        attachments
    }

    /// 判断一个 MIME 部分是否是附件
    ///
    /// 判定规则（满足任一即视为附件）：
    /// 1. Content-Disposition 为 "attachment"
    /// 2. Content-Disposition 包含 filename 参数
    /// 3. Content-Type 包含 name 参数
    fn is_attachment(common: &imap_proto::BodyContentCommon<'_>) -> bool {
        // Content-Disposition 为 "attachment"
        if let Some(ref disposition) = common.disposition {
            if disposition.ty.eq_ignore_ascii_case("attachment") {
                return true;
            }
            // Content-Disposition 有 filename 参数
            if disposition.params.as_ref().is_some_and(|params| {
                params
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("filename"))
            }) {
                return true;
            }
        }

        // Content-Type 有 name 参数
        if common
            .ty
            .params
            .as_ref()
            .is_some_and(|params| params.iter().any(|(k, _)| k.eq_ignore_ascii_case("name")))
        {
            return true;
        }

        false
    }

    /// 从 BODYSTRUCTURE 的单部分节点构建 AttachmentInfo
    fn build_attachment_info(
        common: &imap_proto::BodyContentCommon<'_>,
        other: &imap_proto::BodyContentSinglePart<'_>,
        section: &str,
    ) -> AttachmentInfo {
        let content_type = format!("{}/{}", common.ty.ty, common.ty.subtype);

        // 优先从 Content-Disposition 中获取 filename
        let filename = common
            .disposition
            .as_ref()
            .and_then(|d| {
                d.params.as_ref().and_then(|params| {
                    params
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("filename"))
                        .map(|(_, v)| v.clone().into_owned())
                })
            })
            .or_else(|| {
                // 其次从 Content-Type 的 name 参数获取
                common.ty.params.as_ref().and_then(|params| {
                    params
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("name"))
                        .map(|(_, v)| v.clone().into_owned())
                })
            });

        AttachmentInfo {
            filename,
            content_type,
            size: other.octets,
            section_path: section.to_string(),
            disposition: common
                .disposition
                .as_ref()
                .map(|d| d.ty.clone().into_owned()),
            content_id: other.id.as_ref().map(|id| id.clone().into_owned()),
        }
    }
}

// ─── XOAUTH2 Authenticator ───
#[derive(Debug)]
struct Xoauth2Authenticator {
    user: String,
    access_token: String,
}

impl async_imap::Authenticator for Xoauth2Authenticator {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        let s = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        );
        s.into_bytes()
    }
}

impl Xoauth2Authenticator {
    #[allow(dead_code)]
    fn generate_xoauth2_string(&self, user: &str, access_token: &str) -> String {
        let s = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
        let s_bytes = s.into_bytes();
        BASE64_STANDARD.encode(s_bytes)
    }
}
