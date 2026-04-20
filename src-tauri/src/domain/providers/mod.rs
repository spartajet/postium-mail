//!
//! # 邮件服务提供商模块 (Mail Provider Module)
//!
//! 本模块定义了各种邮件服务提供商的配置和行为，是应用程序与不同邮件服务商交互的基础。
//!
//! ## 模块组织
//!
//! ### detect - 服务商检测
//!
//! 根据邮箱地址自动检测所属的邮件服务提供商：
//! - 解析邮箱域名
//! - 在已注册的服务商中查找匹配项
//! - 返回服务商配置信息
//!
//! ### personal - 个人邮箱服务商
//!
//! 支持常见的个人邮箱服务商，包括：
//! - Gmail（Google 邮箱）
//! - Outlook（Microsoft 邮箱）
//! - QQ 邮箱
//! - 163 邮箱（网易）
//! - 126 邮箱（网易）
//! - Yeah 邮箱（网易）
//!
//! ### enterprise - 企业邮箱服务商
//!
//! 支持企业级邮箱配置：
//! - 自定义域名的企业邮箱
//! - 支持手动配置 IMAP/SMTP 服务器
//! - 支持各种企业邮件系统
//!
//! ### pool - 服务商池
//!
//! 管理所有已注册的服务商实例：
//! - 全局单例模式（`PROVIDER_POOL`）
//! - 服务商注册和查询
//! - 按域名或 ID 查找服务商
//!
//! ## 核心概念
//!
//! ### 值对象
//!
//! - `AccountType`: 账号类型（个人/企业）
//! - `AuthType`: 认证方式（密码/OAuth2）
//! - `ProviderInfo`: 服务商基本信息
//! - `ImapServerConfig`: IMAP 服务器配置
//! - `SmtpServerConfig`: SMTP 服务器配置
//! - `SslMode`: SSL/TLS 加密模式
//! - `StandardFolder`: 标准文件夹映射
//! - `OAuthConfig`: OAuth2 认证配置
//!
//! ### 核心接口
//!
//! `MailProvider` trait 定义了邮件服务商的统一接口：
//! - 获取服务商信息
//! - 获取 IMAP/SMTP 配置
//! - 支持的域名列表
//! - OAuth2 配置（可选）
//! - 文件夹映射
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use crate::domain::providers::pool::ProviderPool;
//!
//! // 检测邮箱服务商
//! let pool = ProviderPool::global();
//! let result = pool.detect("user@gmail.com");
//!
//! // 获取 IMAP 配置
//! let provider = pool.get_by_id("gmail").unwrap();
//! let imap_config = provider.imap_config("user@gmail.com");
//! ```
//!

/// 服务商检测模块
///
/// 根据邮箱地址自动检测对应的邮件服务提供商。
pub mod detect;

/// 企业邮箱服务商模块
///
/// 支持自建域名的企业邮箱，允许用户手动配置服务器信息。
pub mod enterprise;

/// 个人邮箱服务商模块
///
/// 支持常见的个人邮箱服务商（Gmail、Outlook、QQ 邮箱等）。
pub mod personal;

/// 服务商池模块
///
/// 管理所有已注册的服务商实例，提供全局单例访问。
pub mod pool;

/// 重新导出服务商池类型
///
/// `ProviderPool` 是所有服务商信息的唯一来源（Single Source of Truth）。
pub use pool::ProviderPool;

use serde::{Deserialize, Serialize};
use specta::Type;

// ═══════════════════════════════════════════════════════════════
// 值对象 (Value Objects)
// ═══════════════════════════════════════════════════════════════

/// 账号类型枚举
///
/// 区分个人邮箱和企业邮箱两种类型。
/// 不同类型的邮箱可能有不同的配置选项和功能支持。
///
/// # 变体说明
///
/// - `Personal`: 个人邮箱（如 Gmail、QQ 邮箱）
/// - `Enterprise`: 企业邮箱（自建域名邮箱）
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum AccountType {
    /// 个人邮箱
    Personal,
    /// 企业邮箱
    Enterprise,
}

