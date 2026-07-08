///
/// Postium Mail 核心库文件
///
/// 本文件是应用程序的核心库入口，负责所有的初始化工作和服务管理。
/// 它协调各个子系统（数据库、认证、邮件同步、OAuth2等）的启动和运行。
///
/// 主要功能：
/// 1. 日志系统初始化
/// 2. 数据库连接和初始化
/// 3. 服务层组件的构建和依赖注入
/// 4. Tauri 应用框架的配置和启动
/// 5. 后台任务调度（邮件同步）
/// 6. TypeScript 类型绑定导出（用于前端开发）
///
/// 模块组织：
/// - command: Tauri 命令模块，定义前端可调用的 API
/// - domain: 领域层，包含业务逻辑和领域模型
/// - error: 错误类型定义
/// - infrastructure: 基础设施层，数据库、认证等
/// - service: 服务层，协调领域对象和基础设施
/// - sys: 系统相关功能（日志、托盘等）
///
/// 依赖管理：
/// - 使用 Tauri 的 manage 方法将服务实例注入到应用状态中
/// - 所有服务都使用 Arc 包装以支持跨线程共享
///
// 导入子模块
pub mod command;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;
pub mod sys;

use std::{path::PathBuf, sync::Arc};

use tauri::Manager;

use crate::domain::providers::pool::init_provider_pool;

// 在 Debug 模式下，TypeScript 绑定文件导出到前端项目的 bindings.ts
#[cfg(debug_assertions)]
const EXPORT_DIR: &str = "../src/lib/bindings.ts";

fn resolve_data_dir() -> PathBuf {
    let e2e_enabled = std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1");

    if e2e_enabled {
        let data_dir =
            std::env::var("POSTIUM_DATA_DIR").expect("POSTIUM_E2E=1 时必须设置 POSTIUM_DATA_DIR");
        return PathBuf::from(data_dir);
    }

    dirs::home_dir().expect("无法获取数据目录").join(".postium")
}

///
/// 创建 tauri-specta 构建器
///
/// 此函数创建一个 specta 构建器，用于集中管理所有 Tauri 命令和事件的类型定义。
/// specta 会自动生成 TypeScript 类型绑定，使前端能够获得完整的类型提示。
///
/// 工作原理：
/// 1. collect_commands! 宏收集所有使用 #[tauri::command] 标记的函数
/// 2. collect_events! 宏收集所有定义的事件类型
/// 3. 生成的类型绑定导出到 TypeScript 文件，供前端使用
///
/// 注册的命令分类：
/// - 账号管理: list_accounts, get_account, create_account, update_account, delete_account
/// - 邮件操作: list_emails, get_email, search_emails, mark_as_read, send_email 等
/// - 同步功能: sync_account, get_folder_stats
/// - 认证授权: detect_provider, list_providers, start_oauth2, poll_oauth2
/// - 标签管理: list_labels, create_label, update_label 等
///
/// 返回值：
/// - 配置好的 tauri_specta::Builder 实例
///
fn create_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        // 注册所有命令函数，这些函数可以通过前端 invoke 调用
        .commands(tauri_specta::collect_commands![
            command::account::list_accounts,
            command::account::get_account,
            command::account::create_account,
            command::account::update_account,
            command::account::delete_account,
            command::email::list_emails,
            command::email::list_emails_by_category,
            command::email::list_emails_by_category_for_all_accounts,
            command::email::get_email,
            command::email::reload_email,
            command::email::ensure_attachment_cached,
            command::email::save_attachment_as,
            command::email::open_attachment,
            command::email::resolve_inline_attachments,
            command::email::search_emails,
            command::email::mark_as_read,
            command::email::toggle_star,
            command::email::delete_emails,
            command::email::move_email_to_folder,
            command::email::archive_email,
            command::email::describe_local_attachments,
            command::email::save_draft,
            command::email::delete_draft,
            command::email::parse_email_addresses,
            command::email::send_email,
            command::sync::sync_account,
            command::sync::sync_account_with_range,
            command::sync::get_sync_history_state,
            command::sync::sync_older_emails,
            command::sync::get_folder_stats,
            command::sync::get_folder_stats_for_all_accounts,
            command::sync::sync_all_accounts,
            command::auth::detect_provider,
            command::auth::list_providers,
            command::auth::start_oauth2,
            command::auth::poll_oauth2,
            command::account::update_account_password,
            command::label::list_labels,
            command::label::create_label,
            command::label::update_label,
            command::label::delete_label,
            command::label::add_label_to_email,
            command::label::remove_label_from_email,
            command::label::get_labels_for_email,
            command::label::list_emails_by_label,
            command::window::open_settings_window,
        ])
        // 注册所有事件类型，用于从后端向前端发送事件
        .events(tauri_specta::collect_events![
            domain::sync::SyncProgressEvent,
        ])
}

