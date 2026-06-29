# Task 4 报告：AttachmentService 缓存、保存和打开

## 状态

已完成，未创建 commit。

## 实现内容

- 扩展 `MailRemoteOperator`：
  - 新增 `fetch_attachment_section(account, folder, uid, section_path)`。
  - `RealMailRemoteOperator` 通过 `ImapClient::fetch_body_section_with_mime` 拉取指定 MIME section，并在结束后 best-effort logout。
  - 测试用 `FakeMailRemote` 与 `NoopMailRemoteOperator` 已同步实现新 trait 方法。
- 实现 `AttachmentService`：
  - `new(db, auth, data_dir)`：生产构造，内部创建 `RealMailRemoteOperator`，缓存根目录为 `data_dir/attachments-cache`。
  - `new_with_remote(db, remote, cache_root)`：测试/依赖注入构造。
  - `ensure_cached(attachment_id)`：小附件下载、按 Content-Transfer-Encoding 解码、写入安全缓存路径，并更新数据库 `attachments.path`。
  - `save_as(attachment_id, target_path)`：小附件优先走缓存复制；大附件直接远端拉取写入目标路径，不更新缓存 path。
  - `open(attachment_id)`：先 `ensure_cached`，再调用 `tauri_plugin_opener::open_path(path, None::<&str>)`。
  - `resolve_inline_images(email_id)`：筛选 image/* 且小于等于 10MB 的 inline 附件，缓存后返回 `content_id -> 本地路径` 映射。
- 新增解码和路径工具：
  - 支持 `base64`、`quoted-printable`、`7bit`、`8bit`、`binary`。
  - 对文件名中的控制字符、`/`、`\`、`:` 做替换，并避免空文件名。
- 在 `service/mod.rs` 导出 `AttachmentService`。
- 在 `lib.rs` 中创建并 `.manage(attachment_service)`，未新增 Tauri command、dialog plugin 或前端 UI。

## 测试覆盖

- 新增测试：
  - `ensure_cached_downloads_small_attachment_and_updates_path`
    - 验证小附件从 fake remote 下载，写入缓存文件，并更新 DTO/cache path。
  - `save_as_copies_small_attachment_from_cache_and_updates_path`
    - 验证小附件 `save_as` 复用缓存复制到目标路径，并更新数据库 path。
  - `save_as_writes_large_attachment_directly_without_updating_path`
    - 验证大附件直接写目标路径，不更新数据库 path。
- `open` 未直接调用系统 opener 做自动化测试，避免测试环境实际打开文件；实现路径通过 `ensure_cached` 的测试间接覆盖缓存前置逻辑，`open_path` API 已按本地 crate 真实签名确认。
- `resolve_inline_images` 本轮实现但未单独加 focused test，后续 Task 8 接前端本地 URL 时可补充端到端覆盖。

## TDD RED/GREEN

- RED：
  - 命令：`rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml ensure_cached_downloads_small_attachment_and_updates_path`
  - 结果：失败，符合预期。
  - 关键失败：
    - `method fetch_attachment_section is not a member of trait MailRemoteOperator`
    - `cannot find AttachmentService in service`
- GREEN：
  - 实现 trait、服务、导出和 Tauri state 管理后，同一 focused test 通过。
  - 后续补充 `save_as` 两个测试，其中一次失败是测试断言依赖全局账号计数并发顺序，已改为稳定断言 uid/section 调用后通过。

## 验证命令和结果

- `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml ensure_cached_downloads_small_attachment_and_updates_path`
  - 通过：`1 passed`。
- `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml attachment`
  - 通过：相关过滤下 `email_commands` 中 5 个测试通过，其中包含 3 个本次新增附件服务测试。
  - 仍有 `tests/common/real_mail.rs` 的 dead_code warnings，为既有测试辅助代码警告。
- `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo check --manifest-path src-tauri/Cargo.toml`
  - 通过。
- `rtk cargo check --manifest-path src-tauri/Cargo.toml`
  - 通过，未遇到 `src-tauri/target` 空间不足。
- `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - 通过。

## 修改文件

- `src-tauri/src/service/attachment_service.rs`
- `src-tauri/src/service/mail_operation.rs`
- `src-tauri/src/service/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tests/common/mod.rs`
- `src-tauri/tests/email_commands.rs`
- `.superpowers/sdd/task-4-report.md`

## 自审

- 范围符合 Task 4：只做附件缓存/保存/打开/inline 映射、remote section fetch 和服务管理。
- 未新增 Tauri command、dialog plugin、前端 UI。
- 未执行 `git commit`。
- 未 revert 其他任务的改动；当前工作区仍包含 Task 1/2/3 的既有改动。
- `save_as` 对大附件不会写缓存，也不会更新 `attachments.path`。
- 缓存写入使用临时文件再 rename，减少半写文件暴露。
- 缓存命中时会检查磁盘文件存在；数据库 path 指向丢失文件时会重新下载。

## 疑虑

- `resolve_inline_images` 目前返回本地文件路径，符合 brief 中“Task 8 再接入前端本地 URL”的阶段性要求。
- `open` 没有做自动化测试以避免在测试环境启动系统 opener。
- `decode_body` 对 base64 使用标准引擎；如遇 MIME body 含换行但 parser 未清理，需根据真实 IMAP 数据再评估是否要做 whitespace 容忍处理。

## Reviewer 修复记录：base64 MIME 折行

- 修复 `decode_body` 的 `base64` 分支：解码前移除 ASCII whitespace（空格、tab、CR、LF 等），再交给 `base64::engine::general_purpose::STANDARD.decode`。
- 新增聚焦测试 `ensure_cached_decodes_base64_attachment_with_mime_whitespace`：
  - fake remote 返回 `aGVs\r\nbG8g\r\nYXR0YWNobWVudA==`。
  - `transfer_encoding = base64`。
  - 验证 `ensure_cached` 写入缓存后的文件内容为原始 bytes：`hello attachment`。
- RED 验证：
  - `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml ensure_cached_decodes_base64_attachment_with_mime_whitespace`
  - 修改前失败：`AttachmentDecodeFailed("Invalid symbol 13, offset 4.")`。
- GREEN 验证：
  - `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml base64`
  - 通过：`ensure_cached_decodes_base64_attachment_with_mime_whitespace ... ok`。
  - `rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo check --manifest-path src-tauri/Cargo.toml`
  - 通过。
- 未创建 commit。
