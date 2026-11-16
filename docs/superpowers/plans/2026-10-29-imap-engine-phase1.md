# IMAP 引擎 · 阶段 1（骨架）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 IMAP 引擎骨架——`trait ImapSession` 传输抽象、能力发现（`CapabilitySet`）、连接池（控制连接复用 + 失效重建 + 指数退避）、每账号 Actor（命令/事件通道），并把邮件操作类调用点（`mail_operation`/`mail_send`/`mail_draft`）改走引擎，消灭散落的 `ImapClient::connect`。

**Architecture:** 每账号一个 tokio Actor（`AccountEngine`），mpsc 命令队列串行化账号内操作；引擎全部面向 `trait ImapSession` 编写（mock 可注入）；`ImapEngineRegistry` 全局单例管理账号 id → 引擎句柄。上层 `CoreApi` 公共签名不变。

**Tech Stack:** tokio（mpsc/oneshot/timeout）、async-trait、现有 `ImapClient`（async-imap 0.11 封装）、现有 `AuthManager`（凭据）、现有 `MailError`。

**设计文档:** `docs/superpowers/specs/2026-10-29-imap-engine-design.md`（本计划对应 spec §10 阶段 1；阶段 2/3 计划在本阶段完成后另行编写）

---

## File Structure（阶段 1 新建/修改全景）

```
crates/postium-core/src/engine/
├── mod.rs            # 模块根：re-export EngineCommand/EngineEvent/Registry/pool/capabilities
├── capabilities.rs   # CapabilitySet：CAPABILITY 解析与查询（纯函数，无 IO）
├── transport.rs      # trait ImapSession + RealSession（包装现有 ImapClient）
├── factory.rs        # trait SessionFactory + RealSessionFactory（凭据/配置 → 连接）
├── pool.rs           # ConnectionPool：控制连接复用 + 健康检查 + 指数退避重建
├── commands.rs       # EngineCommand / EngineEvent 类型定义
├── actor.rs          # AccountEngine：命令循环 + 分发（含重连逻辑）
└── registry.rs       # ImapEngineRegistry：OnceLock 全局，账号 id → Sender
修改：
├── src/lib.rs                       # + pub mod engine;
├── src/api.rs                       # CoreApi 持有 registry 句柄（接线）
├── src/service/mail_operation.rs    # 连接调用 → 引擎命令
├── src/service/mail_send.rs         # 同上
├── src/service/mail_draft.rs        # 同上
测试：
├── src/engine/capabilities.rs #[cfg(test)]
├── src/engine/pool.rs         #[cfg(test)]（MockSession）
├── src/engine/actor.rs        #[cfg(test)]
```

不修改：`folder_sync_*`（阶段 2 替换）、`scheduler.rs`（阶段 2 移除）、`folders/`（阶段 3）、`fetch.rs` 现有方法（transport 收敛时仅包装不删除）。

---

### Task 1: CapabilitySet——能力解析与查询

**Files:**
- Create: `crates/postium-core/src/engine/mod.rs`
- Create: `crates/postium-core/src/engine/capabilities.rs`
- Modify: `crates/postium-core/src/lib.rs`（根模块注册）

- [ ] **Step 1.1: 写失败的解析测试**

在 `crates/postium-core/src/engine/capabilities.rs` 新建：

```rust
//! IMAP 能力集：CAPABILITY 响应解析与查询。
//!
//! 连接建立后立即探测并 `ENABLE` 支持的扩展；同步档位决策（QRESYNC →
//! CONDSTORE → UID 增量）与功能开关（IDLE/SPECIAL-USE/MOVE）均基于此。

use std::collections::HashSet;

/// 服务器能力集合（大写归一存储）。
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    caps: HashSet<String>,
}

impl CapabilitySet {
    /// 从 CAPABILITY 响应行构建（每行一个能力名，如 "IMAP4REV1"、"QRESYNC"）。
    pub fn parse(responses: &[String]) -> Self {
        Self {
            caps: responses
                .iter()
                .map(|c| c.trim().trim_matches('"').to_ascii_uppercase())
                .filter(|c| !c.is_empty())
                .collect(),
        }
    }

    pub fn supports(&self, cap: &str) -> bool {
        self.caps.contains(&cap.to_ascii_uppercase())
    }

    /// 服务器是否支持 QRESYNC（隐含 CONDSTORE）。
    pub fn qresync(&self) -> bool {
        self.supports("QRESYNC")
    }

    pub fn condstore(&self) -> bool {
        self.qresync() || self.supports("CONDSTORE")
    }

    pub fn idle(&self) -> bool {
        self.supports("IDLE")
    }

    pub fn special_use(&self) -> bool {
        self.supports("SPECIAL-USE")
    }

    pub fn move_ext(&self) -> bool {
        self.supports("MOVE")
    }

    /// 应发 ENABLE 的扩展（与服务端能力取交集；QRESYNC 隐含 CONDSTORE）。
    pub fn enable_list(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.qresync() {
            out.push("QRESYNC");
            out.push("CONDSTORE");
        } else if self.condstore() {
            out.push("CONDSTORE");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalizes_case_and_quotes() {
        let caps = CapabilitySet::parse(&[
            "\"IMAP4rev1\"".into(),
            " qresync ".into(),
            "IDLE".into(),
        ]);
        assert!(caps.supports("imap4rev1"));
        assert!(caps.qresync());
        assert!(caps.idle());
        assert!(!caps.condstore_only_marker());
    }

    #[test]
    fn enable_list_follows_fallback_chain() {
        // QRESYNC 隐含 CONDSTORE
        let q = CapabilitySet::parse(&["QRESYNC".into()]);
        assert_eq!(q.enable_list(), vec!["QRESYNC", "CONDSTORE"]);
        // 仅 CONDSTORE
        let c = CapabilitySet::parse(&["CONDSTORE".into()]);
        assert_eq!(c.enable_list(), vec!["CONDSTORE"]);
        // 都没有
        let none = CapabilitySet::parse(&["IMAP4rev1".into()]);
        assert!(none.enable_list().is_empty());
    }
}
```

