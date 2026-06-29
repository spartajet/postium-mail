# Task 3 报告：IMAP 单附件 section 下载和解码基础

## 实现内容

- 在 `src-tauri/src/error/types.rs` 增加附件相关错误类型：
  - `AttachmentNotFound(i32)`
  - `AttachmentUnavailable(String)`
  - `AttachmentDownloadFailed(String)`
  - `AttachmentDecodeFailed(String)`
  - `FileSystemError(String)`
- 在 `src-tauri/Cargo.toml` 显式加入 `quoted_printable = "0.5"`，并让 `Cargo.lock` 记录根 crate 的显式依赖。
- 在 `src-tauri/src/infrastructure/protocols/types.rs` 增加 `FetchedBodySection`：
  - `body: Vec<u8>`
  - `transfer_encoding: Option<String>`
- 在 `src-tauri/src/infrastructure/protocols/imap/fetch.rs` 增加：
  - `parse_transfer_encoding`：从 MIME header 中解析 `Content-Transfer-Encoding`，并转为小写。
  - `parse_section_path`：把 `"2"`、`"3.1"` 这类 section path 转为 async-imap 当前 API 使用的 `SectionPath::Part`。
  - `ImapClient::fetch_body_section_with_mime(folder, uid, section_path)`：选择 folder 后通过 `UID FETCH` 同时请求 `BODY.PEEK[<section>.MIME]` 和 `BODY.PEEK[<section>]`，再用 `Fetch::section(...)` 读取 MIME 头和 body。

## async-imap API 适配说明

当前依赖是 `async-imap 0.11.2`，`Fetch` 没有 brief 示例里的 `body_sections()` API。源码中可用的是：

- `Fetch::section(&SectionPath) -> Option<&[u8]>`
- `async_imap::imap_proto::types::{SectionPath, MessageSection}`

因此实现中构造：

- body section：`SectionPath::Part(parts, None)`
- MIME header section：`SectionPath::Part(parts, Some(MessageSection::Mime))`

语义等价于读取 `BODY[<section>]` 和 `BODY[<section>.MIME]`。

## 测试/检查命令和结果

- RED：
  - 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml parse_transfer_encoding_should_return_lowercase_header_value`
  - 结果：失败，原因符合预期：`parse_transfer_encoding` 不存在。
- GREEN：
  - 命令：`rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml parse_transfer_encoding_should_return_lowercase_header_value`
  - 结果：通过，`1 passed`。
  - 说明：首次普通 target 构建静态库时 `/home` 分区空间不足，报 `No space left on device`；改用 `/tmp/postium-mail-target` 完成单测验证。
- 格式化：
  - 命令：`rtk cargo fmt --manifest-path src-tauri/Cargo.toml`
  - 结果：通过。
- 编译检查：
  - 命令：`rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo check --manifest-path src-tauri/Cargo.toml`
  - 结果：通过。
  - 命令：`rtk cargo check --manifest-path src-tauri/Cargo.toml`
  - 结果：通过，`Finished dev profile`。

## 修改文件

- `src-tauri/src/infrastructure/protocols/imap/fetch.rs`
- `src-tauri/src/infrastructure/protocols/types.rs`
- `src-tauri/src/error/types.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `.superpowers/sdd/task-3-report.md`

## 自审

- 范围控制在 Task 3：未实现 AttachmentService 缓存、Tauri 命令或前端。
- `section_path` 会先 `trim`，空字符串返回 `AttachmentUnavailable`。
- 非数字 section path 会返回 `AttachmentUnavailable`，避免构造非法 IMAP section 查询。
- IMAP fetch 失败和 fetch stream item 失败映射为 `AttachmentDownloadFailed`。
- fetch 结果为空或 UID 不匹配返回 `Ok(None)`。
- 找不到 body section 返回 `AttachmentDownloadFailed("附件 section 内容为空")`。
- MIME 头缺失时仍返回 body，`transfer_encoding` 为 `None`。

## 疑虑

- 当前只对 MIME 头解析函数补了 RED/GREEN 单元测试；`fetch_body_section_with_mime` 依赖真实 IMAP session，现有 `ImapClient` 结构不便注入假 session，未新增协议交互单元测试。
- 本任务 brief 标题提到“解码基础”，但接口只要求返回 `transfer_encoding`，没有要求在此层做 base64/quoted-printable 解码；因此本实现只下载原始 section body 并暴露编码信息。
- 工作区已有 Task 1/2 未提交改动，本任务没有提交代码，也没有主动 revert 任何已有改动。

## Reviewer fix：拒绝 0 值 MIME section part

- 修复 `parse_section_path`：空路径、非数字路径、以及任一 part 为 `0` 的路径都会返回 `None`，避免接受非法的 `0` 或 `1.0` BODYSTRUCTURE section path。
- 增加单元测试 `parse_section_path_should_reject_zero_parts`：
  - 验证 `"0"` 返回 `None`。
  - 验证 `"1.0"` 返回 `None`。
  - 验证合法 `"2"` 和 `"3.1"` 仍返回 `Some`。

### Reviewer fix 验证

- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml parse_section_path_should_reject_zero_parts`
  - 结果：失败，原因是默认 `src-tauri/target` 所在分区空间不足，报 `No space left on device (os error 28)`。
- 命令：`rtk env CARGO_TARGET_DIR=/tmp/postium-mail-target cargo test --manifest-path src-tauri/Cargo.toml parse_section_path_should_reject_zero_parts`
  - 结果：通过，`1 passed`。
- 命令：`rtk cargo check --manifest-path src-tauri/Cargo.toml`
  - 结果：通过，`Finished dev profile`。
