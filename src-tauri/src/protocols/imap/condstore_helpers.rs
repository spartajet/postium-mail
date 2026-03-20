//! CONDSTORE 原始命令辅助函数
//!
//! async-imap 0.11.0 不支持的功能，需要使用 run_command() + 自定义解析：
//! - SEARCH MODSEQ 命令（async-imap 的 search() 不支持 MODSEQ 语法）
//! - FETCH MODSEQ 响应解析（async-imap 的 fetch() MODSEQ 支持不明确）
//!
//! # 使用场景
//!
//! 当 async-imap 原生 API 不足时，使用这些辅助函数：
//! ```rust,ignore
//! // 1. 构建原始命令
//! let cmd = CondstoreCommands::search_modseq(1234567890);
//!
//! // 2. 执行命令
//! let response = session.run_command(&cmd).await?;
//!
//! // 3. 解析响应
//! let uids = CondstoreCommands::parse_search_modseq(&response.to_string())?;
//! ```
//!
//! # RFC 4551 - IMAP CONDSTORE
//!
//! https://datatracker.ietf.org/doc/html/rfc4551

use anyhow::{anyhow, Result};

/// CONDSTORE 原始命令辅助工具
///
/// 提供 async-imap 0.11.0 不支持的 CONDSTORE 功能
pub struct CondstoreCommands;

impl CondstoreCommands {
    // ========== 命令构建 ==========

    /// 构建 SEARCH MODSEQ 命令
    ///
    /// 搜索自指定 MODSEQ 后修改的邮件
    ///
    /// # 参数
    ///
    /// * `modseq` - 起始 MODSEQ 值
    ///
    /// # 返回
    ///
    /// 命令字符串
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let cmd = CondstoreCommands::search_modseq(1234567890);
    /// // 返回: "SEARCH MODSEQ 1234567890:* ALL"
    /// ```
    ///
    /// # IMAP 协议
    ///
    /// ```text
    /// C: A681 SEARCH MODSEQ 1234567890:* ALL
    /// S: * SEARCH 2 3 4
    /// S: A681 OK Search completed
    /// ```
    pub fn search_modseq(modseq: u64) -> String {
        format!("SEARCH MODSEQ {}:* ALL", modseq)
    }

    /// 构建 FETCH MODSEQ 命令（单个邮件）
    ///
    /// 获取邮件及其 MODSEQ 值
    ///
    /// # 参数
    ///
    /// * `uid` - 邮件 UID
    ///
    /// # 返回
    ///
    /// 命令字符串
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let cmd = CondstoreCommands::fetch_modseq(123);
    /// // 返回: "FETCH 123 (FLAGS MODSEQ)"
    /// ```
    ///
    /// # IMAP 协议
    ///
    /// ```text
    /// C: A682 FETCH 123 (FLAGS MODSEQ)
    /// S: * 123 FETCH (FLAGS (\Seen) MODSEQ (1234567891))
    /// S: A682 OK Fetch completed
    /// ```
    pub fn fetch_modseq(uid: u32) -> String {
        format!("FETCH {} (FLAGS MODSEQ)", uid)
    }

    /// 构建批量 FETCH MODSEQ 命令
    ///
    /// 批量获取邮件及其 MODSEQ 值
    ///
    /// # 参数
    ///
    /// * `uids` - UID 列表
    ///
    /// # 返回
    ///
    /// 命令字符串
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let cmd = CondstoreCommands::fetch_modseq_batch(&[1, 2, 3]);
    /// // 返回: "FETCH 1,2,3 (FLAGS MODSEQ)"
    /// ```
    pub fn fetch_modseq_batch(uids: &[u32]) -> String {
        let uids_str: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
        format!("FETCH {} (FLAGS MODSEQ)", uids_str.join(","))
    }

    // ========== 响应解析 ==========

    /// 解析 SEARCH MODSEQ 响应
    ///
    /// 从 SEARCH 命令响应中提取 UID 列表
    ///
    /// # 参数
    ///
    /// * `response` - 服务器响应（从 `run_command()` 获取）
    ///
    /// # 返回
    ///
    /// UID 列表
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let response = "* SEARCH 1 2 3 4 5\r\nA1 OK Search completed";
    /// let uids = CondstoreCommands::parse_search_modseq(response)?;
    /// // 返回: vec![1, 2, 3, 4, 5]
    /// ```
    ///
    /// # IMAP 响应格式
    ///
    /// ```text
    /// * SEARCH 1 2 3 4 5
    /// A1 OK Search completed
    /// ```
    pub fn parse_search_modseq(response: &str) -> Result<Vec<u32>> {
        let mut uids = Vec::new();

        for line in response.lines() {
            if line.contains("* SEARCH") {
                // 提取 "* SEARCH ..." 后面的 UID 列表
                if let Some(rest) = line.strip_prefix("* SEARCH") {
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    for part in parts {
                        if let Ok(uid) = part.parse::<u32>() {
                            uids.push(uid);
                        }
                    }
                }
            }
        }

        if uids.is_empty() {
            return Err(anyhow!("未找到 SEARCH 结果"));
        }

        Ok(uids)
    }

