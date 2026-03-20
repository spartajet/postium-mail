//! 个人邮件服务商

#![allow(deprecated)]

mod gmail;
mod gmail_oauth;
mod icloud;
mod outlook;
mod outlook_oauth;
mod qq;
mod yahoo;
mod yi;

// 重新导出
pub use gmail::GmailProvider;
pub use icloud::ICloudProvider;
pub use outlook::OutlookProvider;
pub use qq::QqMailProvider;
pub use yahoo::YahooProvider;
pub use yi::Mail163Provider;
