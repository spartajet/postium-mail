// SyncManager 集成测试
//
// 测试 SyncManager 的类型、枚举和序列化

// 引入测试辅助宏
use crate::test_macros::*;

/// 测试同步阶段枚举
#[tokio::test]
#[ignore]
async fn test_sync_stages() {
    test_progress!("测试同步阶段枚举...");

    use postium_mail_lib::sync::{SyncStage, SyncProgress};

    let stages = vec![
        SyncStage::Connecting,
        SyncStage::SyncingFolders,
        SyncStage::SyncingEmails,
        SyncStage::Completed,
        SyncStage::Error,
    ];

    for stage in &stages {
        test_info!("  - {:?}", stage);
    }

    assert_eq!(stages.len(), 5);

    // 测试 SyncProgress 结构
    let progress = SyncProgress {
        stage: SyncStage::SyncingEmails,
        folder: Some("INBOX".to_string()),
        current: 50,
        total: 100,
        message: "正在同步邮件...".to_string(),
    };

    assert_eq!(progress.current, 50);
    assert_eq!(progress.total, 100);
    assert!(matches!(progress.stage, SyncStage::SyncingEmails));

    test_success!(同步阶段枚举测试通过");
}

/// 测试同步结果结构
#[tokio::test]
#[ignore]
async fn test_sync_result() {
    test_info!(测试同步结果结构...");

    use postium_mail_lib::sync::SyncResult;

    let result = SyncResult {
        total_synced: 100,
        folders_synced: 5,
        errors: 0,
        duration_ms: 1500,
    };

    test_info!("同步结果:");
    test_info!("  - 总同步: {}", result.total_synced);
    test_info!("  - 文件夹: {}", result.folders_synced);
    test_info!("  - 错误: {}", result.errors);
    test_info!("  - 耗时: {} ms", result.duration_ms);

    assert_eq!(result.total_synced, 100);
    assert_eq!(result.errors, 0);

    test_success!(同步结果结构测试通过");
}

/// 测试 SyncStage 序列化和反序列化
#[tokio::test]
#[ignore]
async fn test_sync_stage_serialization() {
    test_progress!("测试 SyncStage 序列化...");

    use postium_mail_lib::sync::SyncStage;

    let stage = SyncStage::SyncingEmails;

    // 测试序列化
    let json = serde_json::to_string(&stage).unwrap();
    test_info!("  序列化: {}", json);

    // 测试反序列化
    let deserialized: SyncStage = serde_json::from_str(&json).unwrap();
    assert_eq!(stage, deserialized);

    test_success!(SyncStage 序列化测试通过");
}

/// 测试 SyncProgress 序列化和反序列化
#[tokio::test]
#[ignore]
async fn test_sync_progress_serialization() {
    test_progress!("测试 SyncProgress 序列化...");

    use postium_mail_lib::sync::{SyncProgress, SyncStage};

    let progress = SyncProgress {
        stage: SyncStage::SyncingFolders,
        folder: Some("INBOX".to_string()),
        current: 10,
        total: 100,
        message: "正在同步文件夹...".to_string(),
    };

    // 测试序列化
    let json = serde_json::to_string(&progress).unwrap();
    test_info!("  序列化: {}", json);

    // 测试反序列化
    let deserialized: SyncProgress = serde_json::from_str(&json).unwrap();
    assert_eq!(progress.current, deserialized.current);
    assert_eq!(progress.total, deserialized.total);
    assert_eq!(progress.folder, deserialized.folder);

    test_success!(SyncProgress 序列化测试通过");
}

/// 测试 SyncResult 序列化和反序列化
#[tokio::test]
#[ignore]
async fn test_sync_result_serialization() {
    test_info!(测试 SyncResult 序列化...");

    use postium_mail_lib::sync::SyncResult;

    let result = SyncResult {
        total_synced: 200,
        folders_synced: 8,
        errors: 1,
        duration_ms: 3200,
    };

    // 测试序列化
    let json = serde_json::to_string(&result).unwrap();
    test_info!("  序列化: {}", json);

    // 测试反序列化
    let deserialized: SyncResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result.total_synced, deserialized.total_synced);
    assert_eq!(result.errors, deserialized.errors);

    test_success!(SyncResult 序列化测试通过");
}

/// 测试所有同步阶段的枚举值
#[tokio::test]
#[ignore]
async fn test_all_sync_stages() {
    test_progress!("测试所有同步阶段...");

    use postium_mail_lib::sync::SyncStage;

    // 测试所有阶段
    let stages = [
        (SyncStage::Connecting, "Connecting"),
        (SyncStage::SyncingFolders, "SyncingFolders"),
        (SyncStage::SyncingEmails, "SyncingEmails"),
        (SyncStage::Completed, "Completed"),
        (SyncStage::Error, "Error"),
    ];

    for (stage, name) in stages {
        let json = serde_json::to_string(&stage).unwrap();
        test_info!("  {:?} -> {}", stage, json);

        // 验证序列化结果包含名称
        assert!(json.contains(name));
    }

    test_success!(所有同步阶段测试通过");
}

/// 测试 SyncProgress 字段完整性
#[tokio::test]
#[ignore]
async fn test_sync_progress_fields() {
    test_progress!(测试 SyncProgress 字段完整性...");

    use postium_mail_lib::sync::{SyncProgress, SyncStage};

    let progress = SyncProgress {
        stage: SyncStage::SyncingEmails,
        folder: Some("Sent".to_string()),
        current: 75,
        total: 150,
        message: "已同步 75/150 封邮件".to_string(),
    };

    // 验证所有字段
    assert_eq!(progress.current, 75);
    assert_eq!(progress.total, 150);
    assert_eq!(progress.folder, Some("Sent".to_string()));
    assert_eq!(progress.message, "已同步 75/150 封邮件");

    // 验证阶段
    assert!(matches!(progress.stage, SyncStage::SyncingEmails));

    // 验证进度计算
    let percentage = (progress.current as f64 / progress.total as f64 * 100.0) as usize;
    assert_eq!(percentage, 50);

    test_success!(SyncProgress 字段完整性测试通过");
}
