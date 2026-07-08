# 收件附件功能完善设计

## 背景

当前项目已经有附件元数据表和部分 IMAP BODYSTRUCTURE 解析能力，但附件功能还没有形成完整闭环。

现状问题：

- `attachments` 表已保存文件名、MIME 类型、大小、`section_path`、`content_id`、本地 `path` 等字段，但缺少缓存路径更新和附件所属邮件/账号的组合查询。
- 同步流程可以从 BODYSTRUCTURE 提取部分附件元数据，但 inline/CID 图片识别不完整，可能漏掉没有文件名但带 `Content-ID` 的图片。
- `EmailDetail` 不返回真实附件列表，前端邮件详情页只保留了占位附件 UI。
- `EmailDto.has_attachments` 多处仍是固定值或非数据库真实计算。
- IMAP 层没有按 MIME section 下载单个附件的能力，不能避免为了一个附件重新拉取整封邮件。
- 前端缺少下载、打开、另存为、下载状态、图标和错误反馈。
- 发送邮件目前不支持附件，但本阶段优先完善收件附件。

## 目标

一期完成收件附件的同步、显示、下载、缓存、另存为、打开和 CID 图片显示。

具体目标：

1. 同步邮件头、完整邮件和单封邮件重新加载时，都保存真实附件元数据。
2. 普通附件和 inline/CID 图片都进入附件元数据链路。
3. 邮件列表和详情的 `has_attachments` 从本地数据库真实计算。
4. 邮件详情返回 `attachments: AttachmentDto[]`。
5. 小于等于 `10MB` 的附件按需下载到应用缓存目录，后续打开和另存为复用缓存。
6. 大于 `10MB` 的附件不写入应用缓存，下载时直接弹系统保存对话框并写入用户选择的路径。
7. 小于等于 `10MB` 的 CID 图片按需缓存后替换到 HTML 中显示。
8. 大于 `10MB` 的 CID 图片不自动加载，作为附件项允许用户手动保存。
9. 暂不做自动缓存清理，缓存文件失效时按需重新下载。

## 非目标

- 不实现发信附件。
- 不实现附件缓存自动清理、容量配额、缓存管理界面。
- 不实现跨邮件的附件去重。
- 不实现后台批量预下载所有附件。
- 不实现大附件下载进度条；一期只做下载中、成功、失败的状态。
- 不主动删除历史缓存文件。删除邮件或重新加载邮件时只维护数据库元数据。

## 用户体验

邮件详情页展示真实附件区域。每个附件项显示：

- 文件图标。
- 文件名。
- 文件大小。
- 下载状态。
- 操作按钮。

附件操作规则：

- 小附件：
  - “下载”会保存到应用缓存目录。
  - “打开”会先确保缓存存在，再调用系统打开。
  - “另存为”会弹系统保存对话框；缓存存在时复制缓存，缓存丢失时重新从 IMAP 拉取后保存。
- 大附件：
  - “保存”会先弹系统保存对话框，再从 IMAP 直接写入目标路径。
  - 默认不提供“打开”，因为一期不把大附件作为应用缓存资产管理。
- CID 图片：
  - 小 CID 图片在打开邮件详情后按需缓存并替换 HTML 中的 `cid:` 引用。
  - 大 CID 图片不自动加载，保留为附件项让用户手动保存。

用户取消保存对话框时不展示错误。网络、认证、IMAP、解码或文件写入失败时，保留当前界面状态并展示失败提示。

## 数据模型

复用现有 `attachments` 表：

- `email_id` 关联邮件。
- `filename` 保存文件名；没有文件名的 CID 图片使用基于 content id 或 MIME 类型生成的稳定名称。
- `content_type` 保存 MIME 类型。
- `size` 保存服务端声明大小。
- `section_path` 保存 IMAP MIME section 路径，用于按需下载。
- `disposition` 保存 `attachment`、`inline` 等展示类型。
- `content_id` 保存 CID 图片映射。
- `path` 保存小附件缓存路径。

一期不新增迁移。原因是现有字段已经能表达下载所需信息，额外的缓存状态可以由 `path` 是否存在和磁盘文件是否存在推导。

新增前端绑定类型：

