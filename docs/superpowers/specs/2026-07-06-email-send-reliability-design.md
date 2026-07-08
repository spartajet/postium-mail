# 邮件发送可靠性设计

## 背景

当前项目已经有基础发送链路：前端 `ComposeModal.svelte` 调用 `send_email` 命令，后端 `EmailService::send` 查询账号和 provider 配置，再通过 `SmtpClient` 使用 `lettre` 发送邮件。SMTP 协议层已经支持密码认证、XOAUTH2、隐式 TLS、STARTTLS、明文连接，以及纯文本 + HTML 的 `multipart/alternative` 正文。

现有实现与注释描述存在差距。命令注释和生成的 TypeScript 绑定写到了附件、自动保存到已发送、返回 Message-ID、回复线程等能力，但实际 `SendEmailRequest` 只有账号、收件人、抄送、密送、主题、HTML 正文和纯文本正文。SMTP 成功后返回 `"ok"`，不生成 Message-ID，不保存本地已发送副本，也不通过 IMAP 写入远端 Sent 文件夹。

还有一个发送配置风险：账号创建时会保存 `smtp_host`、`smtp_port`、`smtp_ssl_mode`，但发送时只使用 `provider.smtp_config(&account.email)`，没有优先使用账号表中的手动 SMTP 配置。手动或自定义服务器账号可能添加成功，但发送走了 provider 默认配置。

本设计聚焦第一阶段的可靠基础发送，不把附件、草稿、签名、定时发送等能力混入同一轮。

## 目标

1. 发送请求必须校验收件人、主题和正文，主题和正文不允许为空。
2. SMTP 配置优先使用账号表中的手动配置，缺失时回退 provider 默认配置。
3. 后端生成稳定的 Message-ID 和 Date，并返回真实 Message-ID，而不是固定 `"ok"`。
4. SMTP 投递成功后，本地 `emails` 表写入已发送副本。
5. SMTP 投递成功后，尝试通过 IMAP APPEND 将同一封 RFC822 原文写入远端 Sent 文件夹。
6. 远端 Sent 归档失败时，不提示用户重发，保留本地已发送副本并记录可诊断日志。
7. 发送命令返回结构化响应，表达 Message-ID、本地邮件 ID 和远端 Sent 归档状态。
8. 前端发送失败时保留编辑内容，发送成功时关闭写信窗口；如果当前在 Sent 分类，刷新列表。
9. 修正发送相关注释和绑定描述，避免文档继续声明未实现能力。

## 非目标

1. 不实现附件发送。
2. 不实现草稿保存或自动草稿。
3. 不实现签名、模板、定时发送。
4. 不实现回复线程头 `In-Reply-To` / `References`。
5. 不新增 BCC 输入界面；后端保留 `bcc` 字段，前端仍可暂时传空数组。
6. 不改变现有收信、同步窗口、附件下载逻辑。
7. 不提交代码。仓库指令明确“没有指令，不要提交代码”，因此本设计阶段只写文档，不执行 git commit。

## 推荐方案

采用“业务编排、协议投递、已发送归档”三层边界。

1. `EmailService::send` 负责业务编排：请求校验、账号查询、SMTP 配置解析、凭证获取、发送元数据生成、调用 SMTP、写本地 Sent、触发远端 Sent 归档。
2. `SmtpClient` 负责协议投递：基于已经确定的信封、消息原文和凭证连接 SMTP 服务器，不直接操作数据库。
3. 已发送归档独立成小函数或小服务：本地 Sent 写入和远端 Sent APPEND 有清晰的失败边界。

这样可以让 SMTP 协议层保持可测试，业务层可以对“已投递但归档失败”的状态做明确处理，也方便后续扩展附件和草稿。

## 发送数据流

1. 前端提交 `SendEmailRequest`。
2. 后端校验请求：
   - `account_id` 必须存在。
   - `to`、`cc`、`bcc` 合并后至少有一个地址。
   - `subject.trim()` 不允许为空。
   - `body_text.trim()` 不允许为空。前端富文本编辑器已经能提供纯文本，因此第一阶段以后端 `body_text` 为正文非空判据。
   - 地址解析失败返回 `InvalidParam`。
3. 查询账号和 provider。
4. 解析 SMTP 配置：
   - 如果账号表中存在非空 `smtp_host` 且存在有效 `smtp_port`，使用账号表配置。
   - `smtp_ssl_mode` 解析为 `SslMode`，支持 `Tls` / `TLS` / `Implicit`、`StartTls` / `STARTTLS`、`None`。
   - 手动配置缺失时回退 provider 的 `smtp_config`。
   - 手动端口无法转换为 `u16` 时返回 `InvalidParam`。
5. 解析 Sent 文件夹：
   - 优先通过现有 `FolderRegistry` / 文件夹分类能力解析当前账号的 Sent 文件夹。
   - 若本地尚未同步到文件夹元数据，则回退 provider `folder_mapping().sent` 的第一个值。
   - 如果仍找不到 Sent 文件夹，本地使用 `"Sent"`，远端 APPEND 记录失败日志但不阻断发送成功。
