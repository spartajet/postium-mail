//! Tauri Command 模块
//!
//! 包含所有 Tauri IPC 命令的定义

#![allow(deprecated)]

mod account;
mod auth;
mod connection;
mod email;
mod flow_engine;

mod oauth;
mod provider;
mod sync;

pub use account::*;
pub use auth::*;
// pub use connection::*;
pub use email::*;
pub use oauth::*;
pub use provider::*;
pub use sync::*;

// 导出 OAuth 相关类型供 lib.rs 使用
pub use oauth::OAuthFlowResult;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex as StdMutex};

use crate::auth::OAuthSessionManager;

/// 全局数据库连接状态
pub struct DatabaseState(pub Arc<StdMutex<DbConn>>);

impl DatabaseState {
    pub fn clone_conn(&self) -> DbConn {
        let guard = self.0.lock().unwrap_or_else(|e| {
            tracing::error!("数据库 Mutex 已被污染: {}", e);
            e.into_inner()
        });
        (*guard).clone()
    }
}

/// Keyring 密钥环状态
pub struct KeyringState {
    pub app_handle: tauri::AppHandle,
}

/// AuthManager 状态
///
/// 管理认证管理器的全局单例
pub struct AuthManagerState(pub Arc<crate::auth::AuthManager>);

impl AuthManagerState {
    /// 克隆 AuthManager 实例
    pub fn clone_manager(&self) -> Arc<crate::auth::AuthManager> {
        Arc::clone(&self.0)
    }
}

/// ProviderPool 状态
///
/// 管理服务商池的全局单例
pub struct ProviderPoolState(pub Arc<crate::providers::ProviderPool>);

impl ProviderPoolState {
    /// 克隆 ProviderPool 实例
    pub fn clone_pool(&self) -> Arc<crate::providers::ProviderPool> {
        Arc::clone(&self.0)
    }
}

/// OAuthSessionManager 状态
///
/// 管理 OAuth 会话管理器的全局单例
pub struct OAuthSessionManagerState(pub Arc<OAuthSessionManager>);

impl Clone for OAuthSessionManagerState {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