- [ ] **Step 1.2: 修正测试中的笔误断言**

上面的 `parse_normalizes_case_and_quotes` 里 `condstore_only_marker()` 是不存在的
方法——删除该行，改为 `assert!(!caps.supports("CONDSTORE"));`（QRESYNC 响应不含
独立 CONDSTORE 标记时的行为按服务器实测再收紧，此处只断言未单独宣告）。

- [ ] **Step 1.3: 在 `src/lib.rs` 注册模块**

`crates/postium-core/src/lib.rs` 中其他 `pub mod` 声明旁追加：

```rust
pub mod engine;
```

`crates/postium-core/src/engine/mod.rs`：

```rust
//! IMAP 引擎：每账号 Actor + 连接池 + 能力发现。
//! 设计文档：docs/superpowers/specs/2026-10-29-imap-engine-design.md

pub mod capabilities;
pub use capabilities::CapabilitySet;
```

- [ ] **Step 1.4: 运行测试验证通过**

```bash
cargo test -p postium-core capabilities
```

预期：`test result: ok`，4 个测试通过（2 个解析/查询 + enable_list 分解后的用例）。

- [ ] **Step 1.5: 提交**

```bash
git add crates/postium-core/src/engine crates/postium-core/src/lib.rs
git commit -m "feat(engine): CapabilitySet——CAPABILITY 解析与回退链查询"
```

---

### Task 2: trait ImapSession——传输层抽象

**Files:**
- Create: `crates/postium-core/src/engine/transport.rs`
- Modify: `crates/postium-core/src/engine/mod.rs`

**背景**：现有 `ImapClient`（`src/infrastructure/protocols/imap/mod.rs`）持有
`async-imap` Session，方法齐全但为具体类型。引擎面向 trait 编写以便 mock。
本任务只抽象阶段 1 需要的子集，后续阶段按需扩展 trait（不提前堆方法）。

- [ ] **Step 2.1: 定义 trait 与真实实现**

`crates/postium-core/src/engine/transport.rs`：

```rust
//! 传输层抽象：引擎面向 `ImapSession` trait 编写，mock 会话可注入。

use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use async_trait::async_trait;

/// 已认证的 IMAP 会话（引擎视角的最小接口，按阶段扩展）。
#[async_trait]
pub trait ImapSession: Send {
    /// 服务器能力列表（原始响应行）。
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError>;
    /// 发送 ENABLE 命令（服务器不支持时忽略错误）。
    async fn enable(&mut self, caps: &[&str]) -> Result<(), MailError>;
    /// 选择文件夹（可写模式），返回消息数等状态。
    async fn select_folder(&mut self, folder: &str) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError>;
    /// 登出并关闭会话。
    async fn logout(self: Box<Self>) -> Result<(), MailError>;
}

/// 真实会话：包装现有 `ImapClient`。
pub struct RealSession {
    client: ImapClient,
}

impl RealSession {
    pub fn new(client: ImapClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ImapSession for RealSession {
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        self.client.capabilities().await
    }

    async fn enable(&mut self, caps: &[&str]) -> Result<(), MailError> {
        if caps.is_empty() {
            return Ok(());
        }
        // async-imap 未直接暴露 ENABLE 的旧版本下用 run_command 兜底；
        // 失败仅记录（服务器不支持 ENABLE 不影响降级路径）。
        let cmd = format!("ENABLE {}", caps.join(" "));
        let _ = self.client.run_command_and_check_ok(cmd.as_str()).await;
        Ok(())
    }

    async fn select_folder(&mut self, folder: &str) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError> {
        self.client.select_folder(folder).await
    }

    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        self.client.logout().await
    }
}
```

