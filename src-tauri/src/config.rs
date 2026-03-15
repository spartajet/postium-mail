use anyhow::Result;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// OAuth配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// 客户端ID（公共客户端不需要client_secret）
    pub client_id: String,
    /// 重定向URI
    pub redirect_uri: String,
    /// 租户ID（common表示多租户）
    pub tenant: String,
    /// OAuth权限范围
    pub scopes: Vec<String>,
    /// 授权端点URL
    pub auth_url: String,
    /// Token端点URL
    pub token_url: String,
}

/// 从环境变量加载OAuth配置
pub fn load_oauth_config() -> Result<OAuthConfig> {
    dotenv::dotenv().ok(); // 尝试加载.env文件，失败也没关系

    let client_id = std::env::var("MICROSOFT_CLIENT_ID")
        .unwrap_or_else(|_| "your-client-id-here".to_string());
    let tenant = std::env::var("MICROSOFT_TENANT")
        .unwrap_or_else(|_| "common".to_string());
    let redirect_uri = std::env::var("MICROSOFT_REDIRECT_URI")
        .unwrap_or_else(|_| "postium-mail://oauth/callback".to_string());

    let scopes_str = std::env::var("MICROSOFT_SCOPES")
        .unwrap_or_else(|_| {
            // 默认 scopes: 需要 openid 才能获取 JWT 格式的 access token
            // 不需要 profile 和 email scope，用户信息从 JWT 中提取
            "https://outlook.office.com/SMTP.Send https://outlook.office.com/IMAP.AccessAsUser.All offline_access openid".to_string()
        });
    let scopes: Vec<String> = scopes_str.split_whitespace().map(String::from).collect();

    let auth_url = std::env::var("MICROSOFT_AUTH_URL")
        .unwrap_or_else(|_| "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string());
    let token_url = std::env::var("MICROSOFT_TOKEN_URL")
        .unwrap_or_else(|_| "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string());

    // 调试日志：打印加载的配置
    tracing::info!("========== OAuth 配置加载 ==========");
    tracing::info!("  client_id: {}", client_id);
    tracing::info!("  redirect_uri: {}", redirect_uri);
    tracing::info!("  scopes ({} 个):", scopes.len());
    for (i, scope) in scopes.iter().enumerate() {
        tracing::info!("    [{}] {}", i, scope);
    }
    tracing::info!("====================================");

    Ok(OAuthConfig {
        client_id,
        redirect_uri,
        tenant,
        scopes,
        auth_url,
        token_url,
    })
}

/// 获取用户数据目录
/// 默认为 ~/.postium
pub fn get_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法找到用户主目录"))?;

    let postium_dir = home.join(".postium");

    // 确保目录存在
    std::fs::create_dir_all(&postium_dir)
        .map_err(|e| anyhow::anyhow!("无法创建 .postium 目录: {}", e))?;

    Ok(postium_dir)
}

/// 获取数据库文件路径
/// 返回 ~/.postium/postium.sqlite
pub fn get_db_path() -> Result<String> {
    let db_path = get_data_dir()?.join("postium.sqlite");

    Ok(db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径转换失败"))?
        .to_string())
}

/// 获取附件存储目录
/// 返回 ~/.postium/attachments/
pub fn get_attachments_dir() -> Result<PathBuf> {
    let attachments_dir = get_data_dir()?.join("attachments");

    // 确保目录存在
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| anyhow::anyhow!("无法创建附件目录: {}", e))?;

    Ok(attachments_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_dir() {
        let dir = get_data_dir();
        assert!(dir.is_ok());
        assert!(dir.unwrap().ends_with(".postium"));
    }

    #[test]
    fn test_get_db_path() {
        let path = get_db_path();
        assert!(path.is_ok());
        assert!(path.as_ref().unwrap().contains(".postium"));
        assert!(path.as_ref().unwrap().contains("postium.sqlite"));
    }
}
