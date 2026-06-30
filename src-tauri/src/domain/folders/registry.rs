use crate::domain::folders::keyword_table::guess_category;
use crate::domain::folders::special_use::SpecialUseFlag;
use crate::domain::folders::FolderCategory;
use crate::domain::providers::StandardFolder;
use std::collections::{HashMap, HashSet};
use utf7_imap::decode_utf7_imap;

/// 一个远程文件夹的原始输入（LIST 返回，name 为 IMAP-UTF-7 编码 + 属性）
#[derive(Debug, Clone)]
pub struct RemoteFolder {
    pub name: String,
    pub special_use: Vec<SpecialUseFlag>,
    /// 是否 \NoSelect/\NonExistent（不可选，跳过识别）
    pub no_select: bool,
}

/// 账号文件夹识别引擎。单次 IMAP 连接内构建、有效。
pub struct FolderRegistry {
    /// 编码名 -> 类别（反向索引，构建时算好）
    classified: HashMap<String, FolderCategory>,
    /// 类别 -> 所有归属该类别的编码文件夹名（正向，多选）
    by_category: HashMap<FolderCategory, Vec<String>>,
    /// 全部远程文件夹编码名
    remote_folders: HashSet<String>,
}

impl FolderRegistry {
    pub fn builder() -> FolderRegistryBuilder {
        FolderRegistryBuilder::new()
    }

    /// 多选：返回该类别所有归属的真实文件夹（编码名，可直接用于 IMAP 操作）。
    pub fn resolve(&self, category: FolderCategory) -> Vec<String> {
        self.by_category.get(&category).cloned().unwrap_or_default()
    }

    /// 单选：返回首选那一个。
    pub fn resolve_one(&self, category: FolderCategory) -> Option<String> {
        self.by_category.get(&category).and_then(|v| v.first().cloned())
    }

    /// 反向：真实文件夹名(编码) -> 类别。纯查表，无子串推断。
    pub fn classify(&self, folder_name: &str) -> Option<FolderCategory> {
        self.classified.get(folder_name).copied()
    }

    /// 文件夹是否真实存在于远程。
    pub fn exists(&self, folder_name: &str) -> bool {
        self.remote_folders.contains(folder_name)
    }
}

pub struct FolderRegistryBuilder {
    remote: Vec<RemoteFolder>,
    mapping: Option<StandardFolder>,
}

impl FolderRegistryBuilder {
    fn new() -> Self {
        Self { remote: Vec::new(), mapping: None }
    }

    /// 注入远程文件夹列表（带 SPECIAL-USE 属性）。
    pub fn remote_folders(mut self, folders: Vec<RemoteFolder>) -> Self {
        self.remote = folders;
        self
    }

    /// 注入 provider 候选名（兜底层）。可选。
    pub fn provider_mapping(mut self, mapping: StandardFolder) -> Self {
        self.mapping = Some(mapping);
        self
    }

    /// 构建引擎，执行三层识别。
    pub fn build(self) -> FolderRegistry {
        let mut classified: HashMap<String, FolderCategory> = HashMap::new();
        let mut by_category: HashMap<FolderCategory, Vec<String>> = HashMap::new();
        let mut remote_folders: HashSet<String> = HashSet::new();

        for folder in &self.remote {
            remote_folders.insert(folder.name.clone());
            if folder.no_select {
                continue;
            }
            if let Some(cat) = classify_folder(folder) {
                classified.insert(folder.name.clone(), cat);
                by_category.entry(cat).or_default().push(folder.name.clone());
            }
        }

        // 第三层兜底：provider 候选名精确匹配（仅当某类别三层都为空时）
        if let Some(mapping) = self.mapping {
            for (cat, candidates) in mapping_pairs(&mapping) {
                if by_category.get(&cat).map_or(true, |v| v.is_empty()) {
                    for c in candidates {
                        if remote_folders.contains(c) && !classified.contains_key(c) {
                            classified.insert(c.clone(), cat);
                            by_category.entry(cat).or_default().push(c.clone());
                        }
                    }
                }
            }
        }

        FolderRegistry { classified, by_category, remote_folders }
    }
}

