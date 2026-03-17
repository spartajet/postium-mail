//! 个人邮件服务商

mod gmail;
mod gmail_oauth;
mod outlook;
mod outlook_oauth;
mod yahoo;
mod native;

// 重新导出
pub use gmail::GmailProvider;
pub use gmail_oauth::{GmailOAuthService, GmailTokenResponse};
pub use outlook::OutlookProvider;
pub use outlook_oauth::{OutlookOAuthService, OutlookTokenResponse};
pub use yahoo::YahooProvider;
pub use native::NativeProvider;
