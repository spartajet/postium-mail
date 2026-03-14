use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};

// 使用 base64 v0.22 的新 API
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

// Stronghold 安全存储
pub struct SecureVault {
    // TODO: 使用 tauri-plugin-stronghold 实现
    // 当前先使用内存存储作为占位实现
    _tokens: std::collections::HashMap<String, String>,
}

impl SecureVault {
    pub async fn new() -> Result<Self> {
        // TODO: 初始化 Stronghold vault
        tracing::warn!("SecureVault 当前使用内存存储，请使用 Stronghold 插件");
        Ok(Self {
            _tokens: std::collections::HashMap::new(),
        })
    }

    /// 存储密码（TODO: 使用 Stronghold）
    pub async fn store_password(&self, key: &str, password: &str) -> Result<()> {
        tracing::warn!("store_password 使用占位实现");
        // TODO: self.vault.insert(key, password).await?;
        Ok(())
    }

    /// 获取密码（TODO: 使用 Stronghold）
    pub async fn get_password(&self, key: &str) -> Result<Option<String>> {
        tracing::warn!("get_password 使用占位实现");
        // TODO: self.vault.get(key).await.map_err(|e| anyhow!("获取失败: {}", e))
        Ok(None)
    }

    /// 存储 OAuth Token（TODO: 使用 Stronghold）
    pub async fn store_token(&self, account_id: i32, _token: &crate::services::oauth_service::OAuthToken) -> Result<()> {
        tracing::warn!("store_token 使用占位实现");
        // TODO: 使用 Stronghold 加密存储
        Ok(())
    }

    /// 获取 OAuth Token（TODO: 使用 Stronghold）
    pub async fn get_token(&self, account_id: i32) -> Result<Option<crate::services::oauth_service::OAuthToken>> {
        tracing::warn!("get_token 使用占位实现");
        // TODO: 从 Stronghold 读取并解密
        Ok(None)
    }
}

/// 密码加密器
/// 使用 AES-256-GCM 加密算法
pub struct PasswordEncryptor {
    cipher: Aes256Gcm,
}

impl PasswordEncryptor {
    /// 使用设备特定密钥创建新的加密器
    /// 注意：这是一个简化版本，生产环境应该使用更安全的密钥派生
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
    /// 返回 Base64 编码的加密数据（包含 nonce）
    pub fn encrypt(&self, password: &str) -> Result<String> {
        // 生成随机 nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // 加密数据
        let ciphertext = self.cipher
            .encrypt(&nonce, password.as_bytes())
            .map_err(|e| anyhow!("加密失败: {}", e))?;

        // 将 nonce 和密文组合并编码为 Base64
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);

        // base64 v0.22 使用 engine API
        Ok(BASE64_STANDARD.encode(&combined))
    }

    /// 解密密码
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        // 解码 Base64
        let combined = BASE64_STANDARD.decode(encrypted)
            .map_err(|e| anyhow!("Base64 解码失败: {}", e))?;

        // 分离 nonce 和密文
        if combined.len() < 12 {
            return Err(anyhow!("加密数据格式错误"));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // 解密数据
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

        // 每次加密应该产生不同的结果（因为随机 nonce）
        let enc1 = encryptor.encrypt(password).unwrap();
        let enc2 = encryptor.encrypt(password).unwrap();

        assert_ne!(enc1, enc2);

        // 但解密后应该相同
        assert_eq!(encryptor.decrypt(&enc1).unwrap(), password);
        assert_eq!(encryptor.decrypt(&enc2).unwrap(), password);
    }
}
