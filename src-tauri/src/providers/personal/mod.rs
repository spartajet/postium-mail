//! 个人邮件服务商

#![allow(deprecated)]

mod aol;
mod china;
mod cmcc;
mod cn21;
mod gmail;
mod gmail_oauth;
mod gmx;
mod icloud;
mod mail_com;
mod net263;
mod outlook;
mod outlook_oauth;
mod qq;
mod sina;
mod sohu;
mod tom;
mod yahoo;
mod yi;
mod yandex;
mod zoho;

// 重新导出
pub use aol::AolMailProvider;
pub use china::ChinaMailProvider;
pub use cmcc::CmccMailProvider;
pub use cn21::Cn21MailProvider;
pub use gmail::GmailProvider;
pub use gmx::GmxMailProvider;
pub use icloud::ICloudProvider;
pub use mail_com::MailComProvider;
pub use net263::Net263MailProvider;
pub use outlook::OutlookProvider;
pub use qq::QqMailProvider;
pub use sina::SinaMailProvider;
pub use sohu::SohuMailProvider;
pub use tom::TomMailProvider;
pub use yahoo::YahooProvider;
pub use yi::Mail163Provider;
pub use yandex::YandexMailProvider;
pub use zoho::ZohoMailProvider;
