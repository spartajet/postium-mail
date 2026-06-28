# Task 3 报告：Backend Service and Command

## 状态

已完成。

## 实现内容

- 扩展 `MailRemoteOperator::reload_email`，真实实现通过 `ImapClient::fetch_email_by_uid(folder, uid)` 拉取单封远端邮件。
- 新增 `ReloadEmailResult`：
  - `Reloaded { email }`：远端 UID 存在，替换本地邮件和附件元数据后返回最新详情。
  - `Removed { email_id }`：远端 UID 明确不存在，删除本地邮件和附件元数据。
- 新增 `EmailService::reload_email`，遵循远端优先语义：
  - IMAP/auth/parse/db 错误会直接返回错误，不主动修改本地邮件。
  - 仅当远端返回 `None` 时删除本地邮件和附件元数据。
  - 不触发文件夹同步，不更新 `uidnext`、`uidvalidity`、`last_sync_uid`。
- 新增 Tauri command `reload_email` 并注册到 command handler。
- 扩展后端测试 fake remote 与测试 common noop remote。

## 测试覆盖

- `test_reload_email_replaces_local_email_when_remote_exists`
  - 验证远端存在时替换本地主题、正文、标记和发件人。
- `test_reload_email_removes_local_email_when_remote_uid_missing`
  - 验证远端 UID 不存在时返回 `Removed` 并删除本地邮件。
- `test_reload_email_remote_error_keeps_local_email`
  - 验证远端错误时本地主题和正文保持不变。

## 验证命令

- `rtk cargo test --manifest-path src-tauri/Cargo.toml reload_email -- --nocapture`
  - 通过：3 passed, 94 filtered out。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands reload -- --nocapture`
  - 通过：3 passed, 31 filtered out。
- `rtk cargo check --manifest-path src-tauri/Cargo.toml`
  - 通过：Finished `dev` profile。

## 注意事项

- `cargo check` 在 debug 配置下会导出 `src/lib/bindings.ts`，该自动生成改动已按 brief 要求还原，未纳入提交。
- 现有 `WholeEmailDto` 没有 `updated_at` 字段，测试 helper 已按当前代码模型调整。
- `MailError` 不实现 `Clone`，测试 fake 使用一次性 `Option<Result<...>>` 存储 reload 结果。

## Fix worker 修复记录

- 修复 reviewer 指出的 Important 问题：`EmailService::reload_email` 在 `replace_email_with_attachments` 成功后不再调用 `self.get(updated.id).await?`，避免替换已提交后因额外数据库读取失败而返回错误。
- 新增私有 helper `email_model_to_detail`，直接用 `replace_email_with_attachments` 返回的 `emails::Model` 构造 `EmailDetail`。
- `Reloaded` 响应的 `has_attachments` 使用远端附件列表在替换前计算出的布尔值，其他详情字段来自更新后的模型。
- 补强 `test_reload_email_replaces_local_email_when_remote_exists`：直接 match `ReloadEmailResult::Reloaded { email }`，断言响应 detail 字段，不再依赖二次 `get()` 证明响应内容。

## Fix worker 验证命令

- `rtk cargo test --manifest-path src-tauri/Cargo.toml reload_email -- --nocapture`
  - 通过：3 passed, 94 filtered out。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands reload -- --nocapture`
  - 通过：3 passed, 31 filtered out。
- `rtk cargo check --manifest-path src-tauri/Cargo.toml`
  - 通过：Finished `dev` profile。
