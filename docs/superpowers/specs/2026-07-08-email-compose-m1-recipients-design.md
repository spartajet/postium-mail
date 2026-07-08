# M1 收件人字段与 BCC UI 设计

## 背景

发件里程碑文档将 M1 定义为“收件人字段与 BCC UI”。当前写信窗口仍使用逗号分隔字符串维护 `to` 和 `cc`，没有 BCC 输入界面；后端 `SendEmailRequest` 和 `SaveDraftRequest` 已经包含 `bcc: Vec<String>`，但前端始终传空数组。用户选择 M1 采用“行式表单 + 统一 RecipientField chip 输入”方向，并要求直接解决复杂邮件地址解析问题，而不是只做简单分隔。

本阶段目标是让 To、Cc、Bcc 三类收件人字段具备完整输入体验：支持粘贴复杂地址、逐个展示、删除、标记错误，并保证发送和草稿保存使用同一套有效地址结果。

## 目标

1. 写信窗口提供 To、Cc、Bcc 三类收件人字段。
2. To 默认展示，Cc 和 Bcc 可按行式表单展开。
3. 收件人字段使用 chip/token 展示地址，支持逐个删除。
4. 支持粘贴多个地址并批量解析。
5. 支持 `Name <email@example.com>`、带引号显示名、逗号显示名、多个地址列表等常见邮件地址格式。
6. 使用后端 Rust `mail-parser` 作为权威地址解析器，避免前后端解析规则不一致。
7. 发送时 To、Cc、Bcc 合并后至少需要一个有效地址。
8. 存在无效 chip 时阻止发送，并提示用户修正或删除。
9. 草稿保存包含有效的 Bcc 地址。
10. Bcc 地址只进入 SMTP envelope，不写入最终邮件头。

## 非目标

1. 不做联系人自动补全。
2. 不做通讯录。
3. 不做回复、全部回复和转发语义增强。
4. 不做签名、Reply-To 或发件别名。
5. 不改变发送和草稿请求的核心 DTO 形状；提交时仍传 `Vec<String>`。
6. 不引入新的前端 npm 邮件地址解析依赖。

## 推荐方案

采用“后端统一解析 + 前端 chip 展示”的方案。

前端新增 `RecipientField` 组件，负责输入、粘贴、键盘分隔、chip 展示、错误状态和删除交互。组件不自行实现完整 RFC 邮件地址解析；当用户提交输入片段时，前端调用新增 Tauri 命令 `parse_email_addresses`，把原始文本交给后端解析。后端复用项目已有 Rust `mail-parser` 依赖，把地址解析为标准结构返回给前端。发送和草稿保存时，`ComposeModal` 从 To、Cc、Bcc 三组 chip 中提取有效地址，继续传给现有 `SendEmailRequest` 和 `SaveDraftRequest`。

这个方案的关键收益是解析规则集中在后端，后续 M2 的全部回复、M3 的发件身份、真实邮箱验证都能复用同一套地址规范化结果。前端保持轻量，专注交互。

## 地址解析接口

### 新增 DTO

新增以下结构：

```rust
pub struct ParsedEmailAddress {
    pub name: Option<String>,
    pub email: String,
    pub raw: String,
}

pub struct InvalidEmailAddress {
    pub raw: String,
    pub reason: String,
}

pub struct DuplicateEmailAddress {
    pub raw: String,
    pub email: String,
}

pub struct ParseEmailAddressesResponse {
    pub addresses: Vec<ParsedEmailAddress>,
    pub invalid: Vec<InvalidEmailAddress>,
    pub duplicates: Vec<DuplicateEmailAddress>,
}
```

### 新增命令

```rust
parse_email_addresses(input: String) -> Result<ParseEmailAddressesResponse, MailError>
```

命令职责：

