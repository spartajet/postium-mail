//! 集成测试模块
//!
//! 使用 GreenMail 测试服务器进行协议和逻辑集成测试
//!
//! ## GreenMail 配置
//!
//! - **公网服务器**: 139.59.228.56:3143 (默认)
//! - **本地 Docker**: localhost:3143 (需要 docker-compose.test.yml)
//! - **测试账号**: testuser / testpass
//!
//! ## 环境变量
//!
//! - `GREENMAIL_HOST`: 覆盖默认主机地址
//! - `GREENMAIL_PORT`: 覆盖默认 IMAP 端口
//!
//! ## 运行方式
//!
//! ```bash
//! # 使用公网 GreenMail
//! cargo test --test integration
//!
//! # 启动本地 GreenMail
//! docker-compose -f docker-compose.test.yml up -d
//!
//! # 运行测试
//! cargo test --test integration -- --ignored
//! ```

pub mod helpers;

pub mod imap;
pub mod sync;
pub mod engine;
pub mod e2e;

/// 全局 GreenMail 配置
#[derive(Debug, Clone)]
pub struct GreenMailConfig {
    pub host: String,
    pub imap_port: u16,
    pub smtp_port: u16,
    pub username: &'static str,
    pub password: &'static str,
}

impl GreenMailConfig {
    /// 从环境变量或默认值创建配置
    pub fn from_env() -> Self {
        let host = std::env::var("GREENMAIL_HOST")
            .unwrap_or_else(|_| "139.59.228.56".to_string());

        let imap_port = std::env::var("GREENMAIL_PORT")
            .unwrap_or_else(|_| "3143".to_string())
            .parse()
            .unwrap_or(3143);

        GreenMailConfig {
            host,
            imap_port,
            smtp_port: 3025,
            username: "testuser",
            password: "testpass",
        }
    }

    pub fn default() -> Self {
        Self::from_env()
    }

    /// 获取 IMAP 服务器地址
    pub fn imap_addr(&self) -> String {
        format!("{}:{}", self.host, self.imap_port)
    }

    /// 获取 SMTP 服务器地址
    pub fn smtp_addr(&self) -> String {
        format!("{}:{}", self.host, self.smtp_port)
    }
}
