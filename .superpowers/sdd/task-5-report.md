# Task 5 报告：Tauri 附件命令、dialog 插件和 bindings

## 状态

已完成。

## 实现内容

- `src-tauri/src/command/email.rs` 新增附件命令：
  - `ensure_attachment_cached`
  - `save_attachment_as`
  - `open_attachment`
  - `resolve_inline_attachments`
- `src-tauri/src/lib.rs` 将四个命令注册到 tauri-specta builder，并注册 `tauri_plugin_dialog::init()`。
- `src-tauri/capabilities/default.json` 增加 `dialog:default`。
- `src-tauri/Cargo.toml` 增加 `tauri-plugin-dialog = "2"`。
- `package.json` 增加 `@tauri-apps/plugin-dialog`，`bun.lock` 已同步。
- `src/lib/bindings.ts` 重新生成，包含四个附件命令和附件 DTO。

## 验证结果

- `rtk cargo check --manifest-path src-tauri/Cargo.toml`：通过。
- `rtk timeout 120s cargo run --bin postium-mail`：编译完成并写出 bindings，随后因当前环境无法初始化 GTK panic，符合无桌面会话环境预期。
- `rtk rg -n "ensureAttachmentCached|saveAttachmentAs|openAttachment|resolveInlineAttachments|export type AttachmentDto|export type InlineAttachmentDto" src/lib/bindings.ts`：通过。

## 关注事项

- 未提交代码。
