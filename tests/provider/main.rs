//! 服务商测试入口
//!
//! 运行方式：
//! ```bash
//! # 运行所有服务商测试
//! cargo test --test provider
//!
//! # 运行特定服务商测试（需要配置环境变量）
//! export GMAIL_EMAIL="test@gmail.com"
//! export GMAIL_APP_PASSWORD="app_password"
//! cargo test --test provider test_gmail --
//! ```

mod config;
mod helpers;

mod personal;
mod enterprise;

use std::env;

fn main() {
    println!("Postium Mail 服务商测试");
    println!();
    println!("需要配置环境变量:");
    println!();
    println!("个人邮箱:");
    println!("  GMAIL_EMAIL / GMAIL_APP_PASSWORD");
    println!("  OUTLOOK_EMAIL / OUTLOOK_APP_PASSWORD");
    println!("  EMAIL_163_ADDR / EMAIL_163_PASS");
    println!("  EMAIL_QQ_ADDR / EMAIL_QQ_PASS");
    println!();
    println!("企业邮箱:");
    println!("  MICROSOFT_365_EMAIL / MICROSOFT_365_PASSWORD");
    println!("  GOOGLE_WORKSPACE_EMAIL / GOOGLE_WORKSPACE_PASSWORD");
    println!();

    // 检查是否配置了环境变量
    let has_config = env::vars().any(|(k, _)| {
        k.contains("EMAIL") ||
        k.contains("GMAIL") ||
        k.contains("OUTLOOK") ||
        k.contains("PASSWORD") ||
        k.contains("PASS")
    });

    if has_config {
        println!("✓ 检测到环境变量配置");
    } else {
        println!("⚠ 未检测到环境变量配置");
        println!("  部分测试将被跳过");
    }
}
