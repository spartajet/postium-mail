// 测试日志初始化模块
// 通过 path 属性重用父目录中的实现

#[path = "../test_tracing.rs"]
pub mod test_tracing_impl;

// 重新导出以便简化访问
pub use test_tracing_impl::init_test_tracing;
