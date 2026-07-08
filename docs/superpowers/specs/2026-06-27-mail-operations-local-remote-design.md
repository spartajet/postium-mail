# 邮件操作本地与远端一致性设计

日期：2026-06-27

## 背景

当前邮件操作存在两个层面的问题：

1. 前端部分已有入口没有接通真实操作：
   - `EmailDetail.svelte` 的归档按钮仍是占位。
   - `EmailContextMenu.svelte` 的所有菜单操作仍是 TODO。
   - `EmailList.svelte` 的刷新按钮和列表/网格切换按钮仍是占位。
2. 后端状态操作只更新本地数据库，没有同步到 IMAP：
   - `EmailService::mark_as_read` 只调用 `email_repo::mark_as_read`。
   - `email_repo::mark_as_read` 只执行 `UPDATE emails SET is_read = ?1 WHERE id = ?2`。
   - `toggle_star`、`delete`、`move_to_folder` 也只更新本地字段。

IMAP 协议层已经有基础 flag 能力：

- `ImapClient::add_flags(uid, flags)`
- `ImapClient::remove_flags(uid, flags)`
- 底层使用 `UID STORE`

但这些能力没有被 `EmailService` 的邮件操作路径使用。

## 目标

第一期目标是完善现有邮件操作入口，并保证邮件状态类操作同时处理远端 IMAP 和本地数据库。

需要满足：

1. 已读/未读操作写入 IMAP `\Seen`，远端成功后更新本地 `is_read`。
2. 星标/取消星标操作写入 IMAP `\Flagged`，远端成功后更新本地 `is_starred`。
3. 删除操作第一期采用保守模式：远端移动到 Trash，成功后本地跟随更新；Trash 中再次删除不执行永久删除。
4. 归档操作远端移动到 Archive，成功后本地 `folder` 跟随更新。
5. 移动文件夹能力作为底层能力提供，供归档、删除和未来更多操作复用。
6. 前端接通已有入口：详情页归档、右键菜单星标/已读/删除/转发、列表刷新。
7. 远端失败时不更新本地状态，前端保留原 UI 状态并提示错误。

## 非目标

第一期不做：

1. 不做邮件回复相关增强。
2. 不接入真实标签系统。
3. 不做批量选择 UI。
4. 不做离线操作队列。
5. 不做永久删除，不执行 `EXPUNGE`。
6. 不做完整 IMAP 冲突合并策略。
7. 不实现列表/网格视图切换。
8. 不展开“更多操作”子菜单。

## 推荐方案

采用“新增 `mail_operation` 模块 + 前端接通现有入口”的方案。

### 后端边界

新增 `src-tauri/src/service/mail_operation.rs`，作为邮件操作的统一执行层。

职责：

1. 根据 `email_id` 读取邮件记录，获取：
   - `account_id`
   - `folder`
   - `uid`
2. 读取账号记录。
3. 解析账号 IMAP 配置。
4. 获取认证凭证。
5. 建立 IMAP 连接。
6. 选择邮件当前所在 folder。
7. 执行远端 IMAP 操作。
8. 远端成功后更新本地数据库。

`EmailService` 继续作为 Tauri command 背后的服务入口，具体操作委托给 `mail_operation`。

Repository 继续只负责数据库，不接触 IMAP。

### 一致性策略

采用“远端优先，本地跟随”：

1. 远端 IMAP 操作成功前，不更新本地数据库。
2. 远端 IMAP 操作失败时，直接返回错误，本地状态保持不变。
3. 远端成功、本地更新失败时返回错误，但不尝试回滚 IMAP；后续同步以远端状态为准。
4. 前端不做乐观更新，只在后端命令成功后更新本地 UI 状态。

### 操作语义

#### 已读/未读

`mark_as_read(email_id, true)`：

1. 读取邮件当前 folder 和 uid。
2. `select_folder(folder)`。
3. `add_flags(uid, "\\Seen")`。
4. 本地 `is_read = 1`。

`mark_as_read(email_id, false)`：

1. 读取邮件当前 folder 和 uid。
2. `select_folder(folder)`。
3. `remove_flags(uid, "\\Seen")`。
4. 本地 `is_read = 0`。

#### 星标/取消星标

`toggle_star(email_id)`：

1. 读取邮件当前 `is_starred`、folder 和 uid。
2. 如果当前未星标，远端 `add_flags(uid, "\\Flagged")`。
3. 如果当前已星标，远端 `remove_flags(uid, "\\Flagged")`。
4. 远端成功后，本地更新为相反状态。
5. 返回新状态。

#### 归档

`archive(email_id)`：

1. 根据账号 provider 获取标准 Archive 文件夹。
2. 远端移动邮件到 Archive 文件夹。
3. 本地 `folder = archive_folder`。
4. 如果 provider 没有 Archive 映射，返回明确错误。

#### 删除

`delete(email_ids)`：

1. 对每封邮件读取账号、folder 和 uid。
2. 根据账号 provider 获取标准 Trash 文件夹。
3. 如果邮件不在 Trash：
   - 远端移动到 Trash。
   - 本地 `folder = trash_folder` 或按当前列表语义从当前视图移除。
4. 如果邮件已经在 Trash：
   - 第一期不永久删除。
   - 返回明确错误或 no-op，不执行 `EXPUNGE`，也不只改本地 `is_deleted`。
   - UI 可提示“永久删除将在后续版本支持”。

