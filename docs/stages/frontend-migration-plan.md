# 前端迁移计划 - 适配后端架构重构

## 📋 Context

后端已完成架构重构，从旧的 `services` 层迁移到新的分层架构：
- **protocols/** - IMAP/SMTP 协议层
- **providers/** - 邮件服务商层 (MailProvider, ProviderPool)
- **auth/** - 认证管理层 (AuthManager)
- **engine/** - 流程引擎 (FlowEngine)
- **sync/** - 同步引擎 (SyncManager)
- **storage/** - 存储层
- **error/** - 统一错误处理 (MailError)

前端需要相应调整以对接新的后端 API，同时保持良好的用户体验。

### 当前前端架构

- **框架**: Vue 3 + TypeScript + Vite
- **状态管理**: Pinia
- **UI 库**: Naive UI
- **IPC**: Tauri invoke/event

### 后端 API 变更概览

**新增命令**:
- FlowEngine 管理: `get_flow_engine_status`, `add_sync_task`, `remove_sync_task`, `pause_sync_task`, `resume_sync_task`, `trigger_sync`
- OAuth 认证: `get_oauth_auth_url`, `exchange_oauth_code`, `refresh_oauth_token`, `validate_oauth_token`
- 增强的同步: `sync_account_with_progress` 支持细粒度进度事件

**新增事件**:
- `sync-progress-{accountId}` - 实时同步进度
- `oauth-deep-link-callback` - OAuth 回调
- `new-mail` - 新邮件通知

---

## 🎯 迁移目标

1. **类型定义同步** - 与后端 DTO 保持一致
2. **Store 层增强** - 支持新的后端功能（OAuth、企业邮箱、FlowEngine）
3. **组件适配** - 账号添加、设置界面适配新功能
4. **错误处理** - 统一的错误处理和用户提示
5. **性能优化** - 利用 FlowEngine 的后台任务能力

---

## 📋 实施计划

### 阶段 1: 类型定义更新 (1天)

#### 1.1 更新 Account 类型

**文件**: `src/types/index.ts`

添加与后端对齐的枚举类型：

```typescript
// 账号类型枚举
export enum AccountType {
  Personal = 'personal',
  Enterprise = 'enterprise',
}

// 认证类型枚举
export enum AuthType {
  Password = 'password',
  OAuth2 = 'oauth2',
  AppPassword = 'app_password',
  DomainAuth = 'domain_auth',
  SamlSso = 'saml_sso',
}

// SSL 模式枚举
export enum SslMode {
  None = 'none',
  StartTls = 'start_tls',
  Implicit = 'implicit',
}
```

扩展 Account 接口：

```typescript
export interface Account {
  id: string
  name: string
  email: string
  provider: EmailProvider

  // 新增字段
  accountType: AccountType
  authType: AuthType

  // IMAP/SMTP 配置
  imapHost: string
  imapPort: number
  imapSsl: boolean
  smtpHost: string
  smtpPort: number
  smtpSsl: boolean

  // UI 配置
  color: string
  unreadCount: number
  syncEnabled: boolean
  lastSyncAt?: Date

  // OAuth 相关
  oauthProvider?: string
  oauthTokenExpiry?: Date

  // 企业配置
  enterpriseTenantId?: string
  enterpriseDomain?: string

  created_at: Date
  updated_at: Date
}
```

#### 1.2 新增同步相关类型

```typescript
// 同步阶段枚举
export enum SyncStage {
  Connecting = 'connecting',
  SyncingFolders = 'syncing_folders',
  SyncingEmails = 'syncing_emails',
  Completed = 'completed',
  Error = 'error',
}

// 同步进度接口
export interface SyncProgress {
  accountId: number
  stage: SyncStage
  folder?: string
  current: number
  total: number
  message: string
  startedAt: Date
}

// 同步状态接口
export interface SyncStatus {
  accountId: number
  stage: 'idle' | 'syncing' | 'completed' | 'error'
  progress: number
  message: string
  currentFolder?: string
  result?: SyncResult
  error?: string
}

// 同步结果接口
export interface SyncResult {
  totalSynced: number
  foldersSynced: number
  errors: number
  durationMs: number
}
```

#### 1.3 新增错误处理类型

```typescript
// 错误严重程度
export enum ErrorSeverity {
  Low = 'low',
  Medium = 'medium',
  High = 'high',
  Critical = 'critical',
}

// 邮件错误接口
export interface MailError {
  code: string
  message: string
  severity: ErrorSeverity
  retryable: boolean
  retryAfter?: number
  details?: Record<string, unknown>
}
```

---

### 阶段 2: Store 层重构 (2-3天)

#### 2.1 增强 AccountStore

**文件**: `src/stores/account.ts`

**新增功能**:
1. 支持企业邮箱配置
2. OAuth 认证流程集成
3. FlowEngine 任务管理

```typescript
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const useAccountStore = defineStore('account', () => {
  // ... 现有状态 ...

  // ========== 新增：按账号类型分组 ==========
  const accountsByType = computed(() => {
    const personal = accounts.value.filter(a => a.accountType === AccountType.Personal)
    const enterprise = accounts.value.filter(a => a.accountType === AccountType.Enterprise)
    return { personal, enterprise }
  })

  // ========== 新增：OAuth 认证 ==========

  // 生成 OAuth 授权 URL
  async function getOAuthAuthUrl(provider: string): Promise<string> {
    return await invoke<string>('get_oauth_auth_url', { provider })
  }

  // 交换 OAuth 授权码
  async function exchangeOAuthCode(code: string, state: string): Promise<Account> {
    const dto = await invoke<AccountDto>('exchange_oauth_code', { code, state })
    const account = dtoToAccount(dto)
    accounts.value.push(account)
    return account
  }

  // 刷新 OAuth Token
  async function refreshOAuthToken(accountId: string): Promise<void> {
    await invoke('refresh_oauth_token', {
      accountId: parseInt(accountId)
    })
  }

  // ========== 新增：FlowEngine 任务管理 ==========

  // 添加同步任务到 FlowEngine
  async function addSyncTask(accountId: string, schedule?: string) {
    await invoke('add_sync_task', {
      accountId: parseInt(accountId),
      schedule,
    })
  }

  // 移除同步任务
  async function removeSyncTask(accountId: string) {
    await invoke('remove_sync_task', {
      accountId: parseInt(accountId)
    })
  }

  // 暂停同步任务
  async function pauseSyncTask(accountId: string) {
    await invoke('pause_sync_task', {
      accountId: parseInt(accountId)
    })
  }

  // 恢复同步任务
  async function resumeSyncTask(accountId: string) {
    await invoke('resume_sync_task', {
      accountId: parseInt(accountId)
    })
  }

  return {
    // ... 现有导出 ...

    // 新增导出
    accountsByType,
    getOAuthAuthUrl,
    exchangeOAuthCode,
    refreshOAuthToken,
    addSyncTask,
    removeSyncTask,
    pauseSyncTask,
    resumeSyncTask,
  }
})
```

#### 2.2 增强 SyncStore

**文件**: `src/stores/sync.ts`

**新增功能**:
1. 同步历史记录
2. FlowEngine 状态查询
3. 触发一次性同步

```typescript
export const useSyncStore = defineStore('sync', {
  state: () => ({
    syncingAccounts: new Set<number>(),
    syncStatuses: new Map<number, SyncStatus>(),
    syncHistory: [] as SyncHistoryItem[],
    unlisteners: new Map<number, Promise<UnlistenFn>>(),
  }),

  getters: {
    // 现有 getters...

    // 新增：同步历史（按时间倒序）
    recentHistory: (state) => {
      return state.syncHistory
        .sort((a, b) => b.completedAt.getTime() - a.completedAt.getTime())
        .slice(0, 10)
    },
  },

  actions: {
    // ... 现有 actions ...

    // 新增：触发一次性同步
    async triggerOneTimeSync(accountId: number) {
      await invoke('trigger_sync', { accountId })
    },

    // 获取 FlowEngine 状态
    async getFlowEngineStatus() {
      return await invoke<FlowEngineStatus>('get_flow_engine_status')
    },
  },
})
```

#### 2.3 创建 FlowEngineStore (新建)

**文件**: `src/stores/flowEngine.ts`

```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface FlowEngineStatus {
  isRunning: boolean
  activeTasks: number
  queuedTasks: number
  completedTasks: number
  failedTasks: number
}

export const useFlowEngineStore = defineStore('flowEngine', () => {
  const status = ref<FlowEngineStatus | null>(null)
  const isLoading = ref(false)

  // ========== Getters ==========

  const isRunning = computed(() => status.value?.isRunning ?? false)
  const hasActiveTasks = computed(() => (status.value?.activeTasks ?? 0) > 0)

  // ========== Actions ==========

  async function fetchStatus() {
    isLoading.value = true
    try {
      status.value = await invoke<FlowEngineStatus>('get_flow_engine_status')
    } finally {
      isLoading.value = false
    }
  }

  // 定期刷新状态（每5秒）
  function startPolling() {
    fetchStatus()
    const interval = setInterval(fetchStatus, 5000)

    // 组件卸载时清理
    onUnmounted(() => {
      clearInterval(interval)
    })
  }

  return {
    status,
    isLoading,
    isRunning,
    hasActiveTasks,
    fetchStatus,
    startPolling,
  }
})
```

---

### 阶段 3: 组件适配 (2-3天)

#### 3.1 更新账号添加组件

**文件**: `src/components/common/AddAccountModal.vue`

**更新内容**:
1. 添加账号类型选择（个人/企业）
2. OAuth 登录流程集成
3. 企业邮箱配置表单

#### 3.2 更新同步状态组件

**文件**: `src/components/common/SyncStatus.vue`

**更新内容**:
1. 显示多账号同步状态
2. 细粒度的同步进度
3. 同步历史查看

#### 3.3 创建企业配置表单组件 (新建)

**文件**: `src/components/common/EnterpriseConfigForm.vue`

---

### 阶段 4: 错误处理增强 (1天)

#### 4.1 创建错误处理工具

**文件**: `src/utils/errorHandler.ts` (新建)

#### 4.2 创建 OAuth 辅助工具

**文件**: `src/utils/oauthHelper.ts` (新建)

---

### 阶段 5: 性能优化 (1天)

#### 5.1 监听新邮件事件

**文件**: `src/stores/email.ts`

#### 5.2 虚拟滚动优化

---

## 📁 关键文件清单

### 需要修改的文件：

#### 类型定义
- `src/types/index.ts` - 更新 Account、同步、错误相关类型

#### Store 层
- `src/stores/account.ts` - 增强 AccountStore
- `src/stores/sync.ts` - 增强 SyncStore
- `src/stores/email.ts` - 添加新邮件监听

#### 组件层
- `src/components/common/AddAccountModal.vue` - 账号添加组件
- `src/components/settings/AccountSettingsModal.vue` - 账号设置组件
- `src/components/common/SyncStatus.vue` - 同步状态组件

### 需要创建的文件：

1. `src/stores/flowEngine.ts` - FlowEngine 状态管理
2. `src/utils/errorHandler.ts` - 错误处理工具
3. `src/utils/oauthHelper.ts` - OAuth 辅助工具
4. `src/components/common/EnterpriseConfigForm.vue` - 企业配置表单
5. `src/components/common/OAuthLoginButton.vue` - OAuth 登录按钮

---

## ✅ 验证步骤

### 功能验证

#### 账号管理
- [ ] 添加个人邮箱账号（密码方式）
- [ ] 添加个人邮箱账号（OAuth 方式 - Gmail/Outlook）
- [ ] 添加企业邮箱账号（Microsoft 365/Google Workspace）
- [ ] 编辑账号配置
- [ ] 删除账号
- [ ] 测试账号连接

#### 同步功能
- [ ] 手动触发同步
- [ ] 查看同步进度（细粒度）
- [ ] 多账号并发同步
- [ ] 同步历史查看
- [ ] 暂停/恢复同步任务
- [ ] 添加/移除定时同步任务

#### OAuth 认证
- [ ] Gmail OAuth 登录
- [ ] Outlook OAuth 登录
- [ ] Token 自动刷新
- [ ] 授权过期处理

#### 邮件操作
- [ ] 邮件列表加载
- [ ] 邮件详情查看
- [ ] 标记已读/未读
- [ ] 星标邮件
- [ ] 删除邮件
- [ ] 移动到文件夹
- [ ] 新邮件实时通知

#### 错误处理
- [ ] 网络错误提示
- [ ] 认证错误提示
- [ ] 同步错误提示
- [ ] 可重试错误的自动重试

### 性能验证

1. **响应时间**
   - 账号切换 < 100ms
   - 邮件列表加载 < 500ms
   - 邮件详情打开 < 300ms
   - 同步进度更新 < 100ms

2. **内存使用**
   - 长时间使用无明显内存泄漏
   - 大量邮件（1000+）流畅滚动
   - 多账号同时同步时内存稳定

---

## 🔄 回滚方案

如果迁移出现问题，可通过以下方式回滚：

1. **Git 分支管理** - 在独立分支 `feature/frontend-migration` 进行迁移
2. **功能开关** - 通过配置控制新旧功能切换
3. **API 兼容层** - 保持旧 API 命令可用，逐步迁移

---

## 📝 注意事项

1. **向后兼容** - 确保现有用户数据不丢失
2. **渐进迁移** - 分阶段发布，逐步验证
3. **用户体验** - 迁移过程中保持 UI 响应
4. **错误提示** - 提供清晰的错误信息和解决建议
5. **测试覆盖** - 每个阶段充分测试后继续下一阶段

---

## ⏱️ 时间估算

- **阶段 1**: 1天 - 类型定义更新
- **阶段 2**: 2-3天 - Store 层重构
- **阶段 3**: 2-3天 - 组件适配
- **阶段 4**: 1天 - 错误处理增强
- **阶段 5**: 1天 - 性能优化

**总计**: 约 8-10 个工作日
