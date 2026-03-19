# 阶段2迁移计划 - IMAP客户端迁移到 protocols/imap

**分支**: `feature/engine-migrate`
**日期**: 2026-03-19
**目标**: 将 IMAP 客户端从 services/imap 迁移到 protocols/imap
**状态**: ✅ 已完成（2026-03-19）

---

## 📋 迁移范围

### 源模块
**位置**: `src-tauri/src/services/imap/`

**主要文件**:
- `client.rs` - AsyncImapClient 核心实现
- `types.rs` - 数据类型定义
- `error.rs` - 错误类型
- `condstore_helpers.rs` - CONDSTORE 支持
- `idle_manager.rs` - IDLE 管理
- `parser.rs` - 邮件解析
- `service.rs` - IMAP 服务包装

### 目标模块
**位置**: `src-tauri/src/protocols/imap/`

**目标结构**:
```
protocols/imap/
├── mod.rs              # 模块导出
├── client.rs           # IMAP 客户端（核心）
├── types.rs            # 类型定义
├── error.rs            # 错误类型
├── auth.rs             # 认证逻辑（OAuth2/密码）
├── condstore.rs        # CONDSTORE 支持
├── idle.rs             # IDLE 支持
└── parser.rs           # 邮件解析
```

---

## 🔍 AsyncImapClient 功能清单

### 核心功能（必须迁移）
- [x] 连接与认证
  - `connect()` - IMAP 连接
  - `logout()` - 登出
  - OAuth2/XOAUTH2 认证支持

- [x] 邮箱操作
  - `list_folders()` - 列出邮箱
  - `list_folders_with_attributes()` - 列出邮箱及属性
  - `select_folder()` - 选择邮箱
  - `fetch_folder_metadata()` - 获取邮箱元数据

- [x] 邮件列表
  - `list_uids()` - 获取 UID 列表
  - `list_uids_after()` - 获取大于指定 UID 的列表
  - `list_uids_since_timestamp()` - 按时间戳获取
  - `list_uids_since()` - 按日期获取

- [x] 邮件获取
  - `fetch_email()` - 获取完整邮件
  - `fetch_email_headers()` - 获取邮件头
  - `fetch_email_body()` - 获取邮件正文
  - `fetch_raw_header()` - 获取原始头

- [x] 邮件操作
  - `mark_as_read()` - 标记已读
  - `set_flag()` - 设置标志
  - `delete_email()` - 删除邮件

### 高级功能（已包含在迁移中）
- [x] CONDSTORE 支持（`condstore_helpers.rs`）
  - `check_condstore_support()` - 检查 CONDSTORE 支持
  - `select_with_condstore()` - 使用 CONDSTORE 选择
  - `search_modified_since()` - 搜索修改的邮件
  - `fetch_with_modseq()` - 获取 MODSEQ
  - `fetch_modseqs()` - 获取 MODSEQ 列表

- [x] IDLE 支持（`idle_manager.rs`）
  - `check_idle_support()` - 检查 IDLE 支持
  - `polling_fallback()` - 轮询回退
  - `check_new_emails()` - 检查新邮件

---

## 🎯 迁移步骤

### 步骤 1: 创建基础结构 ✅

**文件**: `protocols/imap/mod.rs`
```rust
//! IMAP 协议实现
//!
//! 提供完整的 IMAP 客户端功能，支持：
//! - 传统密码认证
//! - OAuth2/XOAUTH2 认证
//! - CONDSTORE 增量同步
//! - IDLE 实时通知

mod client;
mod types;
mod error;
mod auth;
mod condstore;
mod idle;
mod parser;

// 重新导出公共接口
pub use client::{AsyncImapClient, ImapAuth, ImapClient};
pub use types::*;
pub use error::{ImapError, Result as ImapResult};
```

### 步骤 2: 迁移类型定义

**源文件**: `services/imap/types.rs`
**目标文件**: `protocols/imap/types.rs`

**主要类型**:
- `EmailData` - 邮件数据
- `EmailFlags` - 邮件标志
- `EmailHeader` - 邮件头
- `EmailAttachment` - 附件
- `FolderInfo` - 文件夹信息
- `FolderMetadata` - 文件夹元数据
- `ImapAuth` - 认证方式
- `SpecialUse` - 特殊用途

### 步骤 3: 迁移错误类型

**源文件**: `services/imap/error.rs`
**目标文件**: `protocols/imap/error.rs`

### 步骤 4: 迁移认证逻辑

**新文件**: `protocols/imap/auth.rs`

**内容**:
- 提取 `client.rs` 中的认证代码
- 使用 `providers::generate_xoauth2_string`
- 统一认证接口

### 步骤 5: 迁移核心客户端

