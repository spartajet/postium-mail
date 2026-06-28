# Task 2 报告：Repository Replacement and Local Removal

## 状态

已完成。

## 修改文件

- `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- `.superpowers/sdd/task-2-report.md`

## 实现内容

- 新增 `replace_email_with_attachments(db, email_id, account_id, folder, email)`：
  - 在单个数据库事务内覆盖目标邮件记录。
  - 删除该邮件旧的附件数据库元数据。
  - 插入替换邮件的新附件数据库元数据。
  - 回读并返回更新后的 `emails::Model`。
- 新增 `delete_one_with_attachments(db, email_id)`：
  - 在单个数据库事务内删除附件数据库元数据和邮件记录。
  - 只删除数据库记录，不删除磁盘附件文件。
  - 返回邮件记录是否实际删除。
- 新增 repository focused test：
  - `replace_email_with_attachments_overwrites_email_and_attachments`
  - 覆盖邮件字段被替换、旧附件元数据被新附件元数据替换。

## TDD 证据

RED：

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml replace_email_with_attachments_overwrites_email_and_attachments -- --nocapture
```

结果：失败，原因符合预期：

```text
error[E0425]: cannot find function `replace_email_with_attachments` in this scope
```

GREEN：

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml replace_email_with_attachments_overwrites_email_and_attachments -- --nocapture
```

结果：

```text
cargo test: 1 passed, 93 filtered out (8 suites, 0.01s)
```

## 额外检查

```bash
rtk git diff --check -- src-tauri/src/infrastructure/storage/repository/email_repo.rs
```

结果：通过，无输出。

曾运行：

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

结果：失败，但失败包含非本任务写入范围文件 `src-tauri/src/infrastructure/protocols/imap/fetch.rs` 的既有格式差异。已按任务边界仅对 `email_repo.rs` 执行：

```bash
rtk rustfmt --edition 2024 src-tauri/src/infrastructure/storage/repository/email_repo.rs
```

## 约束确认

- 未触发 full folder sync。
- 未更新 `uidnext`、`uidvalidity`、`last_sync_uid` 等 folder sync state。
- 未实现 batch reload。
- 未删除磁盘附件文件。
- 未实现 service、command、frontend。
- repository 操作中使用事务，数据库错误会回滚事务内变更。

## Concerns

- `cargo fmt --check` 当前会被非本任务文件 `src-tauri/src/infrastructure/protocols/imap/fetch.rs` 的格式差异阻断；本任务文件单独格式化并通过 `git diff --check`。
