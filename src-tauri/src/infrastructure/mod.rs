//!
//! # 基础设施层 (Infrastructure Layer)
//!
//! 本模块实现了应用程序的基础设施关注点，为上层（Service 和 Domain 层）提供技术支持。
//! 基础设施层负责处理与外部系统的交互，包括数据库、网络协议、文件系统等。
//!
//! ## 模块组织
//!
//! 本模块包含以下子模块：
//!
//! ### auth - 认证基础设施
//!
//! 提供 OAuth2 认证的底层实现，包括：
//! - OAuth2 授权流程管理
//! - 授权码交换
//! - 访问令牌和刷新令牌管理
//! - 本地 HTTP 回调服务器
//!
//! 主要类型：
//! - `OAuth2Manager`: OAuth2 认证管理器，处理完整的 OAuth2 流程
//! - `OAuth2AuthUrl`: 授权 URL 信息
//! - `OAuth2PollResult`: 授权轮询结果
//!
//! ### protocols - 通信协议
//!
//! 实现邮件相关的网络协议，包括：
//! - IMAP 协议（邮件接收）
//! - SMTP 协议（邮件发送）
//! - 连接池管理
//! - TLS/SSL 安全连接
//!
//! 主要功能：
//! - 邮件收取（IMAP）
//! - 邮件发送（SMTP）
//! - 文件夹管理
//! - 邮件搜索
//!
//! ### storage - 数据存储
//!
//! 负责数据的持久化存储，包括：
//! - SQLite 数据库管理（通过 SeaORM）
//! - 数据库迁移
//! - 实体定义（Entity）
//! - 仓库模式（Repository）
//! - 全文搜索（FTS）
//!
//! 主要组件：
//! - `database`: 数据库连接和初始化
//! - `entities`: 数据库表对应的实体模型
//! - `repository`: 数据访问层，封装 CRUD 操作
//! - `search`: 全文搜索功能
//!
//! ## 设计原则
//!
//! 1. **依赖倒置**: 基础设施层实现 Domain 层定义的接口，而不是反过来
//! 2. **关注点分离**: 每个子模块只负责一个特定的技术关注点
//! 3. **可替换性**: 基础设施实现可以替换（例如从 SQLite 换到 PostgreSQL）而不影响业务逻辑
//! 4. **错误转换**: 将底层技术错误转换为领域层的统一错误类型（MailError）
//!
//! ## 依赖关系
//!
//! ```text
//! Infrastructure Layer
//! ├── auth (认证)
//! │   ├── 依赖: reqwest (HTTP 客户端)
//! │   ├── 依赖: oauth2 (OAuth2 库)
//! │   └── 依赖: domain/auth (认证接口)
//! ├── protocols (协议)
//! │   ├── 依赖: imap (IMAP 协议)
//! │   ├── 依赖: lettre (SMTP 协议)
//! │   └── 依赖: tokio (异步运行时)
//! └── storage (存储)
//!     ├── 依赖: sea-orm (ORM)
//!     ├── 依赖: rusqlite (SQLite FTS)
//!     └── 依赖: migration (数据库迁移)
//! ```
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! // 初始化数据库
//! let db = database::init_database(&data_dir).await?;
//!
//! // 创建 OAuth2 管理器
//! let oauth2_manager = OAuth2Manager::new(auth.clone(), db.clone());
//!
//! // 通过仓库模式访问数据
//! let accounts = account_repo::list(&db).await?;
//! ```

// ========== 子模块声明 ==========

/// 认证基础设施模块
///
/// 提供 OAuth2 认证的底层实现，包括授权流程、令牌管理和回调处理。
pub mod auth;

/// 通信协议模块
///
/// 实现 IMAP 和 SMTP 网络协议，用于邮件的收发操作。
pub mod protocols;

/// 数据存储模块
///
/// 提供数据持久化功能，包括数据库管理、实体定义和仓库模式。
pub mod storage;
