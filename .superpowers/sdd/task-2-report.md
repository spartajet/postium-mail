# Task 2 执行报告

## 任务目标

在不扩大任务范围的前提下，完成以下两点：

1. 让 `build_email` 在存在普通本地文件附件时构建 `multipart/mixed` MIME，同时保留正文的 `multipart/alternative`。
2. 确保 raw RFC822 不写出 `Bcc:` 头，也不泄露隐藏地址。

受限文件：

- `src-tauri/src/service/mail_send.rs`
- `src-tauri/tests/email_commands.rs`

## TDD 过程

### Red

先在 `src-tauri/tests/email_commands.rs` 新增并调整了以下测试：

1. `build_email_with_attachment_uses_multipart_mixed`
2. `build_email_does_not_write_bcc_header`

随后运行：

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
```

结果：定向测试失败，确认新行为尚未被现有实现满足，具备 TDD 的 red 证据。

### Green

在 `src-tauri/src/service/mail_send.rs` 中完成实现：

1. 抽出 `build_body_part(req)`，统一构建正文 `multipart/alternative`。
2. 当 `req.attachments` 为空时，保持原有 alternative MIME。
3. 当存在附件时，外层切换为 `multipart/mixed`，逐个读取本地文件并挂载为普通附件。
4. 新增 `sanitize_attachment_filename`，清理文件名中的 `/`、`\\`、`\0`。
5. 新增 `content_type_header`，对附件 MIME 类型做解析和错误转换。
6. BCC 改为仅校验地址格式，不再写入 `lettre::Message` 头字段。

### Verify

实现后运行：

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
rtk cargo fmt --check
rtk git diff -- src-tauri/src/service/mail_send.rs src-tauri/tests/email_commands.rs
```

结果：

- 定向测试通过
- `cargo fmt --check` 通过
- diff 仅落在任务限定文件

## 变更说明

### `src-tauri/src/service/mail_send.rs`

- 新增附件 MIME 构建逻辑，支持普通本地文件附件。
- raw 邮件在有附件时为 `multipart/mixed`，内部正文仍为 `multipart/alternative`。
- BCC 不再进入 RFC822 头；当前只保留地址合法性校验。

### `src-tauri/tests/email_commands.rs`

- 增加附件 MIME 构建测试。
- 增加 BCC 隐私测试。

## 验证摘要

执行命令：

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
rtk cargo fmt --check
```

结果：

- `cargo test`: `3 passed, 54 filtered out`
- `cargo fmt --check`: 通过

## 风险与后续关注

1. 当前实现优先满足隐私要求：raw RFC822 中不再包含 `Bcc:` 与隐藏地址。
2. 由于本任务明确不要扩大范围重写 SMTP envelope，而当前发送路径依赖 `lettre::Message`，未来如果 UI 暴露 BCC，可能需要额外使用 lettre envelope API 来确保“隐藏投递但不暴露头信息”同时成立。
3. 本任务附件第一版仅覆盖普通本地文件附件，符合 brief 约束，未扩展到远端附件、内联资源或草稿附件恢复。
