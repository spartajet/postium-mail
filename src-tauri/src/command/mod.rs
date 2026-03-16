//! Tauri Command 模块
//!
//! 包含所有 Tauri IPC 命令的定义

mod account;
mod connection;
mod email;
mod folder;
mod oauth;
mod sync;

pub use account::*;
pub use connection::*;
pub use email::*;
pub use folder::*;
pub use oauth::*;
pub use sync::*;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex};

use crate::services;

/// 全局数据库连接状态
pub struct DatabaseState(pub Arc<Mutex<DbConn>>);

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
