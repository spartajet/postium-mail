use crate::domain::folders::FolderCategory;
use crate::domain::folders::keyword_table::guess_category;
use crate::domain::folders::special_use::SpecialUseFlag;
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
        self.by_category
            .get(&category)
            .and_then(|v| v.first().cloned())
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
    known_categories: Vec<(String, FolderCategory)>,
    allow_unverified_provider_fallback: bool,
}

#[derive(Debug)]
struct KeywordCandidate {
    folder_name: String,
    category: FolderCategory,
    score: u16,
    depth: usize,
}

impl FolderRegistryBuilder {
    fn new() -> Self {
        Self {
            remote: Vec::new(),
            mapping: None,
            known_categories: Vec::new(),
            allow_unverified_provider_fallback: false,
        }
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

    /// 注入已持久化的文件夹分类，优先级高于关键词和 provider fallback。
    pub fn known_categories(mut self, categories: Vec<(String, FolderCategory)>) -> Self {
        self.known_categories = categories;
        self
    }

    /// 允许 provider 首选候选名在本地/远程集合为空或未包含时作为结果返回。
    ///
    /// 只应用在不连接 IMAP 且需要兼容旧 provider 默认目标的路径，例如历史状态初始展示、
    /// 删除/归档目标解析。已经连接 IMAP 并拥有 LIST 结果的同步路径应保持默认 false，
    /// 避免选择不存在的远程文件夹。
    pub fn allow_unverified_provider_fallback(mut self, allow: bool) -> Self {
        self.allow_unverified_provider_fallback = allow;
        self
    }

    /// 构建引擎，执行三层识别。
    pub fn build(self) -> FolderRegistry {
        let mut classified: HashMap<String, FolderCategory> = HashMap::new();
        let mut by_category: HashMap<FolderCategory, Vec<String>> = HashMap::new();
        let mut remote_folders: HashSet<String> = HashSet::new();
        let known_categories: HashMap<String, FolderCategory> =
            self.known_categories.into_iter().collect();
        let provider_exact: HashMap<String, FolderCategory> = self
            .mapping
            .as_ref()
            .map(|mapping| {
                mapping_pairs(mapping)
                    .into_iter()
                    .flat_map(|(cat, candidates)| {
                        candidates
                            .iter()
                            .cloned()
                            .map(move |candidate| (candidate, cat))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut seen_remote = HashSet::new();
        let mut keyword_candidates = Vec::new();
        for folder in &self.remote {
            if !seen_remote.insert(folder.name.clone()) {
                continue;
            }
            remote_folders.insert(folder.name.clone());
            if folder.no_select {
                continue;
            }
            if let Some(cat) = known_categories
                .get(&folder.name)
                .copied()
                .or_else(|| classify_protocol_folder(folder))
                .or_else(|| provider_exact.get(&folder.name).copied())
            {
                insert_classification(&mut classified, &mut by_category, folder.name.clone(), cat);
            } else if let Some(candidate) = keyword_candidate(folder) {
                keyword_candidates.push(candidate);
            }
        }

        keyword_candidates.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| a.folder_name.cmp(&b.folder_name))
        });
        for candidate in keyword_candidates {
            if by_category
                .get(&candidate.category)
                .is_none_or(|folders| folders.is_empty())
                && !classified.contains_key(&candidate.folder_name)
            {
                insert_classification(
                    &mut classified,
                    &mut by_category,
                    candidate.folder_name,
                    candidate.category,
                );
            }
        }

        // 第三层兜底：provider 候选名精确匹配（仅当某类别三层都为空时）
        if let Some(mapping) = self.mapping {
            for (cat, candidates) in mapping_pairs(&mapping) {
                if by_category.get(&cat).is_none_or(|v| v.is_empty()) {
                    let mut matched_existing = false;
                    for candidate in candidates {
                        if remote_folders.contains(candidate) && !classified.contains_key(candidate)
                        {
                            insert_classification(
                                &mut classified,
                                &mut by_category,
                                candidate.clone(),
                                cat,
                            );
                            matched_existing = true;
                        }
                    }

                    if !matched_existing
                        && self.allow_unverified_provider_fallback
                        && let Some(candidate) = candidates.first()
                        && !classified.contains_key(candidate)
                    {
                        insert_classification(
                            &mut classified,
                            &mut by_category,
                            candidate.clone(),
                            cat,
                        );
                    }
                }
            }
        }

        FolderRegistry {
            classified,
            by_category,
            remote_folders,
        }
    }
}

fn classify_protocol_folder(folder: &RemoteFolder) -> Option<FolderCategory> {
    // 第一层：INBOX 保留名（RFC 3501，先于 SPECIAL-USE）
    if folder.name.eq_ignore_ascii_case("INBOX") {
        return Some(FolderCategory::Inbox);
    }
    // 第一层：SPECIAL-USE 属性
    if let Some(cat) = folder.special_use.iter().find_map(|f| f.to_category()) {
        return Some(cat);
    }
    None
}

fn keyword_candidate(folder: &RemoteFolder) -> Option<KeywordCandidate> {
    // 第二层：跨语言关键词（先 IMAP-UTF-7 解码，只取 leaf 精确匹配）
    let decoded = decode_utf7_imap(folder.name.clone());
    let leaf = decoded
        .rsplit(['/', '.'])
        .next()
        .unwrap_or(decoded.as_str());
    let (category, score) = guess_category(leaf)?;
    Some(KeywordCandidate {
        folder_name: folder.name.clone(),
        category,
        score,
        depth: decoded.matches(['/', '.']).count(),
    })
}

fn insert_classification(
    classified: &mut HashMap<String, FolderCategory>,
    by_category: &mut HashMap<FolderCategory, Vec<String>>,
    folder_name: String,
    category: FolderCategory,
) {
    if classified.insert(folder_name.clone(), category).is_none() {
        by_category.entry(category).or_default().push(folder_name);
    }
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
        RemoteFolder {
            name: name.into(),
            special_use: vec![],
            no_select: false,
        }
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
        assert_eq!(
            reg.resolve_one(FolderCategory::Sent).as_deref(),
            Some("[Gmail]/Sent Mail")
        );
    }

    #[test]
    fn keyword_layer_identifies_netease_sent_decoded() {
        // 网易已发送编码名 &XfJT0ZAB- 解码后为 "已发送"
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("&XfJT0ZAB-")])
            .build();
        assert_eq!(
            reg.resolve_one(FolderCategory::Sent).as_deref(),
            Some("&XfJT0ZAB-")
        );
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
        assert_eq!(
            reg.resolve_one(FolderCategory::Sent).as_deref(),
            Some("XYZ")
        );
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
    fn keyword_guess_returns_one_best_folder_for_same_category() {
        // 多个关键词候选同属一个类别时，只保留最佳候选，避免误扩大同步范围。
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Sent"), folder("&XfJT0ZAB-")])
            .build();
        let resolved = reg.resolve(FolderCategory::Sent);
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn keyword_guess_picks_one_best_folder_per_category() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Sent"), folder("Sent Messages")])
            .build();

        assert_eq!(
            reg.resolve(FolderCategory::Sent),
            vec!["Sent Messages".to_string()]
        );
    }

