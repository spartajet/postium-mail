//! SyncManager 测试
//!
//! 测试同步管理器的基本功能

use crate::integration::helpers;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_db() {
        let db = helpers::create_test_db().await;
        // 成功创建即通过
        drop(db);
    }

    #[tokio::test]
    async fn test_init_test_db() {
        let db = helpers::create_test_db().await;
        helpers::init_test_db(&db).await;
        // 成功初始化即通过
        drop(db);
    }
}