/// 服务商基本信息
///
/// 包含邮件服务提供商的基本描述信息，用于前端展示和服务商识别。
///
/// # 字段说明
///
/// - `id`: 服务商唯一标识（如 "gmail", "outlook", "qq"）
/// - `name`: 服务商显示名称（如 "Gmail", "Outlook", "QQ邮箱"）
/// - `account_type`: 账号类型（个人/企业）
/// - `domains`: 支持的邮箱域名列表（如 ["gmail.com", "googlemail.com"]）
/// - `auth_type`: 支持的认证方式（密码/OAuth2）
/// - `color`: 品牌颜色（十六进制，用于 UI 显示）
/// - `icon`: 图标名称或 URL（用于 UI 显示）
/// - `sort_order`: 排序权重（数值越小越靠前）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderInfo {
    /// 服务商唯一标识
    pub id: String,
    /// 服务商显示名称
    pub name: String,
    /// 账号类型（个人/企业）
    pub account_type: AccountType,
    /// 支持的邮箱域名列表
    pub domains: Vec<String>,
    /// 认证方式
    pub auth_type: AuthType,
    /// 品牌颜色（可选，十六进制颜色值）
    pub color: Option<String>,
    /// 图标（可选，图标名称或 URL）
    pub icon: Option<String>,
    /// 排序权重
    pub sort_order: u32,
}

/// 认证方式枚举
///
/// 定义邮箱支持的认证方式，不同服务商可能支持不同的认证方式。
///
/// # 变体说明
///
/// - `Password`: 传统密码认证（用户名 + 密码）
/// - `OAuth2`: OAuth2 令牌认证（更安全，支持令牌刷新）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum AuthType {
    /// 密码认证
    Password,
    /// OAuth2 认证
    OAuth2,
}

impl AuthType {
    /// 获取认证方式的字符串表示
    ///
    /// # 返回值
    ///
    /// - `Password` → "Password"
    /// - `OAuth2` → "OAuth2"
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::Password => "Password",
            AuthType::OAuth2 => "OAuth2",
        }
    }
}

/// IMAP 服务器配置
///
/// 定义 IMAP 邮件接收服务器的连接参数。
///
/// # 字段说明
///
/// - `host`: 服务器主机名或 IP 地址
/// - `port`: 服务器端口号
/// - `ssl`: SSL/TLS 加密模式
///
/// # 常见配置示例
///
/// | 服务商 | Host | Port | SSL |
/// |--------|------|------|-----|
/// | Gmail | imap.gmail.com | 993 | Implicit |
/// | Outlook | outlook.office365.com | 993 | Implicit |
/// | QQ | imap.qq.com | 993 | Implicit |
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImapServerConfig {
    /// IMAP 服务器主机名
    pub host: String,
    /// IMAP 服务器端口
    pub port: u16,
    /// SSL/TLS 加密模式
    pub ssl: SslMode,
}

/// SMTP 服务器配置
///
/// 定义 SMTP 邮件发送服务器的连接参数。
///
/// # 字段说明
///
/// - `host`: 服务器主机名或 IP 地址
/// - `port`: 服务器端口号
/// - `ssl`: SSL/TLS 加密模式
///
/// # 常见配置示例
///
/// | 服务商 | Host | Port | SSL |
/// |--------|------|------|-----|
/// | Gmail | smtp.gmail.com | 465/587 | Implicit/StartTls |
/// | Outlook | smtp.office365.com | 587 | StartTls |
/// | QQ | smtp.qq.com | 465 | Implicit |
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SmtpServerConfig {
    /// SMTP 服务器主机名
    pub host: String,
    /// SMTP 服务器端口
    pub port: u16,
    /// SSL/TLS 加密模式
    pub ssl: SslMode,
}

/// SSL/TLS 加密模式
///
/// 定义邮件服务器的加密连接方式。
///
/// # 变体说明
///
/// - `None`: 不加密（明文传输，不推荐）
/// - `StartTls`: STARTTLS 模式（先明文连接，再升级为加密）
/// - `Implicit`: 隐式 TLS（直接建立加密连接，推荐）
///
/// # 安全建议
///
/// - 推荐使用 `Implicit` 模式，安全性最高
/// - `StartTls` 适用于不支持隐式 TLS 的服务器
/// - 避免使用 `None` 模式，除非在本地测试环境
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub enum SslMode {
    /// 不加密
    None,
    /// STARTTLS（明文升级为加密）
    StartTls,
    /// 隐式 TLS（直接加密）
    Implicit,
}