6. 构建消息：
   - 生成 Message-ID，例如 `<uuid@postium.local>` 或基于账号域名生成 `<uuid@domain>`。
   - 生成 Date。
   - 设置 From、To、Cc、Bcc、Subject。
   - 构建 `multipart/alternative`，包含 text/plain 和 text/html。
   - 保留 RFC822 原文字节，用于 SMTP 投递和远端 APPEND。
7. 获取凭证：
   - Password provider 使用 Keyring 中的密码或授权码。
   - OAuth2 provider 通过 `AuthManager::get_credentials` 获取 access token，SMTP 使用 XOAUTH2。
8. SMTP 投递：
   - 成功后进入归档。
   - 失败返回 `SmtpSendFailed` 或 `InvalidParam`，不写本地 Sent。
9. 本地 Sent 写入：
   - 写入 `emails` 表，folder 为解析出的 Sent 文件夹。
   - `uid` 使用同账号同 Sent 文件夹当前 `MAX(uid) + 1` 作为本地临时 UID，并标记为普通已发送邮件。后续同步如果发现相同 Message-ID 的远端邮件，应合并到这条本地记录并更新为真实 UID，避免重复显示。
   - `message_id` 使用生成值。
   - `sender_email` 为账号邮箱，`sender_name` 为账号显示名。
   - `recipient_emails`、`cc_emails`、`bcc_emails` 写入逗号分隔字符串。
   - `is_read = true`，`is_draft = false`，`is_answered = false`，`is_deleted = false`。
   - `sent_at`、`received_at`、`created_at`、`updated_at` 使用发送时间。
10. 远端 Sent APPEND：
    - 使用 IMAP 连接和同一凭证登录。
    - 对 Sent 文件夹执行 APPEND，写入 RFC822 原文。
    - 成功后可选择触发该 Sent 文件夹的单文件夹同步，更新真实 UID。
    - 失败不要求用户重发，记录账号、Message-ID、Sent 文件夹和错误。
11. 返回结果：
    - 命令返回 `SendEmailResponse`，包含 `message_id`、`local_email_id`、`remote_archived`、`remote_archive_error`。
    - `remote_archive_error` 只用于诊断和可选提示，不代表 SMTP 投递失败。

## 错误处理

### SMTP 投递前失败

包括账号不存在、provider 不支持、认证凭证获取失败、请求校验失败、地址解析失败、SMTP 配置无效。这类错误返回给前端，前端保留写信窗口和已有内容。

### SMTP 投递失败

SMTP 连接、认证或投递失败时返回错误，不写本地 Sent。用户可以修改后重试。

### SMTP 成功、本地 Sent 写入失败

这是高风险状态：邮件已经投递，但客户端没有可见副本。应返回明确错误并记录 Message-ID、账号 ID、主题、收件人数量。实现时应尽量让本地写入逻辑简单、事务化，降低该失败概率。

### SMTP 成功、远端 Sent APPEND 失败

不把整体发送视为失败。用户不应该因为远端归档失败而重发同一封邮件。本地 Sent 保留副本，日志记录远端失败原因。后续可以增加后台补偿任务，但第一阶段不要求。

## 前端设计

`ComposeModal.svelte` 保持现有交互为主，增加最小校验和状态处理：

1. 发送按钮在无可用账号、无收件人、空主题、空正文时禁用，或点击后显示错误。
2. 后端返回错误时不关闭窗口，不清空表单。
3. 成功返回 Message-ID 后关闭窗口并清空表单。
4. 如果当前邮件列表处于 Sent 分类，触发刷新。
5. 前端文案应区分“发送失败”和“发送成功但远端已发送归档失败”。`remote_archived = false` 时可以用轻量提示说明邮件已发送并已保存在本地，但远端已发送归档稍后可能需要同步修复。

## 后端接口设计

### 请求

现有 `SendEmailRequest` 可以继续保留：

```rust
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}
```

第一阶段不向请求中加入附件、回复线程字段或草稿字段。

### 响应

推荐改为结构化响应：

```rust
pub struct SendEmailResponse {
    pub message_id: String,
    pub local_email_id: i32,
    pub remote_archived: bool,
    pub remote_archive_error: Option<String>,
}
```

发送命令应从 `Result<String, MailError>` 调整为 `Result<SendEmailResponse, MailError>`。这会触发 TypeScript 绑定和前端测试更新，但可以准确表达“已投递但远端归档失败”。

### SMTP 配置解析

建议新增与 `imap_config_from_account` 对称的函数：

```rust
smtp_config_from_account(account: &accounts::Model) -> Result<SmtpServerConfig, MailError>
```

该函数优先使用账号表手动配置，回退 provider 默认配置。不要在 `EmailService::send` 中散落解析逻辑。

### SMTP 客户端

