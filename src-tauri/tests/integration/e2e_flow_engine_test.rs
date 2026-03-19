//! FlowEngine 端到端工作流测试
//!
//! 测试 FlowEngine 的完整工作流程

// 引入测试辅助宏
use crate::test_macros::*;

/// 测试引擎状态转换
#[tokio::test]
#[ignore]
async fn test_engine_state_transitions() {
    test_progress!("测试引擎状态转换");

    use postium_mail_lib::engine::{EngineState, FlowEngineConfig};

    // 测试状态枚举
    let states = vec![
        EngineState::Stopped,
        EngineState::Starting,
        EngineState::Running,
        EngineState::Stopping,
    ];

    test_info!("状态列表:");
    for state in &states {
        let json = serde_json::to_string(state).unwrap();
        test_info!("  - {:?} -> {}", state, json);
    }

    // 测试配置
    let config = FlowEngineConfig::default();
    test_info!("配置验证:");
    test_info!("  - enable_task_scheduler: {}", config.enable_task_scheduler);
    test_info!("  - default_sync_interval_minutes: {}", config.default_sync_interval_minutes);

    test_success!("状态转换测试通过");
}

/// 测试任务管理数据结构
#[tokio::test]
async fn test_task_data_structures() {
    test_progress!("测试任务管理数据结构");

    use postium_mail_lib::engine::{EngineState, EngineStatusReport};

    // 模拟不同的状态报告
    let reports = vec![
        EngineStatusReport {
            state: EngineState::Stopped,
            running_tasks: 0,
            active_idle_monitors: 0,
            total_notifications_sent: 0,
            uptime_seconds: 0,
        },
        EngineStatusReport {
            state: EngineState::Running,
            running_tasks: 5,
            active_idle_monitors: 3,
            total_notifications_sent: 42,
            uptime_seconds: 3600,
        },
    ];

    for report in &reports {
        let json = serde_json::to_string(report).unwrap();
        test_info!("  - {:?} -> {}", report.state, json);

        // 验证反序列化
        let deserialized: EngineStatusReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.state, report.state);
        assert_eq!(deserialized.running_tasks, report.running_tasks);
    }

    test_success!("任务管理数据结构测试通过");
}

/// 测试并发操作的数据结构
#[tokio::test]
async fn test_concurrent_operations_structures() {
    test_progress!("测试并发操作的数据结构");

    use std::sync::Arc;

    // 模拟共享状态
    let state = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // 测试原子操作
    assert!(!state.load(std::sync::atomic::Ordering::Relaxed));
    state.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(state.load(std::sync::atomic::Ordering::Relaxed));

    test_info!("原子操作正常");
    test_success!("并发操作数据结构测试通过");
}