1. 接收用户输入或粘贴的原始字符串。
2. 使用 `mail-parser` 解析地址列表。
3. 返回可用地址、无法解析片段和重复地址。
4. 对同一解析请求内重复出现的邮箱做去重，重复项进入 `duplicates`。
5. 对邮箱大小写做规范化比较；展示使用解析器返回的邮箱文本，去重比较使用 lowercase。

错误策略：

1. 解析命令本身尽量返回结构化结果，不因部分无效地址整体失败。
2. 只有输入完全无法处理或解析器内部异常时才返回 `MailError`。
3. 无效片段返回给前端展示为错误 chip。

## 前端组件设计

### `RecipientField`

新增 `src/lib/components/email/RecipientField.svelte`。

职责：

1. 渲染 label、chip 列表和输入框。
2. 处理输入分隔键：逗号、分号、Enter、Tab。
3. 处理粘贴文本，把完整粘贴内容交给后端解析。
4. 展示有效 chip：
   - 有显示名时显示 `Name <email@example.com>` 或两段式信息。
   - 无显示名时显示邮箱。
5. 展示错误 chip：
   - 使用错误样式。
   - tooltip 或 title 显示原因。
   - 错误 chip 不参与发送和草稿保存。
6. 支持删除 chip。
7. 对重复地址给出轻量反馈，不重复加入 chip。
8. 输入变化后通知父组件触发草稿保存。

组件状态：

```ts
type RecipientChip = {
  id: string;
  name?: string;
  email?: string;
  raw: string;
  valid: boolean;
  reason?: string;
};
```

`ComposeModal` 维护三组状态：

```ts
let toRecipients = $state<RecipientChip[]>([]);
let ccRecipients = $state<RecipientChip[]>([]);
let bccRecipients = $state<RecipientChip[]>([]);
```

提交时派生：

```ts
const to = validEmails(toRecipients);
const cc = validEmails(ccRecipients);
const bcc = validEmails(bccRecipients);
```

### BCC UI

1. To 行默认显示。
2. Cc 和 Bcc 可以通过 To 行右侧的小按钮展开。
3. 如果 Cc 或 Bcc 已有 chip，则对应行始终显示。
4. 展开状态关闭时不删除已有 chip。
5. 关闭写信窗口时清空所有收件人状态。

## 数据流

### 输入或粘贴

1. 用户在任一 `RecipientField` 中输入文本。
2. 用户按逗号、分号、Enter 或 Tab，或粘贴多个地址。
3. 组件调用 `parse_email_addresses(input)`。
4. 后端返回 `addresses`、`invalid`、`duplicates`。
5. 前端追加有效 chip，追加错误 chip，忽略重复项或展示重复提示。
6. 字段状态变化触发 `scheduleDraftSave()`。

### 草稿保存

1. `hasDraftContent()` 把 To、Cc、Bcc 三组 chip 都纳入判断。
2. `saveDraftNow()` 只提交有效地址：
   - `to = validEmails(toRecipients)`
   - `cc = validEmails(ccRecipients)`
   - `bcc = validEmails(bccRecipients)`
3. 错误 chip 保留在当前 UI 中，但不写入远端 Drafts。
4. 重新打开本地草稿时，从 `recipient_emails`、`cc_emails`、`bcc_emails` 恢复有效 chip。

这个取舍优先保证远端 Drafts 可构建、可同步，不把无法解析的地址写入邮件原文。

### 正式发送

1. 发送前检查是否存在错误 chip。
2. 如果存在错误 chip，阻止发送并提示用户修正或删除无效地址。
3. 如果 To、Cc、Bcc 全部没有有效地址，显示“请填写至少一个收件人”。
4. 主题和正文仍沿用已有校验。
5. 发送请求包含 To、Cc、Bcc 的有效地址数组。
6. 后端发送链路继续执行最终地址校验。

## BCC 隐私要求

1. Bcc 地址参与 SMTP envelope。
2. Bcc 地址不写入最终 MIME header。
3. 本地 Sent 可以保存 `bcc_emails` 元数据，便于用户在已发送详情中看到自己发送给了哪些密送对象。
4. 远端 Sent APPEND 使用的 RFC822 原文不应包含 `Bcc:` header。

