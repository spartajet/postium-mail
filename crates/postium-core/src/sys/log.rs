//! 初始化 tracing 日志系统
//!
//! Dev 模式默认 DEBUG 级别，Release 默认 WARN 级别。
//! 可通过 `RUST_LOG` 环境变量覆盖，例如 `RUST_LOG=debug`。

/// 已知无害噪音（按源文件路径压制）：
/// - `directx_devices.rs`：无 D3D 调试层机器上的 DXGI debug 探测失败
///   （log 宏桥接记录，EnvFilter 的 target 过滤对其不生效，故按 file 判定）
fn is_known_noise(file: Option<&str>) -> bool {
    match file {
        Some(f) => f.contains("directx_devices.rs"),
        None => false,
    }
}

pub fn setup_logging() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

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

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
            !is_known_noise(meta.file())
        }));

    let layered = fmt_layer.with_filter(filter);
    tracing_subscriber::registry().with(layered).init();
}