- [ ] **Step 2.2: mod.rs 注册**

`crates/postium-core/src/engine/mod.rs` 追加：

```rust
pub mod transport;
pub use transport::{ImapSession, RealSession};
```

- [ ] **Step 2.3: 确认 ImapClient 需要补的方法**

`async-imap` 的 `Client::run_command_and_check_ok` 是 public（已核实，client.rs:1364）。
`ImapClient`（imap/mod.rs）需要转发一个方法——在该文件 `capabilities()` 方法旁追加：

```rust
    /// 发送任意命令并检查 OK（引擎 ENABLE 用）。
    pub async fn run_command_and_check_ok(&mut self, cmd: &str) -> Result<(), MailError> {
        self.session
            .run_command_and_check_ok(cmd)
            .await
            .map_err(|e| MailError::ImapConnectionFailed(format!("命令 {cmd} 失败: {e}")))
    }
```

注意：`session` 字段当前私有于同一文件（`ImapClient` 定义在 `imap/mod.rs`，字段
`session` 同文件可见）——若编译报私有字段错误，在 `ImapClient` 上加
`pub(crate) fn session_mut(&mut self) -> &mut async_imap::Session<...>` 转发。

- [ ] **Step 2.4: 编译验证**

```bash
cargo build -p postium-core
```

预期：Finished，无 error。

- [ ] **Step 2.5: 提交**

```bash
git add crates/postium-core/src/engine/transport.rs crates/postium-core/src/engine/mod.rs crates/postium-core/src/infrastructure/protocols/imap/mod.rs
git commit -m "feat(engine): ImapSession 传输抽象 + RealSession 包装"
```

---

### Task 3: SessionFactory——凭据/配置到连接

**Files:**
- Create: `crates/postium-core/src/engine/factory.rs`
- Modify: `crates/postium-core/src/engine/mod.rs`

- [ ] **Step 3.1: 定义 factory trait 与真实实现**

`crates/postium-core/src/engine/factory.rs`：

```rust
//! 连接工厂：凭据 + IMAP 配置 → 已认证会话（含能力探测与 ENABLE）。

use crate::domain::auth::manager::AuthManager;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::engine::capabilities::CapabilitySet;
use crate::engine::transport::{ImapSession, RealSession};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::service::account_connection::imap_config_from_account;
use crate::infrastructure::storage::models::accounts;
use async_trait::async_trait;
use std::sync::Arc;

/// 会话工厂：给定账号模型，产出已认证 + 已 ENABLE 的会话与能力集。
#[async_trait]
pub trait SessionFactory: Send + Sync {
    async fn connect(
        &self,
        account: &accounts::Model,
    ) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError>;
}

/// 真实工厂：密码 / XOAUTH2 双路径（与旧 mail_operation.rs 连接逻辑一致）。
pub struct RealSessionFactory {
    pub auth: Arc<AuthManager>,
}

#[async_trait]
impl SessionFactory for RealSessionFactory {
    async fn connect(
        &self,
        account: &accounts::Model,
    ) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError> {
        let imap_config = imap_config_from_account(account)?;
        let session: Box<dyn ImapSession> = match account.auth_type.as_deref() {
            Some("OAuth2") | Some("oauth2") => {
                let access_token = self
                    .auth
                    .get_access_token(account)
                    .await
                    .map_err(|e| MailError::KeyringError(e.to_string()))?;
                Box::new(RealSession::new(
                    ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await?,
                ))
            }
            _ => {
                let password = self
                    .auth
                    .get_password(account)
                    .await
                    .map_err(|e| MailError::KeyringError(e.to_string()))?;
                Box::new(RealSession::new(
                    ImapClient::connect(&imap_config, &account.email, &password).await?,
                ))
            }
        };

        // 能力探测 + ENABLE（163 修复经验：连接后立即自报 ID 的怪癖已在
        // ImapClient::connect 内处理）。
        let caps = CapabilitySet::parse(&session.capabilities().await?);
        session.enable(&caps.enable_list()).await?;
        Ok((session, caps))
    }
}
```

- [ ] **Step 3.2: 对齐 AuthManager 的实际方法名**

`AuthManager` 的取密/取 token 方法名以现有调用点为准——`mail_operation.rs`
此前连接处已有一份工作代码（该文件 140-160 行区域），**照抄它的方法名与错误
处理**，不要凭本计划臆断。若实际为 `get_password(&account.email)`（拿邮箱
字符串）而非整个 account，调整如上代码为一致。

