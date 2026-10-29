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
            // - hyper_util：连接池后台拆除的 DEBUG 噪音（同步批取后服务器
            //   RST 空闲连接，10053 每条一刷）。target 是模块路径下划线
            //   （hyper_util::client::…），写成连字符 hyper-util 永不匹配
            //   （线上反馈③：同步日志刷屏的漏网真因）
            EnvFilter::new(
                "debug,gpui_pre=warn,gpui_pre_windows=warn,tao=warn,globset=warn,reqwest=warn,hyper_util=warn,html5ever=warn,markup5ever=warn,selectors=warn",
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    /// dev 默认过滤器对依赖 crate 噪音 target 的压制断言（端到端：真订阅者
    /// + 捕获写入器）。target 必须按模块路径（下划线）书写——连字符形式
    /// （hyper-util）永不匹配，线上反馈③的连接池拆除噪音正是这样漏网的。
    #[test]
    fn dev_filter_suppresses_dependency_noise_targets() {
        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Capture {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl tracing_subscriber::fmt::MakeWriter<'_> for Capture {
            type Writer = Self;
            fn make_writer(&self) -> Self::Writer {
                self.clone()
            }
        }

        let filter = tracing_subscriber::EnvFilter::new(
            "debug,gpui_pre=warn,gpui_pre_windows=warn,tao=warn,globset=warn,reqwest=warn,hyper_util=warn,html5ever=warn,markup5ever=warn,selectors=warn",
        );
        let captured = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_writer(Capture(Arc::clone(&captured)))
            .with_env_filter(filter)
            .finish();
        let dispatch = tracing::Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatch, || {
            // target 须为字面量（宏约束），逐条展开
            macro_rules! noise {
                ($t:literal) => {
                    tracing::debug!(target: $t, "noise-line")
                };
            }
            noise!("hyper_util::client::legacy::client");
            noise!("selectors::matching");
            noise!("html5ever::tree_builder");
            noise!("markup5ever::serialize");
            noise!("reqwest::connect::connector");
            noise!("tao::platform_impl::platform::event_loop");
            tracing::debug!(target: "postium_core::infrastructure::protocols::imap::fetch", "business-line");
        });

        let out = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
        assert!(!out.contains("noise-line"), "依赖噪音应全部被压制:\n{out}");
        assert!(out.contains("business-line"), "业务 DEBUG 不应被误压制:\n{out}");
    }
}
