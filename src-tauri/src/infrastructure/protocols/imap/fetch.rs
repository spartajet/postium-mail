use crate::error::MailError;
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use crate::infrastructure::protocols::utils::{
    extract_email_from_address, extract_name_from_address,
};
use futures::{StreamExt, TryStreamExt};
use mail_parser::MessageParser;

use super::parser;
use super::{ImapClient, RawEmailHeader};

impl ImapClient {
    /// 批量获取邮件头（用于骨架同步，不获取正文）
    ///
    /// 使用 UID FETCH 命令批量获取多个邮件的头信息，避免设置已读标志。
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

            let mail_envelope = fetch
                .envelope()
                .and_then(|enve| parser::parse_envelope(enve).ok());

            if let Some(enve) = mail_envelope {
                let attachments = fetch
                    .bodystructure()
                    .map(|bs| parser::extract_attachments(bs, ""))
                    .unwrap_or_default();

                tracing::trace!(uid, attachment_count = attachments.len(), "解析附件完成");

                headers.push(EmailHeader {
                    uid,
                    subject: enve.subject,
                    from: enve.from,
                    to: enve.to,
                    cc: enve.cc,
                    date: enve.date,
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
                .map(|s| parser::cow_bytes_to_string(s.as_ref()));
            let from = envelope
                .from
                .as_ref()
                .and_then(|addrs| addrs.first().and_then(|a| parser::address_to_string(a)));
            let to = envelope
                .to
                .as_ref()
                .and_then(|addrs| addrs.first().and_then(|a| parser::address_to_string(a)));
            let date = envelope
                .date
                .as_ref()
                .map(|s| parser::cow_bytes_to_string(s.as_ref()));
            let message_id = envelope
                .message_id
                .as_ref()
                .map(|s| parser::cow_bytes_to_string(s.as_ref()));
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
                .and_then(|enve| parser::parse_envelope(enve).ok());

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
                .map(|bs| parser::extract_attachments(bs, ""))
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
}