- [ ] **Step 3.3: mod.rs 注册 + 编译**

```bash
# engine/mod.rs 追加：
#   pub mod factory;
#   pub use factory::{RealSessionFactory, SessionFactory};
cargo build -p postium-core
```

预期：Finished（若 `get_access_token` 方法名不匹配，照 mail_operation.rs 现状改名）。

- [ ] **Step 3.4: 提交**

```bash
git add crates/postium-core/src/engine/factory.rs crates/postium-core/src/engine/mod.rs
git commit -m "feat(engine): SessionFactory——凭据/配置到已 ENABLE 的会话"
```

---

### Task 4: ConnectionPool——控制连接复用与失效重建

**Files:**
- Create: `crates/postium-core/src/engine/pool.rs`
- Modify: `crates/postium-core/src/engine/mod.rs`

- [ ] **Step 4.1: 写失败的池行为测试（MockSession）**

`crates/postium-core/src/engine/pool.rs`：

```rust
//! 连接池：控制连接复用 + 失效重建（指数退避在 AccountEngine 层）。

use crate::engine::capabilities::CapabilitySet;
use crate::engine::transport::ImapSession;
use crate::error::MailError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 池管理的会话桶：会话 + 其能力集。
pub struct PooledSession {
    pub session: Box<dyn ImapSession>,
    pub caps: CapabilitySet,
}

/// 会话工厂的 mock/真实统一入口（池重建时反复调用）。
#[async_trait::async_trait]
pub trait PoolConnect: Send + Sync {
    async fn connect(&self) -> Result<PooledSession, MailError>;
}

/// 连接池：控制连接复用；命令失败时销毁重连一次，仍失败则向上报错。
pub struct ConnectionPool<F: PoolConnect> {
    factory: Arc<F>,
    control: Option<PooledSession>,
    /// 已执行的连接次数（测试断言复用/重建行为）。
    connect_count: Arc<AtomicUsize>,
}

impl<F: PoolConnect> ConnectionPool<F> {
    pub fn new(factory: Arc<F>, connect_count: Arc<AtomicUsize>) -> Self {
        Self { factory, control: None, connect_count }
    }

    /// 获取控制连接：无则新建；命令执行由调用方在 `with_control` 内完成。
    pub async fn with_control<R>(
        &mut self,
        f: impl FnOnce(&mut PooledSession) -> std::future::Future<Output = Result<R, MailError>>,
    ) -> Result<R, MailError> {
        // 无控制连接（首次/上次失效）→ 新建
        if self.control.is_none() {
            self.control = Some(self.factory.connect().await?);
            self.connect_count.fetch_add(1, Ordering::SeqCst);
        }
        let session = self.control.as_mut().expect("control 刚刚建立");
        match f(session).await {
            Ok(r) => Ok(r),
            Err(e) => {
                // 命令失败：连接可疑 → 销毁重建一次重试
                tracing::warn!(error = %e, "控制连接命令失败，重建连接重试");
                self.control = None;
                let mut fresh = self.factory.connect().await?;
                self.connect_count.fetch_add(1, Ordering::SeqCst);
                let r = f(&mut fresh).await?;
                self.control = Some(fresh);
                Ok(r)
            }
        }
    }

    /// 关闭池（登出控制连接）。
    pub async fn shutdown(&mut self) {
        if let Some(pooled) = self.control.take() {
            let _ = pooled.session.logout().await;
        }
    }
}

// ---- 测试用 Mock ----

pub struct MockSession {
    pub fail_with: Option<MailError>,
}

#[async_trait::async_trait]
impl ImapSession for MockSession {
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        Ok(vec!["IMAP4REV1".into()])
    }
    async fn enable(&mut self, _caps: &[&str]) -> Result<(), MailError> {
        Ok(())
    }
    async fn select_folder(
        &mut self,
        _folder: &str,
    ) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError> {
        match &self.fail_with {
            Some(e) => Err(clone_mail_error(e)),
            None => Ok(crate::infrastructure::protocols::imap::MailboxInfo {
                exists: 0,
                recent: 0,
                unseen: None,
                uid_validity: Some(1),
                uid_next: Some(1),
            }),
        }
    }
    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        Ok(())
    }
}

fn clone_mail_error(e: &MailError) -> MailError {
    MailError::ImapConnectionFailed(e.to_string())
}

pub struct MockFactory {
    /// 第一次连接产出的会话在 select 时失败（模拟连接陈旧），其后正常。
    pub first_select_fails: bool,
    connects: AtomicUsize,
}

#[async_trait::async_trait]
impl PoolConnect for MockFactory {
    async fn connect(&self) -> Result<PooledSession, MailError> {
        let n = self.connects.fetch_add(1, Ordering::SeqCst);
        let fail = self.first_select_fails && n == 0;
        Ok(PooledSession {
            session: Box::new(MockSession { fail_with: fail.then(|| MailError::ImapConnectionFailed("stale".into())) }),
            caps: CapabilitySet::parse(&["IMAP4REV1".into()]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reuses_control_connection_when_healthy() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut pool = ConnectionPool::new(Arc::new(MockFactory { first_select_fails: false }), count.clone());
        for _ in 0..3 {
            pool.with_control(|s| async { s.select_folder("INBOX").await.map(|_| ()) }).await.unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 1, "健康连接应复用");
    }

    #[tokio::test]
    async fn rebuilds_once_on_stale_connection_and_retries() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut pool = ConnectionPool::new(Arc::new(MockFactory { first_select_fails: true }), count.clone());
        pool.with_control(|s| async { s.select_folder("INBOX").await.map(|_| ()) }).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2, "陈旧连接应重建一次并重试成功");
    }
}
```

