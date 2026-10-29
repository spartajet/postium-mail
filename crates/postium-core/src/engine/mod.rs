//! IMAP 引擎：每账号 Actor + 连接池 + 能力发现。
//! 设计文档：docs/superpowers/specs/2026-10-29-imap-engine-design.md

pub mod capabilities;
pub use capabilities::CapabilitySet;

pub mod transport;
pub use transport::{ImapSession, RealSession};

pub mod factory;
pub mod pool;
pub use factory::{RealSessionFactory, SessionFactory};
pub use pool::{ConnectionPool, PooledSession, PoolConnect};
