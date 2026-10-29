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
            // dev 默认 DEBUG，但压制框架/依赖内部的高频噪音：
            // - gpui_pre(_windows)：每帧的无障碍树更新、渲染器/剪贴板内部调试
            // - tao：窗口系统事件；globset：构建 glob 集合
            // - html5ever/markup5ever：HTML 解析器逐 tag 的 tree_builder 调试
            //   （邮件正文解析时每封上千行）
            // - selectors：css-inline 类名匹配的 DEBUG 会把整个文档 DOM
            //   Debug 转储进单条日志（MB 级/条 × 每元素每选择器）——渲染线程
            //   同步写终端直接卡死应用（日志目标文件 selectors/matching.rs）
            EnvFilter::new(
                "debug,gpui_pre=warn,gpui_pre_windows=warn,tao=warn,globset=warn,reqwest=warn,hyper-util=warn,html5ever=warn,markup5ever=warn,selectors=warn",
            )
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
