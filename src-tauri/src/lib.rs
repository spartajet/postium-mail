use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("Greeting {name}");
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    // Backend logs
                    Target::new(TargetKind::LogDir {
                        file_name: Some("backend".to_string()),
                    })
                    .filter(|metadata| {
                        // Filter for backend logs (Rust logs)
                        metadata.target().starts_with("postium_mail")
                            || metadata.target().starts_with("tauri")
                            || !metadata.target().starts_with("[")
                    }),
                    // Frontend logs
                    Target::new(TargetKind::LogDir {
                        file_name: Some("frontend".to_string()),
                    })
                    .filter(|metadata| {
                        // Filter for frontend logs (JavaScript logs)
                        metadata.target().starts_with("[")
                    }),
                    // Also log to stdout in debug mode
                    #[cfg(debug_assertions)]
                    Target::new(TargetKind::Stdout),
                    // Also log to webview console
                    Target::new(TargetKind::Webview),
                ])
                .level(log::LevelFilter::Info)
                .rotation_strategy(RotationStrategy::KeepAll)
                .max_file_size(10_000_000) // 10MB per file
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            // Create a cleanup task for old logs (keep only last 30 days)
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                cleanup_old_logs(&app_handle);
            });

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            log::info!("Postium Mail application started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn cleanup_old_logs(app_handle: &tauri::AppHandle) {
    // use chrono::{Duration, Local};
    use std::fs;

    if let Ok(log_dir) = app_handle.path().app_log_dir() {
        log::info!("Starting log cleanup task for directory: {log_dir:?}");

        loop {
            // Sleep for 24 hours between cleanups
            std::thread::sleep(std::time::Duration::from_secs(86400));

            // let thirty_days_ago = Local::now() - Duration::days(30);

            if let Ok(entries) = fs::read_dir(&log_dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(modified_datetime) = modified.elapsed() {
                                if modified_datetime.as_secs() > 30 * 24 * 60 * 60 {
                                    // File is older than 30 days
                                    if let Some(file_name) = entry.file_name().to_str() {
                                        if file_name.ends_with(".log") {
                                            if let Err(e) = fs::remove_file(entry.path()) {
                                                log::error!("Failed to delete old log file: {:?}, error: {}", entry.path(), e);
                                            } else {
                                                log::info!(
                                                    "Deleted old log file: {:?}",
                                                    entry.path()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
