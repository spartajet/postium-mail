# 发件体系路线图与草稿箱、附件发送设计

## 背景

当前项目已经完成第一阶段的可靠发送基础：发送请求会校验收件人、主题和正文；SMTP 配置优先使用账号表中的手动配置；后端生成 Message-ID；SMTP 投递成功后写入本地 Sent，并尝试通过 IMAP APPEND 归档到远端 Sent。这个基础解决了“能否可靠投递并留下已发送副本”的问题。

发件体验还没有覆盖完整邮件客户端常见能力。当前写信窗口不能保存草稿，切换场景或关闭窗口会丢失未发送内容；发送接口也不支持附件，虽然收信侧已经有附件表、附件 DTO、附件缓存和附件展示能力。用户明确希望草稿第一版支持远端 Drafts，因此草稿设计必须遵循项目现有“远端优先”的邮件操作模型，而不是只写本地数据库。

本设计分为两部分：先给出发件体系整体路线图，再定义本期要落地的“远端 Drafts + 普通附件发送”方案。

## 发送邮件整体路线图

### 阶段 1：可靠发送基线

状态：已基本完成。

目标是把发送链路从“调用 SMTP 返回 ok”提升为可诊断、可归档、可恢复的基础能力：

1. 发送前校验收件人、主题和正文，主题和正文不允许为空。
2. SMTP 配置优先使用账号保存的手动配置，缺失时回退 provider 默认配置。
3. 后端生成 Message-ID 和 Date，返回结构化发送结果。
4. SMTP 成功后写本地 Sent，并尝试 IMAP APPEND 到远端 Sent。
5. 远端 Sent 归档失败不引导用户重发，保留本地副本和错误诊断信息。

### 阶段 2：草稿箱与附件发送

状态：本期实现。

目标是补齐发件最核心的持久化和文件发送能力：

1. 写信窗口自动保存草稿到远端 Drafts。
2. 本地数据库镜像远端 Drafts，保证草稿能在应用内列表、详情和编辑窗口中打开。
3. 重复保存同一草稿时用新远端草稿替换旧远端草稿，避免 Drafts 文件夹堆积多个版本。
4. 发送草稿成功后删除远端 Drafts 中对应草稿，并删除本地草稿镜像。
5. 写信窗口支持选择本地普通附件、展示附件列表、移除附件。
6. 发送 MIME 支持 `multipart/mixed` 外层结构，附件和正文使用同一封 RFC822 原文进入 SMTP、Sent 和 Drafts。

### 阶段 3：收件人与回复语义

目标是让写信、回复、转发更接近完整邮件客户端：

1. 收件人输入从逗号分隔文本升级为 token/chip，支持粘贴多个地址和逐个校验。
2. 支持 BCC 输入界面。
3. 回复和全部回复自动填充收件人、抄送、主题前缀。
4. 回复邮件增加 `In-Reply-To` 和 `References` 头，保持线程关系。
5. 转发保留原邮件正文和附件选择策略。

### 阶段 4：发送队列与失败恢复

目标是处理网络不稳定、认证过期、大附件耗时等场景：

1. 引入发送队列，支持后台发送、重试、取消。
2. SMTP 失败时保留可重试的草稿或发件箱记录。
3. Sent APPEND、Draft 删除失败进入补偿任务。
4. UI 展示发送中、等待重试、失败原因。

### 阶段 5：高级发件能力

这些能力先不进入本期，避免扩大实现面：

1. 签名、模板、发件别名和 Reply-To。
2. 定时发送。
3. 内联图片和富文本图片上传。
4. 大附件限制、压缩、云附件或分块策略。
5. S/MIME、PGP 等端到端安全能力。

## 本期目标

1. 实现远端 Drafts 草稿保存、打开、更新和删除。
2. 实现发送草稿成功后的远端草稿清理。
3. 实现普通本地文件附件发送。
4. 草稿和已发送邮件都能保存附件元数据，并在详情页继续使用现有附件展示能力。
5. 保持默认测试不依赖真实 SMTP/IMAP 服务，通过注入 mock 覆盖业务分支。

