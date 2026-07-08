# 所有账号邮件视图设计

## 背景

当前账号选择器只能选择某一个具体邮箱账号。用户选择账号后，点击侧边栏文件夹时，只会显示该账号下对应分类的邮件，例如收件箱、星标、已发送、草稿、垃圾邮件和回收站。

这对多账号用户不够高效。用户需要在多个邮箱之间切换，才能处理同一类邮件。目标是在账号选择器中增加“所有账号”选项，让用户在一个聚合视图中处理所有邮箱账号下的同类邮件。

现有代码中已经存在 `t.sidebar.allAccounts` 文案兜底，但实际数据流仍围绕 `activeAccountId: number | null` 和单账号后端接口运行。邮件列表、侧边栏统计、分页、刷新同步、同步更早邮件、写邮件发送账号都依赖具体账号 ID。因此本功能不能只增加一个前端下拉项，需要从账号选择状态、后端查询、统计、同步和写信交互一起设计。

## 目标

1. 账号下拉框增加“所有账号”选项。
2. 用户选择“所有账号”后，点击侧边栏文件夹能看到所有账号中该分类的邮件。
3. 所有账号视图下邮件按发送时间统一混排，支持分页和仅未读筛选。
4. 所有账号视图下侧边栏未读角标显示所有账号累加结果。
5. 所有账号视图下邮件列表显示每封邮件所属账号，避免用户无法判断来源。
6. 所有账号视图下点击同步按钮会同步全部账号，完成后刷新当前文件夹和统计。
7. 账号选择状态需要持久化。启动时恢复上次选择；首次启动默认“所有账号”。
8. 如果上次选择的具体账号已删除，自动回退到“所有账号”。
9. 所有账号视图下写邮件时，写信窗口必须让用户选择发件账号。
10. 回复或转发某封邮件时，默认发件账号使用该邮件所属账号。

## 非目标

1. 不重做标签系统。
2. 不改变现有邮件详情、标星、已读、删除、移动等单封邮件操作语义。
3. 不引入新的数据库分类字段或迁移历史邮件分类。
4. 不把所有账号视图做成新的路由。
5. 不要求“同步更早邮件”第一版支持所有账号聚合视图；可在所有账号视图下隐藏或禁用该入口。
6. 不在本设计阶段提交代码。

## 推荐方案

采用“账号范围 Account Scope + 后端跨账号聚合查询”的方案。

前端不在所有账号视图下循环调用每个账号的单账号接口，而是调用明确的跨账号命令。后端负责按账号解析文件夹分类、合并条件、统一排序和分页。

这样可以保证：

1. 分页语义正确。所有账号邮件先统一排序，再分页。
2. 统计语义正确。每个账号先按自身 provider 折叠分类，再累加分类。
3. 前端状态简单。前端只需要根据当前账号范围选择单账号命令或所有账号命令。
4. 现有单账号路径尽量保持稳定，降低回归风险。

## 前端状态设计

### Account Scope

`AccountState` 增加明确的账号选择范围：

```ts
type AccountScope =
  | { kind: "all" }
  | { kind: "account"; accountId: number };
```

保留派生状态，减少调用点重复判断：

```ts
isAllAccounts: boolean;
selectedAccountId: number | null;
selectedAccount: AccountDto | null;
lastConcreteAccountId: number | null;
```

语义：

1. `kind: "all"` 表示当前视图聚合所有账号。
2. `kind: "account"` 表示当前视图只看某个具体账号。
3. `selectedAccountId` 只在具体账号视图下返回账号 ID，否则返回 `null`。
4. `lastConcreteAccountId` 保存最近一次选择过的具体账号，用于所有账号视图下写信窗口默认发件账号。

### 持久化

使用 `localStorage` 保存账号选择：

```text
postium-account-scope
```

保存内容为 JSON：

```json
{ "kind": "all" }
```

或：

```json
{ "kind": "account", "accountId": 1 }
```

恢复规则：

1. 如果没有持久化值，默认使用 `{ "kind": "all" }`。
2. 如果持久化为具体账号，且账号仍存在，则恢复该账号。
3. 如果持久化为具体账号，但账号已不存在，则回退到“所有账号”。
4. 如果没有任何账号，仍保留“所有账号”选择，但邮件列表为空，写信发送不可用。
5. 删除账号时，如果删除的是当前具体账号，则回退到“所有账号”。
6. 删除账号时，如果删除的是 `lastConcreteAccountId`，则更新为第一个剩余账号或 `null`。

