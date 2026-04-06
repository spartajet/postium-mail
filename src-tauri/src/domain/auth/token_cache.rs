use std::collections::HashMap;
use std::sync::Mutex;

/// 内存中的 access_token 缓存
#[derive(Debug)]
pub struct CachedToken {
    pub access_token: String,
    /// Unix 时间戳（秒）
    pub expires_at: i64,
}

/// 以 email 为 key 的 access_token 缓存
pub struct TokenCache {
    tokens: Mutex<HashMap<String, CachedToken>>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// 缓存 access_token
    pub fn store(&self, email: &str, access_token: String, expires_at: i64) {
        let mut tokens = self.tokens.lock().unwrap();
        tokens.insert(
            email.to_string(),
            CachedToken {
                access_token,
                expires_at,
            },
        );
    }

    /// 获取未过期的 access_token（提前 60 秒判定过期）
    pub fn get(&self, email: &str) -> Option<String> {
        let tokens = self.tokens.lock().unwrap();
        let cached = tokens.get(email)?;
        let now = chrono::Utc::now().timestamp();
        if now >= cached.expires_at - 60 {
            return None;
        }
        Some(cached.access_token.clone())
    }

    /// 移除缓存
    pub fn remove(&self, email: &str) {
        let mut tokens = self.tokens.lock().unwrap();
        tokens.remove(email);
    }
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}