    /// 解析 FETCH MODSEQ 响应
    ///
    /// 从 FETCH 命令响应中提取 MODSEQ 和 FLAGS
    ///
    /// # 参数
    ///
    /// * `response` - 服务器响应（从 `run_command()` 获取）
    /// * `uid` - 邮件 UID
    ///
    /// # 返回
    ///
    /// (MODSEQ, FLAGS列表)
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let response = "* 123 FETCH (FLAGS (\\Seen) MODSEQ (1234567890))";
    /// let (modseq, flags) = CondstoreCommands::parse_fetch_modseq(response, 123)?;
    /// // 返回: (Some(1234567890), vec!["\\Seen"])
    /// ```
    ///
    /// # IMAP 响应格式
    ///
    /// ```text
    /// * 123 FETCH (FLAGS (\Seen) MODSEQ (1234567891))
    /// A682 OK Fetch completed
    /// ```
    pub fn parse_fetch_modseq(response: &str, uid: u32) -> Result<(Option<u64>, Vec<String>)> {
        let mut modseq = None;
        let mut flags = Vec::new();

        for line in response.lines() {
            if line.contains(&format!("* {} FETCH", uid)) {
                // 解析 MODSEQ
                // 格式: MODSEQ (1234567890)
                let modseq_pattern = "MODSEQ (";
                if let Some(start) = line.find(modseq_pattern) {
                    let start_pos = start + modseq_pattern.len();
                    if let Some(end) = line[start_pos..].find(')') {
                        let modseq_str = &line[start_pos..start_pos + end];
                        if let Ok(val) = modseq_str.parse::<u64>() {
                            modseq = Some(val);
                        }
                    }
                }

                // 解析 FLAGS
                // 格式: FLAGS (\Seen) 或 FLAGS (\Seen \Flagged)
                let flags_pattern = "FLAGS (";
                if let Some(start) = line.find(flags_pattern) {
                    let start_pos = start + flags_pattern.len();
                    if let Some(end) = line[start_pos..].find(')') {
                        let flags_str = &line[start_pos..start_pos + end];
                        // 分割标志（可能用空格或反斜杠分隔）
                        for flag in flags_str.split_whitespace() {
                            flags.push(flag.to_string());
                        }
                    }
                }
            }
        }

        if modseq.is_none() && flags.is_empty() {
            return Err(anyhow!("未找到 FETCH 结果: UID {}", uid));
        }

        Ok((modseq, flags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_modseq() {
        let cmd = CondstoreCommands::search_modseq(1234567890);
        assert_eq!(cmd, "SEARCH MODSEQ 1234567890:* ALL");
    }

    #[test]
    fn test_fetch_modseq() {
        let cmd = CondstoreCommands::fetch_modseq(123);
        assert_eq!(cmd, "FETCH 123 (FLAGS MODSEQ)");
    }

    #[test]
    fn test_fetch_modseq_batch() {
        let cmd = CondstoreCommands::fetch_modseq_batch(&[1, 2, 3]);
        assert_eq!(cmd, "FETCH 1,2,3 (FLAGS MODSEQ)");
    }

    #[test]
    fn test_parse_search_modseq() {
        let response = "* SEARCH 1 2 3 4 5\r\nA1 OK Search completed";
        let uids = CondstoreCommands::parse_search_modseq(response).unwrap();
        assert_eq!(uids, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_search_modseq_empty() {
        let response = "* SEARCH\r\nA1 OK Search completed";
        let result = CondstoreCommands::parse_search_modseq(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fetch_modseq() {
        let response = "* 123 FETCH (FLAGS (\\Seen) MODSEQ (1234567890))";
        let (modseq, flags) = CondstoreCommands::parse_fetch_modseq(response, 123).unwrap();
        assert_eq!(modseq, Some(1234567890));
        assert_eq!(flags, vec!["\\Seen"]);
    }

    #[test]
    fn test_parse_fetch_modseq_multiple_flags() {
        let response = "* 456 FETCH (FLAGS (\\Seen \\Flagged \\Answered) MODSEQ (9876543210))";
        let (modseq, flags) = CondstoreCommands::parse_fetch_modseq(response, 456).unwrap();
        assert_eq!(modseq, Some(9876543210));
        assert_eq!(flags, vec!["\\Seen", "\\Flagged", "\\Answered"]);
    }

    #[test]
    fn test_parse_fetch_modseq_not_found() {
        let response = "* 123 FETCH (FLAGS (\\Seen))";
        let result = CondstoreCommands::parse_fetch_modseq(response, 999);
        assert!(result.is_err());
    }
}
