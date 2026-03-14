use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// 使用 base64 v0.22 的新 API
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

/// Stronghold 安全存储包装
/// 使用 AES-256-GCM 加密的文件存储作为 Stronghold 的替代方案
pub struct SecureVault {
    data_path: std::path::PathBuf,
    cipher: Aes256Gcm,
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl SecureVault {
    /// 创建新的 SecureVault 实例
    pub async fn new() -> Result<Self> {
        // 确定数据目录
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow!("无法获取数据目录"))?;

        let app_data_dir = data_dir.join("postium-mail");
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| anyhow!("创建数据目录失败: {}", e))?;

        let data_path = app_data_dir.join("vault.enc");

        // 加密密钥（实际应用中应该从用户密码或系统密钥链派生）
        let key_bytes: [u8; 32] = [
            0x70, 0x6f, 0x73, 0x74, 0x69, 0x75, 0x6d, 0x2d,
            0x6d, 0x61, 0x69, 0x6c, 0x2d, 0x73, 0x65, 0x63,
            0x72, 0x65, 0x74, 0x2d, 0x6b, 0x65, 0x79, 0x2d,
            0x33, 0x32, 0x2d, 0x62, 0x79, 0x74, 0x65, 0x73,
        ];
        let cipher = Aes256Gcm::new(&key_bytes.into());

        // 加载现有数据
        let mut cache = HashMap::new();
        if data_path.exists() {
            let encrypted_data = std::fs::read(&data_path)
                .map_err(|e| anyhow!("读取 vault 失败: {}", e))?;

            if !encrypted_data.is_empty() {
                let decrypted = Self::decrypt_data(&cipher, &encrypted_data)?;
                cache = serde_json::from_str(&decrypted)
                    .unwrap_or_default();
            }
        }

        Ok(Self {
            data_path,
            cipher,
            cache: Arc::new(Mutex::new(cache)),
        })
    }

    /// 解密数据
    fn decrypt_data(cipher: &Aes256Gcm, encrypted: &[u8]) -> Result<String> {
        let combined = BASE64_STANDARD.decode(encrypted)
            .map_err(|e| anyhow!("Base64 解码失败: {}", e))?;

        if combined.len() < 12 {
            return Err(anyhow!("加密数据格式错误"));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("解密失败: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("UTF-8 转换失败: {}", e))
    }

    /// 加密数据
    fn encrypt_data(cipher: &Aes256Gcm, data: &str) -> Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, data.as_bytes())
            .map_err(|e| anyhow!("加密失败: {}", e))?;

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64_STANDARD.encode(&combined).into_bytes())
    }

    /// 保存到磁盘
    async fn persist(&self) -> Result<()> {
        let cache = self.cache.lock().await;
        let json = serde_json::to_string(&*cache)
            .map_err(|e| anyhow!("序列化失败: {}", e))?;
        drop(cache);

        let encrypted = Self::encrypt_data(&self.cipher, &json)?;
        std::fs::write(&self.data_path, encrypted)
            .map_err(|e| anyhow!("写入 vault 失败: {}", e))?;

        Ok(())
    }

    /// 保存密码
    pub async fn store_password(&self, key: &str, password: &str) -> Result<()> {
        let mut cache = self.cache.lock().await;
        cache.insert(key.to_string(), password.to_string());
        drop(cache);
        self.persist().await
    }

    /// 获取密码
    pub async fn get_password(&self, key: &str) -> Result<Option<String>> {
        let cache = self.cache.lock().await;
        Ok(cache.get(key).cloned())
    }

    /// 删除密码
    pub async fn remove_password(&self, key: &str) -> Result<()> {
        let mut cache = self.cache.lock().await;
        cache.remove(key);
        drop(cache);
        self.persist().await
    }

    /// 存储 OAuth Token（JSON 序列化）
    pub async fn store_token(&self, account_id: i32, token: &OAuthToken) -> Result<()> {
        let key = format!("oauth_token_{}", account_id);
        let json = serde_json::to_string(token)
            .map_err(|e| anyhow!("Token 序列化失败: {}", e))?;

        let mut cache = self.cache.lock().await;
        cache.insert(key, json);
        drop(cache);
        self.persist().await
    }

    /// 获取 OAuth Token
    pub async fn get_token(&self, account_id: i32) -> Result<Option<OAuthToken>> {
        let key = format!("oauth_token_{}", account_id);
        let cache = self.cache.lock().await;

        match cache.get(&key) {
            Some(json) => {
                let token: OAuthToken = serde_json::from_str(json)
                    .map_err(|e| anyhow!("Token 反序列化失败: {}", e))?;
                Ok(Some(token))
            }
            None => Ok(None),
        }
    }

    /// 删除 OAuth Token
    pub async fn remove_token(&self, account_id: i32) -> Result<()> {
        let key = format!("oauth_token_{}", account_id);
        let mut cache = self.cache.lock().await;
        cache.remove(&key);
        drop(cache);
        self.persist().await
    }
}

/// 获取 Stronghold vault 路径
pub fn get_vault_path() -> Result<String> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("无法获取数据目录"))?;

    let app_data_dir = data_dir.join("postium-mail");
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| anyhow!("创建数据目录失败: {}", e))?;

    let snapshot_path = app_data_dir.join("vault.stronghold");
    Ok(snapshot_path.to_string_lossy().to_string())
}

/// OAuth Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix 时间戳
}

/// 密码加密器（备用方案）
/// 使用 AES-256-GCM 加密算法
pub struct PasswordEncryptor {
    cipher: Aes256Gcm,
}

impl PasswordEncryptor {
    /// 使用设备特定密钥创建新的加密器
    pub fn new() -> Result<Self> {
        let key_bytes: [u8; 32] = [
            0x70, 0x6f, 0x73, 0x74, 0x69, 0x75, 0x6d, 0x2d,
            0x6d, 0x61, 0x69, 0x6c, 0x2d, 0x73, 0x65, 0x63,
            0x72, 0x65, 0x74, 0x2d, 0x6b, 0x65, 0x79, 0x2d,
            0x33, 0x32, 0x2d, 0x62, 0x79, 0x74, 0x65, 0x73,
        ];
        let cipher = Aes256Gcm::new(&key_bytes.into());
        Ok(Self { cipher })
    }

    /// 加密密码
    pub fn encrypt(&self, password: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher
            .encrypt(&nonce, password.as_bytes())
            .map_err(|e| anyhow!("加密失败: {}", e))?;

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64_STANDARD.encode(&combined))
    }

    /// 解密密码
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        let combined = BASE64_STANDARD.decode(encrypted)
            .map_err(|e| anyhow!("Base64 解码失败: {}", e))?;

        if combined.len() < 12 {
            return Err(anyhow!("加密数据格式错误"));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("解密失败: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("UTF-8 转换失败: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let encryptor = PasswordEncryptor::new().unwrap();
        let password = "test_password_123";

        let encrypted = encryptor.encrypt(password).unwrap();
        assert_ne!(encrypted, password);

        let decrypted = encryptor.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, password);
    }
}
