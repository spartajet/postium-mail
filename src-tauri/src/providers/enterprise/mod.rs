//! 企业邮件服务商

mod microsoft_365;
mod google_workspace;
mod custom;

// 重新导出
pub use microsoft_365::Microsoft365Provider;
pub use google_workspace::GoogleWorkspaceProvider;
pub use custom::CustomProvider;
