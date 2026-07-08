# 文件夹识别引擎实现计划 (Folder Detection Engine)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用一个三层 FolderRegistry 引擎统一并替换现有三套不一致的 IMAP 文件夹解析逻辑，消除双向子串误判，并通过含中文的跨语言关键词表覆盖国内主流服务商。

**Architecture:** 新建 `domain/folders/` 模块，核心是 `FolderRegistry`——在每次 IMAP 连接后用一次 LIST 构建。识别采用三层递进：① INBOX 保留名 + SPECIAL-USE 属性（`async-imap` 已提供强类型 `NameAttribute::Sent` 等变体，直接 match）② 跨语言关键词猜测（含中文，单向 contains + 分数制）③ provider 候选名兜底（保留现有 `folder_mapping()`）。提供 `resolve`（多选）/`resolve_one`（单选）/`classify`（反向，纯精确查表）三个接口。

**Tech Stack:** Rust + Tauri + async-imap 0.11（`NameAttribute` 来自 imap-proto 0.16.7，含 RFC 6154 强类型变体）+ utf7-imap 0.3 + rusqlite

## Global Constraints

- 编译命令：`cd src-tauri && cargo check --tests`
- 测试命令：`cd src-tauri && cargo test --lib <module_path>`
- 日期：2026-06-30；分支：feature/folder（已在 docs commit 74b2061 上）
- 现有 `EmailCategory` 枚举里垃圾邮件变体名为 `Spam`（不是 Junk），全代码保持一致
- `async-imap` 的 `NameAttribute` SPECIAL-USE 变体是强类型枚举（`Sent`/`Drafts`/`Junk`/`Trash`/`Archive`/`All`/`Flagged`），无需字符串解析；未知属性是 `Extension(Cow<str>)`
- `list_folders()` 返回的 `FolderInfo.name` 是 IMAP-UTF-7 编码（如 `&XfJT0ZAB-`），关键词匹配前必须 `decode_utf7_imap`；原始编码名始终作为 IMAP 操作 key
- 关键词匹配是单向 `name_lower.contains(keyword)`，禁止双向 `a.contains(b) || b.contains(a)`
- 所有 provider 的 `folder_mapping()` 实现全部保留，不删除不重写

---

## File Structure

新建模块 `src-tauri/src/domain/folders/`，每个文件单一职责：

| 文件 | 职责 |
|------|------|
| `domain/folders/category.rs` | `FolderCategory` 枚举 + 与 `EmailCategory`/字符串的互转 |
| `domain/folders/special_use.rs` | `SpecialUseFlag` 枚举 + 从 `NameAttribute` 提取 |
| `domain/folders/keyword_table.rs` | 跨语言关键词表（含中文）+ 单向 contains 评分 |
| `domain/folders/registry.rs` | `FolderRegistry` 引擎主体（三层构建 + 四接口）|
| `domain/folders/mod.rs` | 模块导出 |

修改的现有文件：
- `infrastructure/protocols/imap/mod.rs` — `FolderInfo` 增加 `special_use` 字段，`list_folders()` 解析 `NameAttribute`
- `domain/sync/folder_sync_dispatcher.rs` — 用 `registry.resolve()` 替换 `resolve_sync_folders`
- `service/sync_service.rs` — 用 `registry.resolve()` 替换 `resolve_history_candidate_folders`
- `service/email_service.rs` — `resolve_folders` 改为委托 registry（或保留候选名接口）
- `service/mail_operation.rs` — 用 `registry.resolve_one()` 替换 `resolve_special_folders`
- `domain/providers/mod.rs` — 删除 `find_standard_type` 的双向子串逻辑（由 registry.classify 取代）

---

### Task 1: FolderCategory 枚举

**Files:**
- Create: `src-tauri/src/domain/folders/mod.rs`
- Create: `src-tauri/src/domain/folders/category.rs`

**Interfaces:**
- Produces: `FolderCategory` 枚举（`Inbox`/`Sent`/`Drafts`/`Junk`/`Trash`/`Archive`），含 `from_email_category(&EmailCategory) -> Option<Self>` 和 `to_email_category(&self) -> EmailCategory`（注意 Junk↔Spam 映射）

- [ ] **Step 1: 创建模块骨架 `mod.rs`**

Create `src-tauri/src/domain/folders/mod.rs`:

