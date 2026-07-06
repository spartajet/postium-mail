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

---

## Review Fix 追加报告

### 背景

Task 2 首轮实现通过了基础附件 MIME 与 BCC 隐私测试，但 review 指出两个重要遗漏：

1. **BCC 投递语义被破坏**：raw RFC822 已隐藏 `Bcc:`，但 SMTP 发送仍走 `transport.send(message)`，而 `message` 本身不再携带 BCC，导致隐藏收件人不会进入 SMTP envelope。
2. **测试覆盖不足**：缺少对附件失败路径、无效 content type、附件 MIME 出现在 raw、文件名清洗，以及 “BCC 进入 envelope 但不泄露到 raw” 的定向证明。

### Root Cause

问题根因有两个：

1. `build_email` 只修了 header 层，不维护独立的 SMTP envelope。
2. `SmtpClient::send_built_email` 仍基于 `lettre::Message` 调用 `AsyncTransport::send(...)`，其 envelope 来源于 message headers；当 message 不再包含 BCC 时，SMTP 投递目标也随之丢失。

### 修复内容

本轮在允许范围内完成最小修复：

#### 1. `src-tauri/src/service/mail_send.rs`

- 为 `BuiltEmail` 增加 `envelope: lettre::address::Envelope`。
- `build_email` 中显式构建 SMTP envelope：
  - `from` 取 `account.email` 解析为 `lettre::address::Address`
  - `recipients` 汇总 `to + cc + bcc`
- raw RFC822 仍仅写入 `To/Cc`，继续保持：
  - 不含 `Bcc:` header
  - 不泄露隐藏地址

#### 2. `src-tauri/src/infrastructure/protocols/smtp.rs`

- 将 `SmtpClient::send_built_email` 改为接收 `&BuiltEmail`
- 发送路径改用：

```rust
transport.send_raw(&email.envelope, &email.raw).await
```

这样 SMTP envelope 与 raw RFC822 解耦，能够同时满足：

- **SMTP 层投递到 BCC**
- **raw 邮件内容不暴露 BCC**

#### 3. `src-tauri/tests/email_commands.rs`

新增 focused tests：

1. `build_email_envelope_includes_bcc_while_raw_hides_it`
2. `build_email_rejects_invalid_attachment_content_type`
3. `build_email_reports_attachment_read_failure`
4. `build_email_attachment_content_type_appears_in_raw`
5. `build_email_sanitizes_attachment_filename`

这些测试连同已有测试一起覆盖 reviewer 要求的行为面。

### TDD / 验证过程

#### Red

先补测试，再运行：

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
```

首次编译失败，直接暴露缺失的 `BuiltEmail.envelope` 与发送链路不匹配问题，构成这轮修复的 red 证据。

#### Green

完成 envelope 与 `send_raw` 接入后，再次运行定向测试；其中 “附件读取失败” 用例最初误命中“附件不存在”分支，后调整为“文件存在但不可读”，从而稳定覆盖目标错误路径。

#### Verify

最终执行并通过：

```bash
rtk cargo test --test email_commands build_email_ -- --nocapture
rtk cargo fmt --check
rtk cargo check --tests
```

结果：

- `cargo test`: `8 passed, 54 filtered out`
- `cargo fmt --check`: 通过
- `cargo check --tests`: 通过（仅有既有 warning，无新增错误）

### 最终结论

这轮修复后，Task 2 的两个 review blocking findings 已处理：

1. **BCC 现在进入 SMTP envelope，并继续从 raw RFC822 中隐藏**
2. **附件 MIME 与错误路径的关键测试覆盖已补齐**

### 备注

`build_email_reports_attachment_read_failure` 当前在 Unix 环境下通过将文件权限设为 `000` 触发读取失败。当前开发/CI 环境为 Linux，行为成立；若后续需要跨平台完全一致，可再抽象更稳定的 I/O 注入点，但这超出本次最小修复范围。
