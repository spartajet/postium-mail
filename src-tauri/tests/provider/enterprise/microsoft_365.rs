//! Microsoft 365 服务商测试
//!
//! 测试 Microsoft 365 企业邮箱的连接和基本功能

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore = "需要真实 Microsoft 365 账号"]
    async fn test_microsoft_365_connection() {
        // TODO: 添加 Microsoft 365 连接测试
        // 需要环境变量：
        // - MICROSOFT_365_EMAIL
        // - MICROSOFT_365_PASSWORD
    }
}