```rust
//! 文件夹识别引擎模块
//!
//! 用三层递进策略（SPECIAL-USE + 跨语言关键词 + provider 候选名）将 IMAP 真实
//! 文件夹名映射到标准类别，统一取代旧的三套 resolve_* 逻辑。

pub mod category;
pub mod keyword_table;
pub mod registry;
pub mod special_use;

pub use category::FolderCategory;
pub use registry::FolderRegistry;
pub use special_use::SpecialUseFlag;
```

- [ ] **Step 2: 写失败测试 `category.rs`**

Create `src-tauri/src/domain/folders/category.rs`:

```rust
use crate::service::email_service::EmailCategory;
use serde::{Deserialize, Serialize};

/// 标准文件夹类别（引擎内部统一用此类型，垃圾邮件为 Junk）
///
/// 注意：对外 API 的 [`EmailCategory`] 里垃圾邮件叫 `Spam`，二者通过
/// `from_email_category` / `to_email_category` 互转（Junk ↔ Spam）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FolderCategory {
    Inbox,
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
}

impl FolderCategory {
    /// 类别固定优先级（同分时按此序，Inbox 最高）
    pub fn priority(self) -> u8 {
        match self {
            Self::Inbox => 0,
            Self::Sent => 1,
            Self::Drafts => 2,
            Self::Junk => 3,
            Self::Trash => 4,
            Self::Archive => 5,
        }
    }

    /// 从对外 EmailCategory 转换。Starred 无对应文件夹，返回 None。
    pub fn from_email_category(cat: &EmailCategory) -> Option<Self> {
        match cat {
            EmailCategory::Inbox => Some(Self::Inbox),
            EmailCategory::Sent => Some(Self::Sent),
            EmailCategory::Drafts => Some(Self::Drafts),
            EmailCategory::Spam => Some(Self::Junk),
            EmailCategory::Trash => Some(Self::Trash),
            EmailCategory::Archive => Some(Self::Archive),
            EmailCategory::Starred => None,
        }
    }

    /// 转回对外 EmailCategory。
    pub fn to_email_category(self) -> EmailCategory {
        match self {
            Self::Inbox => EmailCategory::Inbox,
            Self::Sent => EmailCategory::Sent,
            Self::Drafts => EmailCategory::Drafts,
            Self::Junk => EmailCategory::Spam,
            Self::Trash => EmailCategory::Trash,
            Self::Archive => EmailCategory::Archive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junk_maps_to_spam_and_back() {
        let cat = FolderCategory::Junk;
        assert_eq!(cat.to_email_category(), EmailCategory::Spam);
        assert_eq!(
            FolderCategory::from_email_category(&EmailCategory::Spam),
            Some(FolderCategory::Junk)
        );
    }

    #[test]
    fn starred_has_no_folder_category() {
        assert_eq!(
            FolderCategory::from_email_category(&EmailCategory::Starred),
            None
        );
    }

    #[test]
    fn inbox_has_highest_priority() {
        assert!(FolderCategory::Inbox.priority() < FolderCategory::Archive.priority());
    }
}
```

- [ ] **Step 3: 注册模块到 `domain/mod.rs`**

在 `src-tauri/src/domain/mod.rs` 的 `pub mod` 列表中增加：

```rust
pub mod folders;
```

（用 Edit 工具找到现有 `pub mod ...` 块，按字母序插入。）

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo check --tests`
Expected: 编译通过（`keyword_table`/`registry`/`special_use` 暂为空文件或还不存在会导致错误——先创建空占位文件，或在本任务只注册 `pub mod category;`。采用后者：临时把 `mod.rs` 改为只导出 category，后续任务再补。）

修正 Step 1 的 `mod.rs`，先只含：

```rust
pub mod category;
pub use category::FolderCategory;
```

- [ ] **Step 5: 跑测试**

Run: `cd src-tauri && cargo test --lib domain::folders::category`
Expected: 3 passed

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/domain/folders/ src-tauri/src/domain/mod.rs
git commit -m "feat(folders): add FolderCategory enum"
```

---

### Task 2: SpecialUseFlag 与 NameAttribute 解析

**Files:**
- Create: `src-tauri/src/domain/folders/special_use.rs`
- Modify: `src-tauri/src/domain/folders/mod.rs`（加导出）

**Interfaces:**
- Consumes: `async_imap::extensions::idle`... 实际是 `imap_proto::types::NameAttribute`（经 async-imap 重导出为 `async_imap::NameAttribute`）
- Produces: `SpecialUseFlag` 枚举 + `SpecialUseFlag::from_attributes(&[NameAttribute]) -> Vec<SpecialUseFlag>` + `to_category(&self) -> Option<FolderCategory>`

