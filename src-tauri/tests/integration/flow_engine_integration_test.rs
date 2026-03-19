//! FlowEngine 集成测试
//!
//! 测试 FlowEngine 的数据结构和配置

// 引入测试辅助宏
use crate::test_macros::*;

/// 测试 FlowEngineConfig 默认值
#[tokio::test]
async fn test_flow_engine_config_default() {
    test_progress!("测试 FlowEngineConfig 默认值");

    use postium_mail_lib::engine::FlowEngineConfig;

    let config = FlowEngineConfig::default();

    assert!(config.enable_task_scheduler, "任务调度器应该启用");
    assert!(config.enable_notification_manager, "通知管理器应该启用");
    assert!(config.enable_idle_monitoring, "IDLE 监听应该启用");
    assert_eq!(config.default_sync_interval_minutes, 15, "默认同步间隔应为15分钟");
    assert_eq!(config.idle_polling_interval_secs, 300, "默认轮询间隔应为300秒");

    test_info!("enable_task_scheduler: {}", config.enable_task_scheduler);
    test_info!("enable_notification_manager: {}", config.enable_notification_manager);
    test_info!("enable_idle_monitoring: {}", config.enable_idle_monitoring);
    test_info!("default_sync_interval_minutes: {}", config.default_sync_interval_minutes);
    test_info!("idle_polling_interval_secs: {}", config.idle_polling_interval_secs);

    test_success!("FlowEngineConfig 默认值测试通过");
}

/// 测试 EngineState 序列化
#[tokio::test]
async fn test_engine_state_serialization() {
    test_progress!("测试 EngineState 序列化");

    use postium_mail_lib::engine::EngineState;

    let states = vec![
        EngineState::Stopped,
        EngineState::Starting,
        EngineState::Running,
        EngineState::Stopping,
        EngineState::Error("测试错误".to_string()),
    ];

    for state in &states {
        // 测试序列化
        let json = serde_json::to_string(state).unwrap();
        test_info!("  - {:?} -> {}", state, json);

        // 测试反序列化
        let deserialized: EngineState = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, state);
    }

    test_success!("EngineState 序列化测试通过");
}

/// 测试 EngineStatusReport 结构
#[tokio::test]
async fn test_engine_status_report_structure() {
    test_progress!("测试 EngineStatusReport 结构");

    use postium_mail_lib::engine::{EngineState, EngineStatusReport};

    let report = EngineStatusReport {
        state: EngineState::Running,
        running_tasks: 3,
        active_idle_monitors: 2,
        total_notifications_sent: 10,
        uptime_seconds: 3600,
    };

    // 测试序列化
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("running_tasks"), "JSON 应包含 running_tasks");
    assert!(json.contains("active_idle_monitors"), "JSON 应包含 active_idle_monitors");

    test_info!("JSON: {}", json);

    // 测试反序列化
    let deserialized: EngineStatusReport = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.state, EngineState::Running);
    assert_eq!(deserialized.running_tasks, 3);
    assert_eq!(deserialized.active_idle_monitors, 2);
    assert_eq!(deserialized.total_notifications_sent, 10);
    assert_eq!(deserialized.uptime_seconds, 3600);

    test_success!("EngineStatusReport 结构测试通过");
}

/// 测试 FlowEngine 字段序列化
#[tokio::test]
async fn test_flow_engine_fields() {
    test_progress!("测试 FlowEngine 字段");

    use postium_mail_lib::engine::EngineState;

    // 测试 EngineState 的 PartialEq
    assert_eq!(EngineState::Stopped, EngineState::Stopped);
    assert_eq!(EngineState::Running, EngineState::Running);
    assert_ne!(EngineState::Stopped, EngineState::Running);

    // 测试 Error 状态
    let error_state = EngineState::Error("测试错误".to_string());
    let json = serde_json::to_string(&error_state).unwrap();
    assert!(json.contains("测试错误"));

    test_info!("状态序列化正常");
    test_success!("FlowEngine 字段测试通过");
}
