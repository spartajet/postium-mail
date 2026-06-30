use crate::error::MailError;

use super::ImapClient;

fn build_all_search_command() -> &'static str {
    "ALL"
}

fn build_since_before_search_command(start_date: &str, end_date: &str) -> String {
    format!("SINCE {} BEFORE {}", start_date, end_date)
}

fn build_before_search_command(date_before: &str) -> String {
    format!("BEFORE {}", date_before)
}

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

    pub async fn list_uids_between(
        &mut self,
        folder: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<u32>, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        // IMAP `BEFORE` 不包含 end_date 当天，因此 `end` 是排他上界。
        let search_cmd = build_since_before_search_command(start_date, end_date);
        tracing::info!("📤 使用 IMAP 历史区间搜索命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        Ok(uid_list)
    }

    pub async fn has_uids_before(
        &mut self,
        folder: &str,
        date_before: &str,
    ) -> Result<bool, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let search_cmd = build_before_search_command(date_before);
        tracing::info!("📤 使用 IMAP 历史耗尽探测命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        Ok(!uids.is_empty())
    }

    /// 获取文件夹中的全部邮件 UID 列表。
    pub async fn list_all_uids(&mut self, folder: &str) -> Result<Vec<u32>, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let search_cmd = build_all_search_command();
        tracing::info!("📤 使用 IMAP 全量搜索命令: '{}'", search_cmd);

        let uids = self
            .session
            .uid_search(search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        Ok(uid_list)
    }

    /// 获取指定 UID 之后的邮件 UID 列表（用于增量同步）
    ///
    /// 从 `uid_since` 开始向后探测 100 个 UID 范围内的邮件。
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

#[cfg(test)]
mod tests {
    use super::{
        build_all_search_command, build_before_search_command, build_since_before_search_command,
    };

    #[test]
    fn build_all_search_command_should_request_all_uids() {
        assert_eq!(build_all_search_command(), "ALL");
    }

    #[test]
    fn build_since_before_search_command_should_use_imap_exclusive_end_boundary() {
        assert_eq!(
            build_since_before_search_command("01-Jan-2024", "01-Apr-2024"),
            "SINCE 01-Jan-2024 BEFORE 01-Apr-2024"
        );
    }

    #[test]
    fn build_before_search_command_should_probe_for_older_uids() {
        assert_eq!(
            build_before_search_command("01-Jan-2024"),
            "BEFORE 01-Jan-2024"
        );
    }
}
