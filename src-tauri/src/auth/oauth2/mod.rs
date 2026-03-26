//! OAuth2 认证模块
//!
//! 统一管理所有 OAuth2 相关功能，包括协议处理、会话管理、客户端调用和工具函数。
//!
//! # 模块结构
//!
//! ```text
//! oauth2/
//! ├── mod.rs         # 模块声明和导出
//! ├── protocol.rs    # OAuth2 协议处理（授权 URL、Token 交换、刷新）
//! ├── session.rs     # OAuth 会话管理（状态跟踪、验证、清理）
//! ├── client.rs      # OAuth 客户端（认证流程编排）
//! ├── pkce.rs        # PKCE 工具（code_verifier/challenge 生成）
//! └── types.rs       # OAuth 相关类型定义
//! ```
//!
//! # 职责划分
//!
//! | 模块 | 职责 |
//!------|------|
//! | `protocol` | OAuth2 协议底层实现（HTTP 通信、PKCE 集成） |
//! | `session` | 授权流程会话状态管理（CSRF 防护、过期清理） |
//! | `client` | OAuth 认证流程编排（协调 protocol、session、token） |
//! | `pkce` | PKCE 流程工具函数 |
//! | `types` | 公共类型定义（Token 响应、授权上下文） |
//!
//! # 使用流程
//!
//! ```text
//! 1. protocol.get_authorization_url()  → 生成授权 URL
//! 2. session.create_session()         → 创建会话记录
//! 3. 用户授权 → 回调
//! 4. session.verify_and_get_session() → 验证会话
//! 5. protocol.exchange_code()          → 交换 Token
//! 6. client.handle_callback()         → 完整流程编排
//! ```

mod protocol;
mod session;
mod client;
mod pkce;
mod types;

// 重新导出主要类型
pub use protocol::OAuthHandler;
pub use session::{OAuthSession, OAuthSessionManager, OAuthSessionStatus};
pub use client::OAuthClient;
pub use pkce::{PkceVerifierStore, create_code_challenge, validate_access_token};
pub use types::{AuthorizationContext, OAuthTokenResponse};