### 兼容现有调用点

现有大量代码使用 `activeAccountId`。实现时可以先保留兼容 getter：

```ts
activeAccountId: number | null;
activeAccount: AccountDto | null;
setActive(id: number): void;
setAllAccounts(): void;
```

其中 `activeAccountId` 在所有账号视图下返回 `null`。新增代码优先使用 `accountScope`、`isAllAccounts`、`selectedAccountId`，逐步减少对 `null` 特殊语义的依赖。

## 后端接口设计

新增显式跨账号命令，避免把现有单账号命令参数改成可空而扩大影响范围：

```rust
list_emails_by_category_for_all_accounts(
    category: EmailCategory,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<EmailListResponse, MailError>

get_folder_stats_for_all_accounts() -> Result<Vec<FolderStat>, MailError>

sync_all_accounts() -> Result<SyncAllAccountsResult, MailError>
```

现有命令保持不变：

```rust
list_emails_by_category(account_id, category, page, limit, unread_only)
get_folder_stats(account_id)
sync_account(account_id)
```

前端根据 `accountStore.isAllAccounts` 选择调用单账号命令或所有账号命令。

## 邮件列表 DTO

扩展 `EmailDto`，让前端不需要额外按 `account_id` 查账号：

```rust
pub struct EmailDto {
    // existing fields...
    pub account_id: i32,
    pub account_email: Option<String>,
    pub account_display_name: Option<String>,
}
```

单账号和所有账号接口都返回账号展示字段。前端只在所有账号视图中显示这些字段。

显示优先级：

1. 如果 `account_display_name` 非空，显示 `account_display_name`。
2. 否则显示 `account_email`。
3. 如果账号展示字段缺失，使用 `account_id` 的降级文案，但正常实现不应依赖这个降级。

## 跨账号邮件查询设计

### 星标分类

星标分类不依赖真实 IMAP 文件夹名，可直接跨账号查询：

```sql
WHERE is_starred = 1
  AND is_deleted = 0
  AND (? = 0 OR is_read = 0 OR is_read IS NULL)
ORDER BY sent_at DESC
LIMIT ? OFFSET ?
```

`total` 使用同一条件 `COUNT(*)`。

### 文件夹型分类

`inbox`、`sent`、`drafts`、`spam`、`trash`、`archive` 等分类需要按账号 provider 和本地文件夹数据解析。

流程：

1. 查询所有账号。
2. 对每个账号复用现有分类解析逻辑：
   - 读取账号 provider。
   - 构建 `FolderRegistry`。
   - 将 `EmailCategory` 转为 `FolderCategory`。
   - 解析该账号下分类对应的真实文件夹名列表。
3. 过滤掉没有对应文件夹的账号。
4. 将每个账号解析结果转换为条件组：

```text
(account_id = ? AND folder IN (...))
```

5. 将所有条件组用 `OR` 合并：

```sql
WHERE (
    (account_id = ? AND folder IN (?, ?))
    OR (account_id = ? AND folder IN (?))
)
AND is_deleted = 0
AND (? = 0 OR is_read = 0 OR is_read IS NULL)
ORDER BY sent_at DESC
LIMIT ? OFFSET ?
```

6. `total` 使用同一条件 `COUNT(*)`。

如果所有账号都没有对应分类文件夹，返回空列表和 `total = 0`。

### 错误处理

跨账号查询中，如果单个账号 provider 不支持或账号数据异常：

1. 记录日志并跳过该账号。
2. 继续处理其他账号。
3. 如果至少一个账号成功构造查询，则返回成功结果。
4. 如果没有任何账号可以参与查询，且存在错误，则返回错误。
5. 如果没有任何账号或没有任何账号拥有该分类文件夹，则返回空结果。

## 文件夹统计设计

所有账号统计不直接按原始 `folder` 名称聚合，因为不同 provider 的同一分类可能有不同真实文件夹名。

流程：

1. 查询所有账号。
2. 对每个账号复用现有 `get_folder_stats(account_id)`，得到已经折叠到侧边栏分类的统计。
3. 在 `SyncService` 中按 `folder` 字段累加 `total` 和 `unread`。
4. 星标统计同样由每个账号的现有星标统计累加。
5. 返回按 `folder` 排序后的结果。

如果某个账号统计失败：

