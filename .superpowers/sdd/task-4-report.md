# Task 4 报告：Frontend Bindings and Store

## 状态

已完成。

## 变更

- 通过 debug export 生成 `src/lib/bindings.ts`，新增 `ReloadEmailResult` 类型和 `commands.reloadEmail(emailId)` 绑定。
- 在 `EmailState` 中新增 `reloadEmail(emailId): Promise<boolean>`。
  - `reloaded`：用后端返回的 `EmailDetail` 更新列表项，并在当前选中同一邮件时更新详情。
  - `removed`：从当前列表移除邮件，必要时取消选择，并按实际移除数量扣减 `total`。
  - typed error 或 invoke reject：返回 `false`，保留本地列表/详情状态并设置 `error`。
- 为 store 增加 reloaded、removed、failure 三类测试，并验证 Tauri invoke 参数为 `reload_email` / `{ emailId }`。
- 调整该测试文件的 `beforeEach`，重置 `mockInvoke` 实现，避免 rejected mock 泄漏到后续用例。

## 验证

- `rtk timeout 10s cargo run --manifest-path src-tauri/Cargo.toml --bin postium-mail`
  - 失败：从仓库根目录运行时，`EXPORT_DIR = "../src/lib/bindings.ts"` 解析到仓库外，报 `Read-only file system`。
- `rtk timeout 10s cargo run --bin postium-mail`（工作目录：`src-tauri`）
  - bindings 已生成；随后在 headless 环境初始化 GTK 失败，报 `Failed to initialize GTK`。
- `rtk rg -n "ReloadEmailResult|reloadEmail" src/lib/bindings.ts`
  - 确认生成文件包含 `ReloadEmailResult` 和 `commands.reloadEmail`。
- RED：`rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts --pool threads --maxWorkers 1 --reporter dot`
  - 新增 reload 用例按预期失败：`state.reloadEmail is not a function`。
- GREEN：同一 focused test 命令
  - 通过：`1 passed`，`14 passed`。
- `rtk node node_modules/typescript/bin/tsc --noEmit`
  - 通过。
- `rtk npm run check`
  - 运行超过 90 秒无输出，手动中止，未得到有效结果。
- `rtk npx @sveltejs/mcp svelte-autofixer src/lib/stores/email.svelte.ts --svelte-version 5`
  - 失败：sandbox 中 npm 访问 `https://registry.npmjs.org/@sveltejs%2fmcp` 被拒绝，`connect EPERM 127.0.0.1:7897`。

## 备注

- 未触发 full folder sync。
- 未更新 `uidnext`、`uidvalidity`、`last_sync_uid` 等 folder sync state。
- 未实现 batch reload。
- 未处理或删除附件磁盘文件。
- 未实现打开邮件详情时自动 reload。
