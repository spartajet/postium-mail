//! Tauri Command 模块
//!
//! 包含所有 Tauri IPC 命令的定义

#![allow(deprecated)]

mod account;
mod connection;
mod email;
mod flow_engine;
mod folder;
mod oauth;
mod sync;

pub use account::*;
pub use connection::*;
pub use email::*;
pub use flow_engine::*;
pub use folder::*;
pub use oauth::*;
pub use sync::*;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;

use crate::services;

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

/// OAuth 服务状态
pub struct OAuthState(pub services::oauth_service::OAuthService);

/// FlowEngine 状态
///
/// 管理流程引擎的全局单例
pub struct FlowEngineState(pub Arc<Mutex<crate::engine::FlowEngine>>);

impl FlowEngineState {
    /// 克隆引擎实例
    pub fn clone_engine(&self) -> Arc<Mutex<crate::engine::FlowEngine>> {
        Arc::clone(&self.0)
    }
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
