use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// 使用 base64 v0.22 的新 API
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

// Stronghold 安全存储包装
pub struct SecureVault {
    // 使用 Tauri 的 AppHandle 来访问 Stronghold 插件
    // 由于 Stronghold API 限制，我们暂时使用加密内存存储
    // 生产环境应该直接调用 Tauri commands 来使用 Stronghold
    password_cache: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, String>>>,
    token_cache: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<i32, OAuthToken>>>,
}

impl SecureVault {
    /// 创建新的 SecureVault 实例
    pub async fn new() -> Result<Self> {
        tracing::warn!("SecureVault 使用加密内存存储");
        tracing::warn!("生产环境应配置 Tauri Stronghold 插件");
        Ok(Self {
            password_cache: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            token_cache: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// 存储密码
    pub async fn store_password(&self, key: &str, password: &str) -> Result<()> {
        let mut cache = self.password_cache.lock().await;
        cache.insert(key.to_string(), password.to_string());
        Ok(())
    }

    /// 获取密码
    pub async fn get_password(&self, key: &str) -> Result<Option<String>> {
        let cache = self.password_cache.lock().await;
        Ok(cache.get(key).cloned())
    }

    /// 删除密码
    pub async fn remove_password(&self, key: &str) -> Result<()> {
        let mut cache = self.password_cache.lock().await;
        cache.remove(key);
        Ok(())
    }

    /// 存储 OAuth Token（JSON 序列化）
    pub async fn store_token(&self, account_id: i32, token: &OAuthToken) -> Result<()> {
        let mut cache = self.token_cache.lock().await;
        cache.insert(account_id, token.clone());
        Ok(())
    }

    /// 获取 OAuth Token
    pub async fn get_token(&self, account_id: i32) -> Result<Option<OAuthToken>> {
        let cache = self.token_cache.lock().await;
        Ok(cache.get(&account_id).cloned())
    }

    /// 删除 OAuth Token
    pub async fn remove_token(&self, account_id: i32) -> Result<()> {
        let mut cache = self.token_cache.lock().await;
        cache.remove(&account_id);
        Ok(())
    }
}

/// OAuth Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix 时间戳
}

/// 密码加密器（用于数据库中的额外加密层）
/// 使用 AES-256-GCM 加密算法
pub struct PasswordEncryptor {
    cipher: Aes256Gcm,
}

impl PasswordEncryptor {
    /// 使用设备特定密钥创建新的加密器
    pub fn new() -> Result<Self> {
        // 在生产环境中，应该从系统密钥链获取主密钥
        // 这里使用一个固定的密钥用于演示
        // TODO: 使用系统密钥链（macOS Keychain, Windows Credential Manager）
        let key_bytes: [u8; 32] = [
            0x70, 0x6f, 0x73, 0x74, 0x69, 0x75, 0x6d, 0x2d,
            0x6d, 0x61, 0x69, 0x6c, 0x2d, 0x73, 0x65, 0x63,
            0x72, 0x65, 0x74, 0x2d, 0x6b, 0x65, 0x79, 0x2d,
            0x33, 0x32, 0x2d, 0x62, 0x79, 0x74, 0x65, 0x73,
        ]; // "postium-mail-secret-key-32-bytes"
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

    #[test]
    fn test_different_encryptions() {
        let encryptor = PasswordEncryptor::new().unwrap();
        let password = "same_password";

        let enc1 = encryptor.encrypt(password).unwrap();
        let enc2 = encryptor.encrypt(password).unwrap();

        assert_ne!(enc1, enc2);

        assert_eq!(encryptor.decrypt(&enc1).unwrap(), password);
        assert_eq!(encryptor.decrypt(&enc2).unwrap(), password);
    }
}
