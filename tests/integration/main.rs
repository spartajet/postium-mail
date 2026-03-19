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

mod integration;
mod helpers;

mod imap;
mod sync;
mod engine;
mod e2e;

use integration::GreenMailConfig;

fn main() {
    println!("Postium Mail 集成测试");
    println!();
    println!("GreenMail 配置:");
    let config = GreenMailConfig::default();
    println!("  主机: {}", config.host);
    println!("  IMAP 端口: {}", config.imap_port);
    println!("  SMTP 端口: {}", config.smtp_port);
    println!("  用户名: {}", config.username);
    println!();
    println!("环境变量:");
    println!("  GREENMAIL_HOST: 覆盖默认主机地址");
    println!("  GREENMAIL_PORT: 覆盖默认 IMAP 端口");
    println!();
    println!("运行特定测试:");
    println!("  cargo test --test integration test_greenmail_config");
    println!("  cargo test --test integration test_create_test_db");
}