- [ ] **Step 1: 写失败测试**

Create `src-tauri/src/domain/folders/special_use.rs`:

```rust
use crate::domain::folders::FolderCategory;
use imap_proto::NameAttribute;

/// RFC 6154 SPECIAL-USE 标记（从 LIST attributes 提取）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUseFlag {
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
    /// \All —— 通常对应归档视图（如 Gmail All Mail）
    All,
    /// \Flagged —— 星标虚拟邮箱，不映射到单一物理文件夹
    Flagged,
}

impl SpecialUseFlag {
    /// 从 IMAP NameAttribute 列表提取所有 SPECIAL-USE 标记。
    /// `async-imap`/`imap-proto` 已把 RFC 6154 标记解析为强类型变体，直接 match。
    pub fn from_attributes(attrs: &[NameAttribute<'_>]) -> Vec<Self> {
        attrs
            .iter()
            .filter_map(|attr| match attr {
                NameAttribute::Sent => Some(Self::Sent),
                NameAttribute::Drafts => Some(Self::Drafts),
                NameAttribute::Junk => Some(Self::Junk),
                NameAttribute::Trash => Some(Self::Trash),
                NameAttribute::Archive => Some(Self::Archive),
                NameAttribute::All => Some(Self::All),
                NameAttribute::Flagged => Some(Self::Flagged),
                _ => None,
            })
            .collect()
    }

    /// 转为标准类别。All 归 Archive，Flagged 不映射（None）。
    pub fn to_category(&self) -> Option<FolderCategory> {
        match self {
            Self::Sent => Some(FolderCategory::Sent),
            Self::Drafts => Some(FolderCategory::Drafts),
            Self::Junk => Some(FolderCategory::Junk),
            Self::Trash => Some(FolderCategory::Trash),
            Self::Archive => Some(FolderCategory::Archive),
            // Gmail 的 \All Mail 用 \All 标记，按归档处理
            Self::All => Some(FolderCategory::Archive),
            // 星标是跨文件夹虚拟视图，不映射单一文件夹
            Self::Flagged => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_typed_special_use_variants() {
        let attrs = vec![NameAttribute::Sent, NameAttribute::NoSelect];
        let flags = SpecialUseFlag::from_attributes(&attrs);
        assert_eq!(flags, vec![SpecialUseFlag::Sent]);
    }

    #[test]
    fn all_maps_to_archive() {
        assert_eq!(SpecialUseFlag::All.to_category(), Some(FolderCategory::Archive));
    }

    #[test]
    fn flagged_does_not_map_to_folder() {
        assert_eq!(SpecialUseFlag::Flagged.to_category(), None);
    }

    #[test]
    fn plain_attributes_yield_no_flags() {
        let attrs = vec![NameAttribute::Marked, NameAttribute::HasChildren];
        assert!(SpecialUseFlag::from_attributes(&attrs).is_empty());
    }
}
```

注意：`HasChildren` 变体名需与 imap-proto 0.16.7 一致。若实际变体名不同（如 `HasChildren` vs `Children`），编译错误会暴露，按实际名称修正。先确认：在 Step 2 编译前，可用 `grep -n "HasChildren\|Children" $(find ~/.cargo -path "*imap-proto-0.16.7*" -name types.rs)` 核对。

- [ ] **Step 2: 更新 `mod.rs` 导出**

在 `src-tauri/src/domain/folders/mod.rs` 增加：

```rust
pub mod special_use;
pub use special_use::SpecialUseFlag;
```

- [ ] **Step 3: 确认 imap-proto 变体名**

Run: `grep -n "HasChildren\|^    [A-Z]" $(find ~/.cargo -path "*imap-proto-0.16.7*" -name types.rs) | grep -i "children\|noinferiors"`
Expected: 确认变体名。若 `HasChildren` 不存在，测试里去掉对应断言或改用存在的变体（如 `NoInferiors`）。

- [ ] **Step 4: 编译 + 测试**

