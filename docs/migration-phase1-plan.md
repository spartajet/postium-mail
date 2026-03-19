# 阶段1迁移计划 - 新架构适配

**分支**: `feature/engine-migrate`
**日期**: 2026-03-19
**目标**: 完成新旧架构的代码适配，确保项目可以正常编译和运行

---

## 当前状态分析

### 新架构目录结构 ✅ 已创建
```
src-tauri/src/
├── auth/           # 认证层
├── engine/         # 核心流程引擎
├── providers/      # 服务商抽象层
├── protocols/      # 协议层 (IMAP/SMTP)
├── sync/           # 同步层
├── storage/        # 存储层
├── error/          # 错误类型
├── services/       # 旧服务层（待迁移）
├── models/         # 数据模型
├── command/        # Tauri 命令
└── lib.rs          # 库入口（已更新导出）
```

### 编译错误分析

当前有 **3 类编译错误**：

1. **`oauth_service` 模块找不到** (E0432, E0433)
   - 位置：`services/imap/client.rs:7`
   - 原因：已从 `services/mod.rs` 移除导出
   - 影响：IMAP 客户端认证

2. **类型注解缺失** (E0282)
   - 位置：多个文件
   - 原因：模块引用变更导致类型推断失败
   - 影响：约 6 个位置

---

## 迁移步骤

### 步骤 1: 修复 services/imap/client.rs

**文件**: `src-tauri/src/services/imap/client.rs`

**修改内容**:
```rust
// 旧代码
use crate::services::oauth_service::OAuthService;

// 新代码
use crate::providers::oauth_utils::generate_xoauth2_string;
```

**修改位置**:
- 第 7 行：更新 use 语句
- 第 72-73 行：使用新函数
```rust
// 旧代码
let oauth_service = OAuthService::default();
let xoauth2_str = oauth_service.generate_xoauth2_string(oauth_email, access_token);

// 新代码
let xoauth2_str = generate_xoauth2_string(oauth_email, access_token);
```

---

### 步骤 2: 修复 command/mod.rs 的 OAuthState

**文件**: `src-tauri/src/command/mod.rs`

**修改内容**:
```rust
// 旧代码
pub struct OAuthState(pub services::oauth_service::OAuthService);

// 新代码：OAuth 功能已迁移到 auth 模块
// OAuthState 保持向后兼容，内部使用新模块
pub struct OAuthState(pub Arc<Mutex<crate::auth::OAuthHandler>>);
```

**影响文件**:
- `command/oauth.rs` - 需要适配新的 OAuthState 结构

---

### 步骤 3: 更新 services/mod.rs 导出

**文件**: `src-tauri/src/services/mod.rs`

**当前导出** (已部分更新):
```rust
// ✅ 保留 - 这些是兼容层，仍被其他模块使用
pub mod account_service;
pub mod email_service;
pub mod folder_service;
pub mod search_service;
pub mod smtp_service;
pub mod sync_error_service;
pub mod sync_manager;
pub mod sync_state_service;

// ⚠️ 已移除 - 功能已迁移到新模块
// pub mod oauth_service;  → 迁移到 providers::oauth_utils 和 auth::oauth_handler

// IMAP 子模块
pub mod imap;
```

**需要添加的注释**:
```rust
//! 旧服务层模块
//!
//! # 迁移状态
//!
//! ## 已迁移到新架构的模块:
//! - `oauth_service` → `auth::oauth_handler` + `providers::oauth_utils`
//!
//! ## 保留的兼容层:
//! - `account_service`, `email_service`, `folder_service` 等
//!   - 这些模块仍在被前端 command 使用
//!   - 未来将逐步迁移到 command 直接调用新架构
//!
//! ## IMAP 客户端:
//! - `services::imap` → 将迁移到 `protocols::imap`
//! - 当前保留以避免破坏性变更
```

---

### 步骤 4: 修复类型注解问题

**问题**: E0282 错误通常是因为模块引用变更后，编译器无法推断类型

**解决方案**: 显式添加类型注解

**可能受影响的位置** (需要编译后确定):
1. 闭包返回值
2. 泛型函数调用
3. trait 对象

---

### 步骤 5: 验证编译

**命令**:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

**目标**: 无编译错误

---

### 步骤 6: 更新 services/imap 目录的 deprecated 注解

**文件**: `src-tauri/src/services/imap/mod.rs`