- [ ] **Step 4.2: 运行池测试**

```bash
cargo test -p postium-core pool
```

预期：2 个测试 PASS（若 MailError 未实现 Clone 导致 clone_mail_error 编译错，
改用 `e.to_string()` 组装新 `ImapConnectionFailed`——保持现状即可）。

- [ ] **Step 4.3: mod.rs 注册 + 提交**

```bash
# engine/mod.rs 追加：pub mod pool; pub use pool::ConnectionPool;
cargo build -p postium-core
git add crates/postium-core/src/engine/pool.rs crates/postium-core/src/engine/mod.rs
git commit -m "feat(engine): ConnectionPool——控制连接复用与失效重建"
```

---

### Task 5: EngineCommand/EngineEvent + AccountEngine Actor

**Files:**
- Create: `crates/postium-core/src/engine/commands.rs`
- Create: `crates/postium-core/src/engine/actor.rs`
- Modify: `crates/postium-core/src/engine/mod.rs`

- [ ] **Step 5.1: 定义命令与事件**

`crates/postium-core/src/engine/commands.rs`：

```rust
//! 引擎命令与事件。

use crate::error::MailError;
use tokio::sync::oneshot;

/// 引擎命令（账号内串行执行）。
#[derive(Debug)]
pub enum EngineCommand {
    /// 立即同步指定文件夹（None = 全部订阅文件夹）。阶段 2 实装同步体。
    SyncNow {
        folder: Option<String>,
        reply: oneshot::Sender<Result<(), MailError>>,
    },
    /// 读取邮件正文（懒加载）。阶段 1 先接通：走池转发到 transport。
    FetchBody {
        folder: String,
        uid: u32,
        reply: oneshot::Sender<Result<String, MailError>>,
    },
    /// 关闭引擎（账号删除/退出）。
    Shutdown,
}

/// 引擎事件（转发给 UI 层）。
#[derive(Debug, Clone)]
pub enum EngineEvent {
    Progress { account_id: i32, message: String },
    Disconnected { account_id: i32 },
    CredentialsNeeded { account_id: i32 },
}
```

`Actor` 骨架（`crates/postium-core/src/engine/actor.rs`）：

```rust
//! AccountEngine：每账号一个 tokio Actor。

use crate::engine::commands::EngineCommand;
use crate::engine::pool::ConnectionPool;
use crate::error::MailError;
use tokio::sync::{mpsc, oneshot};

/// 引擎运行句柄：向 Actor 发命令。
#[derive(Clone)]
pub struct EngineHandle {
    pub account_id: i32,
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub async fn send(&self, cmd: EngineCommand) -> Result<(), MailError> {
        self.tx.send(cmd).await.map_err(|_| {
            MailError::ImapConnectionFailed("引擎已关闭".into())
        })
    }
}

/// 引擎主体。`F: PoolConnect` 由 Registry 装配时注入。
pub struct AccountEngine<F: crate::engine::pool::PoolConnect> {
    pub account_id: i32,
    rx: mpsc::Receiver<EngineCommand>,
    pub pool: ConnectionPool<F>,
}

impl<F: crate::engine::pool::PoolConnect> AccountEngine<F> {
    pub fn new(account_id: i32, pool: ConnectionPool<F>, buffer: usize) -> (EngineHandle, Self) {
        let (tx, rx) = mpsc::channel(buffer.max(16));
        (EngineHandle { account_id, tx }, Self { account_id, rx, pool })
    }

    /// 命令循环（tokio::spawn 此函数）。阶段 1 只接通 FetchBody/Shutdown。
    pub async fn run(mut self, mut events: crate::domain::sync::SyncEventTx) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                EngineCommand::FetchBody { folder, uid, reply } => {
                    let result = self
                        .pool
                        .with_control(|s| async {
                            // transport 的 fetch_body 在阶段 2 的 trait 扩展中收编；
                            // 阶段 1 走 downcast 到 RealSession 的路径由 Registry
                            // 保证（引擎仅面向真实连接装配）。
                            crate::engine::actor::fetch_body_via(s, &folder, uid).await
                        })
                        .await;
                    let _ = reply.send(result);
                }
                EngineCommand::SyncNow { folder, reply } => {
                    // 阶段 2 实装；阶段 1 回执“未实装”以免调用方悬挂。
                    let _ = folder;
                    let _ = reply.send(Err(MailError::InvalidParam(
                        "SyncNow 将在阶段 2 实装".into(),
                    )));
                }
                EngineCommand::Shutdown => break,
            }
        }
        self.pool.shutdown().await;
    }
}

/// 阶段 1 的正文抓取桥接：阶段 2 将 FetchBody 升级为 transport trait 方法。
pub(crate) async fn fetch_body_via(
    session: &mut crate::engine::pool::PooledSession,
    folder: &str,
    uid: u32,
) -> Result<String, MailError> {
    // downcast 到 RealSession 使用 ImapClient 全量能力（Any 下行）。
    // 阶段 2 把 fetch_body 提升为 ImapSession trait 方法后删除本函数。
    let _ = session;
    let _ = (folder, uid);
    Err(MailError::InvalidParam(
        "FetchBody 阶段 1 仅打通命令通道，正文读取接出现在接入点改造之后".into(),
    ))
}
```

