//! IMAP 引擎：每账号 Actor + 连接池 + 能力发现。
//! 设计文档：docs/superpowers/specs/2026-10-29-imap-engine-design.md

pub mod capabilities;
pub use capabilities::CapabilitySet;

pub mod transport;
pub use transport::{ImapSession, RealSession};

pub mod factory;
pub mod pool;

pub mod commands;
pub mod actor;
pub mod idle;
pub mod registry;
pub mod access;
pub mod sync;
pub use registry::ensure_engine;
pub use access::{EngineAccess, RegistryEngineAccess};
pub use factory::{RealSessionFactory, SessionFactory};
pub use pool::{ConnectionPool, PooledSession, PoolConnect};
pub use actor::{AccountEngine, EngineHandle};
pub use commands::{EngineCommand, EngineEvent};
pub use idle::{IdleConnect, IdleSupervisor, MAX_WATCHED, run_idle_loop, should_degrade, watch_changes};
