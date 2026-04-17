// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

///
/// Postium Mail 主程序入口文件
///
/// 本文件是整个 Tauri 应用程序的入口点，负责启动应用程序的主循环。
/// 实际的业务逻辑和初始化工作都在 lib.rs 的 run() 函数中完成。
///
/// 文件特性说明：
/// - 使用 windows_subsystem 属性配置，在 Release 模式下不显示控制台窗口
/// - 在 Debug 模式下保留控制台窗口，方便开发调试
///
/// 模块依赖：
/// - postium_mail_lib: 应用程序的核心库，包含所有业务逻辑
///

///
/// 应用程序主函数
///
/// 这是 Tauri 应用程序的入口点，也是程序的第一个执行点。
///
/// 功能：
/// 1. 调用核心库的 run() 函数启动应用程序
/// 2. run() 函数负责所有的初始化工作，包括：
///    - 日志系统初始化
///    - 数据库初始化
///    - 服务层构建
///    - Tauri 应用启动
///    - 后台任务调度
///
/// 注意事项：
/// - 本函数不包含实际业务逻辑，所有逻辑都在 lib.rs 中
/// - 不要删除顶部的 windows_subsystem 属性，否则 Release 模式下会弹出额外的控制台窗口
///
/// 调试说明：
/// - Debug 模式：会显示控制台窗口，可以看到日志输出
/// - Release 模式：不显示控制台窗口，用户界面更整洁
///
fn main() {
    // 调用核心库的 run 函数启动应用
    // 这里会阻塞执行，直到应用程序关闭
    postium_mail_lib::run()
}