现有后端已有相关测试基础，M1 需要补充前端接通和回归测试。

## 错误处理

1. 部分解析失败不应清空已有 chip。
2. 解析命令失败时，当前输入保留在输入框中，并显示错误提示。
3. 错误 chip 可删除。
4. 重复地址不新增第二个 chip。
5. 发送失败时写信窗口保持打开，chip 状态不丢失。
6. 草稿保存失败时沿用现有草稿失败状态，不清空收件人。

## 代码边界

预计涉及文件：

1. `src-tauri/src/service/email_service.rs`
   - 新增地址解析 DTO 和服务方法。
2. `src-tauri/src/command/email.rs`
   - 新增 `parse_email_addresses` 命令。
3. `src-tauri/src/lib.rs`
   - 注册新命令并导出绑定。
4. `src/lib/bindings.ts`
   - 更新生成绑定。
5. `src/lib/components/email/RecipientField.svelte`
   - 新增统一收件人字段组件。
6. `src/lib/components/email/ComposeModal.svelte`
   - 用 `RecipientField` 替换 To/Cc 文本输入，接入 Bcc。
7. `src/lib/i18n/zh-CN.ts`
   - 增加无效地址、重复地址、展开/收起 Cc/Bcc 等文案。
8. `src/lib/i18n/en-US.ts`
   - 增加对应英文文案。
9. `src-tauri/tests/email_commands.rs`
   - 覆盖后端地址解析和 Bcc header 隐私。
10. `src/lib/__tests__/components/RecipientField.test.ts`
    - 覆盖组件输入和 chip 行为。
11. `src/lib/__tests__/components/ComposeModal.test.ts`
    - 覆盖 Bcc、草稿保存和发送校验。

## 测试设计

### Rust 测试

覆盖 `parse_email_addresses`：

1. 普通邮箱：`alice@example.com`。
2. 显示名：`Alice <alice@example.com>`。
3. 带引号显示名：`"Alice A." <alice@example.com>`。
4. 显示名中包含逗号：`"Alice, Inc." <alice@example.com>`。
5. 多地址粘贴：`alice@example.com, Bob <bob@example.com>`。
6. 无效片段：`alice@example.com, not-an-address`。
7. 重复地址：`alice@example.com, Alice <alice@example.com>`。
8. 地址组：解析 group 中的成员地址，group 名不作为收件人；无法拆出的片段进入 `invalid`。
9. Bcc 不写入 MIME header。

### 前端组件测试

覆盖 `RecipientField`：

1. 输入邮箱后按 Enter 生成 chip。
2. 粘贴多个地址生成多个 chip。
3. 删除 chip。
4. 后端返回 invalid 时展示错误 chip。
5. 后端返回 duplicate 时不重复添加。
6. 错误 chip 不参与 `validEmails` 输出。

覆盖 `ComposeModal`：

1. Bcc 展开后可输入地址。
2. 发送请求包含 `bcc`。
3. 草稿保存请求包含有效 `bcc`。
4. 存在错误 chip 时阻止发送。
5. To、Cc、Bcc 都没有有效地址时提示收件人必填。
6. 关闭再打开清空收件人状态。
7. 切换发件账号不破坏当前收件人 chip，除非关闭窗口。

## 验收标准

1. 用户可以在 To、Cc、Bcc 中输入和粘贴复杂邮件地址。
2. UI 以 chip 形式逐个展示收件人。
3. 用户可以删除任意 chip。
4. 无效地址以错误 chip 展示，并阻止正式发送。
5. 重复地址不会重复加入。
6. Bcc 可以发送，且不出现在邮件原文 header 中。
7. 草稿保存和正式发送都使用有效地址。
8. 默认自动化测试覆盖后端解析、前端 chip 行为和 ComposeModal 集成。
