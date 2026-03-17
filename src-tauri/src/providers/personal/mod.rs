//! 个人邮件服务商

mod gmail;
mod gmail_oauth;
mod icloud;
mod mail163;
mod outlook;
mod outlook_oauth;
mod qq;
mod yahoo;
mod native;

// 重新导出
pub use gmail::GmailProvider;
pub use gmail_oauth::{GmailOAuthService, GmailTokenResponse};
pub use icloud::ICloudProvider;
pub use mail163::Mail163Provider;
pub use outlook::OutlookProvider;
pub use outlook_oauth::{OutlookOAuthService, OutlookTokenResponse};
pub use qq::QqMailProvider;
pub use yahoo::YahooProvider;
pub use native::NativeProvider;
