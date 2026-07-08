# 账号删除无响应修复设计

日期：2026-06-23

## 背景

设置页的账号删除按钮点击后没有明显反应，重新启动应用后账号仍然存在。当前前端调用链是：

1. `src/routes/settings/+page.svelte` 的删除按钮调用 `accountStore.deleteAccount(account.id)`。
2. `AccountState.deleteAccount` 调用 `commands.deleteAccount(id)`。
3. Tauri 命令 `delete_account` 透传到 `AccountService::delete`。

当前后端实现只删除账号记录，然后尝试删除 keyring 中的密码：

```rust
account_repo::delete(&self.db, id).await?;
let _ = self.auth.delete_password(&account.email);
```

但命令文档和前端注释都承诺“删除账号及其所有相关数据”。这些相关数据包括邮件、标签、邮件标签关联、同步状态和同步错误。当前实现没有在 service 层显式清理这些数据。

SQLite 外键级联也不能作为唯一保证：

- 运行时数据库初始化没有显式开启 `PRAGMA foreign_keys = ON`。
- 测试环境目前显式关闭 foreign keys，以规避迁移 13 重建 `emails` 表后 FTS 触发器与外键级联之间的问题。
- 因此现有测试只覆盖了账号行和密码删除，没有覆盖账号级关联数据删除。

前端也存在反馈问题：`AccountState.deleteAccount` 只在 `result.status === "ok"` 时更新本地列表；如果后端返回 typed error，它不会设置 `error`，用户看到的就是“点击没反应”。

## 问题判断

这是业务 bug 叠加轻度架构边界问题，不是需要重构整体架构的问题。

核心业务 bug 是：账号删除用例没有完整实现“删除账号及相关数据”的语义。

轻度架构问题是：账号删除是跨多个 repository 的业务事务，应由 `AccountService` 编排删除顺序，而不是依赖 `account_repo::delete` 或数据库外键开关隐式完成。

## 目标

修复后应满足：

1. 点击设置页删除账号后，如果后端删除成功，账号立即从 UI 列表中消失。
2. 重新启动应用后，已删除账号不会重新出现。
3. 删除账号时显式清理该账号相关的本地数据。
4. 删除失败时，前端不再静默失败，而是记录可展示的错误信息。
5. 新测试能覆盖账号存在邮件、标签和关联数据时的删除行为。

## 非目标

本次不做以下事情：

1. 不重设计设置页账号管理 UI。
2. 不引入账号软删除或回收站。
3. 不改变 keyring 删除失败的容忍策略。账号记录和本地数据删除成功后，keyring 缺失或已删除仍不阻断流程。
4. 不重写数据库迁移或 FTS 触发器体系。
5. 不改变账号创建、同步、邮件列表等无关业务。

## 推荐方案

采用“后端显式清理 + 前端错误反馈”的窄范围修复。

### 后端删除流程

`AccountService::delete(id)` 继续作为账号删除的业务入口。流程调整为：

1. 查询账号，账号不存在时返回 `MailError::AccountNotFound(id)`。
2. 显式删除该账号下邮件产生的标签关联。
3. 显式删除该账号下的标签。
4. 显式删除该账号下的附件和邮件。
5. 显式删除该账号的同步状态和同步错误。
6. 删除账号记录。
7. 尝试删除 keyring 密码，忽略“凭证不存在或已删除”类错误。

删除顺序从依赖表到根表，避免依赖 SQLite foreign key 开关。即使 foreign keys 关闭，账号关联数据也能被清理。

### Repository 边界

为保持 service 层简洁，可以在 repository 层补充账号范围清理函数，例如：

- 邮件 repository 提供按 `account_id` 删除邮件及附件的函数。
- 标签 repository 提供按 `account_id` 删除标签及邮件标签关联的函数。
- 同步相关 repository 或 service 内部提供按 `account_id` 删除同步状态与同步错误的函数。

如果现有 repository 已有可复用函数，应优先复用。新增函数应保持窄职责：只做一个表族的账号级清理，不包含账号删除本身。

### 前端错误反馈

`AccountState.deleteAccount` 需要处理 typed error：

1. 调用删除前清空旧错误。
2. `result.status === "ok"` 时维持当前本地列表删除和活跃账号切换逻辑。
3. `result.status === "error"` 时设置 `this.error`，不修改本地账号列表。
4. `catch` 分支继续格式化异常并设置 `this.error`。

设置页当前没有专门显示 `accountStore.error`。本次可以选择最小显示方式：在账号管理区域展示一行错误文本。这样用户点击删除失败时不再没有反馈。

## 备选方案

### 方案 A：只修前端错误提示

优点：

- 改动最小。
- 能快速看到后端返回的真实错误。

缺点：