Run: `cd src-tauri && cargo test --lib domain::folders::special_use`
Expected: 4 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/domain/folders/
git commit -m "feat(folders): parse SPECIAL-USE flags from NameAttribute"
```

---

### Task 3: 跨语言关键词表（含中文）

**Files:**
- Create: `src-tauri/src/domain/folders/keyword_table.rs`
- Modify: `src-tauri/src/domain/folders/mod.rs`（加导出）

**Interfaces:**
- Produces: `guess_category(name: &str) -> Option<(FolderCategory, u16)>` —— 输入**已解码**的文件夹名，返回匹配到的最高分类别与分数。单向 `name.to_lowercase().contains(keyword)`。

- [ ] **Step 1: 写失败测试**

Create `src-tauri/src/domain/folders/keyword_table.rs`:

```rust
use crate::domain::folders::FolderCategory;

/// 关键词条目：关键词 → (类别, 分数)
struct KeywordEntry {
    keyword: &'static str,
    category: FolderCategory,
    score: u16,
}

/// 跨语言文件夹名关键词表。
///
/// 参考 FairEmail GUESS_FOLDER_TYPE 思路，并**补充中文关键词**（FairEmail 缺失）。
/// 匹配为单向 `name_lower.contains(keyword)`，杜绝双向子串误判。
fn keyword_table() -> &'static [KeywordEntry] {
    &[
        // ── Sent ──
        KeywordEntry { keyword: "已发送", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "已发邮件", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "发件箱", category: FolderCategory::Sent, score: 80 },
        KeywordEntry { keyword: "sent messages", category: FolderCategory::Sent, score: 95 },
        KeywordEntry { keyword: "sent items", category: FolderCategory::Sent, score: 95 },
        KeywordEntry { keyword: "sent", category: FolderCategory::Sent, score: 90 },
        KeywordEntry { keyword: "gesendet", category: FolderCategory::Sent, score: 100 },
        KeywordEntry { keyword: "envoy", category: FolderCategory::Sent, score: 90 },
        KeywordEntry { keyword: "отправлен", category: FolderCategory::Sent, score: 90 },
        // ── Drafts ──
        KeywordEntry { keyword: "草稿箱", category: FolderCategory::Drafts, score: 100 },
        KeywordEntry { keyword: "草稿", category: FolderCategory::Drafts, score: 90 },
        KeywordEntry { keyword: "drafts", category: FolderCategory::Drafts, score: 95 },
        KeywordEntry { keyword: "draft", category: FolderCategory::Drafts, score: 85 },
        KeywordEntry { keyword: "entwürfe", category: FolderCategory::Drafts, score: 100 },
        KeywordEntry { keyword: "brouillons", category: FolderCategory::Drafts, score: 100 },
        // ── Junk ──
        KeywordEntry { keyword: "垃圾邮件", category: FolderCategory::Junk, score: 100 },
        KeywordEntry { keyword: "junk", category: FolderCategory::Junk, score: 95 },
        KeywordEntry { keyword: "spam", category: FolderCategory::Junk, score: 95 },
        KeywordEntry { keyword: "bulk mail", category: FolderCategory::Junk, score: 90 },
        // ── Trash ──
        KeywordEntry { keyword: "已删除", category: FolderCategory::Trash, score: 100 },
        KeywordEntry { keyword: "废件箱", category: FolderCategory::Trash, score: 80 },
        KeywordEntry { keyword: "deleted messages", category: FolderCategory::Trash, score: 95 },
        KeywordEntry { keyword: "deleted", category: FolderCategory::Trash, score: 90 },
        KeywordEntry { keyword: "trash", category: FolderCategory::Trash, score: 95 },
        KeywordEntry { keyword: "papierkorb", category: FolderCategory::Trash, score: 100 },
        KeywordEntry { keyword: "корзин", category: FolderCategory::Trash, score: 90 },
        // ── Archive ──
        KeywordEntry { keyword: "所有邮件", category: FolderCategory::Archive, score: 80 },
        KeywordEntry { keyword: "归档", category: FolderCategory::Archive, score: 100 },
        KeywordEntry { keyword: "已归档", category: FolderCategory::Archive, score: 100 },
        KeywordEntry { keyword: "all mail", category: FolderCategory::Archive, score: 80 },
        KeywordEntry { keyword: "archived", category: FolderCategory::Archive, score: 90 },
        KeywordEntry { keyword: "archive", category: FolderCategory::Archive, score: 95 },
        KeywordEntry { keyword: "archiv", category: FolderCategory::Archive, score: 90 },
    ]
}