```ts
type AttachmentDto = {
  id: number;
  email_id: number;
  filename: string;
  content_type: string;
  size: number;
  disposition: string | null;
  content_id: string | null;
  is_inline: boolean;
  is_cached: boolean;
  cache_path: string | null;
};
```

`EmailDetail` 增加：

```ts
attachments: AttachmentDto[];
```

## 后端设计

### 解析层

扩展 BODYSTRUCTURE 附件识别规则：

1. `Content-Disposition: attachment` 视为附件。
2. `Content-Disposition` 或 `Content-Type` 带文件名参数视为附件。
3. 带 `Content-ID` 且 MIME 类型为 `image/*` 的 inline part 视为 CID 图片附件。

`section_path` 必须保留完整路径。没有 `section_path` 的附件不能按需下载，应返回明确错误。

### 仓储层

`attachment_repo` 增加方法：

- 按邮件列出附件。
- 按附件 ID 查询附件及所属邮件、账号、文件夹、UID。
- 更新附件缓存路径。
- 判断附件缓存文件是否仍存在。

`email_repo` 调整列表和详情转换逻辑：

- `has_attachments` 改为数据库真实计算。
- `get_email` 返回邮件详情时同步附带附件列表。

### IMAP 层

新增按单个 MIME section 获取附件内容的方法：

```rust
fetch_body_section(folder: &str, uid: u32, section_path: &str) -> Result<Option<Vec<u8>>, MailError>
```

语义：

- 先 `SELECT folder`。
- 使用 `UID FETCH <uid> BODY.PEEK[<section_path>]`。
- 找不到邮件或 section 时返回 `Ok(None)`。
- 成功时返回该 part 原始字节。

解码策略：

- 优先依据 BODYSTRUCTURE 或解析结果中的 transfer encoding 解码。
- 支持 `base64`、`quoted-printable`、`7bit`、`8bit`、`binary`。
- 解码失败返回明确错误，不写入半文件。

### 附件服务

新增 `AttachmentService`，负责附件下载、缓存、保存和打开。

核心方法：

```rust
list_by_email(email_id: i32) -> Result<Vec<AttachmentDto>, MailError>
ensure_cached(attachment_id: i32) -> Result<AttachmentDto, MailError>
save_as(attachment_id: i32, target_path: String) -> Result<(), MailError>
open(attachment_id: i32) -> Result<(), MailError>
resolve_inline_images(email_id: i32) -> Result<Vec<InlineAttachmentDto>, MailError>
```

小附件缓存流程：

1. 查询附件、邮件、账号、文件夹和 UID。
2. 如果 `attachments.path` 存在且磁盘文件存在，直接返回。
3. 如果缓存缺失，从 IMAP 按 `section_path` 拉取单个 part。
4. 解码内容。
5. 写入缓存目录的临时文件。
6. 原子替换为最终文件。
7. 更新 `attachments.path`。
8. 返回最新附件状态。

大附件保存流程：

1. 前端通过系统对话框拿到目标路径。
2. 后端按 `section_path` 从 IMAP 拉取单个 part。
3. 解码后直接写入目标路径。
4. 不更新 `attachments.path`。

打开附件流程：

1. 只对小附件开放。
2. 先调用 `ensure_cached`。
3. 使用现有 opener 插件打开缓存文件。

缓存目录：

```text
~/.postium/attachments-cache/<account_id>/<email_id>/<attachment_id>-<safe_filename>
```

文件名必须做安全化处理，移除路径分隔符、控制字符和空文件名。最终路径只能落在缓存根目录下。

## Tauri 能力

新增系统保存对话框：

- Rust 侧增加 `tauri-plugin-dialog`。
- 前端增加 `@tauri-apps/plugin-dialog`。
- capabilities 增加 dialog 保存权限。

本地图片显示：

- 前端使用 Tauri 的本地文件 URL 转换能力展示缓存后的 CID 图片。
- 如果实际验证转换结果使用 `asset:` 协议，则在 CSP 的 `img-src` 中加入对应协议。
- 只允许后端返回的附件缓存路径进入 CID 映射，前端不从邮件 HTML 中自行拼本地路径。

## 前端设计

### Store

`EmailState` 增加附件状态：

