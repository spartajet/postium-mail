pub mod command;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;
pub mod sys;

use std::sync::Arc;

use tauri::Manager;

use crate::domain::providers::pool::init_provider_pool;

#[cfg(debug_assertions)]
const EXPORT_DIR: &str = "../src/lib/bindings.ts";

/// 创建 tauri-specta Builder（集中管理 commands 和 events 注册）
fn create_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            command::account::list_accounts,
            command::account::get_account,
            command::account::create_account,
            command::account::update_account,
            command::account::delete_account,
            command::email::list_emails,
            command::email::list_emails_by_category,
            command::email::get_email,
            command::email::search_emails,
            command::email::mark_as_read,
            command::email::toggle_star,
            command::email::delete_emails,
            command::email::move_email_to_folder,
            command::email::send_email,
            command::sync::sync_account,
            command::sync::get_folder_stats,
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
        ])
        .events(tauri_specta::collect_events![
            domain::sync::SyncProgressEvent,
        ])
}

pub fn run() {
    // 0. 初始化日志
    sys::log::setup_logging();
    tracing::info!("Postium Mail 启动中...");

    // 1. 导出 TypeScript 绑定
    let builder = create_specta_builder();
    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), EXPORT_DIR)
        .expect("导出 TypeScript 绑定失败");
    // 2. 初始化数据库
    let db = tauri::async_runtime::block_on(async {
        let data_dir = dirs::home_dir().expect("无法获取数据目录").join(".postium");
        std::fs::create_dir_all(&data_dir).expect("无法创建数据目录");
        tracing::info!("数据目录: {}", data_dir.display());
        infrastructure::storage::database::init_database(&data_dir)
            .await
            .expect("数据库初始化失败")
    });
    tracing::info!("数据库初始化完成");

    // 2.5 构建 ProviderPool（唯一来源）
    init_provider_pool();
    // let provider_pool = Arc::new(domain::providers::ProviderPool::default());

    let auth = Arc::new(domain::auth::AuthManager::default());

    // 3. 构建 Service 层
    let account_service =
        service::account_service::AccountService::new(db.clone(), auth.clone());

    let email_service = service::email_service::EmailService::new(
        auth.clone(),
        db.clone(),
    );

    let sync_service = service::SyncService::new(db.clone(), auth.clone());

    let label_service = service::LabelService::new(db.clone());

    // 4. OAuth2 Manager — 从 pool 中提取支持 OAuth2 的服务商
    let oauth2_manager = Arc::new(
        infrastructure::auth::oauth2::OAuth2Manager::new(auth.clone(), db.clone())
            .expect("初始化OAuth2 Manager失败"),
    );
    auth.set_oauth2_manager(oauth2_manager.clone());

    // 5. 后台同步调度器
    let scheduler_db = db.clone();
    let scheduler_auth = auth.clone();

    // 6. 构建 Tauri 应用
    let mut app_builder = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_opener::init())
        .manage(account_service)
        .manage(email_service)
        .manage(sync_service)
        .manage(label_service)
        .manage(oauth2_manager);

    // Dev 模式下启用 MCP Bridge 插件
    #[cfg(debug_assertions)]
    {
        app_builder = app_builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    app_builder
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // 启动后台同步调度器（每5分钟同步一次）
            let mut scheduler = domain::sync::SyncScheduler::new(
                scheduler_db,
                scheduler_auth,
                Some(app.handle().clone()),
            );
            scheduler.start();
            app.manage(scheduler);

            // 系统托盘
            sys::tray::setup_tray(app)?;

            tracing::info!("Postium Mail 启动完成");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