#### 移动文件夹

`move_to_folder(email_id, target_folder)`：

1. 远端移动到 `target_folder`。
2. 本地 `folder = target_folder`。

IMAP move 能力：

1. 优先使用服务端 MOVE 能力。
2. 如果当前 async-imap/API 不支持直接 MOVE，则使用兼容方案：
   - `UID COPY` 到目标文件夹。
   - 源邮件标记 `\Deleted`。
   - 第一期不执行 `EXPUNGE`，避免不可逆删除。
3. 如果兼容方案会导致源邮件仍保留，需要在 UI 语义中明确“第一期以安全为先，后续同步可能仍看到源邮件”，或者延后该方案直到可以安全实现 MOVE。

## 前端交互范围

第一期只完善已有 UI。

### 详情页

`EmailDetail.svelte`：

1. 星标按钮保留，后端改为远端+本地。
2. 删除按钮保留，后端改为远端+本地。
3. 归档按钮从占位变为真实操作。
4. 转发按钮保留现有行为。

### 右键菜单

`EmailContextMenu.svelte`：

1. 转发：打开写信弹窗，预填 `Fwd:`。
2. 星标/取消星标：调用 `EmailState.toggleStar`。
3. 标为已读/未读：调用 `EmailState.markAsRead`。
4. 删除：调用 `EmailState.deleteEmails`。
5. 回复/全部回复：第一期不做。
6. 更多操作：第一期保留占位或禁用。

右键菜单需要从父组件拿到足够的信息：

1. 对星标、已读、删除，只需要 `emailId/isRead/isStarred`。
2. 对转发，如果列表没有完整正文，则先调用 `getEmail` 获取详情，再打开写信弹窗。

### 列表工具栏

`EmailList.svelte`：

1. 刷新按钮调用当前账号同步。
2. 同步完成后重载当前分类。
3. 列表/网格切换本期不做，保留占位或改成禁用状态。

### Store

`EmailState` 增加或完善：

1. `markAsRead(emailId, isRead)`
2. `archiveEmail(emailId)`
3. `moveEmailToFolder(emailId, folder)`
4. 可选：`operatingIds` 或 `pendingEmailActions`，用于禁用重复操作。

状态更新规则：

1. 命令成功后更新列表和选中详情。
2. 命令失败不更新状态。
3. 错误写入 `EmailState.error`，由组件或 toast 展示。

## 错误处理

后端：

1. 邮件不存在：返回 `EmailNotFound`。
2. 账号不存在：返回 `AccountNotFound`。
3. 邮件缺少 UID：返回明确错误。
4. Provider 不支持目标标准文件夹：返回明确错误。
5. IMAP 连接或 flag/move 失败：返回 IMAP 错误。
6. 远端成功但本地失败：返回数据库错误，不回滚远端。

前端：

1. 操作失败显示 toast 或错误提示。
2. 操作失败不改变当前邮件状态。
3. 操作进行中禁用重复点击。
4. 右键菜单点击后可以立即关闭；失败提示通过 toast 展示。

## 测试策略

### 后端测试

后端测试不访问真实网络。`mail_operation` 需要可注入 IMAP 操作接口。

建议接口：

```rust
#[async_trait]
pub trait MailRemoteOperator: Send + Sync {
    async fn mark_seen(&self, folder: &str, uid: u32, seen: bool) -> Result<(), MailError>;
    async fn set_flagged(&self, folder: &str, uid: u32, flagged: bool) -> Result<(), MailError>;
    async fn move_to_folder(&self, folder: &str, uid: u32, target_folder: &str) -> Result<(), MailError>;
}
```

测试用例：

1. `mark_as_read_remote_success_updates_local`
2. `mark_as_read_remote_failure_keeps_local_state`
3. `toggle_star_remote_success_updates_local`
4. `toggle_star_remote_failure_keeps_local_state`
5. `archive_remote_success_updates_folder`
6. `archive_remote_failure_keeps_folder`
7. `delete_moves_to_trash_without_expunge`

### 前端测试

Store 测试：

1. `markAsRead` 成功更新列表和详情。
2. `markAsRead` 失败不更新列表和详情。
3. `archiveEmail` 成功从当前列表移除或更新 folder。
4. `deleteEmails` 成功从当前列表移除。
5. 失败时写入 `error`。

组件或 E2E 测试：

1. 右键菜单星标调用正确。
2. 右键菜单标已读/未读调用正确。
3. 右键菜单删除调用正确。
4. 详情页归档按钮调用正确。
5. 列表刷新按钮触发同步并重载当前分类。

## 实施顺序建议

1. 后端先补 `mail_operation` 和 fake remote operator 测试。
2. 改造 `EmailService::mark_as_read` 和 `toggle_star`。
3. 增加归档/移动文件夹命令或复用现有 `move_email_to_folder`。
4. 改造删除语义为远端 Trash 移动优先。
5. 前端 `EmailState` 增加缺失方法和错误状态。
6. 接通详情页归档、右键菜单、列表刷新。
7. 跑 Rust 测试、前端测试和 `svelte-check`。

## 待后续设计

以下内容建议单独设计：

1. 标签系统真实接入。
2. 批量选择和批量操作。
3. 永久删除和清空 Trash。
4. 离线操作队列。
5. IMAP 冲突合并和操作重试。
