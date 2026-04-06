use crate::error::MailError;

use super::ImapClient;

impl ImapClient {
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
}