    #[test]
    fn provider_fallback_can_return_default_candidates_without_remote_folders() {
        let mapping = StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec![],
            drafts: vec![],
            spam: vec![],
            trash: vec![],
            archive: vec!["Archive".into()],
        };
        let reg = FolderRegistry::builder()
            .provider_mapping(mapping)
            .allow_unverified_provider_fallback(true)
            .build();

        assert_eq!(
            reg.resolve(FolderCategory::Inbox),
            vec!["INBOX".to_string()]
        );
        assert_eq!(
            reg.resolve(FolderCategory::Archive),
            vec!["Archive".to_string()]
        );
    }

    #[test]
    fn duplicate_remote_folder_names_are_classified_once() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Sent"), folder("Sent")])
            .build();

        assert_eq!(reg.resolve(FolderCategory::Sent), vec!["Sent".to_string()]);
    }

    #[test]
    fn keyword_layer_does_not_classify_nested_custom_leaf_without_exact_match() {
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("Projects/Sent Archive Backup")])
            .build();

        assert!(reg.resolve(FolderCategory::Sent).is_empty());
        assert!(reg.resolve(FolderCategory::Archive).is_empty());
    }

    #[test]
    fn provider_fallback_matches_gmail_chinese_trash_candidate() {
        let mapping = StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec![],
            drafts: vec![],
            spam: vec![],
            trash: vec!["[Gmail]/Trash".into(), "[Gmail]/&V4NXPpCuTvY-".into()],
            archive: vec![],
        };
        let reg = FolderRegistry::builder()
            .remote_folders(vec![folder("[Gmail]/&V4NXPpCuTvY-")])
            .provider_mapping(mapping)
            .allow_unverified_provider_fallback(true)
            .build();

        assert_eq!(
            reg.resolve(FolderCategory::Trash),
            vec!["[Gmail]/&V4NXPpCuTvY-".to_string()]
        );
    }
}
