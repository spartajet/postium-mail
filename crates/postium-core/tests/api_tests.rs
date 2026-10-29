//! CoreApi 装配冒烟测试（临时数据库）。

use postium_core::api::CoreApi;

#[tokio::test]
async fn core_api_boots_and_lists_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let (sync_tx, _sync_rx) = CoreApi::sync_event_channel();
    let api = CoreApi::init(dir.path(), false, false, sync_tx).await.unwrap();

    // 空库：账号列表为空
    let accounts = api.list_accounts().await.unwrap();
    assert!(accounts.is_empty());

    // provider 池可用
    let providers = api.list_providers().unwrap();
    assert!(!providers.is_empty(), "内置 provider 列表不应为空");

    // 邮箱前缀探测
    let det = api.detect_provider("someone@gmail.com");
    assert!(det.detected, "gmail 应被识别");

    // 标签/文件夹统计在空库下可用
    let stats = api.get_folder_stats_for_all_accounts().await.unwrap();
    assert!(stats.is_empty());
}
