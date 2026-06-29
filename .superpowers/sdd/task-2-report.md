# Task 2 报告：BODYSTRUCTURE 解析补齐 CID 图片

## 实现内容

- 在 `src-tauri/src/infrastructure/protocols/imap/parser.rs` 中补齐 BODYSTRUCTURE 附件识别逻辑。
- 将 `is_attachment` 签名从只接收 `BodyContentCommon` 扩展为同时接收 `BodyContentSinglePart`，以便读取单部分 MIME 节点的 `Content-ID`。
- 保留原有附件判定：
  - `Content-Disposition: attachment`
  - `Content-Disposition` 包含 `filename`
  - `Content-Type` 包含 `name`
- 新增判定：`Content-Type` 主类型为 `image` 且 `other.id` 存在时，识别为附件，用于支持 CID 内联图片。
- 更新 `extract_attachments` 中 `Basic`、`Text`、`Message` 三个单部分分支的 `is_attachment` 调用。
- 新增单测 `detects_inline_cid_image_without_filename_as_attachment`，覆盖无 filename、无 disposition、但有 Content-ID 的 `image/png` 节点。

## TDD RED/GREEN

### RED

命令：

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml detects_inline_cid_image_without_filename_as_attachment
```

结果：失败，符合预期。

失败原因：

- `attachments.len()` 实际为 `0`
- 期望为 `1`
- 说明旧逻辑没有把 `image/* + Content-ID` 的内联 CID 图片识别为附件。

### GREEN

命令：

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml detects_inline_cid_image_without_filename_as_attachment
```

结果：通过。

输出摘要：

- `1 passed, 99 filtered out`

## 测试命令和结果

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml detects_inline_cid_image_without_filename_as_attachment
```

结果：通过，`1 passed, 99 filtered out`。

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml parser
```

结果：通过，`6 passed, 94 filtered out`。

## 修改文件

- `src-tauri/src/infrastructure/protocols/imap/parser.rs`
- `.superpowers/sdd/task-2-report.md`

## 自审

- 已按 TDD 先添加失败测试并确认 RED，再实现最小生产代码。
- 未运行 `git commit`。
- 未触碰 Task 1 指定避免修改的文件。
- `BodyParams` 已按实际 `imap-proto-0.16.7` API 使用 `None` 表示空参数；该版本中 `BodyParams<'a>` 是 `Option<Vec<(Cow<'a, str>, Cow<'a, str>)>>`。
- 实现只扩展附件判定输入，不改变 `AttachmentInfo` 构建逻辑；CID 会继续通过现有 `content_id` 字段保存。

## 疑虑

- 当前规则只把 `image/*` 且存在 Content-ID 的 MIME 部分识别为附件，未覆盖其他可能通过 CID 引用的非图片资源；这与 Task 2 范围一致。
- 未运行完整 workspace 测试，仅按任务要求运行了目标测试和 `parser` 筛选测试。
