//! 原始 IMAP 命令执行和 CONDSTORE 辅助函数
//!
//! 用于执行不直接被 async-imap 支持的 IMAP 命令

use anyhow::Result;

/// CONDSTORE 命令构建器
///
/// RFC 4551: IMAP Extension for Conditional STORE
/// https://datatracker.ietf.org/doc/html/rfc4551
pub struct CondstoreCommands;

impl CondstoreCommands {
    /// 构建 CAPABILITY 命令
    ///
    /// 返回用于检测服务器能力的命令字符串
    pub fn capability() -> String {
        "CAPABILITY".to_string()
    }

    /// 构建 SELECT UNCHANGEDSINCE 命令
    ///
    /// # 参数
    ///
    /// * `folder` - 文件夹名称
    /// * `modseq` - 起始 MODSEQ 值
    ///
    /// # 返回
    ///
    /// 命令字符串
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let cmd = CondstoreCommands::select_unchanged_since("INBOX", 1234567890);
    /// // 返回: "SELECT INBOX (UNCHANGEDSINCE 1234567890)"
    /// ```
    pub fn select_unchanged_since(folder: &str, modseq: u64) -> String {
        format!("SELECT {} (UNCHANGEDSINCE {})", folder, modseq)
    }

    /// 构建 SEARCH MODSEQ 命令
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
    pub fn search_modseq(modseq: u64) -> String {
        format!("SEARCH MODSEQ {}:* ALL", modseq)
    }

    /// 构建 FETCH MODSEQ 命令
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
    pub fn fetch_modseq(uid: u32) -> String {
        format!("FETCH {} (FLAGS MODSEQ)", uid)
    }

    /// 构建批量 FETCH MODSEQ 命令
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

    /// 解析 CAPABILITY 响应
    ///
    /// # 参数
    ///
    /// * `response` - 服务器响应
    ///
    /// # 返回
    ///
    /// 能力列表
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let response = "* CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN CONDSTORE";
    /// let caps = CondstoreCommands::parse_capability(response)?;
    /// // 返回: vec!["IMAP4rev1", "STARTTLS", "AUTH=PLAIN", "CONDSTORE"]
    /// ```
    pub fn parse_capability(response: &str) -> Result<Vec<String>> {
        let mut capabilities = Vec::new();

        for line in response.lines() {
            if line.contains("* CAPABILITY") {
                // 提取 "* CAPABILITY ..." 后面的部分
                if let Some(rest) = line.strip_prefix("* CAPABILITY") {
                    let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                    for part in parts {
                        // 处理 AUTH=PLAIN 这样的形式
                        if let Some(clean_part) = part.strip_prefix("AUTH=") {
                            capabilities.push(clean_part.to_string());
                        } else {
                            capabilities.push(part.to_string());
                        }
                    }
                }
            }
        }

        Ok(capabilities)
    }

    /// 检查能力列表中是否包含 CONDSTORE
    ///
    /// # 参数
    ///
    /// * `capabilities` - 能力列表
    ///
    /// # 返回
    ///
    /// 是否支持 CONDSTORE
    pub fn has_condstore(capabilities: &[String]) -> bool {
        capabilities.iter().any(|c| c.as_str() == "CONDSTORE")
    }

    /// 解析 SEARCH MODSEQ 响应
    ///
    /// # 参数
    ///
    /// * `response` - 服务器响应
    ///
    /// # 返回
    ///
    /// UID 列表
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let response = "* SEARCH 1 2 3 4 5\n* OK Search completed";
    /// let uids = CondstoreCommands::parse_search_modseq(response)?;
    /// // 返回: vec![1, 2, 3, 4, 5]
    /// ```
    pub fn parse_search_modseq(response: &str) -> Result<Vec<u32>> {
        let mut uids = Vec::new();

        for line in response.lines() {
            if line.contains("* SEARCH") {
                // 提取 "* SEARCH ..." 后面的 UID 列表
                if let Some(rest) = line.strip_prefix("* SEARCH") {
                    let parts: Vec<&str> = rest.trim().split_whitespace().collect();
                    for part in parts {
                        if let Ok(uid) = part.parse::<u32>() {
                            uids.push(uid);
                        }
                    }
                }
            }
        }

        Ok(uids)
    }

    /// 解析 FETCH MODSEQ 响应
    ///
    /// # 参数
    ///
    /// * `response` - 服务器响应
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

        Ok((modseq, flags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_unchanged_since() {
        let cmd = CondstoreCommands::select_unchanged_since("INBOX", 1234567890);
        assert_eq!(cmd, "SELECT INBOX (UNCHANGEDSINCE 1234567890)");
    }

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
    fn test_parse_capability() {
        let response =
            "* CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN CONDSTORE\r\nA1 OK Capability completed";
        let caps = CondstoreCommands::parse_capability(response).unwrap();
        assert_eq!(caps, vec!["IMAP4rev1", "STARTTLS", "PLAIN", "CONDSTORE"]);
    }

    #[test]
    fn test_has_condstore() {
        let caps = vec!["IMAP4rev1".to_string(), "CONDSTORE".to_string()];
        assert!(CondstoreCommands::has_condstore(&caps));
    }

    #[test]
    fn test_has_condstore_false() {
        let caps = vec!["IMAP4rev1".to_string(), "STARTTLS".to_string()];
        assert!(!CondstoreCommands::has_condstore(&caps));
    }

    #[test]
    fn test_parse_search_modseq() {
        let response = "* SEARCH 1 2 3 4 5\r\nA1 OK Search completed";
        let uids = CondstoreCommands::parse_search_modseq(response).unwrap();
        assert_eq!(uids, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_fetch_modseq() {
        let response = "* 123 FETCH (FLAGS (\\Seen) MODSEQ (1234567890))";
        let (modseq, flags) = CondstoreCommands::parse_fetch_modseq(response, 123).unwrap();
        assert_eq!(modseq, Some(1234567890));
        assert_eq!(flags, vec!["\\Seen"]);
    }
}
