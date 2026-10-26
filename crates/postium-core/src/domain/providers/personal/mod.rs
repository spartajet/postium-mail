//!
//! # 个人邮箱服务商 (Personal Mail Providers)
//!
//! 本模块汇集了应用程序支持的所有个人邮箱服务商实现，每个子模块对应一个服务商，
//! 实现 [`super::MailProvider`] trait，提供 IMAP/SMTP 配置、域名识别与（可选）OAuth2 配置。
//!
//! ## 支持的服务商
//!
//! ### 国际服务商
//! - `gmail`: Google 邮箱（Gmail，支持 XOAUTH2）
//! - `outlook`: Microsoft 个人邮箱（outlook.com / hotmail.com / live.com）
//! - `yahoo`: Yahoo 邮箱
//! - `aol`: AOL 邮箱
//! - `gmx`: GMX 邮箱
//! - `mailcom`: Mail.com 邮箱
//! - `zoho`: Zoho 邮箱
//! - `yandex`: Yandex 邮箱
//! - `icloud`: Apple iCloud 邮箱
//!
//! ### 国内服务商
//! - `qq`: QQ 邮箱（腾讯）
//! - `yi`: 网易邮箱（163 / 126 / yeah）
//! - `sina`: 新浪邮箱
//! - `sohu`: 搜狐邮箱
//! - `tom`: TOM 邮箱
//! - `cmcc`: 中国移动 139 邮箱
//! - `china`: 中华网邮箱
//! - `net263`: 263 邮箱
//! - `cn21`: 21CN 邮箱
//!
//! ## 注册方式
//!
//! 这些服务商的实例在 [`super::pool::ProviderPool::default`] 中按「国际/国内/企业」分组集中注册。
//!

pub mod aol;
pub mod china;
pub mod cmcc;
pub mod cn21;
pub mod gmail;
pub mod gmx;
pub mod icloud;
pub mod mailcom;
pub mod net263;
pub mod outlook;
pub mod qq;
pub mod sina;
pub mod sohu;
pub mod tom;
pub mod yahoo;
pub mod yandex;
pub mod yi;
pub mod zoho;