/// 标准文件夹映射
///
/// 将不同邮件服务商的文件夹名称映射到统一的标准类型。
/// 这样前端可以使用统一的方式来处理不同服务商的文件夹。
///
/// # 字段说明
///
/// - `inbox`: 收件箱（如 "INBOX"）
/// - `sent`: 已发送（如 "Sent", "Sent Items"）
/// - `drafts`: 草稿箱（如 "Drafts"）
/// - `spam`: 垃圾邮件（如 "Spam", "Junk"）
/// - `trash`: 已删除（如 "Trash", "Deleted"）
/// - `archive`: 归档（如 "Archive"）
///
/// # 使用示例
///
/// ```rust,ignore
/// let mapping = provider.folder_mapping();
/// let folder_type = mapping.find_standard_type("INBOX"); // 返回 "inbox"
/// let folder_type = mapping.find_standard_type("Junk");  // 返回 "spam"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StandardFolder {
    /// 收件箱名称列表
    pub inbox: Vec<String>,
    /// 已发送名称列表
    pub sent: Vec<String>,
    /// 草稿箱名称列表
    pub drafts: Vec<String>,
    /// 垃圾邮件名称列表
    pub spam: Vec<String>,
    /// 已删除名称列表
    pub trash: Vec<String>,
    /// 归档名称列表
    pub archive: Vec<String>,
}

impl StandardFolder {
    /// 默认英文文件夹映射
    ///
    /// 返回标准的英文文件夹名称映射，适用于大多数邮件服务商。
    ///
    /// # 映射内容
    ///
    /// - `inbox`: ["INBOX"]
    /// - `sent`: ["Sent", "Sent Items", "Sent Messages"]
    /// - `drafts`: ["Drafts", "Draft"]
    /// - `spam`: ["Spam", "Junk", "Bulk Mail"]
    /// - `trash`: ["Trash", "Deleted", "Deleted Messages"]
    /// - `archive`: ["Archive", "Archived"]
    pub fn default_english() -> Self {
        Self {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into(), "Sent Items".into(), "Sent Messages".into()],
            drafts: vec!["Drafts".into(), "Draft".into()],
            spam: vec!["Spam".into(), "Junk".into(), "Bulk Mail".into()],
            trash: vec!["Trash".into(), "Deleted".into(), "Deleted Messages".into()],
            archive: vec!["Archive".into(), "Archived".into()],
        }
    }

    /// 从 IMAP 文件夹名称查找对应的标准文件夹类型
    ///
    /// 使用两阶段匹配策略：
    /// 1. 精确匹配：文件夹名称完全匹配
    /// 2. 包含匹配：忽略大小写的子字符串匹配
    ///
    /// # 参数
    ///
    /// - `imap_name`: IMAP 服务器返回的文件夹名称
    ///
    /// # 返回值
    ///
    /// 返回标准文件夹类型字符串：
    /// - "inbox": 收件箱
    /// - "sent": 已发送
    /// - "drafts": 草稿箱
    /// - "spam": 垃圾邮件
    /// - "trash": 已删除
    /// - "archive": 归档
    /// - "other": 其他文件夹（不匹配任何标准类型）
    pub fn find_standard_type(&self, imap_name: &str) -> &str {
        let fields: &[(&str, &[String])] = &[
            ("inbox", &self.inbox),
            ("sent", &self.sent),
            ("drafts", &self.drafts),
            ("spam", &self.spam),
            ("trash", &self.trash),
            ("archive", &self.archive),
        ];

        // 精确匹配
        for (name, folders) in fields {
            if folders.iter().any(|n| n == imap_name) {
                return name;
            }
        }

        // 包含匹配（忽略大小写）
        let imap_lower = imap_name.to_lowercase();
        for (name, folders) in fields {
            if folders.iter().any(|n| {
                let n_lower = n.to_lowercase();
                imap_lower.contains(&n_lower) || n_lower.contains(&imap_lower)
            }) {
                return name;
            }
        }

        "other"
    }

    /// 获取所有标准文件夹名称列表
    ///
    /// 将所有标准文件夹的名称合并为一个列表，不包含重复项。
    ///
    /// # 返回值
    ///
    /// 包含所有标准文件夹名称的向量。
    pub fn list(&self) -> Vec<String> {
        self.inbox
            .iter()
            .chain(self.sent.iter())
            .chain(self.drafts.iter())
            .chain(self.spam.iter())
            .chain(self.trash.iter())
            .chain(self.archive.iter())
            .cloned()
            .collect()
    }
}

