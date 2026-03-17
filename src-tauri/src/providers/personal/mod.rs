//! 个人邮件服务商

mod gmail;
mod outlook;
mod yahoo;
mod native;

// 重新导出
pub use gmail::GmailProvider;
pub use outlook::OutlookProvider;
pub use yahoo::YahooProvider;
pub use native::NativeProvider;
