//! 文件夹识别引擎模块
//!
//! 用三层递进策略（SPECIAL-USE + 跨语言关键词 + provider 候选名）将 IMAP 真实
//! 文件夹名映射到标准类别，统一取代旧的三套 resolve_* 逻辑。

pub mod category;
pub mod keyword_table;
pub mod registry;
pub mod special_use;

pub use category::FolderCategory;
pub use registry::{FolderRegistry, RemoteFolder};
pub use special_use::SpecialUseFlag;
