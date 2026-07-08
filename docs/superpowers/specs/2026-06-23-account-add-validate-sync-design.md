# 账号添加校验与首次同步设计

日期：2026-06-23

## 背景

当前账号添加流程存在两个用户可感知问题：

1. 密码账号添加时没有连接校验。用户输入邮箱和密码后，应用会直接创建账号并保存密码，无法确认密码、授权码或服务器配置是否正确。
2. 账号添加成功后不会自动抓取邮件。用户需要等待后台调度或手动同步，添加完成页也无法体现首轮同步状态。

代码现状：

1. `src/lib/components/settings/AddAccountModal.svelte` 的密码提交路径只调用 `commands.createAccount(...)`。
2. `AccountService::create` 只创建数据库记录并调用 `AuthManager::save_password`，没有调用 IMAP 或 SMTP 协议层校验。
3. `AddAccountModal.svelte` 在创建成功后只进入完成页并调用 `accountStore.loadAccounts()`，没有调用已有的 `syncStore.syncAccount(accountId)`。
4. 后端已有 `sync_account` 命令和前端 `SyncState.syncAccount(accountId)`，但账号添加流程没有复用。
5. 手动配置账号保存了 `imap_host`、`imap_port`、`imap_ssl_mode` 等字段，但 `SyncOrchestrator::sync_account` 当前只通过 provider 获取 `imap_config`。这会导致自定义账号后续同步可能连到默认 custom provider 配置，而不是用户输入的服务器。

## 问题判断

这是账号创建用例缺少关键前置校验和后置动作的问题，同时暴露出连接配置解析逻辑没有统一边界。

核心修复点不是重做账号添加 UI，而是让账号创建流程具备正确的业务语义：

1. 密码账号只有在 IMAP 登录成功后才允许创建。
2. 手动配置账号后续同步必须使用用户保存的 IMAP 配置。
3. 账号创建成功后立即触发首次同步，让用户尽快看到邮件。

## 目标

修复后应满足：

1. 密码账号添加前会校验 IMAP 连接和登录。
2. 密码、授权码、IMAP 主机、端口或加密模式错误时，账号不会入库，密码不会保存到 Keyring。
3. 预设服务商账号继续使用 provider 默认 IMAP 配置。
4. 手动配置账号优先使用用户输入并保存到账号表的 IMAP 配置。
5. 账号创建成功后，前端自动将新账号设为当前活跃账号并触发首次同步。
6. 首次同步失败时不回滚账号创建，但前端需要展示同步失败原因，并允许用户稍后重试。
7. OAuth2 添加成功后也触发首次同步。

## 非目标

本次不做以下事情：

1. 不强制校验 SMTP。SMTP 只影响发信，很多服务商收信和发信策略不同，不应因为 SMTP 配置问题阻断用户先收邮件。
2. 不实现“发送测试邮件”功能。
3. 不重做服务商选择 UI。
4. 不引入新的后台任务系统。
5. 不改变已有同步算法的全量/增量策略。
6. 不解决所有服务商 OAuth2 token 刷新策略，只复用现有 OAuth2 创建和同步路径。

## 推荐方案

采用“创建前 IMAP 校验 + 统一连接配置解析 + 创建后首次同步”的方案。

### 后端创建流程

`AccountService::create(req)` 调整为：

1. 记录创建请求日志。
2. 解析用于本次创建的 IMAP 配置。
3. 使用 `ImapClient::connect(&imap_config, &req.email, &req.password)` 尝试登录。
4. 登录成功后立即 `logout`，忽略 logout 阶段的非关键错误或只记录 debug 日志。
5. 登录失败时返回 `MailError`，不创建账号，不保存密码。
6. 登录成功后创建账号记录。
7. 保存密码到 Keyring。
8. 返回 `AccountDto`。

该顺序可以保证 Keyring 和数据库不会保存明显不可用的账号凭据。

### 连接配置解析

新增一个窄职责 helper，用于解析账号 IMAP 配置。位置可以是：

- `src-tauri/src/service/account_connection.rs`
- 或 `src-tauri/src/domain/providers/config.rs`

推荐放在 service 或 domain 边界，不放进 repository。repository 只负责存取数据，不负责解释 provider 和连接策略。

helper 需要提供两类入口：

1. 从 `CreateAccountRequest` 解析 IMAP 配置。
2. 从数据库 `accounts::Model` 解析 IMAP 配置。