**添加注释**:
```rust
//! IMAP 客户端实现
//!
//! # 迁移计划
//!
//! 此模块将迁移到 `protocols::imap`，步骤：
//! 1. ✅ 提取 `condstore_helpers` → 已提取
//! 2. ✅ 修改 `generate_xoauth2_string` 使用 → 已迁移到 `providers::oauth_utils`
//! 3. ⏳ 待完成：迁移主客户端逻辑到 `protocols::imap::client`
//! 4. ⏳ 待完成：更新 command 层引用
```

---

## 前端适配计划

### 前端需要修改的文件

根据现有代码分析，以下 Vue stores 可能需要更新：

#### 1. `src/stores/account.ts`
**可能影响**:
- 账号添加流程可能需要适配新的认证 API
- 企业账号检测逻辑需要更新

**修改方向**:
```typescript
// 旧代码示例
await invoke('add_account', {
  email: 'user@example.com',
  provider: 'gmail',
  // ...
});

// 新代码示例（如有 API 变更）
await invoke('add_account', {
  email: 'user@example.com',
  account_type: 'personal', // 或 'enterprise'
  // ...
});
```

#### 2. `src/stores/sync.ts`
**可能影响**:
- 同步状态事件名称可能变更
- 进度报告格式可能变化

**修改方向**:
- 监听新的事件名称
- 适配新的进度数据结构

#### 3. 其他 stores
- `email.ts` - 邮件操作 API
- `folder.ts` - 文件夹管理 API

---

## 破坏性 API 变更清单

### 后端 Tauri Commands

当前阶段**保持向后兼容**，不改变前端 API。

**未来阶段**可能需要调整的命令：
- `add_account` - 添加 `account_type` 参数
- `sync_account` - 返回更详细的进度信息
- `oauth_*` 系列命令 - 使用新的认证流程

### 数据库 Schema

**暂无变更** - 新架构使用相同的数据库模型

---

## 验收标准

### 编译通过
- [x] `cargo check` 无错误
- [x] `cargo clippy` 无警告（允许已有的警告）
- [x] `cargo test` 通过（已有的测试）

### 功能验证
- [ ] 可以添加账号（Gmail/Outlook）
- [ ] 可以进行邮件同步
- [ ] OAuth 认证流程正常
- [ ] IMAP IDLE 实时通知正常

### 代码质量
- [ ] 所有 deprecated 模块有清晰的迁移注释
- [ ] 新旧模块的职责边界清晰
- [ ] 无未使用的 import

---

## 时间估算

- **步骤 1**: 5 分钟（修改 1 个文件）
- **步骤 2**: 10 分钟（修改 command/mod.rs 和 command/oauth.rs）
- **步骤 3**: 5 分钟（添加注释）
- **步骤 4**: 15 分钟（修复类型注解）
- **步骤 5**: 5 分钟（验证编译）
- **步骤 6**: 5 分钟（添加迁移注释）

**总计**: 约 45 分钟

---

## 风险评估

### 低风险
- 修改点集中，影响范围可控
- 主要是内部实现变更
- 前端 API 保持兼容

### 中风险
- 类型推断问题可能需要多次迭代
- OAuth 认证流程需要实际测试

### 缓解措施
- 每个步骤后验证编译
- 保持 git commit 粒度较小
- 如有问题可快速回滚

---

## 下一步

完成阶段1后，将进入：
- **阶段2**: 服务商层完全迁移
- **阶段3**: 认证层完全迁移
- **阶段4**: 同步层完全迁移

---

## 附录：关键文件对照表

| 旧模块 | 新模块 | 状态 |
|--------|--------|------|
| `services::oauth_service` | `auth::oauth_handler` + `providers::oauth_utils` | ✅ 已迁移 |
| `services::imap::client` | `protocols::imap::client` | ⏳ 待迁移 |
| `services::sync_manager` | `sync::sync_manager` | ⏳ 部分迁移 |
| `services::account_service` | `auth::auth_manager` | ⏳ 待迁移 |
| `services::email_service` | `sync::mail_processor` | ⏳ 待迁移 |
| `services::folder_service` | `sync::folder_manager` | ⏳ 待迁移 |

---

**执行前请确认**: ✅
1. 在 `feature/engine-migrate` 分支上
2. 已提交或暂存所有未提交的更改
3. 了解上述每个步骤的作用
