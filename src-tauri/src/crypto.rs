use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri_plugin_stronghold::stronghold::Stronghold as TauriStronghold;
use tokio::sync::Mutex;

/// 真正的 IOTA Stronghold 安全存储
pub struct SecureVault {
    stronghold: Arc<Mutex<TauriStronghold>>,
    client_path: Vec<u8>,
}

impl SecureVault {
    /// 创建新的 SecureVault 实例，使用 IOTA Stronghold
    pub async fn new() -> Result<Self> {
        // 确定数据目录
        let data_dir = dirs::data_local_dir().ok_or_else(|| anyhow!("无法获取数据目录"))?;

        let app_data_dir = data_dir.join("postium-mail");
        std::fs::create_dir_all(&app_data_dir).map_err(|e| anyhow!("创建数据目录失败: {}", e))?;

        let snapshot_path = app_data_dir.join("vault.stronghold");

        // 使用固定密钥派生（实际应用中应该从用户密码派生）
        let key_bytes: [u8; 32] = [
            0x70, 0x6f, 0x73, 0x74, 0x69, 0x75, 0x6d, 0x2d, 0x6d, 0x61, 0x69, 0x6c, 0x2d, 0x73,
            0x65, 0x63, 0x72, 0x65, 0x74, 0x2d, 0x6b, 0x65, 0x79, 0x2d, 0x33, 0x32, 0x2d, 0x62,
            0x79, 0x74, 0x65, 0x73,
        ];

        let stronghold = TauriStronghold::new(snapshot_path, key_bytes.to_vec())
            .map_err(|e| anyhow!("Stronghold 初始化失败: {}", e))?;

        let client_path = b"postium-mail-client".to_vec();
        let stronghold_arc = Arc::new(Mutex::new(stronghold));

        {
            let stronghold = stronghold_arc.lock().await;
            stronghold
                .create_client(client_path.clone())
                .map_err(|e| anyhow!("创建 Stronghold 客户端失败: {}", e))?;
        }

        Ok(Self {
            stronghold: stronghold_arc,
            client_path,
        })
    }

    /// 保存密码到 Stronghold
    pub async fn store_password(&self, key: &str, password: &str) -> Result<()> {
        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;
        client
            .store()
            .insert(key.as_bytes().to_vec(), password.as_bytes().to_vec(), None)
            .map_err(|e| anyhow!("存储密码失败: {}", e))?;
        Ok(())
    }

    /// 从 Stronghold 获取密码
    pub async fn get_password(&self, key: &str) -> Result<Option<String>> {
        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;
        match client.store().get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                let password =
                    String::from_utf8(bytes).map_err(|e| anyhow!("密码 UTF-8 转换失败: {}", e))?;
                Ok(Some(password))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow!("获取密码失败: {}", e)),
        }
    }

    /// 删除密码
    pub async fn remove_password(&self, key: &str) -> Result<()> {
        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;
        client
            .store()
            .delete(key.as_bytes())
            .map_err(|e| anyhow!("删除密码失败: {}", e))?;
        Ok(())
    }

    /// 存储 OAuth Token 到 Stronghold（JSON 序列化）
    pub async fn store_token(&self, account_id: i32, token: &OAuthToken) -> Result<()> {
        let key = format!("oauth_token_{}", account_id);
        let json = serde_json::to_string(token).map_err(|e| anyhow!("Token 序列化失败: {}", e))?;

        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;
        client
            .store()
            .insert(key.as_bytes().to_vec(), json.as_bytes().to_vec(), None)
            .map_err(|e| anyhow!("存储 Token 失败: {}", e))?;
        Ok(())
    }

    /// 从 Stronghold 获取 OAuth Token
    pub async fn get_token(&self, account_id: i32) -> Result<Option<OAuthToken>> {
        let key = format!("oauth_token_{}", account_id);
        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;

        match client.store().get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                let json = String::from_utf8(bytes)
                    .map_err(|e| anyhow!("Token JSON UTF-8 转换失败: {}", e))?;
                let token: OAuthToken = serde_json::from_str(&json)
                    .map_err(|e| anyhow!("Token 反序列化失败: {}", e))?;
                Ok(Some(token))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow!("获取 Token 失败: {}", e)),
        }
    }

    /// 删除 OAuth Token
    pub async fn remove_token(&self, account_id: i32) -> Result<()> {
        let key = format!("oauth_token_{}", account_id);
        let stronghold = self.stronghold.lock().await;
        let client = stronghold
            .get_client(self.client_path.clone())
            .map_err(|e| anyhow!("获取 Stronghold 客户端失败: {}", e))?;
        client
            .store()
            .delete(key.as_bytes())
            .map_err(|e| anyhow!("删除 Token 失败: {}", e))?;
        Ok(())
    }

    /// 保存 Stronghold 快照
    pub async fn save(&self) -> Result<()> {
        let stronghold = self.stronghold.lock().await;
        stronghold
            .save()
            .map_err(|e| anyhow!("保存 Stronghold 失败: {}", e))
    }
}

/// OAuth Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix 时间戳
}

/// 获取 Stronghold vault 路径（仅用于日志显示）
pub fn get_vault_path() -> Result<String> {
    let data_dir = dirs::data_local_dir().ok_or_else(|| anyhow!("无法获取数据目录"))?;

    let app_data_dir = data_dir.join("postium-mail");
    std::fs::create_dir_all(&app_data_dir).map_err(|e| anyhow!("创建数据目录失败: {}", e))?;

    Ok(app_data_dir
        .join("vault.stronghold")
        .to_string_lossy()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stronghold_store_retrieve() {
        let vault = SecureVault::new().await.unwrap();

        // 测试存储和获取密码
        vault
            .store_password("test_key", "test_password_123")
            .await
            .unwrap();

        let retrieved = vault.get_password("test_key").await.unwrap();
        assert_eq!(retrieved, Some("test_password_123".to_string()));

        // 测试删除
        vault.remove_password("test_key").await.unwrap();
        let deleted = vault.get_password("test_key").await.unwrap();
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    async fn test_token_storage() {
        let vault = SecureVault::new().await.unwrap();

        let token = OAuthToken {
            access_token: "access123".to_string(),
            refresh_token: "refresh456".to_string(),
            expires_at: 1234567890,
        };

        // 存储 token
        vault.store_token(1, &token).await.unwrap();

        // 获取 token
        let retrieved = vault.get_token(1).await.unwrap().unwrap();
        assert_eq!(retrieved.access_token, "access123");
        assert_eq!(retrieved.refresh_token, "refresh456");
        assert_eq!(retrieved.expires_at, 1234567890);

        // 删除 token
        vault.remove_token(1).await.unwrap();
        let deleted = vault.get_token(1).await.unwrap();
        assert!(deleted.is_none());
    }
}
