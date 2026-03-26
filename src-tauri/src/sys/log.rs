//! 日志系统初始化
//!
//! 使用 tracing 库提供结构化日志输出。
//!
//! # 日志配置
//!
//! 通过环境变量 `RUST_LOG` 控制日志级别：
//!
//! ```bash
//! # 默认级别（DEBUG）
//! RUST_LOG=debug
//!
//! # 只显示 INFO 及以上级别
//! RUST_LOG=info
//!
//! # 只显示本模块的 TRACE 日志
//! RUST_LOG=postium_mail=trace
//!
//! # 只显示某个子模块
//! RUST_LOG=postium_mail::sync=debug
//! ```
//!
//! # 过滤第三方日志
//!
//! 默认过滤以下 crate 的冗余日志：
//! - keyring / tauri_plugin_keyring / secret_service
//! - windows / windows_sys
//! - tokio_native_tls / native_tls
//! - async_imap

/// 初始化 tracing 日志系统
///
/// 使用环境变量 `RUST_LOG` 控制日志级别。
///
/// # 示例
///
/// ```rust,no_run
/// postium_mail::sys::log::init_tracing();
/// ```
pub fn init_tracing() {
    // 配置日志过滤器，屏蔽第三方 crate 的冗余日志
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("debug")
            // 过滤 keyring 相关 crate 的日志
            .add_directive("keyring=error".parse().unwrap())
            .add_directive("tauri_plugin_keyring=error".parse().unwrap())
            .add_directive("secret_service=error".parse().unwrap())
            // 过滤 Windows 相关日志
            .add_directive("windows=error".parse().unwrap())
            .add_directive("windows_sys=error".parse().unwrap())
            // 过滤 TLS 相关日志
            .add_directive("tokio_native_tls=warn".parse().unwrap())
            .add_directive("native_tls=warn".parse().unwrap())
            // 过滤 IMAP 相关日志
            .add_directive("async_imap=warn".parse().unwrap())
    });

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false) // 显示模块路径，便于调试
        .with_thread_ids(false) // 线程ID通常不需要
        .with_file(true) // 显示文件名
        .with_line_number(true) // 显示行号
        .compact() // 使用紧凑格式
        .init();
}