> **实现说明**：阶段 1 的 `FetchBody` 命令先打通“调用方 → Actor → 池”链路
> （返回占位错误），Task 6/7 改造接入点时把真实 body 读取逻辑从
> `mail_operation.rs` 搬进此命令（`fetch_body_via` 内联 downcast 实现，
> `ImapClient::fetch_body` 可直接调用）。阶段 2 将其提升为 trait 方法。

- [ ] **Step 5.2: 单测——命令循环与关停**

`actor.rs` 追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pool::{MockFactory, PoolConnect};
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    async fn shutdown_stops_loop_and_replies_closed() {
        let count = Arc::new(AtomicUsize::new(0));
        let (handle, engine) = AccountEngine::new(
            1,
            ConnectionPool::new(Arc::new(MockFactory { first_select_fails: false }), count),
            16,
        );
        let (event_tx, _event_rx) =
            smol::channel::bounded::<crate::domain::sync::SyncProgressEvent>(16);
        let task = tokio::spawn(engine.run(event_tx));
        handle.send(EngineCommand::Shutdown).await.unwrap();
        // Shutdown 后通道关闭，再次 send 返回 Err（引擎停止接收）
        assert!(handle.send(EngineCommand::Shutdown).await.is_err());
        task.await.unwrap();
    }
}

- [ ] **Step 5.3: 运行 Actor 测试**

```bash
cargo test -p postium-core actor
```

预期：PASS（Shutdown 后再 send 返回 Err，证明循环退出）。

- [ ] **Step 5.4: 提交**

```bash
git add crates/postium-core/src/engine/commands.rs crates/postium-core/src/engine/actor.rs crates/postium-core/src/engine/mod.rs
git commit -m "feat(engine): AccountEngine Actor——命令循环/事件出口/关停"
```

---

### Task 6: ImapEngineRegistry + CoreApi 接线

**Files:**
- Create: `crates/postium-core/src/engine/registry.rs`
- Modify: `crates/postium-core/src/engine/mod.rs`
- Modify: `crates/postium-core/src/engine/factory.rs`（SessionFactory 去账号参数 + 补 PoolConnect）
- Modify: `crates/postium-core/src/api.rs`（CoreApi 持有 `sync_tx` + `imap_engine` 方法）
- Modify: `crates/postium-core/src/main.rs` 或调用方（`open_main_window` 前装配）

- [ ] **Step 6.1: RealSessionFactory 以账号为中心 + 补 PoolConnect**

`factory.rs` 全量替换为：