/// 单个文件夹的三层识别（第一层 INBOX 名 / SPECIAL-USE，第二层关键词）。
fn classify_folder(folder: &RemoteFolder) -> Option<FolderCategory> {
    // 第一层：INBOX 保留名（RFC 3501，先于 SPECIAL-USE）
    if folder.name.eq_ignore_ascii_case("INBOX") {
        return Some(FolderCategory::Inbox);
    }
    // 第一层：SPECIAL-USE 属性
    if let Some(cat) = folder.special_use.iter().find_map(|f| f.to_category()) {
        return Some(cat);
    }
    // 第二层：跨语言关键词（先 IMAP-UTF-7 解码）
    let decoded = decode_utf7_imap(folder.name.clone());
    let (cat, _score) = guess_category(&decoded)?;
    Some(cat)
}

/// StandardFolder 六字段转 (类别, 候选名) 序列。
fn mapping_pairs(m: &StandardFolder) -> Vec<(FolderCategory, &[String])> {
    vec![
        (FolderCategory::Inbox, &m.inbox),
        (FolderCategory::Sent, &m.sent),
        (FolderCategory::Drafts, &m.drafts),
        (FolderCategory::Junk, &m.spam),
        (FolderCategory::Trash, &m.trash),
        (FolderCategory::Archive, &m.archive),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::providers::StandardFolder;

    fn folder(name: &str) -> RemoteFolder {
        RemoteFolder { name: name.into(), special_use: vec![], no_select: false }
    }

    #[test]
    fn special_use_layer_identifies_gmail_sent() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![RemoteFolder {
                name: "[Gmail]/Sent Mail".into(),
                special_use: vec![SpecialUseFlag::Sent],
                no_select: false,
            }])
            .build();
        assert_eq!(reg.resolve_one(FolderCategory::Sent).as_deref(), Some("[Gmail]/Sent Mail"));
    }

    #[test]
    fn keyword_layer_identifies_netease_sent_decoded() {
        // 网易已发送编码名 &XfJT0ZAB- 解码后为 "已发送"
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("&XfJT0ZAB-")])
            .build();
        assert_eq!(reg.resolve_one(FolderCategory::Sent).as_deref(), Some("&XfJT0ZAB-"));
    }

    #[test]
    fn provider_fallback_when_layers_miss() {
        // 候选名 "XYZ" 不在关键词表、无 SPECIAL-USE，靠 provider 兜底
        let mapping = StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["XYZ".into()],
            drafts: vec![],
            spam: vec![],
            trash: vec![],
            archive: vec![],
        };
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("INBOX"), folder("XYZ")])
            .provider_mapping(mapping)
            .build();
        assert_eq!(reg.resolve_one(FolderCategory::Sent).as_deref(), Some("XYZ"));
    }

    #[test]
    fn classify_is_exact_lookup_no_substring() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Sent")])
            .build();
        assert_eq!(reg.classify("Sent"), Some(FolderCategory::Sent));
        // "Sent Archive" 不在 classified 里（它根本不是远程文件夹），返回 None
        assert_eq!(reg.classify("Sent Archive"), None);
    }

    #[test]
    fn no_select_folder_is_skipped() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![RemoteFolder {
                name: "Sent".into(),
                special_use: vec![SpecialUseFlag::Sent],
                no_select: true,
            }])
            .build();
        assert!(reg.resolve(FolderCategory::Sent).is_empty());
    }

    #[test]
    fn resolve_returns_multiple_for_same_category() {
        // 网易 sent 两个文件夹都在远程：Sent + &XfJT0ZAB-
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Sent"), folder("&XfJT0ZAB-")])
            .build();
        let resolved = reg.resolve(FolderCategory::Sent);
        assert_eq!(resolved.len(), 2);
    }
}
