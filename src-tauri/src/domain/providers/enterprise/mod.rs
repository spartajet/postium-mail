//! 企业邮箱服务商模块
//!
//! 本模块提供了各类企业邮箱服务商的实现，包括：
//!
//! - [`custom`]: 自定义企业邮箱服务商（用户手动配置 IMAP/SMTP 服务器）
//! - [`google_workspace`]: Google Workspace 企业邮箱服务商
//! - [`microsoft_365`]: Microsoft 365 企业邮箱服务商
//!
//! 这些服务商通常使用 OAuth2 进行身份认证，支持企业自有域名。
//! 企业邮箱与个人邮箱的主要区别在于：
//! - 企业邮箱使用自定义域名
//! - 认证方式通常为 OAuth2
//! - 服务器配置与服务提供商相关，不依赖特定域名

pub mod custom;
pub mod google_workspace;
pub mod microsoft_365;