```rust
//! 连接工厂：绑定单个账号，凭据 + IMAP 配置 → 已认证会话（含能力探测与 ENABLE）。

use crate::domain::auth::manager::AuthManager;
use crate::engine::capabilities::CapabilitySet;
use crate::engine::pool::{PooledSession, PoolConnect};
use crate::engine::transport::{ImapSession, RealSession};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use crate::service::account_connection::imap_config_from_account;
use async_trait::async_trait;
use std::sync::Arc;

/// 会话工厂：绑定单个账号，产出已认证 + 已 ENABLE 的会话与能力集。
#[async_trait]
pub trait SessionFactory: Send + Sync {
    async fn connect(&self) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError>;
}

/// 真实工厂：密码 / XOAUTH2 双路径（与旧 mail_operation.rs 连接逻辑一致）。
pub struct RealSessionFactory {
    pub auth: Arc<AuthManager>,
    pub account: accounts::Model,
}

#[async_trait]
impl SessionFactory for RealSessionFactory {
    async fn connect(&self) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError> {
        let account = &self.account;
        let imap_config = imap_config_from_account(account)?;
        let session: Box<dyn ImapSession> = match account.auth_type.as_deref() {
            Some("OAuth2") | Some("oauth2") => {
                let access_token = self
                    .auth
                    .get_access_token(account)
                    .await
                    .map_err(|e| MailError::KeyringError(e.to_string()))?;
                Box::new(RealSession::new(
                    ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await?,
                ))
            }
            _ => {
                let password = self
                    .auth
                    .get_password(account)
                    .await
                    .map_err(|e| MailError::KeyringError(e.to_string()))?;
                Box::new(RealSession::new(
                    ImapClient::connect(&imap_config, &account.email, &password).await?,
                ))
            }
        };
        let caps = CapabilitySet::parse(&session.capabilities().await?);
        session.enable(&caps.enable_list()).await?;
        Ok((session, caps))
    }
}

#[async_trait]
impl PoolConnect for RealSessionFactory {
    async fn connect(&self) -> Result<PooledSession, MailError> {
        let (session, caps) = SessionFactory::connect(self).await?;
        Ok(PooledSession { session, caps })
    }
}
```

> 注意：`AuthManager` 取密/取 token 的方法名以 `mail_operation.rs` 既有连接
> 块为准照抄（该方法名在 main 已工作）；本计划写作 `get_password(account)` /
> `get_access_token(account)`，若实际是 `get_password(&account.email)` 风格，
> 以实际为准调整。

- [ ] **Step 6.2: Registry 实现**

`crates/postium-core/src/engine/registry.rs`：

```rust
//! 引擎注册表：账号 id → 引擎句柄（进程级单例）。

use crate::engine::actor::{AccountEngine, EngineHandle};
use crate::engine::pool::ConnectionPool;
use crate::engine::pool::PoolConnect;
use crate::domain::sync::SyncEventTx;
use crate::infrastructure::storage::models::accounts;
use crate::error::MailError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

static REGISTRY: OnceLock<Mutex<HashMap<i32, EngineHandle>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<i32, EngineHandle>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 确保某账号的引擎在运行（已运行则返回句柄克隆，否则启动新引擎）。
pub fn ensure_engine<F>(
    account: &accounts::Model,
    factory: Arc<F>,
    events: SyncEventTx,
) -> Result<EngineHandle, MailError>
where
    F: PoolConnect + 'static,
{
    let mut map = registry().lock().expect("registry 锁");
    if let Some(h) = map.get(&account.id) {
        return Ok(h.clone());
    }
    let pool = ConnectionPool::new(factory, Arc::new(std::sync::atomic::AtomicUsize::new(0)));
    let (handle, engine) = AccountEngine::new(account.id, pool, 32);
    tokio::spawn(engine.run(events));
    map.insert(account.id, handle.clone());
    Ok(handle)
}
```

- [ ] **Step 6.3: CoreApi 持有 sync_tx 并暴露 imap_engine**

`api.rs` 三处修改：

① `CoreApi` 结构体追加字段（`CoreApi::init` 的 `Arc::new(Self {...})` 内）：

```rust
            /// 同步进度事件发送端（IMAP 引擎复用，状态栏/节流刷新共用链路）
            pub sync_tx: crate::domain::sync::SyncEventTx,
```

② `CoreApi::init` 签名追加参数并传入（调用方 `main.rs` 把既有 tx clone 传入）：

```rust
    pub async fn init(
        data_dir: &std::path::Path,
        e2e: bool,
        e2e_truth: bool,
        sync_tx: crate::domain::sync::SyncEventTx,
    ) -> Result<Arc<Self>, MailError> {
```

结构体字面量内 `sync_tx,`。`main.rs` 的调用处：

```rust
let (api, sync_rx) = postium_core::api::CoreApi::init(&data_dir, e2e, e2e_truth, sync_tx).await...
```

（`main.rs` 现有 `let (api, sync_rx) = ...init(...)` 处先创建 tx：
`let (sync_tx, sync_rx) = crate::stores::sync::sync_event_channel();`——注意
GPUI 侧的 `sync_event_channel` 在 postium-ui；core 侧直接
`smol::channel::bounded(256)`，与 `SyncEventTx` 类型别名一致。）

③ `CoreApi` 增加方法（`imap_engine` 挂在 `// ===== account =====` 区）：