- 当前邮件附件列表。
- 按附件 ID 记录操作中状态。
- 按附件 ID 记录错误。
- 解析后的 HTML 内容。

行为：

- 打开邮件详情后使用返回的 `attachments` 渲染附件区域。
- 如果邮件包含小 CID 图片，调用后端解析并缓存 inline 图片，然后替换 HTML。
- 下载、打开、另存为操作只更新对应附件项，不刷新整封邮件。

### 详情组件

`EmailDetail.svelte` 附件区域改为真实数据渲染。

图标规则：

- `image/*` 使用图片图标。
- `application/pdf` 使用 PDF/文档图标。
- `application/zip`、`application/x-7z-compressed`、`application/x-rar-compressed` 使用压缩包图标。
- `audio/*` 使用音频图标。
- `video/*` 使用视频图标。
- 表格类型使用表格图标。
- 文本类型使用文本图标。
- 其他类型使用通用文件图标。

按钮规则：

- 小附件显示下载、打开、另存为。
- 大附件显示保存。
- 正在操作时禁用该附件按钮。
- 错误只影响该附件项，不影响整封邮件。

### HTML 与 CID

CID 替换流程：

1. 后端返回 `content_id -> local_url` 映射。
2. 前端只替换匹配当前邮件附件的 `cid:` 引用。
3. 替换后继续走 DOMPurify 清理。
4. 清理后的 HTML 才进入 `{@html}`。

## 错误处理

- 附件不存在：返回附件不存在错误，前端刷新或保留当前状态。
- 邮件不存在：返回邮件不存在错误。
- 邮件没有 UID 或 section_path：返回不可下载错误。
- IMAP 连接失败、认证失败、网络失败：返回远端错误。
- 远端 section 不存在：返回附件远端不存在错误。
- 解码失败：返回附件解码错误，不写入缓存或目标文件。
- 缓存路径失效：重新下载；重下失败则保留原状态。
- 用户取消保存对话框：前端静默处理。
- 文件写入失败：返回文件系统错误，并保留原缓存路径。

## 测试计划

### Rust

- BODYSTRUCTURE 解析普通附件。
- BODYSTRUCTURE 解析带文件名的 inline 附件。
- BODYSTRUCTURE 解析无文件名但带 `Content-ID` 的 `image/*` CID 图片。
- `has_attachments` 从数据库真实计算。
- 邮件详情返回真实附件列表。
- 小附件 `ensure_cached` 首次下载后写入缓存路径。
- 小附件缓存存在时复用缓存，不重复拉取 IMAP。
- 缓存路径存在但文件丢失时重新下载。
- 大附件 `save_as` 写入目标路径但不更新缓存路径。
- `open` 会先确保小附件已缓存。
- 解码失败不会留下半文件。

### Frontend

- 邮件详情渲染真实附件列表。
- 不同 MIME 类型显示对应图标。
- 小附件显示下载、打开、另存为。
- 大附件只显示保存。
- 点击保存时打开系统保存对话框。
- 用户取消保存时不报错。
- 附件操作中只禁用当前附件按钮。
- CID 图片映射后 HTML 正常显示。
- 下载失败只展示对应附件错误。

### 验证命令

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json`
- `node node_modules/vitest/vitest.mjs run --pool threads --maxWorkers 1 --reporter dot`
- `npm run check`
- `npm run build`

### 手动验证

至少准备四封真实 IMAP 邮件：

1. 小于等于 `10MB` 的普通附件。
2. 大于 `10MB` 的普通附件。
3. HTML 正文内嵌 CID 小图片。
4. 附件或 CID 远端 section 无法获取的异常样本。

验证结果：

- 小附件能缓存、打开、另存为。
- 大附件能直接保存到用户选择路径。
- CID 小图片能在正文中显示。
- 失败场景不会破坏邮件详情或附件列表状态。

## 实施顺序

1. 完善附件解析和数据库读取能力。
2. 让邮件列表和详情返回真实附件状态。
3. 增加 IMAP section 下载和附件服务。
4. 增加 Tauri 命令、dialog 插件和 capabilities。
5. 改造前端 store 和邮件详情附件 UI。
6. 实现 CID 图片缓存和 HTML 替换。
7. 补齐 Rust、前端和手动验证。

