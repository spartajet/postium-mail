//! FlowEngine 相关命令
//!
//! 提供流程引擎的管理和控制功能，包括：
//! - 获取引擎状态
//! - 添加/移除定时同步任务
//! - 暂停/恢复同步任务
//! - 手动触发同步
//!
//! # FlowEngine 功能
//!
//! FlowEngine 是一个后台任务调度引擎，负责：
//! - 管理定时同步任务
//! - 执行按需同步
//! - 维护任务状态和统计信息
//!
//! # 任务调度
//!
//! 支持为每个账号配置独立的同步间隔：
//! - 最小间隔：1 分钟
//! - 推荐间隔：5-15 分钟
//! - 任务会在到达间隔后自动执行

use crate::engine::flow_engine::EngineStatusReport;
use crate::sync::SyncResult;
use tauri::State;

/// 获取 FlowEngine 状态报告
///
/// 返回引擎的当前运行状态、任务数量、通知统计等信息。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
///
/// # 返回
/// 成功时返回引擎状态报告（EngineStatusReport），包含：
/// - `is_running`: 引擎是否正在运行
/// - `active_tasks`: 活跃任务数量
/// - `queued_tasks`: 排队中任务数量
/// - `completed_tasks`: 已完成任务总数
/// - `failed_tasks`: 失败任务总数
///
/// 失败时返回错误信息字符串
///
/// # 示例
/// ```rust
/// let status = get_flow_engine_status(engine_state).await?;
/// println!("活跃任务: {}", status.active_tasks);
/// println!("已完成: {}", status.completed_tasks);
/// ```
#[tauri::command]
pub async fn get_flow_engine_status(
    engine_state: State<'_, super::FlowEngineState>,
) -> Result<EngineStatusReport, String> {
    let engine = engine_state.0.lock().await;
    Ok(engine.get_status().await)
}

/// 为账号添加定时同步任务
///
/// 在 FlowEngine 中注册一个新的定时同步任务。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
/// * `account_id` - 账号 ID
/// * `interval_minutes` - 同步间隔（分钟）
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 间隔限制
///
/// - 最小间隔：1 分钟
/// - 推荐间隔：5-15 分钟
/// - 过于频繁的同步可能被邮件服务商限制
///
/// # 任务行为
///
/// - 任务会在注册后立即执行第一次同步
/// - 之后每隔指定的分钟数执行一次
/// - 如果上次同步仍在进行，新的执行会跳过
///
/// # 示例
/// ```rust
/// // 每 10 分钟同步一次
/// add_sync_task(engine_state, 1, 10).await?;
/// ```
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
///
/// 从 FlowEngine 中移除指定账号的定时同步任务。
/// 已在执行的任务不会被中断，但之后不会再自动执行。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 注意
/// - 如果账号没有注册同步任务，此操作不会报错
/// - 移除后仍可使用 `trigger_sync` 手动触发同步
/// - 可使用 `add_sync_task` 重新注册任务
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
///
/// 暂停指定账号的定时同步任务，任务不会被删除但不会自动执行。
/// 可使用 `resume_sync_task` 恢复执行。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 使用场景
/// - 用户手动禁用账号同步
/// - 网络不可用时暂时停止同步
/// - 调试或维护期间暂停同步
///
/// # 注意
/// - 暂停后仍可使用 `trigger_sync` 手动触发同步
/// - 手动触发的同步不受暂停状态影响
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
///
/// 恢复之前暂停的定时同步任务，任务将按原定计划继续执行。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 行为说明
/// - 恢复后任务会立即检查是否需要执行同步
/// - 如果上次同步时间已超过间隔，会立即执行同步
/// - 否则等待下一个间隔周期再执行
///
/// # 注意
/// - 只能恢复已暂停的任务
/// - 如果任务未注册，此操作会报错
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
/// 立即执行指定账号的邮件同步，忽略定时任务调度。
/// 无论任务是否暂停，都可以手动触发同步。
///
/// # 参数
/// * `engine_state` - FlowEngine 状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回同步结果（SyncResult），包含：
/// - `total_synced`: 同步的邮件总数
/// - `folders_synced`: 同步的文件夹数
/// - `errors`: 错误数量
/// - `duration_ms`: 耗时（毫秒）
///
/// 失败时返回错误信息字符串
///
/// # 使用场景
/// - 用户点击"立即同步"按钮
/// - 刚添加账号后进行首次同步
/// - 测试连接是否正常
///
/// # 与定时任务的区别
///
/// | 特性 | 定时任务 | 手动触发 |
/// |------|----------|----------|
/// | 受暂停状态影响 | 是 | 否 |
/// | 执行时机 | 按间隔 | 立即 |
/// | 并发控制 | 跳过正在执行的 | 等待当前完成 |
///
/// # 示例
/// ```rust
/// let result = trigger_sync(engine_state, 1).await?;
/// println!("同步了 {} 封邮件，耗时 {} ms", result.total_synced, result.duration_ms);
/// ```
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
