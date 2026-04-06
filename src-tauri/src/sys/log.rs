/// 初始化 tracing 日志系统
///
/// Dev 模式默认 DEBUG 级别，Release 默认 WARN 级别。
/// 可通过 `RUST_LOG` 环境变量覆盖，例如 `RUST_LOG=debug`。
pub fn setup_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        #[cfg(debug_assertions)]
        {
            EnvFilter::new("debug,tao=warn,reqwest=warn,hyper-util=warn")
        }
        #[cfg(not(debug_assertions))]
        {
            EnvFilter::new("warn")
        }
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .init();
}
