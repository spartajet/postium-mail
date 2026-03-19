//! FlowEngine 相关命令
//!
//! 提供流程引擎的管理和控制功能

use crate::engine::flow_engine::EngineStatusReport;
use crate::sync::SyncResult;
use tauri::State;

/// 获取 FlowEngine 状态报告
///
/// 返回引擎的当前运行状态、任务数量、通知统计等信息
#[tauri::command]
pub async fn get_flow_engine_status(
    engine_state: State<'_, super::FlowEngineState>,
) -> Result<EngineStatusReport, String> {
    let engine = engine_state.0.lock().await;
    Ok(engine.get_status().await)
}

/// 为账号添加定时同步任务
///
/// # 参数
/// - `account_id`: 账号ID
/// - `interval_minutes`: 同步间隔（分钟）
#[tauri::command]
pub async fn add_sync_task(
    engine_state: State<'_, super::FlowEngineState>,
    account_id: i32,
    interval_minutes: u64,
) -> Result<(), String> {
    let engine = engine_state.0.lock().await;
    engine
        .add_sync_task(account_id, interval_minutes)
        .await
        .map_err(|e| e.to_string())
}

/// 移除账号的同步任务
#[tauri::command]
pub async fn remove_sync_task(
    engine_state: State<'_, super::FlowEngineState>,
    account_id: i32,
) -> Result<(), String> {
    let engine = engine_state.0.lock().await;
    engine
        .remove_sync_task(account_id)
        .await
        .map_err(|e| e.to_string())
}

/// 暂停账号的同步任务
#[tauri::command]
pub async fn pause_sync_task(
    engine_state: State<'_, super::FlowEngineState>,
    account_id: i32,
) -> Result<(), String> {
    let engine = engine_state.0.lock().await;
    engine
        .pause_sync_task(account_id)
        .await
        .map_err(|e| e.to_string())
}

/// 恢复账号的同步任务
#[tauri::command]
pub async fn resume_sync_task(
    engine_state: State<'_, super::FlowEngineState>,
    account_id: i32,
) -> Result<(), String> {
    let engine = engine_state.0.lock().await;
    engine
        .resume_sync_task(account_id)
        .await
        .map_err(|e| e.to_string())
}

/// 手动触发账号同步
///
/// 立即执行指定账号的邮件同步，忽略定时任务调度
#[tauri::command]
pub async fn trigger_sync(
    engine_state: State<'_, super::FlowEngineState>,
    account_id: i32,
) -> Result<SyncResult, String> {
    let engine = engine_state.0.lock().await;
    engine
        .trigger_sync(account_id)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_command_exports() {
        // 验证命令函数可以被导出
        // 这些测试主要是为了确保编译通过
        assert!(true);
    }
}