解析规则：

1. 如果存在完整的手动 IMAP 配置，优先使用手动配置：
   - `imap_host`
   - `imap_port`
   - `imap_ssl_mode`
2. 如果手动配置不完整，则使用 provider pool 中对应 provider 的 `imap_config(email)`。
3. provider 不存在时返回 `ProviderNotSupported`。
4. `imap_ssl_mode` 字符串需要转换为 `SslMode`。

前端当前使用的字符串是：

- `"Tls"`：对应 `SslMode::Implicit`
- `"StartTls"`：对应 `SslMode::StartTls`
- `"None"`：对应 `SslMode::None`

后端也应兼容已有 provider 文档中可能出现的 `"Implicit"`，避免未来配置命名不一致。

### 同步流程配置修复

`SyncOrchestrator::sync_account(account_id)` 当前通过 provider 直接获取 IMAP 配置：

```rust
let imap_config = provider.imap_config(&account.email);
```

应改为使用同一个配置解析 helper：

1. 从数据库读取账号。
2. 从账号记录解析 IMAP 配置。
3. 使用解析结果连接 IMAP。

这样手动配置账号创建后，首次同步和后续手动/后台同步都使用同一套真实配置。

### 前端密码账号流程

`AddAccountModal.svelte` 增加同步状态：

- `phase: "idle" | "validating" | "creating" | "syncing" | "done"`
- `syncError: string`

密码提交路径调整为：

1. 提交后清空 `error` 和 `syncError`。
2. `phase = "validating"`，按钮显示“正在验证...”。
3. 调用 `commands.createAccount(...)`。
4. 后端返回错误时，展示错误，`phase = "idle"`。
5. 后端返回成功时：
   - `phase = "syncing"`
   - `await accountStore.loadAccounts()`
   - `accountStore.setActive(result.data.id)`
   - `await syncStore.syncAccount(result.data.id)`
6. 如果同步失败，记录 `syncStore.error` 或命令错误到 `syncError`。
7. 不论同步是否成功，只要账号创建成功，都进入完成页。

完成页文案需要区分：

1. 同步成功或无错误：账号已添加并完成首次同步。
2. 同步失败：账号已添加，但首次同步失败，展示错误文本，用户可稍后手动同步。

### OAuth2 账号流程

OAuth2 完成后，当前后端已自动创建账号，前端只能通过轮询结果和刷新账号列表获取账号。

推荐流程：

1. `pollOauth2` 返回 Completed 后调用 `accountStore.loadAccounts()`。
2. 根据 Completed 信息中的邮箱找到新账号。
3. 找到后设置为活跃账号。
4. 调用 `syncStore.syncAccount(account.id)`。
5. 同步失败时记录 `syncError`，但不撤销账号。

如果 Completed 信息没有足够字段定位账号，则使用轮询前输入的 `email` 在刷新后的账号列表里查找。

## 备选方案

### 方案 A：只添加创建前 IMAP 校验

优点：

- 改动最小。
- 能直接阻止错误密码或错误 IMAP 配置入库。

缺点：

- 添加成功后仍然不会自动抓取邮件。
- 手动配置账号后续同步仍可能使用错误的 provider 默认配置。

结论：不足以解决用户提出的两个问题。

### 方案 B：只在前端创建成功后触发同步

优点：

- 改动小。
- 用户添加账号后能立即看到同步错误或邮件结果。

缺点：

- 错误账号仍会先入库，Keyring 仍会保存错误密码。
- 用户需要再删除或修改账号，体验差。
- 手动配置同步错误仍可能来自配置解析 bug，而不是用户输入错误。

结论：不作为最终方案。

### 方案 C：推荐方案，校验、配置解析和首次同步一起做

优点：

- 阻止明显错误账号入库。
- 添加成功后自动抓取邮件。
- 修复手动配置账号同步走错配置的隐患。
- 复用已有 IMAP 和同步能力，不引入大范围架构变化。

缺点：

- 需要新增少量后端 helper 和测试。
- 首次同步会让添加账号流程变长，需要前端清晰展示进度。

结论：采用该方案。

## 错误处理

后端：

1. IMAP TCP 连接失败返回连接错误。
2. TLS/greeting 失败返回连接错误。
3. 登录失败返回认证错误。
4. provider 不存在返回 provider 不支持错误。
5. 创建账号或保存密码失败按现有错误类型返回。
6. IMAP 校验失败后不能执行数据库写入和 Keyring 写入。