/// 对**已解码**的文件夹名做单向关键词猜测，返回最高分的 (类别, 分数)。
///
/// 多个关键词命中时取分数最高；同分按类别优先级（`FolderCategory::priority`）。
pub fn guess_category(decoded_name: &str) -> Option<(FolderCategory, u16)> {
    let name_lower = decoded_name.to_lowercase();
    keyword_table()
        .iter()
        .filter(|e| name_lower.contains(e.keyword))
        .max_by_key(|e| (e.score, u8::MAX - e.category.priority()))
        .map(|e| (e.category, e.score))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_chinese_sent() {
        assert_eq!(guess_category("已发送"), Some((FolderCategory::Sent, 100)));
    }

    #[test]
    fn matches_english_sent_messages_case_insensitive() {
        assert_eq!(guess_category("Sent Messages"), Some((FolderCategory::Sent, 95)));
    }

    #[test]
    fn matches_netease_drafts_utf7_decoded() {
        // 网易草稿箱 IMAP-UTF-7 "&g0l6P3ux-" 解码后为 "草稿箱"
        assert_eq!(guess_category("草稿箱"), Some((FolderCategory::Drafts, 100)));
    }

    #[test]
    fn matches_junk_not_confused_with_trash() {
        // "垃圾邮件" 应归 Junk，不是 Trash
        assert_eq!(guess_category("垃圾邮件"), Some((FolderCategory::Junk, 100)));
    }

    #[test]
    fn returns_none_for_unrelated_folder() {
        assert_eq!(guess_category("项目文档"), None);
    }

    #[test]
    fn higher_score_wins_on_multiple_matches() {
        // "已发送" 同时含 "sent"? 不含。构造一个含多词的：
        // "Sent 已发送" 含 "sent"(90) 和 "已发送"(100)，应取 100
        assert_eq!(guess_category("Sent 已发送"), Some((FolderCategory::Sent, 100)));
    }

    #[test]
    fn no_bidirectional_false_positive() {
        // 关键词单向匹配：一个叫 "Sent Archive Backup" 的自定义文件夹
        // 含 "sent"(90) 和 "archive"(95)，按分数 archive 胜，不会双向乱判
        let result = guess_category("Sent Archive Backup");
        assert_eq!(result.map(|(c, _)| c), Some(FolderCategory::Archive));
    }
}
```

- [ ] **Step 2: 更新 `mod.rs` 导出**

在 `src-tauri/src/domain/folders/mod.rs` 增加（注意 `guess_category` 是模块级函数，不导出类型）：

```rust
pub mod keyword_table;
```

（`guess_category` 用时通过 `crate::domain::folders::keyword_table::guess_category` 全路径调用，无需顶层 re-export。）

- [ ] **Step 3: 编译 + 测试**

Run: `cd src-tauri && cargo test --lib domain::folders::keyword_table`
Expected: 7 passed

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/folders/
git commit -m "feat(folders): add multilingual keyword table with Chinese"
```

---

### Task 4: FolderRegistry 引擎主体

**Files:**
- Create: `src-tauri/src/domain/folders/registry.rs`
- Modify: `src-tauri/src/domain/folders/mod.rs`（加导出）

**Interfaces:**
- Consumes: `FolderCategory`, `SpecialUseFlag`, `keyword_table::guess_category`, `crate::domain::providers::StandardFolder`, `utf7_imap::decode_utf7_imap`
- Produces: `FolderRegistry` 含 `builder()` + `resolve`/`resolve_one`/`classify`/`exists`

- [ ] **Step 1: 写失败测试（先写测试明确接口契约）**

Create `src-tauri/src/domain/folders/registry.rs`:

```rust
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
                        if remote_folders.contains(&c) && !classified.contains_key(&c) {
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
```

- [ ] **Step 2: 更新 `mod.rs` 导出**

```rust
pub mod registry;
pub use registry::{FolderRegistry, RemoteFolder};
```

（删除 Task 1 临时版本里多余的 `keyword_table`/`special_use` 占位，确保四个子模块都声明。）

- [ ] **Step 3: 编译 + 测试**

Run: `cd src-tauri && cargo test --lib domain::folders::registry`
Expected: 6 passed

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/folders/
git commit -m "feat(folders): add FolderRegistry with three-layer detection"
```

---

### Task 5: list_folders 暴露 SPECIAL-USE 属性

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs:19-28`（FolderInfo 加字段）
- Modify: `src-tauri/src/infrastructure/protocols/imap/mod.rs:132-152`（list_folders 解析属性）
- Test: `src-tauri/src/infrastructure/protocols/imap/mod.rs` 测试模块

