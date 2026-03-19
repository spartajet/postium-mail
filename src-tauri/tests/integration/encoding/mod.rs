//! 编码测试模块
//!
//! 测试邮件编码相关的纯逻辑（不依赖外部服务）

pub mod subject_decoding;  // RFC 2047 主题解码测试
pub mod imap_utf7;         // IMAP UTF-7 编码测试