## 本期非目标

1. 不做纯本地草稿模式，除非远端保存失败时保留当前编辑内容并提示用户。
2. 不实现内联图片、拖拽上传、大文件分块或云附件。
3. 不实现发送队列和离线发送。
4. 不实现回复线程头、转发附件策略和签名模板。
5. 不改变收信侧附件下载、缓存、另存为和打开逻辑。
6. 不提交代码；仓库指令要求没有明确提交指令时不要提交。

## 推荐方案

采用“远端草稿为权威，本地库做镜像”的方案。

草稿保存时，后端构建一封带 `\Draft` 语义的完整 RFC822 邮件，通过 IMAP APPEND 写入账号的 Drafts 文件夹。APPEND 成功后，本地数据库保存一条草稿镜像，`is_draft = true`，正文和附件元数据来自本次保存请求。再次保存同一草稿时，先 APPEND 新版本，再尽力删除旧远端草稿和旧本地镜像。这样即使删除旧版本失败，也不会丢失用户最新内容；最坏情况是远端 Drafts 临时出现多个版本，后续同步或补偿再清理。

附件发送采用“前端传路径，后端读文件”的方案。前端通过 Tauri 文件选择器拿到本地路径，调用后端命令解析文件名、大小和 MIME 类型。保存草稿和发送邮件时只传附件路径与展示元数据，后端在构建 MIME 时读取文件字节。这样避免通过 IPC 传输大二进制，也复用桌面应用对本地文件的访问能力。

## 草稿数据流

### 新建草稿

1. 用户在写信窗口输入内容。
2. 前端 debounce 触发 `save_draft`。空白草稿不保存；只要存在收件人、抄送、密送、主题、正文或附件之一，就可以保存草稿。
3. 后端校验账号存在，并解析 Drafts 文件夹：
   - 优先使用本地 FolderRegistry 中识别到的 Drafts 文件夹。
   - 其次使用 provider `folder_mapping().drafts` 的第一个值。
   - 仍找不到时回退 `"Drafts"`，远端 APPEND 失败则返回错误。
4. 后端构建草稿 RFC822：
   - 生成草稿 Message-ID。
   - 设置 From、To、Cc、Bcc、Subject。
   - 设置 Date。
   - MIME 结构与正式发送保持一致。
   - 有附件时使用 `multipart/mixed`，第一部分是正文 alternative，后续是附件。
5. IMAP APPEND 到 Drafts，标志使用 `(\Draft \Seen)`。
6. APPEND 成功后，本地写入草稿镜像：
   - `folder = drafts_folder`
   - `is_draft = true`
   - `is_read = true`
   - `sender_email = account.email`
   - `recipient_emails`、`cc_emails`、`bcc_emails` 来自请求
   - `message_id` 使用本次草稿 Message-ID
   - `uid` 先使用本地临时 UID，随后通过同步或 UIDPLUS 能力更新为真实 UID
7. 返回 `SaveDraftResponse`，包含本地 `draft_id`、`message_id`、`folder` 和保存时间。

### 更新草稿

1. 前端后续保存时带上 `draft_id`。
2. 后端读取旧草稿，确认它属于同一账号且 `is_draft = true`。
3. 先 APPEND 新草稿到远端 Drafts。
4. 新草稿本地镜像写入成功后，删除旧草稿：
   - 远端优先：对旧草稿 UID 标记 `\Deleted`，必要时执行 expunge 或使用现有删除策略扩展。
   - 本地删除旧邮件及附件行。
5. 返回新的 `draft_id`。前端把当前窗口绑定到新草稿 ID。

这个顺序优先保证“不丢最新草稿”。如果旧草稿远端删除失败，保存仍然成功，并返回诊断字段或记录日志；用户不应该因为清理旧版本失败而丢失当前编辑内容。

### 打开草稿

