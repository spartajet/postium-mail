//! OAuth 通用工具
//!
//! 提供 PKCE、token 管理、XOAUTH2 生成等通用功能

use std::sync::Mutex;
use std::collections::HashMap;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::info;

/// PKCE 验证器存储
pub struct PkceVerifierStore {
    verifiers: Mutex<HashMap<String, PkceVerifier>>,
}

impl PkceVerifierStore {
    pub fn new() -> Self {
        Self {
            verifiers: Mutex::new(HashMap::new()),
        }
    }

    /// 生成并存储新的 PKCE 验证器
    pub fn generate_and_store(&self, state: &str) -> Result<(String, String), String> {
        // 生成 code_verifier (43-128 个字符)
        let code_verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(128)
            .map(char::from)
            .collect();

        // 生成 code_challenge (SHA256 + base64url)
        let code_challenge = Self::create_code_challenge(&code_verifier);

        let verifier = PkceVerifier {
            code_verifier: code_verifier.clone(),
            created_at: chrono::Utc::now(),
        };

        self.verifiers
            .lock()
            .map_err(|e| format!("锁定失败: {}", e))?
            .insert(state.to_string(), verifier);

        info!("生成 PKCE 验证器: state={}", state);

        Ok((code_verifier, code_challenge))
    }

    /// 获取并移除验证器
    pub fn take(&self, state: &str) -> Option<String> {
        self.verifiers
            .lock()
            .ok()?
            .remove(state)
            .map(|v| {
                info!("移除 PKCE 验证器: state={}", state);
                v.code_verifier
            })
    }

    /// 创建 code_challenge（公开方法，供外部使用）
    pub fn create_code_challenge(code_verifier: &str) -> String {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();

        // Base64 URL-safe encoding
        BASE64.encode(hash)
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string()
    }

    /// 清理过期的验证器（超过 10 分钟）
    pub fn cleanup_expired(&self) {
        let threshold = chrono::Utc::now() - chrono::Duration::minutes(10);

        if let Ok(mut verifiers) = self.verifiers.lock() {
            let before = verifiers.len();
            verifiers.retain(|_, v| v.created_at > threshold);
            let after = verifiers.len();

            if before > after {
                info!("清理 {} 个过期的 PKCE 验证器", before - after);
            }
        }
    }
}

impl Default for PkceVerifierStore {
    fn default() -> Self {
        Self::new()
    }
}

/// PKCE 验证器
#[derive(Debug, Clone)]
struct PkceVerifier {
    code_verifier: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// OAuth Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub id_token: Option<String>,
}

/// 生成 XOAUTH2 认证字符串
///
/// 格式: user={email}\x01auth=Bearer {access_token}\x01\x01
/// 然后进行 base64 编码
pub fn generate_xoauth2_string(email: &str, access_token: &str) -> String {
    let auth_string = format!(
        "user={}\x01auth=Bearer {}\x01\x01",
        email, access_token
    );
    BASE64.encode(auth_string)
}

/// 验证 access token 格式
pub fn validate_access_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("access_token 不能为空".to_string());
    }

    if token.len() < 10 {
        return Err("access_token 长度不足".to_string());
    }

    // 检查是否包含非法字符
    if token.contains(' ') || token.contains('\n') || token.contains('\r') {
        return Err("access_token 包含非法字符".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_xoauth2_string() {
        let xoauth2 = generate_xoauth2_string("user@example.com", "test_token");

        // 验证是 base64 编码
        assert!(xoauth2.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));

        // 验证解码后格式正确
        let decoded = BASE64.decode(&xoauth2).unwrap();
        let decoded_str = String::from_utf8(decoded).unwrap();
        assert!(decoded_str.starts_with("user=user@example.com\x01auth=Bearer test_token\x01"));
    }

    #[test]
    fn test_validate_access_token() {
        assert!(validate_access_token("valid_token_123").is_ok());
        assert!(validate_access_token("").is_err());
        assert!(validate_access_token("short").is_err());
        assert!(validate_access_token("token with space").is_err());
        assert!(validate_access_token("token\nwith\nnewline").is_err());
    }

    #[test]
    fn test_pkce_verifier_store() {
        let store = PkceVerifierStore::new();

        let (verifier, challenge) = store.generate_and_store("test_state").unwrap();

        assert!(!verifier.is_empty());
        assert!(!challenge.is_empty());
        assert_ne!(verifier, challenge); // 应该不同

        let retrieved = store.take("test_state");
        assert_eq!(retrieved, Some(verifier));

        // 再次获取应该返回 None
        let retrieved_again = store.take("test_state");
        assert!(retrieved_again.is_none());
    }

    #[test]
    fn test_create_code_challenge() {
        let verifier = "test_verifier_123";
        let challenge = PkceVerifierStore::create_code_challenge(verifier);

        // challenge 应该是 base64 编码的
        assert!(challenge.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
        assert!(!challenge.contains('='));

        // 相同的 verifier 应该生成相同的 challenge
        let challenge2 = PkceVerifierStore::create_code_challenge(verifier);
        assert_eq!(challenge, challenge2);
    }
}
