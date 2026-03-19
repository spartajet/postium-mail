# Postium Mail 实现计划及进度文档

> **版本**: 1.3.1
> **创建日期**: 2024-01-15
> **最后更新**: 2026-03-19
> **目标版本**: v2.0.0
> **预计工期**: 8-10 周

---

## 📋 目录

1. [概述](#概述)
2. [进度概览](#进度概览)
3. [架构迁移计划](#架构迁移计划)
4. [详细实施记录](#详细实施记录)
5. [测试策略](#测试策略)
6. [验收标准](#验收标准)
7. [风险与缓解](#风险与缓解)

---

## 概述

本文档记录 Postium Mail 从现有架构迁移到新 FlowEngine 架构体系的完整计划，包括重构计划（refactoring-plan）和迁移计划（migration-plan）的所有内容。

### 迁移目标

- 从现有命令式架构迁移到事件驱动的 FlowEngine 架构
- 实现统一的认证管理（OAuth、密码、企业认证）
- 建立服务商抽象层，支持个人和企业邮箱
- 重构同步引擎，支持高效的增量同步
- 提供完整的测试覆盖

### 迁移范围

- ✅ **允许破坏性 API 变更**
- ✅ **前后端完整迁移**
- ✅ **数据库迁移**

---

## 进度概览

### 整体进度

```
┌─────────────────────────────────────────────────────────────────────┐
│                         迁移进度总览                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  阶段 1: 基础设施搭建        ████████████████████ 100% ✅           │
│  阶段 2: 服务商层实现        ████████████████████ 100% ✅           │
│  阶段 3: 认证层重构          ████████████████████ 100% ✅           │
│  阶段 4: 同步引擎重构        ████████████████████ 100% ✅           │
│  阶段 5: 通知与调度系统      ████████████████████ 100% ✅           │
│  阶段 6: Services 层迁移     ████████████████████ 100% ✅           │
│  阶段 7: 前端适配            █████████░░░░░░░░░░░ 30% ⏳             │
│  阶段 8: 优化和扩展          ░░░░░░░░░░░░░░░░░░░░   0% ⏸             │
│                                                                     │
│  整体进度: ████████████████░░░░  75%                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 测试覆盖

| 模块 | 测试数量 | 状态 |
|------|---------|------|
| 认证模块 | 84+ | ✅ |
| 服务商层 | 88+ | ✅ |
| 同步引擎 | 196+ | ✅ |
| 引擎层 | 38+ | ✅ |
| 存储层 | 30+ | ✅ |
| 协议层 | 20+ | ✅ |
| **总计** | **456+** | ✅ |

---

## 架构迁移计划

### 现有架构问题

```
┌─────────────────────────────────────────────────────────┐
│                    现有架构（已重构）                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  前端 Vue                                              │
│    ↓ 直接调用 Tauri Commands                           │
│  命令层 (command/)                                     │
│    ↓                                                   │
│  引擎层 (engine/) - FlowEngine 协调中心 ✅             │
│    ↓                                                   │
│  业务层 (auth/ + providers/ + sync/ + protocols/) ✅   │
│    ↓                                                   │
│  兼容服务层 (services/) ✅                             │
│    ↓                                                   │
│  数据层 (storage/) ✅                                  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 新架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    新架构（当前状态）                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  前端 Vue                                                  │
│    ↓ 通过统一的 API 调用                                    │
│  命令层 (command/) - 薄层，仅做转换                         │
│    ↓                                                       │
│  引擎层 (engine/) - FlowEngine 协调中心 ✅                 │
│    ├─ flow_engine.rs      - 核心引擎 ✅                   │
│    ├─ task_scheduler.rs   - 定时任务 ✅                   │
│    └─ notification_manager.rs - 通知管理 ✅                │
│    ↓                                                       │
│  业务层 ✅                                                │
│    ├─ auth/               - 认证管理 ✅                    │
│    ├─ providers/          - 服务商抽象 ✅                  │
│    ├─ sync/               - 同步引擎 ✅                    │
│    └─ protocols/          - 协议实现 ✅                    │
│    ↓                                                       │
│  兼容服务层 (services/) - 保留兼容 ✅                      │
│    ├─ account_service   - 账号 CRUD                       │
│    ├─ email_service     - 邮件操作                         │
│    ├─ folder_service    - 文件夹查询                       │
│    └─ search_service    - 邮件搜索                         │
│    ↓                                                       │
│  存储层 (storage/) ✅                                     │
│    ├─ accounts ✅  - emails ✅  - folders ✅               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 详细实施记录

### 阶段 1: 基础设施搭建 ✅

**状态**: 100% 完成
**时间**: 2024-01-15 ~ 2024-01-17

#### 已完成任务

1. ✅ 创建新的目录结构
2. ✅ 实现核心错误类型 (error.rs)
3. ✅ 数据库迁移文件创建
4. ✅ 模型定义 (models/)

#### 关键成果

- 统一的错误类型系统
- sea-orm v2.0.0-rc.37 集成
- 数据库迁移框架

---

### 阶段 2: 服务商层实现 ✅

**状态**: 100% 完成
**时间**: 2024-01-18 ~ 2024-01-22

#### 已完成任务

1. ✅ **MailProvider Trait 定义** (traits.rs)
   - 统一的服务商接口
   - 支持个人和企业邮箱
   - OAuth 配置抽象

2. ✅ **个人邮箱服务商** (personal/)
   - Gmail provider + OAuth 配置
   - Outlook provider + OAuth 配置
   - Yahoo provider + OAuth 配置
   - Native providers (163, QQ, iCloud)

3. ✅ **企业邮箱服务商** (enterprise/)
   - Microsoft 365 + 租户管理
   - Google Workspace + 域名支持
   - Custom provider + 完全自定义

4. ✅ **ProviderPool** (provider_pool.rs)
   - 服务商注册与获取
   - 智能邮箱检测
   - 个人/企业区分

#### 测试结果

- **88 个测试全部通过**
- 个人邮箱：36 个测试
- 企业邮箱：37 个测试
- 工具和池：15 个测试

---

### 阶段 3: 认证层重构 ✅

**状态**: 100% 完成
**时间**: 2024-01-23 ~ 2024-01-28

#### 已完成任务

1. ✅ **TokenManager** (token_manager.rs)
   - Token 存储到 Keyring
   - 内存缓存优化
   - 自动过期检测
   - access_token 缓存（5 分钟 TTL）
   - **19 个单元测试**

2. ✅ **OAuthHandler** (oauth_handler.rs)
   - OAuth 2.0 + PKCE 流程
   - 授权码交换
   - Token 刷新
   - XOAUTH2 字符串生成
   - **10 个单元测试**

3. ✅ **PasswordAuth** (password_auth.rs)
   - 密码存储到 Keyring
   - 密码验证框架
   - **6 个单元测试**

4. ✅ **EnterpriseAuth** (enterprise_auth.rs)
   - 企业配置验证
   - 企业类型检测
   - **10 个单元测试**

5. ✅ **AuthManager** (auth_manager.rs)
   - 统一认证入口
   - OAuth / 密码认证流程
   - Token 自动刷新
   - IMAP/SMTP 认证信息获取
   - **13 个单元测试**

#### 测试结果

- **75 个认证相关测试全部通过**
- **175 个总计测试通过**

---

### 阶段 4: 同步引擎重构 ✅

**状态**: 100% 完成
**时间**: 2024-01-29 ~ 2024-02-10

#### 已完成任务

1. ✅ **DeltaSync** (delta_sync.rs)
   - SyncStrategy 枚举
   - CONDSTORE 支持
   - UID 搜索策略
   - **3 个单元测试**

2. ✅ **ChangeDetector** (change_detector.rs)
   - UidSet 辅助结构
   - EmailFlags 结构体
   - 变更检测逻辑
   - **15 个单元测试**

3. ✅ **FolderManager** (folder_manager.rs)
   - RFC 6154 Special-Use 支持
   - 文件夹同步
   - **4 个单元测试**

4. ✅ **MailProcessor** (mail_processor.rs)
   - 批量处理邮件
   - 数据库存储
   - **2 个单元测试**

5. ✅ **SyncManager** (sync_manager.rs)
   - 同步流程框架
   - 进度报告
   - **3 个单元测试**

6. ✅ **AsyncImapClient** (protocols/imap/client.rs)
   - CONDSTORE 支持
   - IDLE 支持
   - OAuth 认证

7. ✅ **数据库迁移** (m008_add_modseq_support.rs)
   - highest_modseq 字段
   - modseq 字段
   - 索引优化

#### 测试结果

- **204 个单元测试全部通过**
- **56 个集成测试**

---

### 阶段 5: 通知与调度系统 ✅

**状态**: 100% 完成
**时间**: 2024-02-11 ~ 2024-02-15

#### 已完成任务

1. ✅ **TaskScheduler** (task_scheduler.rs)
   - 定时同步任务管理
   - 任务状态跟踪
   - 任务控制
   - 优雅关闭
   - **12 个测试**

2. ✅ **NotificationManager** (notification_manager.rs)
   - 通知去重（5 秒窗口）
   - 通知合并（10 秒窗口）
   - Tauri 事件发射
   - 统计追踪
   - **13 个测试**

3. ✅ **IMAP IDLE** (idle_manager.rs)
   - IDLE 监听循环
   - 断线重连
   - 事件通道通信
   - **6 个测试**

4. ✅ **FlowEngine** (flow_engine.rs)
   - 组件生命周期管理
   - 状态报告
   - 任务管理 API
   - IDLE 监听启动
   - **7 个测试**

#### 测试结果

- **242 个单元测试累计通过**
- **38 个新增测试全部通过**

---

### 阶段 6: Services 层迁移 ✅

**状态**: 100% 完成
**时间**: 2024-02-16 ~ 2026-03-19

#### 已完成任务

1. ✅ **SMTP 协议层迁移**
   - `services/smtp_service` → `protocols::smtp`
   - 完全迁移到新架构

2. ✅ **同步管理器迁移**
   - `services/sync_manager` → `sync::sync_manager`
   - 完全迁移到新架构

3. ✅ **IMAP 协议层迁移**
   - `services/imap` → `protocols::imap`
   - 完全迁移到新架构

4. ✅ **同步状态迁移**
   - `services/sync_state_service` → `sync::sync_state`
   - 功能整合到 SyncStateManager

5. ✅ **同步错误迁移**
   - `services/sync_error_service` → `sync::sync_error`
   - 创建 SyncErrorManager

6. ✅ **OAuth 功能整合**
   - `services/oauth_service` → `auth::AuthManager`
   - 完全整合，移除 deprecated 模块

7. ✅ **搜索服务迁移**
   - `services/search_service` → `storage::search`
   - 使用 FTS5 全文搜索

8. ✅ **操作管理模块迁移**
   - `services/operations/conflict_resolver` → `engine::conflict_resolver`
   - `services/operations/operation_manager` → `engine::operation_manager`
   - 离线操作和冲突解决

9. ✅ **存储层整合**
   - `src/models/` → `storage/models/`
   - `src/migration/` → `storage/migration/`
   - 数据库相关模块统一到 storage 下

10. ✅ **Services 目录删除**
    - 所有功能迁移完成后删除 services 目录
    - Command 层直接使用 storage repositories

#### 最终迁移对照表

| 旧模块 | 新模块 | 状态 |
|--------|--------|------|
| `oauth_service` | `auth::AuthManager` | ✅ 已整合 |
| `imap/*` | `protocols::imap/*` | ✅ 已迁移 |
| `smtp_service` | `protocols::smtp/*` | ✅ 已迁移 |
| `sync_manager` | `sync::sync_manager` | ✅ 已迁移 |
| `sync_state_service` | `sync::SyncStateManager` | ✅ 已迁移 |
| `sync_error_service` | `sync::SyncErrorManager` | ✅ 已迁移 |
| `search_service` | `storage::search` | ✅ 已迁移 |
| `conflict_resolver` | `engine::conflict_resolver` | ✅ 已迁移 |
| `operation_manager` | `engine::operation_manager` | ✅ 已迁移 |
| `models/` | `storage/models/` | ✅ 已迁移 |
| `migration/` | `storage/migration/` | ✅ 已迁移 |

#### 架构优化成果

- **模块职责更清晰**: 数据库相关代码集中在 storage，业务逻辑分散在 auth/sync/protocols
- **层级更扁平**: 移除中间 services 层，command 直接调用对应功能模块
- **测试覆盖完整**: 所有模块都有单元测试，总计 280+ 测试通过

---

### 阶段 7: 前端适配 ⏳

**状态**: 30% 进行中
**时间**: 2024-02-21 ~ 预计 2024-03-10

#### 待完成任务

1. ⏳ **API 客户端更新**
   - 适配新的 FlowEngine API
   - 更新 Tauri 事件监听
   - 更新错误处理

2. ⏳ **状态管理更新**
   - 更新 Pinia stores
   - 适配新的同步状态
   - 适配新的通知机制

3. ⏳ **UI 组件更新**
   - 同步进度显示
   - 新邮件通知
   - 错误提示优化

---

### 阶段 8: 优化和扩展 ⏸

**状态**: 未开始
**时间**: 预计 2024-03-11 ~ 2024-03-20

#### 计划任务

1. ⏸ **性能优化**
   - 大文件夹同步优化
   - 内存使用优化
   - 启动速度优化

2. ⏸ **功能扩展**
   - 离线模式支持
   - 邮件搜索增强
   - 邮件过滤规则

3. ⏸ **错误处理改进**
   - 网络错误重试
   - 用户友好的错误消息
   - 错误恢复机制

---

## 测试策略

### 单元测试

每个模块都包含单元测试，覆盖：
- 核心功能
- 边界情况
- 错误处理

### 集成测试

集成测试覆盖：
- 端到端同步流程
- CONDSTORE 功能
- 完整同步工作流
- 错误恢复

### 测试运行

```bash
# 运行所有测试
cargo test --package src-tauri

# 运行特定模块测试
cargo test --package src-tauri auth
cargo test --package src-tauri sync
cargo test --package src-tauri providers
```

---

## 验收标准

### 功能验收

- [x] OAuth 认证流程正常
- [x] 密码认证流程正常
- [x] 邮件同步功能正常
- [x] 文件夹同步功能正常
- [x] IMAP IDLE 实时监听正常
- [x] 新邮件通知正常
- [ ] 前端适配完成
- [ ] 性能指标达标

### 质量验收

- [x] 所有单元测试通过（456+ 测试）
- [x] 所有集成测试通过
- [x] 代码覆盖率 > 80%
- [x] Clippy 检查通过
- [x] 文档完整

---

## 风险与缓解

### 已识别风险

| 风险 | 影响 | 缓解措施 | 状态 |
|------|------|---------|------|
| async-imap CONDSTORE 支持不完整 | 中 | UID 搜索降级策略 | ✅ 已缓解 |
| OAuth Token 刷新失败 | 高 | 自动重试 + 错误提示 | ✅ 已缓解 |
| 大文件夹同步性能 | 中 | 增量同步 + 批处理 | ⏳ 进行中 |
| 前端 API 变更 | 中 | 渐进式迁移 | ⏳ 进行中 |

---

## 附录

### 文档结构

```
docs/
├── architecture-design.md      # 架构设计文档 ✅
├── implementation-plan.md       # 本文档 ✅
└── stages/                      # 迁移过程记录
    ├── mail-flow-engine-design.md   # 原始 FlowEngine 设计
    ├── refactoring-plan.md          # 原始重构计划
    └── migration-plan.md            # 原始迁移计划
```

### 相关提交

- `10ca529` feat: 实现 TaskScheduler 任务调度器
- `5952ea2` feat: 实现 NotificationManager 通知管理器
- `13588e4` feat: 实现 IMAP IDLE 轮询监听支持
- `0ce23c1` feat: 完成 FlowEngine 集成实现
- `8889155` refactor: 迁移 sync_state_service 和 sync_error_service

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 1.4.0 | 2026-03-19 | 完成 Services 层完全迁移，删除 services 目录 |
| 1.3.1 | 2026-03-19 | 添加阶段 6 完成记录，更新进度 |
| 1.3.0 | 2026-03-18 | 添加 OAuth 整合记录 |
| 1.2.0 | 2026-03-18 | 添加 IMAP 迁移记录 |
| 1.1.0 | 2026-03-17 | 添加阶段 4、5 完成记录 |
| 1.0.0 | 2024-01-15 | 初始版本 |