1. 用户从 Drafts 列表打开邮件详情。
2. 如果 `is_draft = true`，界面提供“继续编辑”入口，或者直接用写信窗口打开。
3. 写信窗口从 `EmailDetail` 填充账号、收件人、抄送、主题、正文和附件列表。
4. 本地附件元数据中的 `path` 如果指向原始本地文件且文件仍存在，则可继续发送；如果文件不存在，发送和保存时提示用户移除或重新选择。

### 删除草稿

1. 用户显式删除草稿，或发送草稿成功后触发删除。
2. 后端先删除远端 Drafts 中对应 UID。
3. 远端删除成功后删除本地邮件和附件元数据。
4. 如果远端删除失败，本地不先删，避免本地看不到但远端仍存在造成下一次同步又出现。

## 附件发送数据流

### 选择附件

1. 前端使用 Tauri dialog 打开本地文件选择器，支持多选。
2. 前端把路径列表传给后端 `describe_local_attachments(paths)`。
3. 后端读取 metadata，返回：

```rust
pub struct LocalAttachmentDraft {
    pub path: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}
```

4. 前端展示文件名、大小和移除按钮。
5. 后端限制附件路径必须是普通文件，不接受目录；文件不存在或不可读时返回明确错误。

### 构建 MIME

保存草稿和发送正式邮件复用同一套 MIME 构建逻辑：

1. 无附件：继续使用 `multipart/alternative`，包含 text/plain 和 text/html。
2. 有附件：外层使用 `multipart/mixed`：
   - 第一部分是 `multipart/alternative` 正文。
   - 后续每个附件设置 `Content-Type`、`Content-Disposition: attachment` 和 filename。
3. BCC 只参与 SMTP envelope，不应写入最终邮件头；如果当前 lettre 构建路径会把 BCC 写进头部，实施时必须修正。
4. 附件读取失败时不进入 SMTP 投递，也不更新远端草稿；前端保留编辑内容。

### 发送带附件邮件

1. 前端调用 `send_email`，请求中带附件数组和可选 `draft_id`。
2. 后端校验收件人、主题、正文仍不允许为空。
3. 后端读取附件文件并构建同一封 RFC822 原文。
4. SMTP 投递成功后：
   - 本地 Sent 写邮件和附件元数据。
   - 远端 Sent APPEND 使用同一封 RFC822 原文。
   - 如果请求带 `draft_id`，删除远端和本地草稿。
5. Sent APPEND 失败不代表发送失败；草稿删除失败也不代表发送失败，但应返回诊断字段或记录日志，供后续补偿。

## 后端接口设计

### 附件输入

```rust
pub struct ComposeAttachmentInput {
    pub path: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}
```

`filename`、`content_type`、`size` 由后端 `describe_local_attachments` 返回，发送和保存时仍由后端重新校验。前端传入值只作为展示和一致性检查，不能作为安全边界。

### 发送请求扩展

```rust
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<ComposeAttachmentInput>,
    pub draft_id: Option<i32>,
}
```

### 草稿保存请求

```rust
pub struct SaveDraftRequest {
    pub draft_id: Option<i32>,
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<ComposeAttachmentInput>,
}
```

草稿允许主题和正文为空，但完全空白的草稿不保存。正式发送仍沿用“收件人、主题、正文不允许为空”的规则。

### 草稿保存响应

```rust
pub struct SaveDraftResponse {
    pub draft_id: i32,
    pub message_id: String,
    pub folder: String,
    pub saved_at: i64,
    pub remote_saved: bool,
    pub cleanup_error: Option<String>,
}
```

`remote_saved` 在推荐方案中成功返回时必须为 `true`。保留这个字段是为了前端状态表达和未来可能的本地兜底模式；本期不把远端失败伪装成保存成功。

### 新增命令

1. `describe_local_attachments(paths: Vec<String>) -> Result<Vec<LocalAttachmentDraft>, MailError>`
2. `save_draft(request: SaveDraftRequest) -> Result<SaveDraftResponse, MailError>`
3. `delete_draft(draft_id: i32) -> Result<(), MailError>`
4. `send_email` 使用扩展后的 `SendEmailRequest`