1. 记录日志并跳过。
2. 如果至少一个账号成功，返回部分成功结果。
3. 如果全部账号失败，返回错误。

## 同步设计

新增 `sync_all_accounts()`，用于所有账号视图下的同步按钮和邮件列表刷新按钮。

返回结构：

```rust
pub struct SyncAllAccountsResult {
    pub total: usize,
    pub succeeded: Vec<i32>,
    pub failed: Vec<SyncAccountFailure>,
}

pub struct SyncAccountFailure {
    pub account_id: i32,
    pub email: Option<String>,
    pub message: String,
}
```

行为：

1. 查询所有账号。
2. 逐个执行现有 `sync_account(account_id)`。
3. 单个账号失败不阻断其他账号。
4. 至少一个账号成功时返回 `Ok(SyncAllAccountsResult)`，前端显示部分成功提示。
5. 全部账号失败时返回错误。
6. 同步结束后前端刷新当前分类邮件列表和所有账号统计。

并发策略第一版建议串行同步，原因：

1. 复用现有同步服务，避免同时打开多个 IMAP 连接带来的资源和事件状态问题。
2. 前端已有 `syncing` 是单一布尔状态，串行更容易与现有进度 UI 兼容。
3. 后续如果需要优化速度，再设计并发和独立进度。

## UI 与交互设计

### 账号下拉框

账号下拉顶部增加固定选项“所有账号”，放在具体账号列表之前。

选中“所有账号”后：

1. 触发按钮显示“所有账号”。
2. 头像位置使用聚合标识，可以使用简短文字或合适的 lucide 图标。
3. 下拉中“所有账号”选项高亮。
4. 具体账号不高亮。
5. 点击具体账号后切换回单账号视图，并更新 `lastConcreteAccountId`。

### 侧边栏文件夹

点击文件夹时：

1. 更新当前文件夹分类。
2. 如果当前是所有账号视图，调用所有账号邮件查询。
3. 如果当前是具体账号视图，沿用现有单账号查询。
4. 导航到 `/`，保持现有行为。

未读角标：

1. 所有账号视图显示 `get_folder_stats_for_all_accounts()` 的结果。
2. 单账号视图显示 `get_folder_stats(account_id)` 的结果。

### 邮件列表

所有账号视图下，每封邮件显示来源账号标识。

建议位置：

1. 发件人行右侧、时间左侧。
2. 或主题行开头。

第一版推荐放在发件人行右侧、时间左侧：

```text
发件人                         工作邮箱  10:32
主题
预览
```

显示要求：

1. 账号标识使用小字号、低强调色。
2. 超长邮箱截断。
3. 单账号视图不显示来源账号，保持现有列表密度。
4. 搜索结果如果处于所有账号视图，也应显示账号来源。

### 写邮件

所有账号视图下打开写信窗口：

1. 显示“发件账号”选择行。
2. 默认选择 `lastConcreteAccountId` 对应账号。
3. 如果 `lastConcreteAccountId` 不存在，则默认第一个账号。
4. 发送按钮要求发件账号、收件人和主题都有效。
5. 发送请求中的 `account_id` 使用用户选择的发件账号。

单账号视图下打开写信窗口：

1. 第一版隐藏“发件账号”选择行。
2. 发送账号使用当前具体账号。

回复或转发：

1. 如果从某封邮件打开回复或转发，默认发件账号使用该邮件所属账号。
2. 这条规则优先于 `lastConcreteAccountId`。
3. 所有账号视图下仍显示发件账号选择，允许用户切换。

### 搜索

现有搜索命令已经支持 `accountId` 可为空时跨账号搜索。

搜索行为：

1. 所有账号视图下传 `accountId = null`。
2. 单账号视图下传具体 `accountId`。
3. 所有账号视图下搜索结果显示账号来源。

## 同步更早邮件

现有“同步更早邮件”依赖具体账号和分类历史状态。第一版所有账号视图下不支持该入口。

行为：

1. 单账号视图保持现有逻辑。
2. 所有账号视图下隐藏或禁用“同步更早邮件”按钮。
3. 如果需要支持所有账号历史同步，后续单独设计每账号历史状态和部分失败展示。

## 边界情况

### 没有账号

1. 账号选择器显示“所有账号”。
2. 邮件列表为空。
3. 同步按钮禁用或显示无账号可同步错误。
4. 写邮件发送按钮不可用，提示需要先添加账号。

### 账号删除