为了同时支持投递和远端 APPEND，建议协议层能够返回或接收 RFC822 原文：

1. 新增消息构建函数，返回 `BuiltEmail { message_id, raw, envelope }`。
2. SMTP 发送函数接收 `BuiltEmail` 或 `lettre::Message`。
3. 远端 APPEND 使用 `raw`。

第一阶段可以先在 `EmailService` 中构建一次消息并把 `lettre::Message` 传给 SMTP，但要避免 SMTP 内部构建一份、IMAP APPEND 又构建另一份，导致 Message-ID 或 MIME 边界不一致。

## 数据库设计

不新增表。

本地 Sent 副本写入现有 `emails` 表。需要注意现有唯一索引是 `(account_id, folder, uid)`，而已发送本地副本在远端同步前没有真实 UID。本设计采用以下策略：

1. 为本地发送副本新增 repository 函数，不复用依赖远端 UID 的同步 upsert 入口。
2. 本地临时 UID 取同账号同 Sent 文件夹当前 `MAX(uid) + 1`。
3. 后续 Sent 同步保存完整邮件时，如果远端邮件 Message-ID 与本地发送副本一致，应更新本地发送副本的 `uid`、正文和标志，而不是插入第二封邮件。
4. 如果远端 Sent APPEND 失败，本地副本会长期保留临时 UID；后续用户重新同步时如果服务商已经自动保存 Sent，同样通过 Message-ID 合并。

该策略不需要新增表，但要求同步写入路径支持 Message-ID 合并，避免本地临时副本和远端真实副本重复显示。

## 测试设计

### Rust 单元测试

1. `SendEmailRequest` 校验：
   - 无 `to/cc/bcc` 返回 `InvalidParam`。
   - 空主题返回 `InvalidParam`。
   - 空正文返回 `InvalidParam`。
2. SMTP 配置解析：
   - 手动 SMTP 配置覆盖 provider 默认配置。
   - 手动端口无效返回 `InvalidParam`。
   - 未配置手动 SMTP 时使用 provider 默认配置。
3. Message-ID：
   - 成功发送返回非空 Message-ID。
   - 本地 Sent 记录保存同一个 Message-ID。
4. 认证分支：
   - Password 账号走普通 SMTP。
   - OAuth2 账号走 XOAUTH2。
5. 归档失败：
   - SMTP 成功、本地 Sent 成功、远端 APPEND 失败时不要求重发。
   - 返回 `remote_archived = false`。

### 前端单元测试

1. 空主题禁止发送或显示错误。
2. 空正文禁止发送或显示错误。
3. 后端错误时窗口保持打开，输入内容不清空。
4. 成功发送后窗口关闭。
5. 所有账号视图下仍使用选中的发件账号。

### E2E 测试

第一阶段不连接真实 SMTP。E2E 继续 mock Tauri 命令，覆盖写信窗口校验、发送成功关闭和失败保留内容。

真实账号 SMTP/IMAP APPEND 测试应放到手动或 opt-in 测试，不进入默认 CI。

## 实施顺序

1. 增加发送请求校验和测试。
2. 抽出 SMTP 配置解析函数，补齐手动配置优先逻辑和测试。
3. 重构消息构建，生成 Message-ID、Date 和单一 RFC822 原文。
4. 调整 SMTP 客户端发送接口，避免重复构建消息。
5. 增加本地 Sent 写入函数和测试。
6. 增加远端 Sent APPEND，失败降级为日志和响应状态。
7. 调整前端校验和发送成功后刷新行为。
8. 修正注释和生成绑定，确保文档只声明已实现能力。

## 风险和取舍

1. **本地临时 UID**：现有 schema 要求 `uid` 非空且唯一，发送副本没有真实 UID。本设计使用 `MAX(uid) + 1` 临时 UID，并要求后续 Sent 同步按 Message-ID 合并，这是实现时最大的数据库细节风险。
2. **远端 Sent 文件夹解析**：不同服务商 Sent 文件夹名称不同，且本地可能尚未同步文件夹列表。第一阶段允许 fallback 到 provider mapping，再失败则仅保留本地副本。
3. **OAuth2 SMTP 兼容性**：`lettre` 的 XOAUTH2 路径已有代码，但需要 mock 测试覆盖认证机制选择，真实服务商仍建议 opt-in 验证。
4. **响应类型兼容**：从 `String` 改为结构化响应会影响前端绑定和测试。本设计接受这次小破坏，因为它能表达远端归档降级。

## 验收标准

1. 空收件人、空主题、空正文不能进入 SMTP 投递。
2. 手动 SMTP 配置账号发送时使用账号表配置。
3. 成功发送返回包含 Message-ID、本地邮件 ID 和远端归档状态的结构化响应。
4. SMTP 成功后，本地 Sent 能看到该邮件。
5. 远端 Sent APPEND 失败不会引导用户重复发送。
6. 前端发送失败不丢失编辑内容。
7. 默认测试不依赖真实 SMTP/IMAP 服务。
