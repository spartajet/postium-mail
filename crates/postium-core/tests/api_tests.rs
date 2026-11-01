//! CoreApi 装配冒烟测试（临时数据库）。

use postium_core::api::CoreApi;

#[tokio::test]
async fn core_api_boots_and_lists_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();

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

/// 会话列表 API 转发冒烟（Task 8）：三个入口经 CoreApi 直调
/// EmailService 均可用；空库返回空列表 + 零总数（转发层不加逻辑，
/// 聚合正确性由 thread_repo / email_service 各自的测试锁）。
#[tokio::test]
async fn core_api_forwards_conversation_list_apis() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();

    let by_folder = api.list_conversations(1, "INBOX", 1, 50).await.unwrap();
    assert!(by_folder.conversations.is_empty());
    assert_eq!(by_folder.total, 0);

    let by_category = api
        .list_conversations_by_category(1, postium_core::service::email_service::EmailCategory::Inbox, 1, 50)
        .await
        .unwrap();
    assert!(by_category.conversations.is_empty());
    assert_eq!(by_category.total, 0);

    let all_accounts = api
        .list_conversations_by_category_for_all_accounts(
            postium_core::service::email_service::EmailCategory::Inbox,
            1,
            50,
        )
        .await
        .unwrap();
    assert!(all_accounts.conversations.is_empty());
    assert_eq!(all_accounts.total, 0);
}
