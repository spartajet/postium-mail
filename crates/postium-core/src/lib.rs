pub mod api;
pub mod domain;
pub mod engine;
pub mod error;
pub mod infrastructure;
pub mod rt;
pub mod service;
pub mod sys;

/// 文件夹 wire 名的 modified UTF-7 解码（RFC 3502；UI 显示用，IMAP 操作仍用 wire 名）。
pub fn utf7_imap_folder_decode(folder: &str) -> String {
    utf7_imap::decode_utf7_imap(folder.to_string())
}

pub use sys::log::setup_logging;
