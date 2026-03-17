//! 密码认证处理器
//!
//! 处理用户名/密码认证

use crate::error::Result;

/// 密码认证处理器
#[allow(unused_variables)]
pub struct PasswordAuthHandler;

impl PasswordAuthHandler {
    pub fn new() -> Self {
        Self
    }

    /// 验证密码
    pub async fn verify(&self, email: &str, password: &str) -> Result<bool> {
        // TODO: 实现密码验证
        tracing::info!("验证密码: {}", email);
        Ok(true)
    }
}

impl Default for PasswordAuthHandler {
    fn default() -> Self {
        Self::new()
    }
}
