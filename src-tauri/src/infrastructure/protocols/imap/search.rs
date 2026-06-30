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

fn build_uid_before_search_command(before_uid: u32) -> Option<String> {
    before_uid
        .checked_sub(1)
        .filter(|last_uid| *last_uid >= 1)
        .map(|last_uid| format!("UID 1:{last_uid}"))
}

fn build_uid_since_search_command(uid_since: u32) -> Option<String> {
    (uid_since >= 1).then(|| format!("UID {uid_since}:*"))
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
    /// 从 `uid_since` 开始查询所有后续 UID。
    pub async fn list_uids_since_uid(
        &mut self,
        folder: &str,
        uid_since: u32,
    ) -> Result<Vec<u32>, MailError> {
        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let Some(search_cmd) = build_uid_since_search_command(uid_since) else {
            return Ok(Vec::new());
        };

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

    /// 获取指定 UID 之前最近的一批邮件 UID 列表（用于历史回填）。
    ///
    /// UID 在同一文件夹和同一 UIDVALIDITY 下单调递增，因此历史回填使用
    /// `UID 1:<before_uid - 1>` 向前分页，避免按时间窗口遇到长空档时反复空转。
    pub async fn list_uids_before_uid(
        &mut self,
        folder: &str,
        before_uid: u32,
        limit: usize,
    ) -> Result<Vec<u32>, MailError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let Some(search_cmd) = build_uid_before_search_command(before_uid) else {
            return Ok(Vec::new());
        };

        self.session
            .select(folder)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        tracing::info!("📤 使用 IMAP 历史 UID 搜索命令: '{}'", search_cmd);
        let uids = self
            .session
            .uid_search(&search_cmd)
            .await
            .map_err(|e| MailError::ImapSearchFailed(e.to_string()))?;

        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort();
        if uid_list.len() > limit {
            uid_list = uid_list.split_off(uid_list.len() - limit);
        }
        Ok(uid_list)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_all_search_command, build_before_search_command, build_since_before_search_command,
        build_uid_before_search_command, build_uid_since_search_command,
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

    #[test]
    fn build_uid_before_search_command_should_request_uids_below_cursor() {
        assert_eq!(
            build_uid_before_search_command(42).as_deref(),
            Some("UID 1:41")
        );
    }

    #[test]
    fn build_uid_before_search_command_should_return_none_for_first_uid() {
        assert_eq!(build_uid_before_search_command(1), None);
    }

    #[test]
    fn build_uid_since_search_command_should_request_all_newer_uids() {
        assert_eq!(
            build_uid_since_search_command(42).as_deref(),
            Some("UID 42:*")
        );
    }

    #[test]
    fn build_uid_since_search_command_should_return_none_for_zero() {
        assert_eq!(build_uid_since_search_command(0), None);
    }
}
