# 邮件同步流程文档

本文档详细记录了邮件同步的完整流程，包括增量同步、变更检测、文件夹管理等核心逻辑。

---

## 目录

1. [同步架构概述](#同步架构概述)
2. [完整同步流程](#完整同步流程)
3. [增量同步机制](#增量同步机制)
4. [变更检测](#变更检测)
5. [文件夹管理](#文件夹管理)
6. [邮件处理](#邮件处理)
7. [前后端交互](#前后端交互)
8. [调试指南](#调试指南)

---

## 同步架构概述

### 系统组件

```
┌─────────────────────────────────────────────────────────────────┐
│                         前端 (Vue 3)                            │
│  ┌──────────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   SyncStore      │  │ AccountStore │  │   EmailStore     │  │
│  └────────┬─────────┘  └──────┬───────┘  └──────────────────┘  │
│           │                   │                                  │
└───────────┼───────────────────┼──────────────────────────────────┘
            │ Tauri IPC          │
            ▼                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                      后端 (Rust/Tauri)                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    SyncManager                           │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │         AuthManager  │    ProviderPool            │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │     ChangeDetector  │   DeltaSync                │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │    FolderManager   │   MailProcessor            │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │    IMAP Client     │   Database                 │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 关键文件位置

| 组件 | 文件路径 |
|------|----------|
| 同步管理器 | `src-tauri/src/sync/sync_manager.rs` |
| 增量同步 | `src-tauri/src/sync/delta_sync.rs` |
| 变更检测 | `src-tauri/src/sync/change_detector.rs` |
| 文件夹管理 | `src-tauri/src/sync/folder_manager.rs` |
| 邮件处理 | `src-tauri/src/sync/mail_processor.rs` |
| 同步命令 | `src-tauri/src/command/sync.rs` |
| 前端同步 Store | `src/stores/sync.ts` |

---

## 完整同步流程

### 流程图

```
用户触发同步
   │
   ▼
1. 前端调用 syncStore.syncAccount(accountId)
   │
   ├─ startListening(accountId) - 监听进度事件
   ├─ invoke('sync_account_with_progress', { accountId })
   │
   ▼
2. 后端接收命令 (command/sync.rs)
   │
   ├─ 创建 SyncManager
   └─ sync_manager.sync_account(account_id)
   │
   ▼
3. 连接到 IMAP 服务器
   │
   ├─ 获取账号信息
   ├─ 检测服务商
   ├─ 获取 IMAP 配置
   ├─ 获取认证信息 (AuthManager)
   └─ imap_client.connect()
   │
   ▼
4. 同步文件夹列表
   │
   ├─ imap_client.list_folders_with_attributes()
   ├─ folder_manager.sync_folders_from_info()
   │  └─ 创建/更新文件夹记录
   │
   ▼
5. 对每个文件夹执行增量同步
   │
   ├─ 发送进度事件 (syncing_emails)
   ├─ sync_folder_internal()
   │  │
   │  ├─ 5.1 检查 CONDSTORE 支持
   │  │      └─ imap_client.check_condstore_support()
   │  │
   │  ├─ 5.2 获取服务器 UID 列表
   │  │      └─ imap_client.list_uids_since(folder, date_since)
   │  │      └─ 默认获取最近3个月的邮件
   │  │
   │  ├─ 5.3 CONDSTORE 增量同步
   │  │      └─ imap_client.search_modified_since(last_modseq)
   │  │      └─ 返回修改的 UID 列表
   │  │
   │  ├─ 5.4 变更检测
   │  │      └─ change_detector.detect_changes()
   │  │      └─ 对比本地和服务器 UID
   │  │      └─ 检测新增、修改、删除
   │  │
   │  ├─ 5.5 获取邮件内容
   │  │      └─ imap_client.fetch_email(folder, uid)
   │  │      └─ 对新邮件和修改的邮件
   │  │
   ├─ 5.6 批量处理邮件
   │  │      └─ mail_processor.process_mails()
   │  │      └─ 保存或更新到数据库
   │  │
   ├─ 5.7 删除已移除的邮件
   │  │      └─ mail_processor.delete_mails()
   │  │
   └─ 5.8 更新同步状态
      └─ sync_state_manager.update_sync_completed()
```

### 函数调用栈

#### 前端（Vue + TypeScript）

```typescript
// 1. 用户触发同步
syncStore.syncAccount(accountId, accountEmail)
  → startListening(accountId)
     → listen(`sync-progress-${accountId}`, handleProgressEvent)
  → invoke('sync_account_with_progress', { accountId })

// 2. 处理进度事件
syncStore.handleProgressEvent(accountId, progress)
  → 根据 progress.stage 更新状态
  → 计算进度百分比
  → 更新 syncStatuses

// 3. 完成后
syncStore.syncAccount()
  → await invoke('sync_account_with_progress', ...)
  → const status = this.syncStatuses.get(accountId)
  → addToHistory(accountId, accountEmail, result)
```

#### 后端（Rust）

```rust
// 1. 命令入口
command/sync.rs::sync_account_with_progress()
  → sync_manager = SyncManager::new(db, app_handle, auth_manager, provider_pool)
  → sync_manager.sync_account(account_id)
     → 返回 SyncResult

// 2. 同步管理器
sync/sync_manager.rs::sync_account()
  → // 1. 获取账号信息
  → storage::AccountRepository::get_by_id(&db, account_id)

  → // 2. 检测服务商
  → provider_pool.detect_provider(&account.email)

  → // 3. 连接 IMAP
  → connect_imap(account_id, &imap_config, &account.email, &account.auth_type)
     → auth_manager.get_imap_auth(account_id, email, &auth_type)
     → AsyncImapClient::connect(host, port, email, imap_auth)

  → // 4. 同步文件夹
  → imap_client.list_folders_with_attributes()
  → folder_manager.sync_folders_from_info(account_id, &folder_infos)

  → // 5. 同步每个文件夹
  → for folder_info in folder_infos:
      → sync_folder_internal(account_id, &folder_info.name, &mut imap_client)
         → check_condstore_support()
         → list_uids_since(folder, &date_since)
         → search_modified_since(last_modseq)
         → sync_folder(account_id, folder, server_uids, imap_client, ...)
            → change_detector.detect_changes()
            → imap_client.fetch_email(folder, uid)
            → mail_processor.process_mails()
            → mail_processor.delete_mails()
            → sync_state_manager.update_sync_completed()

  → // 6. 发送完成事件
  → emit_progress(account_id, SyncProgress { stage: Completed, ... })
```

---

## 增量同步机制

### 同步策略

```rust
pub enum SyncStrategy {
    Condstore,  // 使用 MODSEQ 增量同步
    UidSearch,  // 使用 UID 搜索对比
    FullSync,    // 完整同步
}
```

### CONDSTORE 策略

```
支持 CONDSTORE 的服务器:
  1. SELECT folder (CONDSTORE)
  2. 获取 HIGHESTMODSEQ
  3. SEARCH MODSEQ <last_modseq>:*
  4. 只获取修改的邮件

优势:
  - 只下载变更的邮件
  - 减少网络流量
  - 提高同步速度
```

### UID 搜索策略

```
不支持 CONDSTORE 的服务器:
  1. SEARCH SINCE <date>
  2. 获取所有 UID 列表
  3. 对比本地 UID 检测变更

变更检测:
  - 新邮件: 服务器有，本地没有
  - 删除邮件: 本地有，服务器没有
  - 标志变更: 对比标志状态
```

### 时间窗口

```rust
// 默认同步最近3个月的邮件
let three_months_ago = chrono::Utc::now() - chrono::Duration::days(90);
let date_since = format_imap_date(three_months_ago);

// IMAP SINCE 格式: dd-MMM-yyyy
// 示例: "20-Dec-2025"
```

---

## 变更检测

### EmailFlags 结构

```rust
pub struct EmailFlags {
    pub seen: bool,       // \Seen - 已读
    pub flagged: bool,    // \Flagged - 已标记
    pub answered: bool,   // \Answered - 已回复
    pub draft: bool,      // \Draft - 草稿
    pub deleted: bool,    // \Deleted - 已删除
    pub recent: bool,     // \Recent - 最近（只读）
}
```

### 变更检测类型

```rust
pub enum ChangeType {
    NewEmail { uid: u32 },
    FlagsChanged { uid: u32, old_flags: Vec<String>, new_flags: Vec<String> },
    EmailDeleted { uid: u32 },
}
```

### 检测流程

```
detect_changes(server_uids, last_sync_uid, supports_condstore)
  │
  ├─ 1. 检测新邮件
  │  ├─ 获取本地 UID 列表
  │  ├─ 过滤出服务器有但本地没有的 UID
  │  └─ 进一步按 last_sync_uid 过滤
  │
  ├─ 2. 检测删除的邮件
  │  ├─ 获取本地 UID 列表
  │  └─ 计算差集 (本地 - 服务器)
  │
  └─ 3. 检测标志变更
     ├─ 获取本地标志
     ├─ 获取服务器标志
     └─ 对比标志差异
```

### 差集计算

```rust
// UidSet 工具
local_set.difference(&server_set)
  → 返回 local 有但 server 没有的 UID

// 应用场景: 检测删除邮件
let deleted_emails = local_set.difference(&server_set)
```

---

## 文件夹管理

### Special-Use 文件夹类型

```rust
pub enum SpecialUse {
    Inbox,      // 收件箱 (\Inbox)
    Drafts,     // 草稿箱 (\Drafts)
    Sent,       // 已发送 (\Sent)
    Trash,      // 垃圾箱 (\Trash)
    Junk,       // 垃圾邮件 (\Junk)
    Important,  // 重要邮件 (\Important)
    Archive,    // 归档 (\Archive)
    All,        // 全部邮件 (\All)
    Flagged,    // 标记邮件 (\Flagged)
    Normal,     // 普通文件夹
}
```

### 文件夹同步流程

```
sync_folders_from_info(account_id, folder_infos)
  │
  ├─ 1. 转换 FolderInfo → ImapFolder
  │  ├─ 解析 Special-Use 标志
  │  └─ 推断文件夹类型
  │
  ├─ 2. 获取本地文件夹列表
  │  └─ folder::Entity::find().all()
  │
  ├─ 3. 同步每个文件夹
  │  ├─ 如果本地存在: update_folder()
  │  │  └─ 更新 uidvalidity, uidnext, highest_modseq
  │  └─ 如果本地不存在: create_folder()
  │     └─ 插入新记录
  │
  └─ 4. 返回同步结果
     └─ FolderSyncResult { new_folders, updated_folders, total_folders }
```

### 文件夹名称推断

```rust
infer_special_use_from_name(name: &str) -> SpecialUse
  → "inbox" / "INBOX" → Inbox
  → contains("sent") / "已发送" → Sent
  → contains("draft") / "草稿" → Drafts
  → contains("trash") / "已删除" → Trash
  → contains("junk") / "spam" / "垃圾邮件" → Junk
  → contains("archive") / "归档" → Archive
  → 其他 → Normal (默认为 Inbox)
```

---

## 邮件处理

### 处理流程

```
process_mails(account_id, folder, mails)
  │
  ├─ for each mail in mails:
  │
  ├─ 1. 检查邮件是否已存在
  │  └─ check_mail_exists(account_id, folder, uid)
  │
  ├─ 2a. 如果已存在
  │     └─ update_mail_flags()
  │        └─ 更新 is_read, is_starred, is_draft
  │
  └─ 2b. 如果不存在
        └─ insert 新邮件
           ├─ 设置基本信息 (uid, folder, subject, ...)
           ├─ 设置发件人/收件人
           ├─ 设置正文 (body_text, body_html)
           ├─ 设置标志 (is_read, is_starred, ...)
           └─ 设置时间戳 (sent_at, received_at)
```

### EmailData → MailData 转换

```rust
from_imap_email(email_data, folder)
  → 标准化文件夹名称
  → 提取发件人名称和邮箱
  → 序列化收件人列表 (JSON)
  → 解析 IMAP 标志
  → 转换时间戳
```

### 批量处理结果

```rust
pub struct MailProcessResult {
    pub success_count: usize,   // 成功处理数
    pub failed_count: usize,    // 失败数
    pub skipped_count: usize,   // 跳过数（已存在）
    pub total_count: usize,     // 总数
}
```

---

## 前后端交互

### 事件系统

```typescript
// 后端发送事件
emit("sync-progress-{account_id}", SyncProgress)

// 前端监听事件
listen(`sync-progress-${accountId}`, (event) => {
    const progress = event.payload
    // 更新进度条
})
```

### SyncProgress 事件

```typescript
interface SyncProgressEvent {
  account_id: number
  stage: 'connecting' | 'syncing_folders' | 'syncing_emails' | 'completed' | 'error'
  folder?: string           // 当前处理的文件夹
  current: number           // 当前进度
  total: number             // 总数
  message: string           // 状态消息
}
```

### 阶段转换

```
connecting → syncing
  ├─ syncing_folders → syncing
  ├─ syncing_emails → syncing
  └─ completed → completed
  └─ error → error

状态管理:
  - syncingAccounts: Set<account_id>
  - syncStatuses: Map<account_id, SyncStatus>
```

### 命令接口

| 命令 | 说明 | 返回值 |
|------|------|--------|
| `sync_account_with_progress` | 带进度的同步 | `()` |
| `sync_account` | 简化版同步 | `usize` (邮件总数) |
| `trigger_sync` | 触发一次性同步 | `()` |

---

## 调试指南

### 前端调试

1. **查看同步状态**
   ```javascript
   // 在浏览器控制台
   import { useSyncStore } from '@/stores'
   const syncStore = useSyncStore()
   console.log(syncStore.syncStatuses)
   console.log(syncStore.syncingAccounts)
   ```

2. **监听进度事件**
   ```javascript
   // 查看事件监听器
   console.log(syncStore.unlisteners)
   ```

3. **查看同步历史**
   ```javascript
   syncStore.recentHistory
   ```

### 后端调试

1. **启用详细日志**
   ```bash
   RUST_LOG=sync=debug,imap=debug npm run tauri dev
   ```

2. **关键日志点**
   ```
   [sync] - 同步操作
   [change_detector] - 变更检测
   [mail_processor] - 邮件处理
   [folder_manager] - 文件夹管理
   ```

3. **数据库查询**
   ```sql
   -- 查看同步状态
   SELECT * FROM sync_state WHERE account_id = 1;

   -- 查看邮件数量
   SELECT folder, COUNT(*) as count
   FROM emails
   WHERE account_id = 1
   GROUP BY folder;

   -- 查看最新同步时间
   SELECT MAX(updated_at) as last_sync
   FROM emails
   WHERE account_id = 1;
   ```

4. **性能分析**
   ```rust
   // 在代码中添加计时
   let start = std::time::Instant::now();
   // ... 同步操作 ...
   let duration = start.elapsed();
   tracing::info!("同步耗时: {:?}", duration);
   ```

### 常见问题排查

| 问题 | 可能原因 | 排查方法 |
|------|----------|----------|
| 同步无邮件 | 时间窗口过小 | 检查 date_since 设置 |
| 邮件重复 | UID 列表错误 | 检查 check_mail_exists |
| 标志未更新 | 变更检测失败 | 检查 EmailFlags 解析 |
| 同步卡住 | IMAP 连接超时 | 检查网络和服务器状态 |
| 文件夹错误 | Special-Use 推断失败 | 检查文件夹名称映射 |

### IMAP 调试

```bash
# 手动测试 IMAP 连接
openssl sconnect -crlf imap.gmail.com:993

# 登录测试
# . LOGIN user@example.com password
# . SELECT INBOX
# . LIST "" *
# . SEARCH SINCE 20-Dec-2025
# . FETCH 1 (BODY.PEEK[])
```

---

## 附录：数据结构

### SyncProgress (后端)

```rust
pub struct SyncProgress {
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}
```

### SyncResult (后端)

```rust
pub struct SyncResult {
    pub total_synced: usize,
    pub folders_synced: usize,
    pub errors: usize,
    pub duration_ms: u64,
}
```

### DeltaSyncResult (后端)

```rust
pub struct DeltaSyncResult {
    pub strategy_used: SyncStrategy,
    pub new_emails: usize,
    pub modified_emails: usize,
    pub deleted_emails: usize,
    pub flags_changed: usize,
    pub duration_ms: u64,
}
```

### ChangeDetectionResult (后端)

```rust
pub struct ChangeDetectionResult {
    pub new_emails: Vec<u32>,
    pub modified_emails: Vec<u32>,
    pub deleted_emails: Vec<u32>,
}
```

### FolderSyncResult (后端)

```rust
pub struct FolderSyncResult {
    pub new_folders: usize,
    pub updated_folders: usize,
    pub deleted_folders: usize,
    pub total_folders: usize,
}
```

---

*最后更新: 2026-03-20*