/// OAuth2 认证配置
///
/// 定义 OAuth2 授权流程所需的参数，用于支持 OAuth2 认证的邮件服务商。
///
/// # 字段说明
///
/// - `client_id`: OAuth2 客户端 ID（从服务商开发者控制台获取）
/// - `client_secret`: OAuth2 客户端密钥（可选，PKCE 流程不需要）
/// - `auth_url`: 授权端点 URL
/// - `token_url`: 令牌端点 URL
/// - `scopes`: OAuth2 权限范围列表
/// - `pkce_enabled`: 是否启用 PKCE（推荐启用，增强安全性）
/// - `tenant_id`: 租户 ID（企业版可能需要）
///
/// # 安全说明
///
/// - 推荐使用 PKCE 模式（`pkce_enabled = true`），不需要 `client_secret`
/// - `client_secret` 仅用于服务器端应用，桌面应用不推荐使用
/// - 所有令牌交换通过 HTTPS 进行
///
/// # 支持的服务商
///
/// - Google (Gmail): 支持 OAuth2 + PKCE
/// - Microsoft (Outlook): 支持 OAuth2
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    /// 是否启用 PKCE（Proof Key for Code Exchange）
    ///
    /// PKCE 增强了 OAuth2 的安全性，防止授权码截获攻击。
    pub pkce_enabled: bool,
    /// 租户 ID（仅企业版需要）
    pub tenant_id: Option<String>,
}

// ═══════════════════════════════════════════════════════════════
// 核心接口 (Core Trait)
// ═══════════════════════════════════════════════════════════════

/// 邮件服务提供商核心接口
///
/// 定义了所有邮件服务提供商必须实现的方法。
/// 不同服务商通过实现此 trait 来提供具体的配置和行为。
///
/// # 必须实现的方法
///
/// - `provider_info()`: 返回服务商基本信息
/// - `imap_config()`: 返回 IMAP 服务器配置
/// - `smtp_config()`: 返回 SMTP 服务器配置
/// - `supported_domains()`: 返回支持的邮箱域名列表
///
/// # 可选覆盖的方法
///
/// - `detect()`: 默认实现基于域名检测
/// - `oauth_config()`: 默认返回 None（不支持 OAuth2）
/// - `folder_mapping()`: 默认使用英文文件夹映射
/// - `generate_xoauth2()`: 默认返回空字符串（不支持 XOAUTH2）
/// - `generate_redirect_uri()`: 默认使用 localhost 回调
///
/// # 线程安全
///
/// 此 trait 要求实现 `Send + Sync`，确保可以安全地在多线程环境中使用。
///
/// # 实现示例
///
/// ```rust,ignore
/// pub struct GmailProvider {
///     info: ProviderInfo,
/// }
///
/// impl MailProvider for GmailProvider {
///     fn provider_info(&self) -> &ProviderInfo {
///         &self.info
///     }
///
///     fn imap_config(&self, _email: &str) -> ImapServerConfig {
///         ImapServerConfig {
///             host: "imap.gmail.com".to_string(),
///             port: 993,
///             ssl: SslMode::Implicit,
///         }
///     }
///
///     fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
///         SmtpServerConfig {
///             host: "smtp.gmail.com".to_string(),
///             port: 465,
///             ssl: SslMode::Implicit,
///         }
///     }
///
///     fn supported_domains(&self) -> Vec<&'static str> {
///         vec!["gmail.com", "googlemail.com"]
///     }
/// }
/// ```
pub trait MailProvider: Send + Sync {
    /// 获取服务商基本信息
    ///
    /// 返回服务商的描述信息，包括名称、图标、颜色等。
    fn provider_info(&self) -> &ProviderInfo;

    /// 获取 IMAP 服务器配置
    ///
    /// 根据邮箱地址返回对应的 IMAP 服务器配置。
    ///
    /// # 参数
    ///
    /// - `_email`: 用户邮箱地址（某些服务商可能需要根据地址生成配置）
    ///
    /// # 返回值
    ///
    /// IMAP 服务器连接配置
    fn imap_config(&self, _email: &str) -> ImapServerConfig;

    /// 获取 SMTP 服务器配置
    ///
    /// 根据邮箱地址返回对应的 SMTP 服务器配置。
    ///
    /// # 参数
    ///
    /// - `_email`: 用户邮箱地址（某些服务商可能需要根据地址生成配置）
    ///
    /// # 返回值
    ///
    /// SMTP 服务器连接配置
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig;