```rust
    /// 确保账号 IMAP 引擎运行并返回句柄（操作类命令走引擎连接池）。
    pub async fn imap_engine(
        &self,
        account: &crate::infrastructure::storage::models::accounts::Model,
    ) -> Result<crate::engine::registry::EngineHandle, MailError> {
        crate::engine::registry::ensure_engine(
            account,
            Arc::new(crate::engine::factory::RealSessionFactory {
                auth: self.auth.clone(),
                account: account.clone(),
            }),
            self.sync_tx.clone(),
        )
    }
```

- [ ] **Step 6.4: 编译 + 全量测试**

```bash
cargo build -p postium-core
cargo test -p postium-core
```

预期：全部通过（engine 新模块无回归；既有测试不受影响——尚未切换调用点）。

- [ ] **Step 6.5: 提交**

```bash
git add crates/postium-core/src/engine crates/postium-core/src/api.rs crates/postium-core/src/main.rs Cargo.lock
git commit -m "feat(engine): ImapEngineRegistry + CoreApi.imap_engine 接线"
```

---

### Task 7: 接入点改造——mail_operation / mail_send / mail_draft 走引擎

**Files:**
- Modify: `crates/postium-core/src/service/mail_operation.rs`（连接块 ~140-160 行）
- Modify: `crates/postium-core/src/service/mail_send.rs`（~100 行区域连接块）
- Modify: `crates/postium-core/src/service/mail_draft.rs`（~91/126 行两处连接块）
- Modify: `crates/postium-core/src/api.rs`（三个服务构造处传入 registry 句柄获取能力）

- [ ] **Step 7.1: 逐文件替换连接块**

三个文件的连接块结构相同（`match account.auth_type ... ImapClient::connect /
connect_xoauth2`），统一替换为：

```rust
let engine = self.imap_engine(account).await?;
let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
engine
    .send(crate::engine::commands::EngineCommand::FetchBody {
        folder: folder.clone(),
        uid,
        reply: reply_tx,
    })
    .await?;
let body = reply_rx.await.map_err(|e| {
    MailError::ImapConnectionFailed(format!("引擎通道关闭: {e}"))
})??;
```

（各操作对应的命令在 Task 5 的 `EngineCommand` 上按需补充变体——
`Move { uid, to }` / `SetFlags { uid, add, remove }` / `Append { ... }` /
`Delete { uid }`，每个变体的处理在 `actor.rs` 的 `run` 循环里调用
`pool.with_control(|s| ...)` 执行对应 transport 方法。命令变体的增加与
接入点替换在同一个提交内完成，保持每步可编译。）

注意：三个服务的结构体需要能拿到 `CoreApi` 引用（或 registry 句柄）——
若服务构造函数已有 `Arc<CoreApi>` 之外的字段注入路径，遵循现有模式
（参考 `mail_operation.rs` 构造处），新增 `imap: Arc<EngineHandleProvider>`
性质的字段或直接传 `Arc<CoreApi>`。

- [ ] **Step 7.2: 每改一个文件即编译**

```bash
cargo build -p postium-core
```

预期：每步 Finished（命令变体与调用点成对出现，不留半成品）。

- [ ] **Step 7.3: 全量测试**

```bash
cargo test -p postium-core
cargo test -p postium-ui
```

预期：全部通过。特别注意 `mail_draft` 的草稿往返测试
（`draft_save_reports_cleanup_error_and_keeps_old_local_when_old_remote_delete_fails`
等）——它们 mock 的是 `MailRemoteOperator` trait；若引擎化破坏了 mock 路径，
保留该 trait 作为测试 seam：引擎命令处理内仍调用 `MailRemoteOperator`，
即引擎替换的是"连接获取"而非"远程操作语义"。

- [ ] **Step 7.4: 提交**

```bash
git add -A
git commit -m "refactor(engine): 邮件操作/发信/草稿接入点改走 IMAP 引擎命令"
```

---

### Task 8: 验证与清理

- [ ] **Step 8.1: 确认无散落连接调用**

```bash
rg -n "ImapClient::connect" crates/postium-core/src --type rust
```

预期：仅 `engine/`（factory.rs 与 transport.rs 内部）命中。

- [ ] **Step 8.2: 全量测试 + clippy**

```bash
cargo test --workspace 2>&1 | grep -E "test result|FAILED"
cargo clippy -p postium-core -p postium-ui --all-targets 2>&1 | grep -cE "^error"
```

预期：全部 ok；clippy error 数 0。

- [ ] **Step 8.3: 提交（如有遗漏收尾）并推送分支**

```bash
git push -u origin feat/imap-engine
```

阶段 2/3 的计划（同步引擎三档策略、IDLE、文件夹 registry）在阶段 1 合入后
基于实际产出的接口形态另行编写（`docs/superpowers/plans/` 分册惯例）。
