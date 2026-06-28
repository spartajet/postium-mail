# 单封邮件重新加载设计

## 背景

邮件右键菜单需要新增“重新加载”操作，用于对指定邮件重新从 IMAP 服务器加载完整内容。现有“刷新”按钮是按当前分类触发同步，不适合用户只想修复或更新单封邮件的场景。

当前代码中已有可复用能力：

- 前端右键菜单位于 `src/lib/components/email/EmailContextMenu.svelte`，由 `EmailList.svelte` 传入操作回调。
- 邮件状态管理位于 `src/lib/stores/email.svelte.ts`，已有 `deleteEmails`、`markAsRead`、`toggleStar`、`archiveEmail` 等远端优先操作。
- 后端邮件操作集中在 `EmailService` 与 `MailOperationService`。
- IMAP 层已有按 UID 获取完整正文的基础能力，批量同步也已有完整邮件解析逻辑。
- 本地仓储已有按 `account_id + folder + uid` 更新正文，以及邮件、附件的保存和删除能力。

## 目标

新增单封邮件重载链路：

1. 用户在邮件列表右键菜单点击“重新加载”。
2. 前端调用新的 `reload_email(email_id)` 命令。
3. 后端按本地邮件的 `account_id + folder + uid` 从 IMAP 重新加载完整 RFC822。
4. 远端存在时，覆盖本地邮件头部、正文、预览、标志和附件元数据。
5. 远端不存在时，删除本地邮件和附件，前端从列表移除该邮件。

## 非目标

- 不触发整个文件夹同步。
- 不更新文件夹同步状态，例如 `uidnext`、`uidvalidity`、`last_sync_uid`。
- 不实现批量重载。
- 不处理已下载附件文件的磁盘删除；本阶段只覆盖附件数据库元数据。
- 不在打开邮件详情时自动强制重载。

## 用户体验

右键菜单新增“重新加载”菜单项，位置放在标记操作和删除操作之间，属于非破坏性维护操作。

点击后：

- 菜单关闭。
- 当前邮件进入操作中状态，复用现有 `operatingIds` 禁用重复操作。
- 成功重载时，列表摘要和详情内容更新为服务器最新数据。
- 如果远端邮件不存在，本地列表移除该邮件；若详情正在显示它，则取消选择。
- 失败时保留本地状态，并通过现有错误状态展示失败原因。

## 后端设计

### 命令

新增 Tauri command：

```rust
reload_email(email_id: i32) -> Result<ReloadEmailResult, MailError>
```

返回值使用明确枚举，避免前端依赖错误字符串：

```rust
enum ReloadEmailResult {
    Reloaded { email: EmailDetail },
    Removed { email_id: i32 },
}
```

### 服务层

新增 `EmailService::reload_email(email_id)`，职责是协调本地记录、远端 IMAP 和本地覆盖写入。

流程：

1. 根据 `email_id` 查询本地邮件；不存在则返回 `EmailNotFound`。
2. 查询账号并建立 IMAP 连接。
3. 选择邮件当前本地 `folder`。
4. 按本地 `uid` 获取完整 RFC822，并解析成完整邮件 DTO。
5. 如果远端未返回该 UID：
   - 在事务中删除该邮件及附件。
   - 返回 `ReloadEmailResult::Removed { email_id }`。
6. 如果远端返回邮件：
   - 在事务中覆盖本地 `emails` 行。
   - 删除旧附件元数据，写入新附件元数据。
   - 保留原本的本地 `id`、`account_id`、`folder`、`uid`。
   - 返回最新 `EmailDetail`。

### IMAP 层

新增按单个 UID 获取完整邮件的方法，命名为：

```rust
fetch_email_by_uid(folder: &str, uid: u32) -> Result<Option<WholeEmailDto>, MailError>
```

语义：

- 先 `SELECT folder`。
- 使用 `UID FETCH <uid> (FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[] BODYSTRUCTURE UID)`。
- 没有 fetch 结果时返回 `Ok(None)`。
- 有结果时复用批量完整邮件解析逻辑，返回 `Ok(Some(dto))`。
- 网络、认证、协议和解析错误返回 `Err(MailError)`。

### 本地仓储

新增事务型覆盖方法，命名为：

```rust
replace_email_with_attachments(
    db: &DbConn,
    email_id: i32,
    account_id: i32,
    folder: &str,
    email: WholeEmailDto,
) -> Result<emails::Model, MailError>
```

要求：

- 更新既有 `emails.id = email_id`，不插入新行。
- 覆盖主题、发件人、收件人、抄送、密送、正文、HTML、预览、message_id、标志、时间戳等字段。
- 删除该邮件旧附件元数据，再插入新附件元数据。
- 事务内完成，避免邮件和附件状态不一致。

远端不存在时复用或新增本地删除方法，必须同时删除附件元数据和邮件记录。

## 前端设计

### 绑定

`src/lib/bindings.ts` 增加 `reloadEmail(emailId: number)`，返回 `ReloadEmailResult`。

### Store

`EmailState` 增加：

```ts
async reloadEmail(emailId: number): Promise<void>
```

行为：

- 开始时将 `emailId` 加入 `operatingIds`。
- 调用 `commands.reloadEmail(emailId)`。
- 返回 `Reloaded`：
  - 用返回的 `EmailDetail.email` 更新 `emails` 列表中的同 ID 项。
  - 如果 `selectedEmailId === emailId`，同步替换 `selectedEmail`。
- 返回 `Removed`：
  - 从 `emails` 列表移除该 ID。
  - 如果当前选中该 ID，调用 `deselectEmail()`。
  - `total` 按实际移除数量扣减。
- 返回错误：
  - 设置 `error`。
  - 不修改本地列表和详情。
- 结束时移除 `operatingIds`。

### 菜单

`EmailContextMenu.svelte` 增加 `onReload` prop，并在 `handleAction("reload")` 中调用。

UI：

- 使用 `RefreshCw` 图标。
- 文案为“重新加载”。
- `disabled={disabled}`，和其他远端操作共享禁用状态。

`EmailList.svelte` 增加 `handleContextReload(emailId)`，传给菜单。

## 错误处理

- 本地邮件不存在：返回 `EmailNotFound`，前端保留当前状态。
- 账号不存在：返回 `AccountNotFound`，前端保留当前状态。
- IMAP 连接失败、认证失败、网络失败：返回错误，前端保留当前状态。
- 远端 UID 不存在：返回 `Removed`，本地删除邮件和附件。
- 邮件解析失败：返回错误，前端保留当前状态。
- 本地覆盖写入失败：返回错误；由于写入在事务内完成，不应产生半更新状态。

## 测试计划

### Rust

- `reload_email` 远端存在时覆盖本地邮件详情、正文、标志和附件。
- `reload_email` 远端不存在时删除本地邮件和附件，并返回 `Removed`。
- 远端错误时不修改本地邮件。
- 本地邮件不存在时返回 `EmailNotFound`。
- 仓储覆盖方法事务测试：邮件更新和附件替换一致。

### Frontend

- 右键菜单点击“重新加载”会调用 `onReload(emailId)`。
- `EmailState.reloadEmail` 收到 `Reloaded` 后更新列表项和当前详情。
- `EmailState.reloadEmail` 收到 `Removed` 后移除列表项并取消当前选择。
- 命令失败时保留本地列表和详情，并设置错误状态。
- 操作过程中该邮件进入 `operatingIds`，避免重复触发。

### 验证命令

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json`
- `node node_modules/vitest/vitest.mjs run --pool threads --maxWorkers 1 --reporter dot`
- `npm run build`