## 后端模块边界

### `mail_send.rs`

继续承担发件消息构建和 SMTP 发送相关能力，并扩展为支持附件：

1. `build_email` 接收附件输入，返回 `BuiltEmail { message_id, raw, ... }`。
2. MIME 构建函数独立出来，覆盖无附件和有附件两种结构。
3. 附件文件读取、文件名清洗、Content-Type 兜底放在可测试的小函数中。

### 草稿服务

建议新增 `mail_draft.rs`，不要把草稿逻辑全部塞进 `email_service.rs`：

1. `DraftRemoteWriter` trait：封装 APPEND Drafts、删除远端草稿。
2. `RealDraftRemoteWriter`：基于 IMAP 连接实现。
3. `save_draft`：编排构建 MIME、远端 APPEND、本地镜像、旧草稿清理。
4. `delete_draft`：远端优先删除，再删本地。

### IMAP 协议层

现有 `append_email(folder, raw)` 固定使用 `(\Seen)`，需要扩展为可传 flags：

```rust
append_email_with_flags(folder: &str, flags: Option<&str>, raw: &[u8])
```

Sent 使用 `(\Seen)`，Drafts 使用 `(\Draft \Seen)`。

删除远端草稿需要现有 IMAP 能力补齐：

1. `mark_uid_deleted` 从私有函数调整为可用方法，或新增 `delete_uid`。
2. 是否执行 `EXPUNGE` 要谨慎。第一版可以先标记 `\Deleted`，并依赖服务商/同步最终清理；如果产品上要求立即消失，再实现当前选中 Drafts 文件夹内的 expunge。

### 仓储层

复用 `emails` 和 `attachments` 表，不新增表。

需要新增或扩展 repository 函数：

1. `insert_or_replace_draft_email`：写入草稿镜像和附件元数据。
2. `delete_one_with_attachments`：已有能力可复用。
3. `insert_sent_email` 扩展为支持附件元数据。
4. 保存远端同步邮件时继续按 `(account_id, folder, uid)` 合并，并保留现有 Message-ID 合并策略，避免 Sent 或 Drafts 重复显示。

附件表字段使用约定：

1. `filename`：展示文件名。
2. `content_type`：后端探测或 `application/octet-stream`。
3. `size`：本地文件大小。
4. `section_path`：本地发件附件没有远端 MIME section，使用 `compose:{index}`。
5. `disposition`：`attachment`。
6. `content_id`：普通附件为 `None`。
7. `path`：原始本地文件路径。

## 前端设计

### 写信窗口状态

`ComposeModal.svelte` 增加以下状态：

1. `draftId`
2. `attachments`
3. `draftSaveStatus`: `idle | saving | saved | failed`
4. `lastSavedAt`
5. `draftError`

用户修改收件人、抄送、主题、正文或附件后，debounce 调用 `save_draft`。发送中暂停自动保存，避免并发写同一草稿。窗口关闭时如果存在未保存变更，应尝试最后保存一次；保存失败时提示用户确认是否放弃。

### 草稿打开

从 Drafts 邮件详情进入编辑时：

1. 使用邮件详情填充写信窗口。
2. 设置 `draftId = email.id`。
3. 附件列表来自 `EmailDetail.attachments`。
4. 发送成功后关闭窗口，并触发 Drafts 和 Sent 列表刷新。

### 附件 UI

1. 写信窗口工具栏增加附件按钮，使用图标按钮。
2. 附件列表显示在正文区域下方，包含文件名、大小、移除按钮。
3. 附件读取失败、文件不存在、文件过大时显示 toast，并保留其他附件。
4. 第一版不做拖拽区域，避免界面复杂度和测试面扩大。

## 错误处理

### 草稿保存失败

远端 APPEND 失败时返回错误，前端显示“草稿保存失败”，但不关闭窗口、不清空内容。用户继续编辑时下一次 debounce 可重试。

### 旧草稿清理失败

新草稿已保存成功但旧草稿删除失败时，保存整体视为成功，返回 `cleanup_error` 或记录日志。前端状态显示已保存，不打断用户。