前端：

1. 创建失败显示 `error`，保留用户输入，允许修改后重试。
2. 同步失败显示 `syncError`，账号保留。
3. 同步失败不应让添加流程停留在提交中状态。
4. 用户关闭完成页后，后续可通过侧边栏或同步按钮重试。

## 测试策略

### Rust 后端测试

新增或扩展 `src-tauri/tests/account_commands.rs`。

需要避免真实网络依赖。推荐把 IMAP 登录校验抽成 trait 或窄接口，测试中注入 fake verifier：

1. 成功 verifier：返回 Ok。
2. 失败 verifier：返回 `MailError::AuthFailed` 或连接错误。

测试用例：

1. `create_account_should_not_persist_when_imap_verification_fails`
   - 调用创建账号。
   - fake verifier 返回失败。
   - 断言账号表没有新增记录。
   - 断言 Keyring 没有保存密码，或通过 fake auth manager 断言没有调用保存。
2. `create_account_should_persist_after_imap_verification_succeeds`
   - fake verifier 返回成功。
   - 断言账号存在。
   - 断言密码保存被调用。
3. `manual_imap_config_should_override_provider_config`
   - 构造包含 `imap_host/imap_port/imap_ssl_mode` 的请求或账号模型。
   - 断言解析出的 `ImapServerConfig` 使用手动配置。
4. `provider_imap_config_should_be_used_when_manual_config_missing`
   - 构造预设 provider 账号。
   - 断言解析出的配置来自 provider。

如果当前 `AccountService` 不方便注入 fake verifier，可以先重构为最小依赖注入，但不要把真实网络测试放进单元测试。

### 前端测试

扩展添加账号相关测试。如果当前没有组件级测试，可以优先补 store/command 调用层测试，组件测试作为后续增强。

测试用例：

1. `createAccount` 成功后调用 `syncAccount(newId)`。
2. 创建失败时不调用同步。
3. 同步失败时进入完成态并记录同步错误。
4. OAuth2 Completed 后刷新账号并触发同步。

### Svelte 校验

本次会修改 `.svelte` 文件，完成后需要运行：

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/settings/AddAccountModal.svelte
```

如果该命令因网络或依赖下载受限失败，需要说明未能运行的原因，并至少运行项目已有前端测试。

### 验证命令

实现完成后运行：

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands
rtk cargo test --manifest-path src-tauri/Cargo.toml
rtk bun run test:frontend
rtk bun run check
rtk rustfmt --edition 2024 --check <本次触及的 Rust 文件>
```

## 风险与缓解

风险：真实 IMAP 校验让账号添加耗时变长。

缓解：前端显示“正在验证连接”，禁用重复提交，失败时保留表单输入。

风险：部分服务商要求应用专用密码或授权码，用户误以为普通登录密码可用。

缓解：后端返回认证失败，前端展示错误；后续可在 provider 文案中补充授权码提示，本次不扩展文案体系。

风险：首次同步可能因为文件夹不存在、网络抖动或服务商限制失败。

缓解：创建成功后不回滚账号，只展示首次同步失败，并允许用户稍后重试。

风险：手动配置字段不完整导致解析结果不明确。

缓解：只有完整手动 IMAP 配置才作为 override；不完整时使用 provider 默认配置或返回明确错误。手动模式前端已经要求填写 IMAP host 和端口。

风险：测试如果依赖真实邮箱服务器会不稳定。

缓解：IMAP 校验通过 trait/fake verifier 测试，不做真实网络单元测试。

## 完成标准

实现完成后应满足：

1. 密码账号创建前有 IMAP 登录校验。
2. IMAP 校验失败时账号不入库，密码不保存。
3. 手动配置账号同步时使用账号表保存的 IMAP 配置。
4. 账号添加成功后自动触发首次同步。
5. 首次同步失败时账号保留，UI 展示同步错误。
6. OAuth2 添加完成后也触发首次同步。
7. 新增后端测试覆盖校验成功、校验失败和配置解析。
8. 新增或更新前端测试覆盖创建后同步、创建失败不同步、同步失败展示。
9. 本次触及的 Rust 文件通过 rustfmt 检查。
10. 相关 Rust、前端测试和 Svelte 检查按验证命令执行并记录结果。
