//! 测试专用的 tracing 初始化
//!
//! 提供测试环境的日志配置，与生产环境分离

use tracing_subscriber::{fmt, EnvFilter};

/// 初始化测试环境的 tracing
///
/// 与生产环境相比，测试环境：
/// - 默认日志级别为 DEBUG
/// - 显示更详细的信息用于调试
/// - 支持通过 RUST_LOG 环境变量覆盖
///
/// # 示例
///
/// ```rust,no_run
/// use test_tracing::init_test_tracing;
///
/// // 在测试开始前调用
/// init_test_tracing();
/// ```
pub fn init_test_tracing() {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug"))
        )
        .with_target(true)  // 测试时显示模块路径
        .with_thread_ids(false)
        .with_file(true)  // 测试时显示文件名便于定位
        .with_line_number(true)
        .pretty()  // 测试时使用更易读的格式
        .init();
}