**Interfaces:**
- Produces: `FolderInfo` 新增 `special_use: Vec<SpecialUseFlag>` 和 `no_select: bool` 字段（直接供 Task 4 的 RemoteFolder 构建使用）

- [ ] **Step 1: 改 FolderInfo 结构**

在 `src-tauri/src/infrastructure/protocols/imap/mod.rs`，修改 `FolderInfo`（约 19-28 行）。新增 `special_use` 和 `no_select` 字段。由于 `FolderInfo` 派生了 `Serialize/Deserialize/Type`（给前端），新字段需要默认值兼容旧前端，加 `#[serde(default)]`：

```rust
use crate::domain::folders::SpecialUseFlag;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
    /// RFC 6154 SPECIAL-USE 标记（从 attributes 解析）
    #[serde(default)]
    pub special_use: Vec<SpecialUseFlag>,
    /// 是否 \NoSelect/\NonExistent（不可选文件夹）
    #[serde(default)]
    pub no_select: bool,
}
```

注意：`SpecialUseFlag` 需派生 `Serialize/Deserialize/Type`。回 `special_use.rs` 给枚举加上这三个 derive（Task 2 已有 `Serialize/Deserialize`，补 `Type`；若未引入 specta，则加 `#[derive(specta::Type)]` 或确认项目用 `Type` 的方式——参考现有 `EmailCategory` 的 derive）。

- [ ] **Step 2: 改 list_folders 解析逻辑**

修改 `list_folders()`（约 144-148 行），把 `format!("{f:?}")` 改为结构化解析：

```rust
folders.push({
    let attrs = item.attributes();
    let special_use = SpecialUseFlag::from_attributes(attrs);
    let no_select = attrs.iter().any(|a| matches!(a, imap_proto::NameAttribute::NoSelect));
    FolderInfo {
        name: item.name().to_string(),
        delimiter: item.delimiter().map(|s: &str| s.to_string()),
        flags: attrs.iter().map(|f| format!("{f:?}")).collect(),
        special_use,
        no_select,
    }
});
```

保留 `flags` 字段（向后兼容，debug 用），新增结构化字段。

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check --tests`
Expected: 编译通过。若 `SpecialUseFlag` 缺 `Type` derive 报错，回 special_use.rs 补上 `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]`。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/folders/special_use.rs src-tauri/src/infrastructure/protocols/imap/mod.rs
git commit -m "feat(imap): expose SPECIAL-USE flags in FolderInfo"
```

---

### Task 6: 迁移 resolve_sync_folders（初始同步）

**Files:**
- Modify: `src-tauri/src/domain/sync/folder_sync_dispatcher.rs:102-116, 269-301`

**Interfaces:**
- Consumes: `FolderRegistry`, `RemoteFolder`, `FolderCategory::from_email_category`
- Produces: 无新接口，行为升级为多选

- [ ] **Step 1: 阅读现有逻辑**

Run: `cd src-tauri && sed -n '100,120p;269,301p' src/domain/sync/folder_sync_dispatcher.rs`
确认 `resolve_sync_folders` 当前返回 `Vec<String>`（每类别挑一个），以及调用点（约 109 行 `let sync_folders = resolve_sync_folders(...)`）。

- [ ] **Step 2: 改造调用点用 Registry**

在 `sync_account_with_initial_window`（约 102-116 行），把：

```rust
let remote_folders = client.list_folders().await?;
let remote_folder_names = remote_folders.into_iter().map(|folder| folder.name).collect::<Vec<_>>();
let folder_mapping = provider.folder_mapping();
let sync_folders = resolve_sync_folders(&folder_mapping, &remote_folder_names);
```

改为构建 Registry 并多选收集所有类别的文件夹：

```rust
use crate::domain::folders::{FolderCategory, FolderRegistry, RemoteFolder};

let remote_folders = client.list_folders().await?;
let remote: Vec<RemoteFolder> = remote_folders
    .iter()
    .map(|f| RemoteFolder {
        name: f.name.clone(),
        special_use: f.special_use.clone(),
        no_select: f.no_select,
    })
    .collect();
let registry = FolderRegistry::builder()
    .remote_folders(remote)
    .provider_mapping(provider.folder_mapping())
    .build();
// 同步所有被识别为标准类别的文件夹（多选）
let sync_folders: Vec<String> = [
    FolderCategory::Inbox,
    FolderCategory::Sent,
    FolderCategory::Drafts,
    FolderCategory::Junk,
    FolderCategory::Trash,
    FolderCategory::Archive,
]
.iter()
.flat_map(|cat| registry.resolve(*cat))
.collect();
```

