//! 集成测试入口
//!
//! 运行方式：
//! ```bash
//! # 使用公网 GreenMail
//! cargo test --test integration
//!
//! # 使用本地 GreenMail
//! docker-compose -f docker-compose.test.yml up -d
//! cargo test --test integration -- --ignored
//! ```

use std::env;

// 引入测试辅助模块
mod test_tracing;
mod test_macros;

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
        let host = env::var("GREENMAIL_HOST")
            .unwrap_or_else(|_| "139.59.228.56".to_string());

        let imap_port = env::var("GREENMAIL_PORT")
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

    /// 获取 IMAP 服务器地址
    pub fn imap_addr(&self) -> String {
        format!("{}:{}", self.host, self.imap_port)
    }

    /// 获取 SMTP 服务器地址
    pub fn smtp_addr(&self) -> String {
        format!("{}:{}", self.host, self.smtp_port)
    }
}

impl Default for GreenMailConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn main() {
    // 初始化测试环境 tracing
    test_tracing::init_test_tracing();

    test_section!("Postium Mail 集成测试");
    tracing::info!("");
    test_section!("GreenMail 配置");
    let config = GreenMailConfig::default();
    tracing::info!("  主机: {}", config.host);
    tracing::info!("  IMAP 端口: {}", config.imap_port);
    tracing::info!("  SMTP 端口: {}", config.smtp_port);
    tracing::info!("  用户名: {}", config.username);
    tracing::info!("");
    test_section!("环境变量");
    tracing::info!("  GREENMAIL_HOST: 覆盖默认主机地址");
    tracing::info!("  GREENMAIL_PORT: 覆盖默认 IMAP 端口");
    tracing::info!("");
    test_section!("运行特定测试");
    tracing::info!("  cargo test --test integration test_greenmail_config");
    tracing::info!("  cargo test --test integration test_create_test_db");
}

// ========== 测试模块 ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_greenmail_config() {
        let config = GreenMailConfig::default();
        assert_eq!(config.username, "testuser");
        assert_eq!(config.password, "testpass");
        assert_eq!(config.imap_port, 3143);
    }

    #[tokio::test]
    async fn test_greenmail_config_from_env() {
        let config = GreenMailConfig::from_env();
        assert!(!config.host.is_empty());
        assert!(config.imap_port > 0);
    }

    #[tokio::test]
    async fn test_greenmail_imap_addr() {
        let config = GreenMailConfig {
            host: "localhost".to_string(),
            imap_port: 3143,
            smtp_port: 3025,
            username: "testuser",
            password: "testpass",
        };
        assert_eq!(config.imap_addr(), "localhost:3143");
        assert_eq!(config.smtp_addr(), "localhost:3025");
    }
}