///
/// 应用程序主运行函数
///
/// 这是整个应用程序的启动函数，负责初始化所有必要的组件并启动 Tauri 应用。
/// 函数会阻塞执行，直到应用程序关闭。
///
/// 执行流程：
/// 1. 初始化日志系统
/// 2. 导出 TypeScript 类型绑定（仅 Debug 模式）
/// 3. 初始化数据库连接
/// 4. 初始化邮箱服务商池
/// 5. 构建服务层组件（账号、邮件、同步、标签）
/// 6. 初始化 OAuth2 管理器
/// 7. 配置并启动 Tauri 应用
/// 8. 启动后台同步调度器
/// 9. 设置系统托盘
///
/// 错误处理：
/// - 初始化失败会 panic 并终止应用
/// - 使用 expect() 确保关键步骤必须成功
///
pub fn run() {
    // ========== 步骤 1: 初始化日志系统 ==========
    // 设置日志级别和输出格式
    sys::log::setup_logging();
    tracing::info!("Postium Mail 启动中...");

    // ========== 步骤 2: 导出 TypeScript 类型绑定 ==========
    // 创建 specta 构建器，用于管理所有命令和事件的类型定义
    let builder = create_specta_builder();

    // 仅在 Debug 模式下导出 TypeScript 绑定文件
    // 这样前端可以获得完整的类型提示，提高开发效率
    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), EXPORT_DIR)
        .expect("导出 TypeScript 绑定失败");

    // ========== 步骤 3: 初始化数据库 ==========
    // 使用 block_on 在同步上下文中执行异步初始化
    let data_dir = resolve_data_dir();
    let db = tauri::async_runtime::block_on(async {
        // 获取用户主目录，创建应用数据目录
        // 确保数据目录存在，如果不存在则创建
        std::fs::create_dir_all(&data_dir).expect("无法创建数据目录");

        tracing::info!("数据目录: {}", data_dir.display());

        // 初始化数据库连接，创建必要的表结构
        let db = infrastructure::storage::database::init_database(&data_dir)
            .await
            .expect("数据库初始化失败");

        let e2e_enabled = std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1");
        let e2e_truth_enabled = std::env::var("POSTIUM_E2E_TRUTH").ok().as_deref() == Some("1");

        if e2e_enabled && !e2e_truth_enabled {
            infrastructure::testing::e2e_seed::seed_e2e_data(&db)
                .await
                .expect("E2E seed 数据初始化失败");
        }

        db
    });
    tracing::info!("数据库初始化完成");

    // ========== 步骤 3.5: 初始化邮箱服务商池 ==========
    // 构建 ProviderPool，这是所有邮箱服务商信息的唯一来源
    // 支持的服务商包括 Gmail、Outlook、QQ邮箱、163邮箱等
    init_provider_pool();

    // ========== 步骤 4: 初始化认证管理器 ==========
    // AuthManager 负责管理所有账号的认证信息
    let auth = Arc::new(domain::auth::AuthManager::default());

    // ========== 步骤 5: 构建服务层组件 ==========
    // 服务层是业务逻辑的核心，协调领域对象和基础设施

    // 账号服务：负责账号的 CRUD 操作
    let account_service = service::account_service::AccountService::new(db.clone(), auth.clone());

    // 邮件服务：负责邮件的收发、搜索等操作
    let email_service = service::email_service::EmailService::new(auth.clone(), db.clone());

    // 同步服务：负责与邮件服务器同步数据
    let sync_service = service::SyncService::new(db.clone(), auth.clone());

    // 标签服务：负责邮件标签的管理
    let label_service = service::LabelService::new(db.clone());

    // 附件服务：负责附件缓存、保存和系统打开
    let attachment_service =
        service::AttachmentService::new(db.clone(), auth.clone(), data_dir.clone());

    // ========== 步骤 6: 初始化 OAuth2 管理器 ==========
    // OAuth2Manager 处理 OAuth2 授权流程
    // 支持通过浏览器进行 OAuth2 授权，获取访问令牌
    let oauth2_manager = Arc::new(
        infrastructure::auth::oauth2::OAuth2Manager::new(auth.clone(), db.clone())
            .expect("初始化OAuth2 Manager失败"),
    );
    // 将 OAuth2 管理器注册到认证管理器中
    auth.set_oauth2_manager(oauth2_manager.clone());

    // ========== 步骤 7: 准备后台同步调度器的依赖 ==========
    // 克隆必要的引用，供后台任务使用
    let scheduler_db = db.clone();
    let scheduler_auth = auth.clone();

    // ========== 步骤 8: 构建 Tauri 应用 ==========
    let mut app_builder = tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::new().build())
        // 窗口定位器插件：支持窗口位置管理
        .plugin(tauri_plugin_positioner::init())
        // 对话框插件：支持文件选择和保存路径选择
        .plugin(tauri_plugin_dialog::init())
        // URL 打开器插件：支持在浏览器中打开链接
        .plugin(tauri_plugin_opener::init())
        // 使用 manage 方法将服务实例注入到应用状态中
        // 这些服务可以在命令函数中通过 State 参数访问
        .manage(account_service)
        .manage(email_service)
        .manage(sync_service)
        .manage(label_service)
        .manage(attachment_service)
        .manage(oauth2_manager);

    // 在 Debug 模式下启用 MCP Bridge 插件
    // MCP (Model Context Protocol) Bridge 用于与 AI 模型集成
    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    // 启动 Tauri 应用
    app_builder
        // 注册命令处理器，将前端调用的命令路由到对应的函数
        .invoke_handler(builder.invoke_handler())
        // 应用启动时的设置
        .setup(move |app| {
            // 挂载事件处理器，使后端能够向前端发送事件
            builder.mount_events(app);

            // ========== 步骤 8.1: 启动后台同步调度器 ==========
            // 创建同步调度器，定期同步邮件
            let mut scheduler = domain::sync::SyncScheduler::new(
                scheduler_db,
                scheduler_auth,
                // 传入应用句柄，用于发送同步进度事件
                Some(app.handle().clone()),
            );
            // 启动调度器，默认每5分钟同步一次所有账号
            scheduler.start();
            // 将调度器实例管理起来
            app.manage(scheduler);

            // ========== 步骤 8.2: 设置系统托盘 ==========
            // 在系统托盘中显示应用图标，支持最小化和退出功能
            sys::tray::setup_tray(app)?;
            if let Some(main_window) = app.get_webview_window("main")
                && let Ok(()) = main_window.show()
            {
                tracing::info!("窗体加载成功");
            }

            tracing::info!("Postium Mail 启动完成");
            Ok(())
        })
        // 运行应用，传入 tauri.conf.toml 的配置
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
