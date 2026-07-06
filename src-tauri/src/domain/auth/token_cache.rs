//!
//! # OAuth2 access_token 内存缓存 (OAuth2 access_token In-Memory Cache)
//!
//! 本模块提供一个以用户邮箱为键的轻量级 access_token 内存缓存。
//!
//! 设计要点：
//! - **仅缓存于内存**：access_token 不落盘，应用重启后通过 refresh_token 重新获取，
//!   兼顾性能与安全（refresh_token 由系统 Keyring 加密存储）。
//! - **提前过期判定**：在 [`TokenCache::get`] 中提前 60 秒将 token 视为已过期，
//!   避免使用一个「尚在有效期内、但在请求往返过程中刚刚过期」的 token，
//!   从而防止因 token 在飞行中过期导致的请求 401 失败与不必要的重试。
//! - **线程安全**：内部使用 [`Mutex`] 包裹 [`HashMap`]，可在多线程下安全访问。
//!

use std::collections::HashMap;
use std::sync::Mutex;

/// 内存中的 access_token 缓存条目。
#[derive(Debug)]
pub struct CachedToken {
    /// OAuth2 access_token 字符串。
    pub access_token: String,
    /// access_token 的过期时间，Unix 时间戳（秒）。
    pub expires_at: i64,
}

/// 以 email 为 key 的 access_token 缓存。
pub struct TokenCache {
    tokens: Mutex<HashMap<String, CachedToken>>,
}

impl TokenCache {
    /// 创建一个空的 access_token 缓存。
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// 缓存指定邮箱的 access_token。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱，作为缓存键。
    /// - `access_token`: OAuth2 access_token 字符串。
    /// - `expires_at`: access_token 的过期时间（Unix 秒）。若该邮箱已有缓存则覆盖。
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

    /// 获取尚未过期的 access_token。
    ///
    /// 采用**提前 60 秒判定过期**的策略：若当前时间已进入过期时间前最后 60 秒，
    /// 即视为已过期返回 `None`。
    ///
    /// # 为什么提前 60 秒
    ///
    /// 一个刚取出的 token 在真正发往服务器时，可能因网络往返消耗数秒甚至数十秒。
    /// 若严格按 `expires_at` 判定，可能出现 token 在「判定有效」后、
    /// 「请求到达服务器」前这一窗口内恰好过期，导致 401 失败与额外重试。
    /// 预留 60 秒安全余量可吸收这种飞行时间，提高请求成功率。
    ///
    /// # 参数
    ///
    /// - `email`: 用户邮箱，作为缓存键。
    ///
    /// # 返回
    ///
    /// - `Some(token)`: 存在缓存且未（提前）过期。
    /// - `None`: 无缓存，或缓存已（提前）过期，调用方应使用 refresh_token 重新获取。
    pub fn get(&self, email: &str) -> Option<String> {
        let tokens = self.tokens.lock().unwrap();
        let cached = tokens.get(email)?;
        let now = chrono::Utc::now().timestamp();
        if now >= cached.expires_at - 60 {
            return None;
        }
        Some(cached.access_token.clone())
    }

    /// 移除指定邮箱的缓存条目（例如登出或强制刷新时调用）。
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