    /// 获取支持的邮箱域名列表
    ///
    /// 返回此服务商支持的所有邮箱域名后缀。
    ///
    /// # 返回值
    ///
    /// 域名字符串列表，如 ["gmail.com", "googlemail.com"]
    fn supported_domains(&self) -> Vec<&'static str>;

    /// 检测邮箱地址是否属于此服务商
    ///
    /// 通过比较邮箱域名与服务商支持的域名列表来判断。
    ///
    /// # 参数
    ///
    /// - `email`: 待检测的邮箱地址
    ///
    /// # 返回值
    ///
    /// - `true`: 邮箱属于此服务商
    /// - `false`: 邮箱不属于此服务商
    fn detect(&self, email: &str) -> bool {
        let email_domain = email.split('@').next_back().unwrap_or("").to_lowercase();
        self.supported_domains()
            .iter()
            .any(|d| d.to_lowercase() == email_domain)
    }

    /// 获取 OAuth2 配置（可选）
    ///
    /// 返回 OAuth2 认证所需的配置信息。
    /// 不支持 OAuth2 的服务商返回 None。
    ///
    /// # 返回值
    ///
    /// - `Some(OAuthConfig)`: 支持 OAuth2，返回配置
    /// - `None`: 不支持 OAuth2
    fn oauth_config(&self) -> Option<OAuthConfig> {
        None
    }

    /// 获取文件夹映射配置
    ///
    /// 返回服务商的 IMAP 文件夹到标准文件夹的映射。
    /// 默认使用英文文件夹映射。
    ///
    /// # 返回值
    ///
    /// 文件夹映射配置
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder::default_english()
    }

    /// 生成 XOAUTH2 认证字符串（可选）
    ///
    /// 用于 IMAP/SMTP 的 XOAUTH2 认证方式。
    /// 某些服务商（如 Gmail）支持此认证方式。
    ///
    /// # 参数
    ///
    /// - `_email`: 用户邮箱地址
    /// - `_access_token`: OAuth2 访问令牌
    ///
    /// # 返回值
    ///
    /// XOAUTH2 认证字符串，不支持时返回空字符串
    fn generate_xoauth2(&self, _email: &str, _access_token: &str) -> String {
        String::new()
    }

    /// 生成 OAuth2 回调 URL
    ///
    /// 生成 OAuth2 授权流程的本地回调 URL。
    /// 默认使用 localhost 和指定端口。
    ///
    /// # 参数
    ///
    /// - `port`: 本地 HTTP 服务器监听端口
    ///
    /// # 返回值
    ///
    /// 回调 URL，格式为 `http://127.0.0.1:{port}/oauth/callback`
    fn generate_redirect_uri(&self, port: u16) -> String {
        format!("http://127.0.0.1:{}/oauth/callback", port)
    }
}

#[cfg(test)]
mod standard_folder_tests {
    use super::StandardFolder;

    fn default_folders() -> StandardFolder {
        StandardFolder::default_english()
    }

    #[test]
    fn test_find_inbox() {
        let folders = default_folders();
        assert_eq!(folders.find_standard_type("INBOX"), "inbox");
    }

    #[test]
    fn test_find_sent() {
        let folders = default_folders();
        assert_eq!(folders.find_standard_type("Sent"), "sent");
        assert_eq!(folders.find_standard_type("Sent Items"), "sent");
    }

    #[test]
    fn test_find_spam() {
        let folders = default_folders();
        assert_eq!(folders.find_standard_type("Spam"), "spam");
        assert_eq!(folders.find_standard_type("Junk"), "spam");
    }

    #[test]
    fn test_find_unknown_folder() {
        let folders = default_folders();
        assert_eq!(folders.find_standard_type("MyCustomFolder"), "other");
    }

    #[test]
    fn test_list_all_folders() {
        let folders = default_folders();
        let all = folders.list();
        assert!(all.contains(&"INBOX".to_string()));
        assert!(all.contains(&"Sent".to_string()));
        assert!(all.contains(&"Drafts".to_string()));
    }

    #[test]
    fn test_case_insensitive_match() {
        let folders = default_folders();
        assert_eq!(folders.find_standard_type("trash"), "trash");
        assert_eq!(folders.find_standard_type("TRASH"), "trash");
    }
}
