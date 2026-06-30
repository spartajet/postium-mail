//! 文件夹识别引擎模块
//!
//! 用三层递进策略（SPECIAL-USE + 跨语言关键词 + provider 候选名）将 IMAP 真实
//! 文件夹名映射到标准类别，统一取代旧的三套 resolve_* 逻辑。
//!
//! 本任务（Task 1）只实现 [`category::FolderCategory`]；后续任务会补上
//! `keyword_table`、`registry`、`special_use` 子模块。

pub mod category;

pub use category::FolderCategory;
