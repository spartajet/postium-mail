# 前端适配分析 - 新架构迁移

**日期**: 2026-03-19
**阶段**: 阶段1完成后
**目标**: 分析前端需要做的适配工作

---

## 📊 当前前后端 API 通信分析

### 后端 Tauri Commands 清单

#### 1. 账号管理 (command/account.rs)
```rust
list_accounts()          // ✅ 前端使用
add_account()            // ✅ 前端使用
update_account()         // ✅ 前端使用
delete_account()         // ✅ 前端使用
```

#### 2. 连接测试 (command/connection.rs)
```rust
test_account_connection() // ✅ 前端使用
```

#### 3. 邮件操作 (command/email.rs)
```rust
fetch_emails()          // ✅ 前端使用
fetch_email_detail()     // ✅ 前端使用
mark_as_read()          // ✅ 前端使用
toggle_star()           // ✅ 前端使用
move_email_to_folder()  // ✅ 前端使用
delete_emails()         // ✅ 前端使用
mark_multiple_read()     // ✅ 前端使用
```

#### 4. 文件夹管理 (command/folder.rs)
```rust
list_folders()          // ✅ 前端使用
```

#### 5. 同步管理 (command/sync.rs)
```rust
sync_account_with_progress()  // ✅ 前端使用 (旧实现)
sync_account()               // ✅ 前端使用 (旧实现)
send_email()                // ✅ 前端使用
```

#### 6. OAuth 认证 (command/oauth.rs)
```rust
get_auth_url()          // ✅ 前端使用
exchange_code()         // ✅ 前端使用
```

#### 7. FlowEngine 新架构 (command/flow_engine.rs)
```rust
get_flow_engine_status() // ⚠️ 前端未使用
add_sync_task()         // ⚠️ 前端未使用
remove_sync_task()      // ⚠️ 前端未使用
pause_sync_task()       // ⚠️ 前端未使用
resume_sync_task()      // ⚠️ 前端未使用
trigger_sync()          // ⚠️ 前端未使用 (替代 sync_account)
```

---

## 🎯 关键发现

### 1. 账号类型支持

**前端已有字段** (`src/types/index.ts`):
```typescript
export interface Account {
  authType?: string;      // ✅ 已定义，可能未使用
  oauthProvider?: string;  // ✅ 已定义，可能未使用
}
```

**新架构支持**:
- `AccountType::Personal` (个人邮箱)
- `AccountType::Enterprise` (企业邮箱)

**适配需求**:
- ✅ 类型定义已存在，无需修改
- ⚠️ 可能需要在 UI 上显示账号类型
- ⚠️ 添加企业邮箱时需要额外参数（tenant_id, domain 等）

---

### 2. 同步进度事件

**前端监听** (`src/stores/sync.ts`):
```typescript
listen<SyncProgressEvent>(`sync-progress-${accountId}`)
```

**当前事件格式**:
```typescript
interface SyncProgressEvent {
  account_id: number
  stage: 'connecting' | 'syncing_folders' | 'syncing_emails' | 'completed' | 'error'
  folder?: string
  current: number
  total: number
  message: string
}
```

**新架构引擎事件** (`engine/flow_engine.rs`):
```rust
pub enum EngineEvent {
    SyncProgress { account_id, progress },
    SyncCompleted { account_id, result },
    SyncError { account_id, error },
    // ...
}
```

**适配需求**:
- ⚠️ 事件名称可能需要统一（保持兼容性）
- ⚠️ 事件数据格式可能需要适配
- ✅ 当前 `SyncProgressEvent` 格式基本兼容

---

### 3. 同步 API 替代方案

**旧 API** (command/sync.rs):
```typescript
invoke('sync_account_with_progress', { accountId })
```

**新 API** (command/flow_engine.rs):
```typescript
invoke('trigger_sync', { accountId })
```

**适配需求**:
- 🔧 选项 A: 保持旧 API，内部调用新引擎
- 🔧 选项 B: 前端切换到新 API `trigger_sync`
- ✅ 推荐: 选项 A，保持向后兼容

---

### 4. 定时同步任务管理

**新功能** - FlowEngine 支持:
- `add_sync_task(account_id, interval_minutes)` - 添加定时同步
- `pause_sync_task(account_id)` - 暂停同步
- `resume_sync_task(account_id)` - 恢复同步
- `remove_sync_task(account_id)` - 移除任务

**适配需求**:
- 🆕 前端可添加"自动同步"设置界面
- 🆕 可添加同步间隔配置（15分钟、30分钟、1小时等）
- 🆕 可添加暂停/恢复同步按钮

---

## 📋 前端适配任务清单

### 优先级 P0 (必须，阶段2)

#### 任务 1: 账号类型显示
**位置**: 账号列表、账号详情
- [ ] 在账号卡片上显示个人/企业标签
- [ ] 添加账号时自动识别类型（后端自动检测）
- [ ] 企业账号显示域名信息

**代码示例**:
```typescript
// stores/account.ts - 添加账号时
async function addAccount(accountData: Partial<Account>) {
  // 后端会自动检测 AccountType
  const dto = await invoke<AccountDto>('add_account', { account: request })

  // 可能返回 account_type 字段
  const account = dtoToAccount(dto)
  account.accountType = dto.account_type // 'personal' | 'enterprise'
}
```

#### 任务 2: 同步 API 兼容性
**位置**: stores/sync.ts
- [ ] 保持使用 `sync_account_with_progress`
- [ ] 后端内部将请求转发给 FlowEngine
- [ ] 确保事件名称不变

**无需修改前端** ✅

