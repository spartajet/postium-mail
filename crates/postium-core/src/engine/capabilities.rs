//! IMAP 能力集：CAPABILITY 响应解析与查询。
//!
//! 连接建立后立即探测并 `ENABLE` 支持的扩展；同步档位决策（QRESYNC →
//! CONDSTORE → UID 增量）与功能开关（IDLE/SPECIAL-USE/MOVE）均基于此。

use std::collections::HashSet;

/// 服务器能力集合（大写归一存储）。
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    caps: HashSet<String>,
}

impl CapabilitySet {
    /// 从 CAPABILITY 响应行构建（每行一个能力名，如 "IMAP4REV1"、"QRESYNC"）。
    pub fn parse(responses: &[String]) -> Self {
        Self {
            caps: responses
                .iter()
                .map(|c| c.trim().trim_matches('"').to_ascii_uppercase())
                .filter(|c| !c.is_empty())
                .collect(),
        }
    }

    pub fn supports(&self, cap: &str) -> bool {
        self.caps.contains(&cap.to_ascii_uppercase())
    }

    /// 服务器是否支持 QRESYNC（隐含 CONDSTORE）。
    pub fn qresync(&self) -> bool {
        self.supports("QRESYNC")
    }

    pub fn condstore(&self) -> bool {
        self.qresync() || self.supports("CONDSTORE")
    }

    pub fn idle(&self) -> bool {
        self.supports("IDLE")
    }

    pub fn special_use(&self) -> bool {
        self.supports("SPECIAL-USE")
    }

    pub fn move_ext(&self) -> bool {
        self.supports("MOVE")
    }

    /// 应发 ENABLE 的扩展（与服务端能力取交集；QRESYNC 隐含 CONDSTORE）。
    pub fn enable_list(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.qresync() {
            out.push("QRESYNC");
            out.push("CONDSTORE");
        } else if self.condstore() {
            out.push("CONDSTORE");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalizes_case_and_quotes() {
        let caps =
            CapabilitySet::parse(&["\"IMAP4rev1\"".into(), " qresync ".into(), "IDLE".into()]);
        assert!(caps.supports("imap4rev1"));
        assert!(caps.qresync());
        assert!(caps.idle());
        assert!(!caps.supports("CONDSTORE"));
    }

    #[test]
    fn enable_list_follows_fallback_chain() {
        // QRESYNC 隐含 CONDSTORE
        let q = CapabilitySet::parse(&["QRESYNC".into()]);
        assert_eq!(q.enable_list(), vec!["QRESYNC", "CONDSTORE"]);
        // 仅 CONDSTORE
        let c = CapabilitySet::parse(&["CONDSTORE".into()]);
        assert_eq!(c.enable_list(), vec!["CONDSTORE"]);
        // 都没有
        let none = CapabilitySet::parse(&["IMAP4rev1".into()]);
        assert!(none.enable_list().is_empty());
    }
}