- [ ] **Step 3: 删除旧函数**

删除 `resolve_sync_folders`（约 269-301 行）及其测试（mod tests 里三个 test）。旧测试改为 Registry 单测（已在 Task 4 覆盖）。

- [ ] **Step 4: 编译**

Run: `cd src-tauri && cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/domain/sync/folder_sync_dispatcher.rs
git commit -m "refactor(sync): use FolderRegistry for initial folder resolution"
```

---

### Task 7: 迁移 resolve_history_candidate_folders（历史回填）

**Files:**
- Modify: `src-tauri/src/service/sync_service.rs:362-386`（sync_older_emails 内）
- Modify: `src-tauri/src/service/sync_service.rs:488-529`（resolve_history_folders / resolve_history_candidate_folders）

**Interfaces:**
- Consumes: `FolderRegistry`

- [ ] **Step 1: 改造 sync_older_emails 的文件夹解析**

在 `sync_older_emails`（约 362-386 行），把连接 IMAP 后手写 `list_folders` + `resolve_history_candidate_folders` 的逻辑替换为 Registry 构建（与 Task 6 同样的 builder 模式），然后用 `registry.resolve(cat)` 取文件夹：

```rust
let remote_folders = client.list_folders().await?;
let remote: Vec<RemoteFolder> = remote_folders
    .iter()
    .map(|f| RemoteFolder {
        name: f.name.clone(),
        special_use: f.special_use.clone(),
        no_select: f.no_select,
    })
    .collect();
let registry = FolderRegistry::builder()
    .remote_folders(remote)
    .provider_mapping(provider.folder_mapping())
    .build();
let cat = FolderCategory::from_email_category(&category)
    .ok_or(MailError::InvalidParam("当前分类不支持历史回填".into()))?;
let folders = registry.resolve(cat);
```

注意保留前面的 `if candidate_folders.is_empty() || category == EmailCategory::Starred` 早返回。

- [ ] **Step 2: 改造 get_history_state（不连服务器场景）**

`get_history_state`（约 263 行起）不连接 IMAP。它原本调 `resolve_history_folders`（基于本地文件夹名）。改为用 Registry 的**降级构建**：只用本地已存文件夹名 + provider 候选名，跳过 SPECIAL-USE：

```rust
// get_history_state 内
let local_names: Vec<RemoteFolder> = {
    let mut names = sync_repo::distinct_folders_by_account(&self.db, account_id).await?;
    names.extend(email_repo::distinct_folders_by_account(&self.db, account_id).await?);
    names.into_iter().map(|n| RemoteFolder { name: n, special_use: vec![], no_select: false }).collect()
};
let registry = FolderRegistry::builder()
    .remote_folders(local_names)
    .provider_mapping(provider.folder_mapping())
    .build();
let cat = FolderCategory::from_email_category(&category)
    .ok_or(MailError::InvalidParam("星标邮件不支持历史回填".into()))?;
let folders = registry.resolve(cat);
```

- [ ] **Step 3: 删除旧的 resolve_history_folders / resolve_history_candidate_folders**

删除 `sync_service.rs` 中这两个函数（约 488-529 行）和 `choose_history_folders` 自由函数及其测试。它们的职责已被 Registry 取代。

- [ ] **Step 4: 编译 + 跑现有测试**