---

### 优先级 P1 (建议，阶段3)

#### 任务 3: 自动同步设置
**位置**: 设置页面、账号设置
- [ ] 添加"自动同步"开关
- [ ] 同步间隔选择器（15/30/60/120 分钟）
- [ ] 调用 `add_sync_task` / `pause_sync_task`

**代码示例**:
```typescript
// stores/sync.ts - 新增
async function setAutoSync(accountId: number, enabled: boolean, interval?: number) {
  if (enabled) {
    await invoke('add_sync_task', {
      account_id: accountId,
      interval_minutes: interval || 30
    })
  } else {
    await invoke('pause_sync_task', { account_id: accountId })
  }
}
```

#### 任务 4: FlowEngine 状态监控
**位置**: 设置页面、调试面板
- [ ] 调用 `get_flow_engine_status`
- [ ] 显示引擎状态（运行中/暂停）
- [ ] 显示任务数量、通知统计

**UI 建议**:
```
┌─────────────────────────────┐
│ FlowEngine 状态             │
├─────────────────────────────┤
│ 状态: 运行中              │
│ 活跃任务: 3               │
│ 今日同步: 156 封邮件        │
│ 通知发送: 12 次            │
└─────────────────────────────┘
```

---

### 优先级 P2 (可选，阶段4)

#### 任务 5: 企业邮箱配置
**位置**: 添加账号页面
- [ ] 显示企业邮箱字段（tenant_id, domain）
- [ ] 条件访问提示（MFA required）
- [ ] 自定义服务器配置

#### 任务 6: 高级同步选项
**位置**: 同步设置
- [ ] 增量同步开关（CONDSTORE）
- [ ] 首次全量同步选项
- [ ] 同步排除文件夹

---

## 🔄 数据流对比

### 旧流程（当前）
```
前端 → add_account
     → account_service::add
     → sync_manager::sync_account (旧实现)
     → IMAP 客户端
     → 数据库
```

### 新流程（未来）
```
前端 → add_account
     → AuthManager::authenticate
     → ProviderPool::detect_provider
     → FlowEngine::add_account
     → FlowEngine::trigger_sync (或定时任务)
     → SyncManager (新实现)
     → protocols::imap::Client
     → 数据库
```

---

## 🎨 UI 改进建议

### 1. 账号列表卡片
```vue
<AccountCard>
  <AccountTypeBadge>{{ account.accountType }}</AccountBadge>
  <Email>{{ account.email }}</Email>
  <SyncBadge v-if="account.autoSync">
    自动同步: {{ account.syncInterval }}分钟
  </SyncBadge>
</AccountCard>
```

### 2. 同步控制按钮
```vue
<template>
  <Button @click="toggleSync">
    {{ isAutoSync ? '暂停同步' : '启用自动同步' }}
  </Button>
  <Select v-model="syncInterval">
    <Option value="15">15 分钟</Option>
    <Option value="30">30 分钟</Option>
    <Option value="60">1 小时</Option>
  </Select>
</template>
```

---

## 📊 API 破坏性变更风险评估

### 无破坏性变更 ✅
- 所有现有 commands 保持不变
- 事件名称和格式保持兼容
- 前端类型定义已支持扩展字段

### 新增功能 🆕
- FlowEngine commands (可选使用)
- 自动同步任务管理
- 企业邮箱参数

---

## 🚀 实施建议

### 阶段 1.1: 准备工作（当前）
- ✅ 后端编译通过
- ✅ 文档完善
- [ ] 前端类型定义审查

### 阶段 1.2: 测试现有功能
- [ ] 测试账号添加（Gmail/Outlook/IMAP）
- [ ] 测试手动同步
- [ ] 测试邮件操作
- [ ] 确认无破坏性变更

### 阶段 2.1: 账号类型适配
- [ ] 后端返回 `account_type` 字段
- [ ] 前端显示账号类型标签
- [ ] 测试企业邮箱检测

### 阶段 2.2: 自动同步功能
- [ ] 实现自动同步设置 UI
- [ ] 集成 `add_sync_task` API
- [ ] 测试暂停/恢复功能

### 阶段 3: 监控与调试
- [ ] FlowEngine 状态面板
- [ ] 同步日志查看器
- [ ] 性能指标展示

---

## 📝 代码变更预估

### 前端需要修改的文件

| 文件 | 变更类型 | 估时 |
|------|---------|------|
| `types/index.ts` | ✅ 无需修改（已支持） | 0h |
| `stores/account.ts` | 🔧 可选：添加自动同步方法 | 1h |
| `stores/sync.ts` | 🔧 可选：添加任务管理方法 | 1h |
| `views/Settings.vue` | 🆕 添加同步配置界面 | 2h |
| `components/AccountCard.vue` | 🔧 显示账号类型 | 0.5h |

**总计**: 约 4.5 小时（可选功能）

---

## ✅ 验收标准

### 最小可用版本 (MVP)
- [x] 后端编译通过
- [ ] 现有功能全部正常
- [ ] 无破坏性变更
- [ ] 账号类型正确显示

### 完整功能版本
- [ ] 自动同步功能正常
- [ ] FlowEngine 状态可查看
- [ ] 企业邮箱支持完整

---

## 🎯 下一步行动

**选项 A**: 继续阶段2 - 迁移 IMAP 客户端到 protocols/imap

**选项 B**: 先测试当前更改，确保功能正常

**选项 C**: 实现前端自动同步 UI（优先级 P1）

**选项 D**: 编写集成测试，验证新旧架构兼容性

请告诉我您想继续哪个方向！