1. 删除当前具体账号后，回退到“所有账号”。
2. 删除非当前账号后，如果当前是所有账号，刷新所有账号邮件和统计。
3. 删除 `lastConcreteAccountId` 后，重置为第一个剩余账号或 `null`。

### 部分账号查询失败

1. 查询聚合时跳过失败账号。
2. 如果仍有结果可返回，邮件列表正常显示。
3. 错误信息记录到日志；如后续需要用户可见提示，可在 toast 中提示“部分账号加载失败”。

### 部分账号同步失败

1. 继续同步其他账号。
2. 同步完成后刷新当前邮件列表和统计。
3. 前端提示部分失败账号，避免用户误以为全部成功。

## 测试策略

### Rust 测试

新增或更新测试覆盖：

1. 跨账号 `inbox` 查询能合并不同账号的真实收件箱文件夹，并按 `sent_at DESC` 排序。
2. 跨账号 `sent` 查询能根据不同 provider 文件夹名返回正确邮件。
3. 跨账号 `starred` 查询能跨所有账号和所有文件夹返回星标邮件。
4. `unread_only` 在所有账号查询中生效。
5. 跨账号分页先全局排序再分页。
6. 所有账号统计能累加多个账号的 `inbox`、`sent`、`starred` 未读数。
7. 某账号没有对应分类文件夹时不影响其他账号结果。
8. `sync_all_accounts()` 单账号失败时继续同步其他账号，并返回失败详情。

### 前端单元测试

新增或更新测试覆盖：

1. `AccountState` 能持久化并恢复 `all/account` 选择。
2. 首次启动默认“所有账号”。
3. 上次选择账号不存在时回退到“所有账号”。
4. Sidebar 下拉显示“所有账号”选项，并能切换高亮状态。
5. 所有账号视图下点击文件夹调用所有账号邮件加载。
6. 单账号视图下点击文件夹仍调用单账号邮件加载。
7. EmailList 在所有账号视图显示账号来源标识。
8. ComposeModal 在所有账号视图显示发件账号选择。
9. ComposeModal 回复/转发时默认使用原邮件账号。
10. 所有账号视图下搜索传 `accountId = null`。

### E2E 测试

新增或更新测试覆盖：

1. 两个种子账号下，选择“所有账号”后点击收件箱，能看到两个账号的收件箱邮件。
2. 所有账号列表项显示来源账号。
3. 切回具体账号后，只显示该账号邮件。
4. 所有账号视图下同步按钮触发所有账号同步路径。
5. 所有账号视图下写邮件窗口显示发件账号选择器。

## 验证命令

实现完成后至少运行：

```bash
rtk bun run check
rtk bun run test:frontend
rtk cargo test
rtk cargo fmt --check
```

如果修改了 e2e 行为，还应运行相关 e2e 测试：

```bash
rtk bun run test:e2e
```

## 风险

1. 账号选择从单账号 ID 扩展为账号范围，会影响多个前端调用点，需要逐一处理。
2. 后端跨账号分类查询需要正确复用每个账号的 `FolderRegistry`，否则不同 provider 的文件夹映射会出错。
3. 所有账号分页必须在数据库统一排序后分页，不能前端合并多个单账号分页结果。
4. 同步所有账号如果继续复用单一 `syncing` 状态，进度展示可能只能表达整体同步，不能精确展示每个账号进度。
5. 写信窗口增加发件账号选择后，需要避免单账号视图出现不必要 UI 变化。
6. 扩展 `EmailDto` 会影响 bindings 生成和前端测试 mock。

## 验收标准

1. 账号下拉框包含“所有账号”选项。
2. 首次启动默认选中“所有账号”。
3. 切换账号选择后刷新页面，能恢复上次选择。
4. 选择“所有账号”后点击收件箱，列表显示所有账号的收件箱邮件。
5. 所有账号邮件列表按时间统一倒序排列。
6. 所有账号列表项显示来源账号。
7. 切换到具体账号后，列表只显示该账号邮件。
8. 所有账号视图下侧边栏未读角标显示所有账号累加结果。
9. 所有账号视图下刷新或同步会同步全部账号，并刷新当前邮件列表和统计。
10. 所有账号视图下写邮件窗口显示发件账号选择器，选择账号后能发送。
11. 回复或转发邮件时，默认发件账号为原邮件所属账号。
12. 单账号视图现有邮件浏览、搜索、刷新、写信行为不回退。