**源文件**: `services/imap/client.rs`
**目标文件**: `protocols/imap/client.rs`

**迁移策略**:
1. 复制 `AsyncImapClient` 结构
2. 更新 `use` 语句
3. 提取认证逻辑到 `auth.rs`
4. 保持方法签名不变

### 步骤 6: 迁移 CONDSTORE 支持

**源文件**: `services/imap/condstore_helpers.rs`
**目标文件**: `protocols/imap/condstore.rs`

### 步骤 7: 迁移 IDLE 支持

**源文件**: `services/imap/idle_manager.rs`
**目标文件**: `protocols/imap/idle.rs`

### 步骤 8: 更新引用

**需要更新的文件**:
1. `services/imap/mod.rs` - 重新导出新模块
2. `command/sync.rs` - 更新导入路径
3. 其他使用 `services::imap` 的地方

**替换规则**:
```rust
// 旧
use crate::services::imap::{AsyncImapClient, types};

// 新
use crate::protocols::imap::{AsyncImapClient, types};
```

---

## 🔄 兼容性策略

### 方案 A: 保留旧模块作为转发层（推荐）

```rust
// services/imap/mod.rs
//! ⚠️ 此模块已迁移到 protocols::imap
//! 保留此模块作为兼容层

// 重新导出新模块的所有内容
pub use crate::protocols::imap::*;
```

**优点**:
- ✅ 无需修改使用方代码
- ✅ 渐进式迁移
- ✅ 易于回滚

**缺点**:
- ⚠️ 临时增加代码重复

### 方案 B: 直接替换所有引用

**操作**: 全局搜索替换

**优点**:
- ✅ 代码更清晰
- ✅ 无冗余

**缺点**:
- ⚠️ 需要修改多处
- ⚠️ 回滚较复杂

**选择**: 方案 B（直接迁移） - 已完成 ✅

**执行结果**:
- ✅ 直接删除 `services/imap` 目录
- ✅ 全局更新所有引用 `services::imap` → `protocols::imap`
- ✅ 编译通过，无错误
- ✅ 使用 `providers::generate_xoauth2_string` 替代旧的 `oauth_service`

---

## 📊 文件变更清单

### 新增文件
- [x] `protocols/mod.rs`
- [x] `protocols/imap/mod.rs`
- [x] `protocols/imap/client.rs`
- [x] `protocols/imap/types.rs`
- [x] `protocols/imap/error.rs`
- [x] `protocols/imap/auth.rs`
- [x] `protocols/imap/condstore_helpers.rs`
- [x] `protocols/imap/idle_manager.rs`
- [x] `protocols/imap/parser.rs`
- [x] `protocols/imap/raw_commands.rs`
- [x] `protocols/imap/service.rs`
- [x] `protocols/imap/tests.rs`

### 修改文件
- [x] `services/mod.rs` - 移除 imap 模块引用
- [x] `command/connection.rs` - 更新导入为 protocols::imap
- [x] `services/sync_manager.rs` - 更新导入为 protocols::imap
- [x] `lib.rs` - 添加 protocols 和 storage 模块

### 删除文件（已完成）
- [x] `services/imap/` - 整个目录已删除

---

## ✅ 验收标准

### 编译验证
- [x] `cargo check` 通过
- [x] `cargo clippy` 无新增警告
- [x] 无未使用的导入警告（仅有 1 个预存的 anyhow 未使用警告）

### 功能验证
- [ ] 连接测试正常（待运行时测试）
- [ ] 文件夹列表正常（待运行时测试）
- [ ] 邮件获取正常（待运行时测试）
- [ ] OAuth 认证正常（待运行时测试）
- [ ] CONDSTORE 功能正常（待运行时测试）
- [ ] IDLE 功能正常（待运行时测试）

### 测试覆盖
- [x] 编译时测试通过
- [ ] 运行时测试待执行

---

## ⏱️ 时间估算

| 步骤 | 任务 | 时间 |
|------|------|------|
| 1 | 创建基础结构 | 10min |
| 2 | 迁移类型定义 | 5min |
| 3 | 迁移错误类型 | 5min |
| 4 | 迁移认证逻辑 | 15min |
| 5 | 迁移核心客户端 | 30min |
| 6 | 迁移 CONDSTORE | 15min |
| 7 | 迁移 IDLE | 15min |
| 8 | 更新引用和验证 | 20min |
| | **总计** | **~2小时** |

---

## 🎯 下一步

完成步骤1-8后：

1. **提交阶段2更改**
2. **运行集成测试**
3. **更新文档**
4. **继续阶段3** - 认证层完全迁移

---

**执行前确认**:
- [x] 在 feature/engine-migrate 分支
- [x] 阶段1 已完成
- [x] 了解迁移范围
- [x] 预计时间: ~2小时
