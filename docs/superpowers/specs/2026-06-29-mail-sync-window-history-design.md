# 邮件同步范围与历史回填设计

日期：2026-06-29

## 背景

当前邮件同步有两个用户体验问题：

1. 添加账号并验证成功后，应用会直接进入主界面并后台同步，用户不能选择首次同步最近多久的邮件。
2. 邮件列表滚动到底部后，没有入口继续同步更早的远端邮件。

代码现状：

1. `AddAccountModal.svelte` 在账号创建或 OAuth2 完成后，会调用 `continueAfterAccountAdded` 进入主界面并后台触发 `syncStore.syncAccount(accountId)`。
2. `sync_folder_full` 当前硬编码使用 `three_months_ago_imap_format()`，首次“全量同步”实际只同步近三个月邮件。
3. `sync_state` 只有 `last_sync_uid` 等增量同步字段，能表示“新邮件同步到哪个 UID”，不能表示“历史邮件已经回填到多早”。
4. `EmailState` 已有 `page`、`limit`、`total`，后端也支持分页查询，但 `EmailList.svelte` 当前只展示第一页，没有“加载更多”。
5. IMAP 层已有按日期起点查询的 `list_uids_since`，但没有按日期区间查询旧邮件窗口的能力。

## 目标

实现后应满足：

1. 添加账号并验证成功后，弹出同步范围选择，让用户选择首次同步范围。
2. 首次同步范围固定为：最近 1 周、最近 1 个月、最近 3 个月、最近 1 年、全部邮件。
3. 默认选中最近 3 个月，保持当前行为。
4. 邮件列表底部支持先加载本地下一页；本地已加载完时，显示“同步更久邮件”。
5. 每次点击“同步更久邮件”，只针对当前列表对应的文件夹或分类，向前回填 3 个月邮件。
6. 搜索结果和星标邮件这类聚合列表不显示“同步更久邮件”。
7. 本地邮件不足一页时，只要后端未确认历史耗尽，仍显示“同步更久邮件”。
8. 历史回填状态必须持久化，避免重复扫描和状态混乱。
9. 兼容已有数据库，不要求旧用户重新创建数据库或重新全量同步。

## 非目标

本次不做以下事情：

1. 不改变账号创建前 IMAP 验证逻辑。
2. 不实现自动无限滚动远端同步。远端历史回填必须由用户明确点击触发。
3. 不在搜索结果和星标邮件中做跨文件夹历史回填。
4. 不引入完整数据库迁移框架，只做当前字段所需的幂等轻量迁移。
5. 不重新设计同步任务队列或后台调度系统。

## 推荐方案

采用“显式同步窗口 + 持久化历史边界”的方案。

核心思路：

1. 普通同步继续使用 `last_sync_uid` 追踪最新邮件增量。
2. 历史回填新增独立边界字段，追踪每个账号、每个文件夹已经向历史同步到哪一天。
3. 首次同步由用户选择同步窗口，后端按窗口执行初始同步并写入历史边界。
4. “同步更久邮件”按历史边界每次向前回填 3 个月。
5. 前端显示逻辑只依赖后端历史状态，不用 `emails.length` 推断是否有更早邮件。

## 方案比较

### 方案 A：只把硬编码三个月改为用户可选范围

优点：

1. 改动最小。
2. 能解决添加账号时选择同步多久的问题。

缺点：

1. 不能稳定支持“同步更久邮件”。
2. 没有历史边界，后续点击容易重复扫描或只能粗糙推断。

结论：不采用。

### 方案 B：用本地最早邮件时间推断历史边界

优点：

1. 不需要新增数据库字段。
2. 初期实现较快。

缺点：

1. 邮件日期可能乱序。
2. 用户本地删除邮件后，最早邮件不再等于已同步边界。
3. 同步失败和跨文件夹映射会让边界推断不可靠。

结论：不作为主方案，只用于旧账号首次回填时的一次性兼容推断。

### 方案 C：新增明确的历史同步边界字段

优点：