- 不解决账号删除失败或数据残留问题。
- 用户仍然无法完成删除。

结论：不作为最终修复，只能作为诊断增强。

### 方案 B：只修数据库外键和迁移

优点：

- 理论上可以把级联删除放回数据库层。
- 数据一致性约束更强。

缺点：

- 现有测试环境已经因为 FTS 与迁移历史关闭 foreign keys。
- 对已有用户数据库的兼容性和迁移风险更高。
- 不能解决前端静默失败。

结论：不适合作为本次窄范围修复。

### 方案 C：推荐方案，service 显式编排删除

优点：

- 直接匹配业务语义。
- 不依赖 SQLite foreign key 开关。
- 测试容易覆盖。
- 前端会给出失败反馈。

缺点：

- 需要补充少量 repository 清理函数。
- 后端删除流程需要注意删除顺序。

结论：采用该方案。

## 数据清理范围

本次账号删除应清理以下本地数据库数据：

- `accounts`：目标账号记录。
- `emails`：目标账号的所有邮件。
- `attachments`：目标账号邮件关联的附件。
- `labels`：目标账号的所有标签。
- `email_labels`：目标账号邮件或标签产生的关联记录。
- `sync_state`：目标账号同步状态。
- `sync_errors`：目标账号同步错误记录。

清理不应影响其他账号的数据。测试需要覆盖“另一个账号的数据仍存在”。

## 错误处理

后端：

- 账号不存在返回 `AccountNotFound(id)`。
- 数据库删除失败返回数据库错误，不继续伪装成功。
- keyring 删除失败不阻断数据库删除结果，保持当前容忍策略。

前端：

- 后端 typed error 应写入 `AccountState.error`。
- 删除失败时不从本地 `accounts` 列表中移除账号。
- 删除成功时清空旧错误并更新本地列表。

## 测试策略

### Rust 后端测试

新增或扩展 `src-tauri/tests/account_commands.rs`：

1. 构造账号 A 和账号 B。
2. 给账号 A 插入邮件、标签、邮件标签关联、同步状态或同步错误。
3. 给账号 B 插入至少一条不会被删除的数据。
4. 调用 `svc.account_service.delete(account_a.id)`。
5. 断言账号 A 不存在。
6. 断言账号 A 的邮件、标签、关联、同步数据不存在。
7. 断言账号 B 仍存在，账号 B 的数据不受影响。
8. 断言账号 A 的 keyring 密码不可再读取。

该测试应先在当前实现下失败，再实现修复让它通过。

### 前端 store 测试

扩展 `src/lib/__tests__/stores/account-state.test.ts`：

1. 模拟 `delete_account` 返回 `{ status: "error", error: ... }`。
2. 调用 `state.deleteAccount(id)`。
3. 断言 `state.error` 被设置。
4. 断言 `state.accounts` 未被删除。

成功路径已有测试覆盖删除当前账号和删除最后一个账号后的 active account 切换；如果实现改动影响这些测试，应保持它们继续通过。

### 验证命令

完成实现后运行：

```bash
rtk bun run test:frontend
rtk bun run test:rust
rtk bun run check
rtk rustfmt --edition 2024 --check <本次触及的 Rust 文件>
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/stores/account.svelte.ts
```

全仓库 `cargo fmt --manifest-path src-tauri/Cargo.toml --check` 当前存在既有未格式化文件，不作为本次修复完成的唯一门禁；本次触及的 Rust 文件必须通过 rustfmt 检查。

## 风险与缓解

风险：删除顺序不完整导致残留数据。

缓解：用后端测试覆盖账号有邮件、标签、关联和同步记录的场景。

风险：删除逻辑误删其他账号数据。

缓解：测试中保留账号 B，并断言账号 B 数据仍存在。

风险：前端仍无可见反馈。

缓解：store 测试覆盖 typed error；设置页展示 `accountStore.error`。

风险：keyring 删除失败导致用户误以为删除失败。

缓解：保持当前策略，keyring 删除失败不阻断数据库删除；日志可记录但不改变用户流程。

## 实施顺序

1. 写后端失败测试，确认当前实现无法清理账号关联数据。
2. 写前端失败测试，确认 typed error 现在被静默吞掉。
3. 实现后端显式账号级清理。
4. 实现前端错误处理和设置页错误展示。
5. 运行验证命令。

## 验收标准

1. 删除账号成功后，设置页账号列表立即移除该账号。
2. 重新启动应用后，被删除账号不再出现。
3. 删除失败时，用户能看到错误提示或至少状态中有错误信息。
4. Rust 后端测试证明账号关联数据被清理，其他账号不受影响。
5. 前端 store 测试证明 typed error 不再被静默吞掉。
6. 指定验证命令通过，既有全仓库 fmt 问题单独说明。
