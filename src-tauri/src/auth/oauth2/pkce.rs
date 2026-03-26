//! PKCE (Proof Key for Code Exchange) 工具
//!
//! 实现 RFC 7636 PKCE 扩展，用于 OAuth 2.0 公共客户端的安全授权

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rand::Rng;
use rand::distributions::Alphanumeric;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::info;

/// PKCE 验证器存储
///
/// 临时存储 OAuth 流程中的 PKCE code_verifier
pub struct PkceVerifierStore {
    verifiers: Mutex<HashMap<String, PkceVerifier>>,
}

impl PkceVerifierStore {
    /// 创建新的存储实例
    pub fn new() -> Self {
        Self {
            verifiers: Mutex::new(HashMap::new()),
        }
    }

    /// 生成并存储新的 PKCE 验证器
    ///
    /// # 参数
    ///
    /// - `state`: OAuth state 参数（用作存储键）
    ///
    /// # 返回
    ///
    /// 返回 (code_verifier, code_challenge) 元组
    pub fn generate_and_store(&self, state: &str) -> Result<(String, String), String> {
        // 生成 code_verifier (43-128 个字符)
        let code_verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(128)
            .map(char::from)
            .collect();

        // 生成 code_challenge (SHA256 + base64url)
        let code_challenge = create_code_challenge(&code_verifier);

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
    ///
    /// # 参数
    ///
    /// - `state`: OAuth state 参数
    ///
    /// # 返回
    ///
    /// 返回 code_verifier（如果存在），否则返回 None
    pub fn take(&self, state: &str) -> Option<String> {
        self.verifiers.lock().ok()?.remove(state).map(|v| {
            info!("移除 PKCE 验证器: state={}", state);
            v.code_verifier
        })
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

/// 创建 code_challenge
///
/// # 参数
///
/// - `code_verifier`: PKCE code verifier
///
/// # 返回
///
/// 返回 base64url 编码的 code_challenge
///
/// # 示例
///
/// ```rust
/// use postium_mail::auth::oauth2::create_code_challenge;
///
/// let verifier = "test_verifier_123";
/// let challenge = create_code_challenge(verifier);
/// assert!(challenge.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
/// ```
pub fn create_code_challenge(code_verifier: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();

    // Base64 URL-safe encoding
    BASE64
        .encode(hash)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

/// 验证 access token 格式
///
/// # 参数
///
/// - `token`: access token 字符串
///
/// # 返回
///
/// 成功返回 Ok(())，失败返回错误信息
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
        let challenge = create_code_challenge(verifier);

        // challenge 应该是 base64 编码的
        assert!(
            challenge
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        );
        assert!(!challenge.contains('='));

        // 相同的 verifier 应该生成相同的 challenge
        let challenge2 = create_code_challenge(verifier);
        assert_eq!(challenge, challenge2);
    }

    #[test]
    fn test_pkce_verifier_store_default() {
        let store = PkceVerifierStore::default();
        let (verifier, _) = store.generate_and_store("state").unwrap();
        assert!(!verifier.is_empty());
    }

    #[test]
    fn test_code_challenge_is_url_safe() {
        let verifier = "abcdefghijklmnopqrstuvwxyz123456";
        let challenge = create_code_challenge(verifier);

        // 确保不包含需要 URL 编码的字符
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        assert!(!challenge.contains('='));
    }
}