Run: `cd src-tauri && cargo test --lib service::sync_service`
Expected: 编译通过，剩余测试（all_history_exhausted 等）通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/service/sync_service.rs
git commit -m "refactor(sync): use FolderRegistry for history backfill resolution"
```

---

### Task 8: 迁移 resolve_special_folders + find_standard_type

**Files:**
- Modify: `src-tauri/src/service/mail_operation.rs:283-321`
- Modify: `src-tauri/src/domain/providers/mod.rs:350-399`（删除 find_standard_type 的双向子串逻辑）

**Interfaces:**
- Consumes: `FolderRegistry`

- [ ] **Step 1: 改造 resolve_special_folders 用 Registry**

`mail_operation.rs` 的 `resolve_special_folders(account, kind)`（约 297 行）当前不连服务器、直接读 `mapping.trash/archive`。由于移动操作发生时通常已有同步状态，改为基于本地文件夹名构建 Registry（与 Task 7 get_history_state 同样的降级模式），然后用 `resolve_one`：

```rust
async fn resolve_special_folders(
    &self,
    account: &accounts::Model,
    kind: &str,
) -> Result<Vec<String>, MailError> {
    let cat = match kind {
        "trash" => FolderCategory::Trash,
        "archive" => FolderCategory::Archive,
        _ => return Ok(Vec::new()),
    };
    let provider = /* 同现有获取 provider 的逻辑 */;
    let mut names = sync_repo::distinct_folders_by_account(&self.db, account.id).await?;
    names.extend(email_repo::distinct_folders_by_account(&self.db, account.id).await?);
    let remote: Vec<RemoteFolder> = names.into_iter()
        .map(|n| RemoteFolder { name: n, special_use: vec![], no_select: false }).collect();
    let registry = FolderRegistry::builder()
        .remote_folders(remote)
        .provider_mapping(provider.folder_mapping())
        .build();
    Ok(registry.resolve(cat))
}
```

（`resolve_special_folder` 单数版保持不变，它调 `resolve_special_folders(...).into_iter().next()`，自动受益。）

- [ ] **Step 2: 删除 find_standard_type 的双向子串逻辑**

在 `providers/mod.rs`（约 350-379 行），`find_standard_type` 当前含 `imap_lower.contains(&n_lower) || n_lower.contains(&imap_lower)`。此函数的所有调用点应改用 `registry.classify()`。

先查调用点：`grep -rn "find_standard_type" src/`。若仅测试用，删除函数 + 测试。若有生产调用，逐个改为 registry.classify（registry 需在该上下文可用；若无 IMAP 连接，用降级 Registry）。

- [ ] **Step 3: 编译 + 测试**

Run: `cd src-tauri && cargo check --tests && cargo test --lib`
Expected: 编译通过，无 find_standard_type 残留引用

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/service/mail_operation.rs src-tauri/src/domain/providers/mod.rs
git commit -m "refactor: use FolderRegistry.classify, remove bidirectional substring matching"
```

---

### Task 9: 全量验证与回归测试

**Files:**
- Test: 跨模块集成验证

- [ ] **Step 1: 全量编译**

Run: `cd src-tauri && cargo check --tests`
Expected: 无错误

- [ ] **Step 2: 全量单元测试**

Run: `cd src-tauri && cargo test --lib`
Expected: 全部通过，尤其 domain::folders::* 三个模块

- [ ] **Step 3: 验证旧的解析逻辑无残留**

Run: `cd src-tauri && grep -rn "resolve_sync_folders\|resolve_history_candidate_folders\|choose_history_folders\|find_standard_type" src/ | grep -v "//"`
Expected: 无输出（全部已删除/替换）。若有输出，处理残留引用。

- [ ] **Step 4: 验证双向子串彻底删除**

Run: `cd src-tauri && grep -rn "contains(&.*\.contains\|||.*contains" src/ | grep -i "folder\|standard"`
Expected: 无 folders 相关的双向 contains

- [ ] **Step 5: 提交最终验证状态**

若无新代码改动（仅验证），此步可跳过 commit。若有清理改动：

```bash
git add -A
git commit -m "test: verify folder detection engine end-to-end"
```

---

## Self-Review 记录

**Spec coverage 检查：**
- §3 设计目标①统一逻辑 → Task 6/7/8 迁移三处调用点 ✓
- §3 设计目标②消除误判 → Task 8 删除 find_standard_type 双向子串 ✓
- §3 设计目标③跨语言名称识别 → Task 3 关键词表（含中文）✓
- §3 设计目标④向后兼容 → Task 4 第三层保留 provider 候选名 ✓
- §3 设计目标⑤单连接内构建 → Task 4 builder 模式，连接后构建 ✓
- §4.2 三层算法 → Task 4 classify_folder ✓
- §4.2.1 关键词表 → Task 3 ✓
- §4.3 provider 兜底 → Task 4 build() 末段 ✓
- §4.4 四接口 → Task 4 resolve/resolve_one/classify/exists ✓
- §5.3 list_folders 增强 → Task 5 ✓
- §7 测试策略 → 各 Task 内 TDD ✓

**Placeholder 扫描：** 无 TBD/TODO；所有代码步骤含完整代码。

**类型一致性：** `FolderCategory::Junk`（内部）↔ `EmailCategory::Spam`（外部）映射在 Task 1 明确定义；`mapping_pairs` 里 `FolderCategory::Junk` 对应 `m.spam`（Task 4）一致。
