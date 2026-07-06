//! IMAP SEARCH 命令处理模块
//!
//! 本模块实现了通过 IMAP SEARCH 命令查找邮件的功能，包括：
//! - 按日期范围搜索邮件 UID
//! - 按 UID 范围搜索（用于增量同步和历史回填）
//! - 检测指定日期之前是否存在邮件

use crate::error::MailError;

use super::ImapClient;

/// 构建获取全部邮件的搜索命令
///
/// # 返回
/// 返回 "ALL" 搜索命令
fn build_all_search_command() -> &'static str {
    "ALL"
}

/// 构建日期范围搜索命令
///
/// # 参数
/// - `start_date`: 起始日期（IMAP 格式，如 "01-Jan-2025"）
/// - `end_date`: 结束日期（IMAP 格式，排他上界）
///
/// # 返回
/// 返回 "SINCE {start} BEFORE {end}" 格式的搜索命令
///
/// # 注意
/// IMAP 的 BEFORE 不包含 end_date 当天，因此 end 是排他上界
fn build_since_before_search_command(start_date: &str, end_date: &str) -> String {
    format!("SINCE {} BEFORE {}", start_date, end_date)
}

/// 构建早于指定日期的搜索命令
///
/// # 参数
/// - `date_before`: 日期界限（IMAP 格式）
///
/// # 返回
/// 返回 "BEFORE {date}" 格式的搜索命令
fn build_before_search_command(date_before: &str) -> String {
    format!("BEFORE {}", date_before)
}

/// 构建早于指定 UID 的搜索命令
///
/// # 参数
/// - `before_uid`: UID 界限（排他）
///
/// # 返回
/// 成功时返回 "UID 1:{before_uid-1}"，UID 为 1 时返回 None
///
/// # 功能
/// 用于历史回填，向前分页获取邮件
fn build_uid_before_search_command(before_uid: u32) -> Option<String> {
    before_uid
        .checked_sub(1)
        .filter(|last_uid| *last_uid >= 1)
        .map(|last_uid| format!("UID 1:{last_uid}"))
}

/// 构建晚于指定 UID 的搜索命令
///
/// # 参数
/// - `uid_since`: 起始 UID（包含）
///
/// # 返回
/// UID >= 1 时返回 "UID {uid_since}:*"，否则返回 None
///
/// # 功能
/// 用于增量同步，获取新邮件
fn build_uid_since_search_command(uid_since: u32) -> Option<String> {
    (uid_since >= 1).then(|| format!("UID {uid_since}:*"))
}

// ─── ImapClient SEARCH 实现 ───

impl ImapClient {
    /// 获取指定日期之后的邮件 UID 列表（用于增量同步）
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    /// - `date_since`: 起始日期（IMAP 格式，如 "01-Jan-2025"）
    ///
    /// # 返回
    /// 成功时返回 UID 列表（已排序）
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

    /// 获取指定日期范围内的邮件 UID 列表
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    /// - `start_date`: 起始日期（IMAP 格式，包含）
    /// - `end_date`: 结束日期（IMAP 格式，不包含）
    ///
    /// # 返回
    /// 成功时返回 UID 列表（已排序）
    ///
    /// # 功能
    /// 用于历史同步，按时间窗口批量获取邮件
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

    /// 检测指定日期之前是否存在邮件
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    /// - `date_before`: 日期界限（IMAP 格式）
    ///
    /// # 返回
    /// 存在邮件时返回 true
    ///
    /// # 功能
    /// 用于探测历史邮件是否已耗尽
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

    /// 获取文件夹中的全部邮件 UID 列表
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    /// 成功时返回 UID 列表（已排序）
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
    /// # 参数
    /// - `folder`: 文件夹名称
    /// - `uid_since`: 起始 UID（包含）
    ///
    /// # 返回
    /// 成功时返回 UID 列表（已排序）
    ///
    /// # 功能
    /// - 从 `uid_since` 开始查询所有后续 UID
    /// - 用于按 UID 的增量同步
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

    /// 获取指定 UID 之前的最近一批邮件 UID 列表（用于历史回填）
    ///
    /// # 参数
    /// - `folder`: 文件夹名称
    /// - `before_uid`: UID 界限（排他）
    /// - `limit`: 最多返回的 UID 数量
    ///
    /// # 返回
    /// 成功时返回 UID 列表（已排序，最多 limit 个）
    ///
    /// # 功能
    /// - UID 在同一文件夹和同一 UIDVALIDITY 下单调递增
    /// - 使用 `UID 1:<before_uid - 1>` 向前分页
    /// - 避免按时间窗口遇到长空档时反复空转
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

// ─── 测试模块 ───

#[cfg(test)]
mod tests {
    use super::{
        build_all_search_command, build_before_search_command, build_since_before_search_command,
        build_uid_before_search_command, build_uid_since_search_command,
    };

    /// 测试构建全量搜索命令
    #[test]
    fn build_all_search_command_should_request_all_uids() {
        assert_eq!(build_all_search_command(), "ALL");
    }

    /// 测试日期范围命令使用 IMAP 排他上界
    #[test]
    fn build_since_before_search_command_should_use_imap_exclusive_end_boundary() {
        assert_eq!(
            build_since_before_search_command("01-Jan-2024", "01-Apr-2024"),
            "SINCE 01-Jan-2024 BEFORE 01-Apr-2024"
        );
    }

    /// 测试构建探测较早邮件的命令
    #[test]
    fn build_before_search_command_should_probe_for_older_uids() {
        assert_eq!(
            build_before_search_command("01-Jan-2024"),
            "BEFORE 01-Jan-2024"
        );
    }

    /// 测试构建 UID 范围命令获取游标以下的 UID
    #[test]
    fn build_uid_before_search_command_should_request_uids_below_cursor() {
        assert_eq!(
            build_uid_before_search_command(42).as_deref(),
            Some("UID 1:41")
        );
    }

    /// 测试第一个 UID 时返回 None
    #[test]
    fn build_uid_before_search_command_should_return_none_for_first_uid() {
        assert_eq!(build_uid_before_search_command(1), None);
    }

    /// 测试构建 UID 范围命令获取所有较新的 UID
    #[test]
    fn build_uid_since_search_command_should_request_all_newer_uids() {
        assert_eq!(
            build_uid_since_search_command(42).as_deref(),
            Some("UID 42:*")
        );
    }

    /// 测试 UID 为 0 时返回 None
    #[test]
    fn build_uid_since_search_command_should_return_none_for_zero() {
        assert_eq!(build_uid_since_search_command(0), None);
    }
}
