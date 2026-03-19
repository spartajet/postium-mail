//! 测试辅助宏
//!
//! 提供测试中常用的辅助宏和工具

/// 测试专用的信息输出宏
///
/// 与 println! 相比，使用 tracing::info! 可以：
/// - 统一日志格式
/// - 支持日志级别过滤
/// - 更好的结构化输出
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_info!("处理 {} 封邮件", 100);
/// ```
#[macro_export]
macro_rules! test_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*)
    };
}

/// 测试专用的成功输出
///
/// 自动添加 ✅ 表情符号
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_success!("数据库初始化成功");
/// ```
#[macro_export]
macro_rules! test_success {
    ($($arg:tt)*) => {
        tracing::info!("✅ {}", format!($($arg)*))
    };
}

/// 测试专用的警告输出
///
/// 自动添加 ⚠️ 表情符号
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_warn!("GreenMail 未运行，跳过测试");
/// ```
#[macro_export]
macro_rules! test_warn {
    ($($arg:tt)*) => {
        tracing::warn!("⚠️  {}", format!($($arg)*))
    };
}

/// 测试专用的错误输出
///
/// 自动添加 ❌ 表情符号
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_error!("连接失败: {}", error);
/// ```
#[macro_export]
macro_rules! test_error {
    ($($arg:tt)*) => {
        tracing::error!("❌ {}", format!($($arg)*))
    };
}

/// 测试进度输出
///
/// 自动添加 🔍 表情符号，用于标记测试步骤
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_progress!("测试 IMAP 连接");
/// ```
#[macro_export]
macro_rules! test_progress {
    ($($arg:tt)*) => {
        tracing::info!("🔍 {}", format!($($arg)*))
    };
}

/// 测试开始标记
///
/// 自动添加 📋 表情符号
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_section!("同步流程测试");
/// ```
#[macro_export]
macro_rules! test_section {
    ($($arg:tt)*) => {
        tracing::info!("📋 {}", format!($($arg)*))
    };
}

/// 测试数据标记
///
/// 自动添加 📊 表情符号，用于输出数据统计
///
/// # 示例
///
/// ```rust,no_run
/// test_macros::test_stats!("邮件数", 100);
/// ```
#[macro_export]
macro_rules! test_stats {
    ($label:expr, $value:expr) => {
        tracing::info!("📊 {}: {}", $label, $value)
    };
}