### 附件读取失败

保存草稿或发送前重新校验附件路径。任一附件不存在、不可读或不是普通文件时，本次保存/发送失败，前端提示具体文件，保留编辑内容。

### 发送成功但草稿删除失败

SMTP 成功后不允许提示用户重发。草稿删除失败只记录诊断；前端可以刷新 Drafts，若旧草稿仍出现，后续补偿或用户删除处理。

## 测试设计

### Rust 测试

1. 草稿保存：
   - 空白草稿不保存。
   - 空主题、空正文但有收件人或附件时允许保存。
   - APPEND Drafts 使用 `(\Draft \Seen)`。
   - 远端 APPEND 成功后写本地 `is_draft = true`。
   - 更新草稿时先保存新草稿，再清理旧草稿。
2. 草稿删除：
   - 远端删除成功后删除本地邮件和附件。
   - 远端删除失败时不删除本地。
3. 附件：
   - `describe_local_attachments` 拒绝目录和不存在路径。
   - MIME 无附件时是 `multipart/alternative`。
   - MIME 有附件时是 `multipart/mixed`，并包含正文和附件。
   - 附件读取失败时不调用 SMTP。
4. 发送草稿：
   - 带 `draft_id` 发送成功后调用草稿删除。
   - 草稿删除失败不改变发送成功状态。
   - Sent 本地记录包含附件元数据。

### 前端测试

1. 选择附件后展示文件名和大小。
2. 移除附件后发送请求不包含该附件。
3. 编辑内容变化触发自动保存，并展示保存状态。
4. 草稿保存失败时窗口保持打开。
5. 从草稿详情打开写信窗口能填充正文和附件。
6. 发送成功后刷新 Drafts 和 Sent。

### 手动验证

1. 使用真实账号新建草稿，在其他邮件客户端能看到 Drafts 中的草稿。
2. 修改草稿多次，最终远端保留最新版本，旧版本不会长期重复。
3. 发送带附件邮件，收件端能下载附件，Sent 中也能看到附件。
4. 发送草稿后，远端 Drafts 中草稿消失，Sent 中出现已发送邮件。

真实账号验证不进入默认 CI，避免依赖外部 SMTP/IMAP 服务和账号凭证。

## 风险和取舍

1. **IMAP APPEND 不一定返回 UID**：如果 async-imap 当前 APPEND API 无法拿到 APPENDUID，本地草稿只能先使用临时 UID。后续同步 Drafts 时应按 Message-ID 或本地标记合并，避免重复。
2. **旧草稿删除策略**：IMAP 标记 `\Deleted` 后是否立即 expunge 涉及服务商差异。第一版优先保证最新草稿不丢，旧版本清理失败降级为日志和后续同步处理。
3. **远端 Drafts 文件夹名称差异**：必须复用 provider mapping 和 FolderRegistry，不能硬编码单一 `"Drafts"`。
4. **附件路径稳定性**：第一版附件只保存本地原始路径，不复制到应用缓存。用户移动或删除文件后，保存/发送会失败并要求重新选择。
5. **大附件内存占用**：构建 MIME 需要读取附件内容。第一版应设置单文件和总大小上限，避免一次性读取过大文件；具体阈值在实施计划中确定。
6. **BCC 头泄露风险**：正式发送邮件不应在 RFC822 头中包含 BCC。实现附件 MIME 重构时必须把该点纳入测试。

## 验收标准

1. 写信窗口有内容后能自动保存到远端 Drafts，其他客户端可见。
2. Drafts 列表能看到本地草稿镜像，并能重新打开编辑。
3. 多次保存同一草稿不会以丢内容为代价清理旧版本。
4. 发送草稿成功后，草稿从本地和远端 Drafts 消失。
5. 普通本地文件附件可以随邮件发送，收件端和 Sent 副本都能看到附件。
6. 附件缺失或不可读时不进入 SMTP 投递，写信窗口内容不丢。
7. 默认自动化测试不依赖真实邮箱服务。
