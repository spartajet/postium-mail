//! postium-html：邮件 HTML 渲染预处理 crate（纯 Rust 管线，不依赖 GPUI）。
//!
//! # 管线（对应 spec §2 方案 A）
//!
//! ```text
//! 原始 HTML
//!   → strip_mso_comments   （MSO 条件注释整体删除，本 crate）
//!   → sanitize_lenient     （ammonia 白名单净化，保留 class/id/style，本 crate）
//!   → inline_css           （css-inline，Task 2 接入 pipeline.rs）
//!   → build_document       （html5ever 解析 → [types::HtmlDocument] 中间表示，Task 4 接入）
//! ```
//!
//! 安全边界：树构建期再做终审白名单（SafeUrl / ImageRef，见 spec §4.4），
//! 本 crate 的净化只负责结构白名单与脚本剔除。

pub mod error;
pub mod sanitize;
pub mod types;

pub use error::HtmlError;
pub use sanitize::{sanitize_lenient, strip_mso_comments};
pub use types::{
    Align, Block, BlockStyle, Border, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle,
    Limits, Rgba, SafeUrl, Spacing, TableCell, TableRow, Width,
};