1. 行为可解释、可断点、可测试。
2. `last_sync_uid` 管新邮件，历史边界管旧邮件，职责清楚。
3. 本地不足一页、空文件夹、旧账号等场景都能用统一状态表达。

缺点：

1. 需要修改数据库 schema、后端同步接口、前端 store 和列表 UI。

结论：采用该方案。

## 数据库设计

在 `sync_state` 表新增字段：

```sql
ALTER TABLE sync_state ADD COLUMN history_synced_since INTEGER;
ALTER TABLE sync_state ADD COLUMN history_exhausted INTEGER DEFAULT 0;
```

字段语义：

1. `history_synced_since`：当前账号、当前文件夹已经向历史同步到的最早 Unix 时间戳。
2. `history_exhausted`：是否已确认服务端没有更早邮件。`0` 表示还可能有更早邮件，`1` 表示历史已耗尽。

建议补充唯一索引：

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid
ON emails(account_id, folder, uid);
```

原因：

1. 历史回填可能扫描到已有邮件。
2. 当前 `message_id UNIQUE` 不足以稳定表达同一账号、同一文件夹、同一 IMAP UID 的幂等性。
3. 保存邮件头和正文时应支持 upsert 或忽略重复记录。

## 现有数据库兼容

不能只修改 `schema.sql`，因为 `CREATE TABLE IF NOT EXISTS` 不会给已有表新增列。

启动数据库时保持执行 `schema.sql`，同时在 `DbConn::initialize_schema()` 后增加幂等迁移：

1. 查询 `PRAGMA table_info(sync_state)`。
2. 如果缺少 `history_synced_since`，执行 `ALTER TABLE sync_state ADD COLUMN history_synced_since INTEGER`。
3. 如果缺少 `history_exhausted`，执行 `ALTER TABLE sync_state ADD COLUMN history_exhausted INTEGER DEFAULT 0`。
4. 如果缺少 `idx_emails_account_folder_uid`，先检查并处理旧数据中的重复 `(account_id, folder, uid)`，再创建唯一索引。

唯一索引迁移前的重复数据处理：

1. 查询重复键：

```sql
SELECT account_id, folder, uid, COUNT(*)
FROM emails
GROUP BY account_id, folder, uid
HAVING COUNT(*) > 1;
```

2. 对重复记录保留 `updated_at` 最新、`body_text/body_html` 信息最完整的一条。
3. 删除其余重复记录前，需要依赖外键级联清理对应附件记录。
4. 重复清理完成后再创建唯一索引。
5. 如果清理失败，迁移应返回错误并阻止继续启动，避免后续同步在不一致数据上运行。

旧数据默认语义：

1. `history_synced_since = NULL` 表示历史边界未知。
2. `history_exhausted = 0` 表示不能认为没有更早邮件。

旧账号第一次点击“同步更久邮件”时：

1. 如果当前文件夹有本地邮件，用该文件夹最早 `sent_at` 初始化历史边界。
2. 如果当前文件夹没有本地邮件，用默认范围“现在减 3 个月”初始化历史边界。
3. 初始化后按“边界再往前 3 个月”执行回填。
4. 回填结束后写入明确的 `history_synced_since`，后续不再依赖本地最早邮件推断。

## 后端设计

### 同步范围类型

新增同步范围类型，供前后端绑定生成使用：

```rust
pub enum InitialSyncRange {
    Week,
    Month,
    ThreeMonths,
    Year,
    All,
}
```

后端转换规则：

1. `Week`：现在减 7 天。
2. `Month`：现在减 1 个月。
3. `ThreeMonths`：现在减 3 个月。
4. `Year`：现在减 1 年。
5. `All`：不设置起始时间，尽量同步全部邮件。

### 初始同步流程

`sync_account` 支持可选初始范围参数，或新增专门命令 `sync_account_with_range(account_id, range)`。

推荐保留普通 `sync_account(account_id)` 的现有语义，新增带范围命令，降低对现有调用点的影响。

流程：

1. 前端账号创建成功后，展示同步范围选择。
2. 用户选择后调用带范围同步命令。
3. `SyncOrchestrator` 将范围传给首次全量同步。
4. `sync_folder_full` 不再硬编码 `three_months_ago_imap_format()`，而是接收 `SyncWindow`。
5. 同步完成后为涉及文件夹写入 `history_synced_since`。
6. 如果范围为 `All`，同步完成后写入 `history_exhausted = 1`。

普通增量同步仍使用 `last_sync_uid`，不受初始范围影响。

### 历史回填命令

新增命令：

```rust
sync_older_emails(account_id, category) -> OlderSyncResult
```

`OlderSyncResult` 建议字段：

```rust
pub struct OlderSyncResult {
    pub new_emails: u64,
    pub updated_emails: u64,
    pub window_start: i64,
    pub window_end: i64,
    pub history_exhausted: bool,
    pub folders: Vec<String>,
}
```

流程：

1. 校验 `category` 是否支持历史回填。`starred` 和搜索不支持。
2. 根据账号 provider 的 folder mapping，将 category 解析为一个或多个 IMAP 文件夹。
3. 对每个文件夹读取 `sync_state`。
4. 计算回填窗口：
   - `end = history_synced_since`
   - `start = end - 3 个月`
   - 如果 `history_synced_since IS NULL`，按旧账号兼容策略初始化。
5. 使用 IMAP 日期区间搜索该窗口内的 UID。
6. 拉取邮件头和正文，保存到本地数据库。
7. 成功后更新 `history_synced_since = start`。
8. 如果确认服务端无更早邮件，更新 `history_exhausted = 1`。

### IMAP 查询能力

新增 IMAP 方法：

```rust
list_uids_between(folder, start_date, end_date)
```

内部使用 IMAP search：

```text
SINCE 01-Jan-2026 BEFORE 01-Apr-2026
```

对于“全部邮件”的初始同步，可使用 `ALL` 或 `UID 1:*`。由于大邮箱成本高，前端必须提示可能耗时较长。

### 历史耗尽判断

当某次回填窗口没有邮件时，不应立即认为服务端没有更早邮件。它可能只是这个三个月窗口没有邮件。

推荐判断方式：

1. 如果服务端提供的最小 UID 或 `UID 1:*` 探测能确认不存在更早 UID，才写 `history_exhausted = 1`。
2. 如果不能确认，只推进 `history_synced_since`，允许用户继续点击。
3. 如果初始范围为 `All` 且同步成功，直接写 `history_exhausted = 1`。

## 前端设计

### 添加账号后的同步范围步骤

`AddAccountModal.svelte` 增加步骤，例如 `sync-scope`。

账号创建成功或 OAuth2 完成并定位新账号后：

1. 不直接调用 `enterMainAndStartSync()`。
2. 保存新账号 ID。
3. 进入同步范围选择步骤。

范围选择 UI：

1. 单选项：最近 1 周、最近 1 个月、最近 3 个月、最近 1 年、全部邮件。
2. 默认选中最近 3 个月。
3. “全部邮件”显示简短提示：可能耗时较长。
4. 主按钮：开始同步。

点击开始同步：

1. 关闭添加账号流程或进入主界面。
2. 调用带范围同步命令。
3. 通过现有 `syncStore` 展示进度和 toast。
4. 同步失败不删除账号。

### 邮件列表底部逻辑

`EmailList.svelte` 底部显示分两层：

1. 本地分页。
2. 远端历史回填。

本地分页：

1. 当 `emailState.emails.length < emailState.total` 时，底部显示“加载更多”。
2. 点击后加载下一页本地邮件并 append 到当前列表。
3. 本地分页未加载完时，不显示“同步更久邮件”。

历史回填显示条件：

1. 不在搜索模式。
2. 当前分类不是 `starred`。
3. 当前分类能映射到具体远端文件夹。
4. 本地分页已加载完。
5. 后端历史状态 `history_exhausted = false`。
6. 当前没有正在回填这个账号、这个分类。

重要规则：

1. 不用 `emails.length < limit` 判断是否隐藏“同步更久邮件”。
2. 本地只有几封邮件甚至没有邮件时，只要 `history_exhausted = false`，仍可显示“同步更久邮件”。

底部状态：

1. `加载更多`：本地还有下一页。
2. `同步更久邮件`：本地已加载完，服务端可能有更早邮件。
3. `正在同步更久...`：回填进行中，按钮禁用。
4. 不显示：搜索、星标、历史耗尽、无账号或分类不支持。

### Store 设计

在 `syncStore` 中增加：

1. `loadHistoryState(accountId, category)`
2. `syncOlderEmails(accountId, category)`
3. 当前回填中的账号和分类状态。

`EmailList.svelte` 调用 `syncOlderEmails` 成功后刷新当前分类邮件列表。第一版可以回到第一页；后续再优化为保留当前已加载页并 append。

## 错误处理

后端：

1. 初始同步失败不回滚账号创建。
2. 历史回填失败时，不推进 `history_synced_since`，不设置 `history_exhausted`。
3. 某个文件夹回填失败时，成功文件夹可提交状态，失败文件夹保持原边界。
4. IMAP 日期窗口为空不算错误，只推进边界，除非能确认历史耗尽。
5. `UIDVALIDITY` 变化时，沿用现有全量同步逻辑。若全量同步清空并重建文件夹数据，需要重新写入本次同步窗口边界，避免旧边界和新 UID 空间混淆。
6. 后端应避免同一账号同一文件夹并发回填造成重复状态更新。第一版可通过前端禁用按钮加数据库幂等保存降低风险。

前端：

1. 初始同步失败显示 toast，账号保留。
2. 历史回填失败显示 toast，底部按钮恢复可点击。
3. 回填中禁用按钮，避免重复点击。
4. 搜索和星标列表不展示历史回填入口。

## 测试策略

### Rust 后端测试

新增或扩展数据库、同步相关测试：

1. `database_should_migrate_existing_sync_state_table`
   - 构造旧版 `sync_state` 表。
   - 启动数据库初始化。
   - 断言补齐 `history_synced_since` 和 `history_exhausted`。
2. `initial_sync_should_store_selected_history_boundary`
   - 选择最近 1 周、3 个月、1 年。
   - 断言 `sync_state.history_synced_since` 正确。
3. `sync_older_should_use_existing_history_boundary`
   - 已有边界时，断言回填窗口为 `[边界-3个月, 边界)`。
   - 成功后边界更新为窗口起点。
4. `sync_older_should_initialize_boundary_for_legacy_account`
   - 旧账号字段为 `NULL`。
   - 有本地邮件时，用最早 `sent_at` 初始化。
   - 无本地邮件时，用默认三个月初始化。
5. `sync_older_should_not_advance_boundary_on_error`
   - IMAP 查询失败时边界不变。
6. `sync_older_should_be_idempotent_for_existing_uid`
   - 重复回填同一窗口不插入重复邮件。
7. `starred_or_search_should_not_support_history_sync`
   - 聚合列表不允许触发回填。

### 前端测试

新增或扩展组件和 store 测试：

1. 添加账号成功后显示同步范围步骤。
2. 默认选中最近 3 个月。
3. OAuth2 完成后也显示同步范围步骤。
4. 点击开始同步时传递正确 `initialRange`。
5. `EmailList` 本地未加载完时显示“加载更多”，不显示“同步更久邮件”。
6. 本地不足一页但 `history_exhausted = false` 时显示“同步更久邮件”。
7. 搜索模式不显示“同步更久邮件”。
8. 星标分类不显示“同步更久邮件”。
9. 回填中按钮禁用，失败后恢复可点击。

## 实施顺序建议

1. 数据库 schema 与轻量迁移。
2. 邮件保存幂等性和唯一索引。
3. 同步范围类型与带范围初始同步。
4. 历史回填后端命令和 IMAP 日期区间查询。
5. 前端账号添加后的同步范围步骤。
6. `syncStore` 历史状态与回填动作。
7. `EmailList` 本地分页和底部历史回填入口。
8. 后端和前端测试补齐。
