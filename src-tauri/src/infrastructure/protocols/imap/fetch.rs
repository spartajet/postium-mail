use crate::error::MailError;
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use futures::{StreamExt, TryStreamExt};
use mail_parser::MessageParser;

use super::parser::{self, extract_headers_from_message, ParsedHeaders};
use super::{ImapClient, RawEmailHeader};

impl ImapClient {
    /// 批量获取邮件头（用于骨架同步，不获取正文）
    ///
    /// 使用 `BODY.PEEK[HEADER]` 获取 RFC822 原始头部，再通过 `mail_parser` 解析。
    /// BODYSTRUCTURE 用于提取附件 section_path。
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

        let uid_range = format!("{}:{}", start_uid, end_uid);

        tracing::trace!("批量获取邮件头: folder={}, uid_range={}", folder, uid_range);

        let fetches = self
            .session
            .uid_fetch(
                &uid_range,
                "(FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER] BODYSTRUCTURE UID)",
            )
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?
            .try_collect::<Vec<async_imap::types::Fetch>>()
            .await
            .map_err(|e| MailError::ImapError(e.to_string()))?;

        tracing::debug!("获取邮件头message count: {}", fetches.len());

        let now_ts = chrono::Utc::now().timestamp();
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

            // 使用 mail_parser 解析 BODY.PEEK[HEADER] 中的原始 RFC822 头部
            let raw_header = fetch.header();

            if let Some(raw) = raw_header {
                if let Some(parsed) = parser::parse_headers_from_raw(raw, now_ts) {
                    let attachments = fetch
                        .bodystructure()
                        .map(|bs| parser::extract_attachments(bs, ""))
                        .unwrap_or_default();

                    tracing::trace!(uid, attachment_count = attachments.len(), "解析附件完成");

                    // 优先使用 INTERNALDATE 作为邮件日期（服务器端时间，更可靠）
                    let date = fetch
                        .internal_date()
                        .map(|d| d.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|| {
                            chrono::DateTime::from_timestamp(parsed.sent_at, 0)
                                .unwrap_or_else(chrono::Utc::now)
                        });

                    headers.push(EmailHeader {
                        uid,
                        subject: parsed.subject.unwrap_or_default(),
                        from: parsed.from_display,
                        to: parsed.to_display,
                        cc: parsed.cc_emails.unwrap_or_default(),
                        date,
                        flags: super::super::types::EmailFlags {
                            seen,
                            flagged,
                            answered,
                            deleted,
                            draft,
                            recent,
                        },
                        attachments,
                    });
                } else {
                    tracing::warn!(uid, "mail_parser 解析邮件头失败，跳过");
                }
            } else {
                tracing::warn!(uid, "BODY.PEEK[HEADER] 为空，跳过");
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
    ///
    /// 使用 `BODY.PEEK[HEADER]` + `mail_parser` 解析邮件头。
    #[allow(dead_code)]
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
            .uid_fetch(&range, "(FLAGS BODY.PEEK[HEADER] UID)")
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("获取邮件头失败: {e}")))?;

        let now_ts = chrono::Utc::now().timestamp();
        let mut headers = Vec::new();
        while let Some(fetch_result) = fetches.next().await {
            let fetch = fetch_result
                .map_err(|e| MailError::ImapConnectionFailed(format!("解析邮件头失败: {e}")))?;
            let uid = fetch.uid.unwrap_or(0);

            if let Some(raw) = fetch.header() {
                if let Some(parsed) = parser::parse_headers_from_raw(raw, now_ts) {
                    let flags: Vec<String> = fetch.flags().map(|f| format!("{f:?}")).collect();
                    headers.push(RawEmailHeader {
                        uid,
                        flags,
                        subject: parsed.subject,
                        from: Some(parsed.from_display).filter(|s| !s.is_empty()),
                        to: Some(parsed.to_display).filter(|s| !s.is_empty()),
                        date: parsed.sent_at.to_string().into(),
                        message_id: parsed.message_id,
                    });
                } else {
                    let flags: Vec<String> = fetch.flags().map(|f| format!("{f:?}")).collect();
                    headers.push(RawEmailHeader {
                        uid,
                        flags,
                        subject: None,
                        from: None,
                        to: None,
                        date: None,
                        message_id: None,
                    });
                }
            } else {
                let flags: Vec<String> = fetch.flags().map(|f| format!("{f:?}")).collect();
                headers.push(RawEmailHeader {
                    uid,
                    flags,
                    subject: None,
                    from: None,
                    to: None,
                    date: None,
                    message_id: None,
                });
            }
        }
        tracing::debug!(count = headers.len(), "IMAP: 获取邮件头完成");
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
    /// 使用 `mail_parser` 统一解析所有邮件头和正文内容，
    /// BODYSTRUCTURE 仅用于提取附件 section_path（IMAP 按需下载所需）。
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

        // 批量 FETCH：FLAGS + BODYSTRUCTURE + BODY.PEEK[] + INTERNALDATE + UID
        // 不再使用 ENVELOPE，所有邮件头通过 mail_parser 从 BODY.PEEK[] 解析
        let fetches = self
            .session
            .uid_fetch(
                &uid_range,
                "(FLAGS INTERNALDATE BODYSTRUCTURE BODY.PEEK[] UID)",
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

            // ─── BODY.PEEK[] → mail_parser 统一解析 ───
            let mut body_text: Option<String> = None;
            let mut body_html: Option<String> = None;
            let mut preview: Option<String> = None;
            let mut headers: Option<ParsedHeaders> = None;

            if let Some(raw) = fetch.body() {
                // 单次解析，同时提取头部和正文
                if let Some(msg) = mail_parser::MessageParser::default().parse(raw) {
                    body_text = msg.body_text(0).map(|t| t.to_string());
                    body_html = msg.body_html(0).map(|t| t.to_string());
                    preview = body_text.as_ref().map(|t| t.chars().take(200).collect());
                    headers = Some(extract_headers_from_message(&msg, now));
                }
            }

            // 使用 INTERNALDATE 作为 fallback
            let fallback_date = fetch
                .internal_date()
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(chrono::Utc::now);

            // ─── BODYSTRUCTURE → 附件 ───
            let attachments = fetch
                .bodystructure()
                .map(|bs| parser::extract_attachments(bs, ""))
                .unwrap_or_default();

            tracing::trace!(uid, attachment_count = attachments.len(), "解析附件完成");

            // ─── 组装 WholeEmailDto ───
            let h = headers.as_ref();
            let subject = h.and_then(|h| h.subject.clone()).filter(|s| !s.is_empty());

            emails.push(WholeEmailDto {
                // 数据库侧字段，暂用默认值，由调用者持久化后填充
                id: 0,
                account_id: 0,
                folder: folder.to_string(),
                uid,
                message_id: h.and_then(|h| h.message_id.clone()),
                sender_name: h.and_then(|h| h.sender_name.clone()),
                sender_email: h.map(|h| h.sender_email.clone()).unwrap_or_default(),
                recipient_emails: h.map(|h| h.recipient_emails.clone()).unwrap_or_default(),
                cc_emails: h.and_then(|h| h.cc_emails.clone()),
                bcc_emails: h.and_then(|h| h.bcc_emails.clone()),
                subject,
                preview,
                body_text,
                body_html,
                attachments,
                is_read: seen,
                is_starred: flagged,
                is_draft: draft,
                is_answered: answered,
                is_deleted: deleted,
                sent_at: h.map(|h| h.sent_at).unwrap_or_else(|| fallback_date.timestamp()),
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
}
