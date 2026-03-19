//! 配置层集成测试
//!
//! 测试 OAuth 配置的加载和验证
//!
//! 注意：这些测试会修改环境变量，可能需要串行运行
//! 使用 `cargo test --test config_tests -- --test-threads=1` 来运行

use postium_mail_lib::OAuthConfig;

/// 测试从环境变量加载 Microsoft OAuth 配置
#[test]
fn test_load_microsoft_oauth_config() {
    // 先清除可能被 dotenv 加载的环境变量
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("MICROSOFT_REDIRECT_URI");
    std::env::remove_var("MICROSOFT_TENANT");

    // 设置测试环境变量
    std::env::set_var("MICROSOFT_CLIENT_ID", "test-client-id-9876");
    std::env::set_var(
        "MICROSOFT_REDIRECT_URI",
        "postium-mail://oauth/callback-test",
    );
    std::env::set_var("MICROSOFT_TENANT", "test-tenant-1234");

    let config = postium_mail_lib::config::load_microsoft_oauth_config();
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.client_id, "test-client-id-9876");
    assert_eq!(config.redirect_uri, "postium-mail://oauth/callback-test");
    assert_eq!(config.tenant, "test-tenant-1234");
    assert!(!config.scopes.is_empty());

    // 清理测试环境变量
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("MICROSOFT_REDIRECT_URI");
    std::env::remove_var("MICROSOFT_TENANT");
}

/// 测试从环境变量加载 Google OAuth 配置
#[test]
fn test_load_google_oauth_config() {
    // 先清除可能被 dotenv 加载的环境变量
    std::env::remove_var("GOOGLE_CLIENT_ID");
    std::env::remove_var("GOOGLE_REDIRECT_URI");

    // 设置测试环境变量
    std::env::set_var("GOOGLE_CLIENT_ID", "test-google-client-id-9876");
    std::env::set_var("GOOGLE_REDIRECT_URI", "postium-mail://oauth/callback-test");

    let config = postium_mail_lib::config::load_google_oauth_config();
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.client_id, "test-google-client-id-9876");
    assert_eq!(config.redirect_uri, "postium-mail://oauth/callback-test");
    assert!(config.tenant.is_empty()); // Google 不需要 tenant
    assert!(!config.scopes.is_empty());

    // 清理测试环境变量
    std::env::remove_var("GOOGLE_CLIENT_ID");
    std::env::remove_var("GOOGLE_REDIRECT_URI");
}

/// 测试通用配置加载器
#[test]
fn test_load_oauth_config_for_provider() {
    // 先清除可能被 dotenv 加载的环境变量
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("GOOGLE_CLIENT_ID");

    // 测试 Microsoft / Outlook
    std::env::set_var("MICROSOFT_CLIENT_ID", "test-client-id");
    let result = postium_mail_lib::config::load_oauth_config_for_provider("outlook");
    assert!(result.is_ok());

    let result = postium_mail_lib::config::load_oauth_config_for_provider("microsoft365");
    assert!(result.is_ok());

    // 测试 Gmail / Google Workspace
    std::env::set_var("GOOGLE_CLIENT_ID", "test-google-client-id");
    let result = postium_mail_lib::config::load_oauth_config_for_provider("gmail");
    assert!(result.is_ok());

    let result = postium_mail_lib::config::load_oauth_config_for_provider("googleworkspace");
    assert!(result.is_ok());

    // 测试不支持的服务商
    let result = postium_mail_lib::config::load_oauth_config_for_provider("unsupported");
    assert!(result.is_err());

    // 清理
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("GOOGLE_CLIENT_ID");
}

/// 测试 OAuthConfig::from_env_for_provider
#[test]
fn test_oauth_config_from_env_for_provider() {
    // 先清除可能被 dotenv 加载的环境变量
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("MICROSOFT_REDIRECT_URI");
    std::env::remove_var("MICROSOFT_TENANT");

    // 设置测试环境变量
    std::env::set_var("MICROSOFT_CLIENT_ID", "test-client-id-from-env-5555");
    std::env::set_var("MICROSOFT_REDIRECT_URI", "postium-mail://oauth/callback");
    std::env::set_var("MICROSOFT_TENANT", "test-tenant-from-env-5555");

    let result = OAuthConfig::from_env_for_provider("outlook");
    assert!(result.is_ok());

    let config = result.unwrap();
    assert_eq!(config.client_id, "test-client-id-from-env-5555");
    assert_eq!(config.redirect_uri, "postium-mail://oauth/callback");
    assert!(config.pkce_enabled);
    assert_eq!(
        config.tenant_id,
        Some("test-tenant-from-env-5555".to_string())
    );
    assert!(config.client_secret.is_none());

    // 清理测试环境变量
    std::env::remove_var("MICROSOFT_CLIENT_ID");
    std::env::remove_var("MICROSOFT_REDIRECT_URI");
    std::env::remove_var("MICROSOFT_TENANT");
}

/// 测试 OAuthConfig 验证
#[test]
fn test_oauth_config_validate() {
    // 创建一个有效的配置
    let config = OAuthConfig {
        client_id: "test-client-id".to_string(),
        client_secret: None,
        auth_url: "https://example.com/auth".to_string(),
        token_url: "https://example.com/token".to_string(),
        redirect_uri: "postium-mail://oauth/callback".to_string(),
        scopes: vec!["scope1".to_string(), "scope2".to_string()],
        pkce_enabled: true,
        tenant_id: None,
    };

    assert!(config.validate().is_ok());

    // 测试无效的 client_id
    let mut invalid_config = config.clone();
    invalid_config.client_id = String::new();
    assert!(invalid_config.validate().is_err());

    // 测试无效的 redirect_uri
    invalid_config = config.clone();
    invalid_config.redirect_uri = String::new();
    assert!(invalid_config.validate().is_err());

    // 测试无效的 auth_url
    invalid_config = config.clone();
    invalid_config.auth_url = "http://insecure.com".to_string();
    assert!(invalid_config.validate().is_err());

    // 测试无效的 token_url
    invalid_config = config.clone();
    invalid_config.token_url = "not-a-url".to_string();
    assert!(invalid_config.validate().is_err());

    // 测试空的 scopes
    invalid_config = config;
    invalid_config.scopes = Vec::new();
    assert!(invalid_config.validate().is_err());
}
