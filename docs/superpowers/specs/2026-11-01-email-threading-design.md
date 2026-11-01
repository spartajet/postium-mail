# 邮件会话线程化（Email Threading）设计

- 日期：2026-11-01
- 分支：`email-threading`（实现 kickoff 时创建）
- 状态：设计定稿（brainstorming 四项决策 + 八节设计均经用户确认）

## 0. 背景与目标

邮件客户端当前按平铺列表组织邮件：同一主题的往来回复散落在收件箱/已发送
等处，用户需人工拼对话脉络。本设计实现 **Gmail 式会话线程化**：

- 列表一行 = 一个会话（主题 + 参与者 + 最后动态 + 未读/封数）；
- 详情 = 会话时间线（所有邮件按时间竖排、最新展开）；
- 回复自动进会话（写侧补 In-Reply-To/References）。

### 已确认的四项决策

| 决策点 | 结论 |
|---|---|
| 归组范围 | 全库全局：跨文件夹、跨账号（同一线程在 A/B 账号与收件箱/已发送均聚为一条会话） |
| 归组算法 | 混合：头链（In-Reply-To/References → Message-ID 链）优先，主题规范化 + 参与者交集兜底 |
| UI 形态 | 全会话式：会话列表行 + 详情堆叠时间线 |
| 存储策略 | 物化 `thread_id`（threads 表 + emails 加列 + 存量回填迁移），同步落库事务内归属 |
| 视图切换 | 默认会话式，列表头部可切回平铺（`ui-state.json` 持久化），两态并存 |

### 非目标（v1 明确不做）

- 会话内搜索/跳转、会话归档整理工具、摘要生成；
- 按文件夹粒度的差异化会话策略；
- 主题兜底的机器学习去重；
- IMAP `THREAD` 扩展（服务端线程）——本地归组自足。

## 1. 数据模型与迁移

### 1.1 新表 `threads`

| 列 | 类型 | 说明 |
|---|---|---|
| id | INTEGER PK AUTOINCREMENT | |
| subject_key | TEXT NOT NULL | 规范化主题键（见 §2.1）；空串表示"空主题"（恒不参与主题匹配） |
| participants | TEXT NOT NULL DEFAULT '' | 小写邮箱去重集合，逗号 join；新邮件并入线程时追加合并；主题兜底交集判定的对侧 |
| created_at / updated_at | INTEGER NOT NULL | updated_at = 线程内最新邮件 received_at，会话列表排序键 |

索引：`threads(subject_key)`。

### 1.2 `emails` 加列

| 列 | 类型 | 说明 |
|---|---|---|
| thread_id | INTEGER REFERENCES threads(id) ON DELETE SET NULL | 归属（索引） |
| in_reply_to | TEXT | In-Reply-To 与 References 头的 Message-ID 并集：小写、逗号 join、截断至 2048 字符；链判定输入与排障依据 |

### 1.3 迁移与回填

- 建表/加列/建索引后执行**一次性回填**：按 `received_at ASC` 遍历全部存量
  邮件（`is_draft=1` 跳过，与同步路径口径一致），逐封跑 §2 归组算法
  （存量无头信息，实际走主题 + 参与者路径）。
- 回填在迁移事务内同步完成（现实量级数千封，秒级内）；进度与结果计数写
  INFO 日志。
- 空主题邮件：subject_key 记空串，仅可通过头链归组（存量即各自成线）。

### 1.4 主题规范化键（subject_key）

1. 叠加剥前缀：`Re:`、`Fwd:`、`FW:`、`回复：`、`答复：`、`转发：`（大小写
   不敏感；冒号全/半角均认；循环剥至无前缀）；
2. 合并连续空白为单空格、trim；
3. ASCII 段小写化（CJK 不受影响）。

## 2. 归组引擎（`postium-core/src/domain/threading/`）

纯函数模块，输入输出均为简单类型；数据库访问经查询闭包注入，可全量单测。

### 2.1 输入

- `subject: &str` → 规范化键；
- `chain_ids: Vec<String>`——邮件头解析出的 Message-ID 集合（In-Reply-To
  全部 + References 全部；尖括号提取、小写、去重）；
- `participants: BTreeSet<String>`——本封发件人 + 收件人（含抄送）邮箱
  小写集合。

### 2.2 决策（顺序短路）

1. **头链**：任一 `chain_id` 命中存量 `emails.message_id`（SQL：`SELECT
   thread_id FROM emails WHERE message_id IN (…) AND thread_id IS NOT NULL
   LIMIT 1`）→ `Existing(thread_id)`；
2. **主题兜底**：`threads.subject_key = 键 AND 键 ≠ ''` 的候选线程中存在
   `participants ∩ thread.participants ≠ ∅` 者 → `Existing(该 thread_id)`
   （多候选取 `updated_at` 最新）；
3. **新建**：`New`（调用方建 threads 行：subject_key + participants +
   created_at/updated_at）。

### 2.3 单测重点（表驱动）

中文/全角前缀、嵌套 `Re: Re:`、`Fw:Fwd:` 混叠、空主题、纯空白主题、
同主题无参与者交集（不聚）、链优先于主题（同封两路冲突时）、链多命中
（任一即可）、References 含自引用。

## 3. 同步与查询接线

### 3.1 同步落库归属

- 同步管线解析邮件（mail-parser 已暴露 `in_reply_to` / `references`）后、
  INSERT 事务内：构造 §2 输入 → 决策 → 新建线程或并线程 → 写
  `emails.thread_id`；并线程时把本封 participants 合并进
  `threads.participants` 并刷新 `updated_at`。
- 已发送邮件（SMTP / 已发送文件夹同步）同路径归属——回复经头链进会话，
  全局归组的自然结果。
- 草稿：不参与归组（`is_draft` 邮件跳过 assign；发送落已发送时再归组）。

### 3.2 `infrastructure/storage/thread_repo.rs`

- `assign(conn, email_row) -> thread_id`：§2 决策的落库封装（含建线程）；
- `backfill(conn)`：§1.3 回填；
- `list_conversations(conn, filter, offset, limit) -> Vec<ConversationSummary>`：
  按现有 list* 的过滤形态（单账号文件夹 / 虚拟分类 / 全账号），`GROUP BY
  thread_id` 聚合：
  - last_email 的 subject / sender_name / sender_email / preview / received_at；
  - `COUNT(*)` 总封数、`SUM(is_read=0)` 未读数、`MAX(is_starred)` 任一星标；
  - 排序 `threads.updated_at DESC`，LIMIT/OFFSET 分页；
- `conversation_emails(conn, thread_id, filter?) -> Vec<emails 行>`：
  received_at ASC（时间线装载；沿用列表同等可见性过滤，如 is_deleted）。

`ConversationSummary`（core 出参，UI 消费）：

```
{ thread_id, last: {subject, sender_name, sender_email, preview, received_at},
  total, unread, starred_any }
```

### 3.3 `email_service` 会话 API

- `list_conversations(folder/account)`、`list_conversations_by_category(...)`、
  `list_conversations_by_category_for_all_accounts(...)`——与现有三个 list*
  一一对应的会话变体；
- `conversation_emails(thread_id)`；
- `mark_conversation_read(thread_id)`：线程内全部 `is_read=1`（打开会话即
  全读，Gmail 语义；时间线内仍可看到历史，未读点清零）；
- 星标：会话行星标 = 作用于线程内**最新一封**（`toggle_conversation_star`，
  Gmail 同语义）；时间线内星标作用于单封（现有能力）。

### 3.4 删除与移动语义（v1）

- 会话行删除 = 删除线程内全部邮件（复用现有逐封删除流；确认交互沿用
  现有删除确认）；
- 单封移动/删除不拆线程（线程是逻辑聚合，邮件物理位置不动语义）；
- 孤儿线程（邮件全删/移出后 count=0）：v1 容忍不清理，回填与维护路径顺带
  GC（`DELETE FROM threads WHERE id NOT IN (SELECT DISTINCT thread_id FROM
  emails WHERE thread_id IS NOT NULL)`）。

## 4. 写侧：回复进线程

- compose 的 `Reply` / `ReplyAll` 构造草稿时：
  - `In-Reply-To: <原邮件 Message-ID>`（原文有 Message-ID 才带）；
  - `References: <原邮件 in_reply_to 链> <原邮件 Message-ID>`（去重、
    截断至 2048）；
  - 主题：原主题无 `Re:` 系前缀时补 `Re: `；
- `build_draft_email`（mail_draft）增加上述头输出（lettre builder）；
- 原邮件 Message-ID / in_reply_to 经 EmailDto 透出到 compose 入参。

## 5. UI：会话列表

- `EmailsStore` 增加视图档位 `view_mode: Threaded | Flat`（默认 Threaded）；
  `ui-state.json` 新键 `email_view_mode: "threaded" | "flat"`，切换即重载列表。
- 会话行渲染（`email_list.rs`）：
  `主题 ｜ 参与者摘要（去重对方邮箱/姓名，排除本账号自己的地址，超过 3 个示 +N）｜ 最后发件人 +
  预览 ｜ 最后时间 ｜ 未读点（unread>0）｜ 封数角标（total>1）｜ 星标指示
  （starred_any）`；
- 选中模型：会话行选中 → `selected_thread: Option<i64>`，详情装载时间线；
  平铺档维持现有 `selected: email_id` 路径，两套互斥；
- 未读语义：进入会话详情即调 `mark_conversation_read`（列表未读点随通知
  刷新）；
- 搜索/虚拟分类过滤在会话查询上等价生效（filter 下推进 SQL）。

## 6. UI：详情时间线

- `EmailDetailView` 增加会话形态：装载 `conversation_emails(thread_id)`
  （时间升序 Vec<EmailDto>）；
- 时间线容器：每封一张卡竖排；**折叠卡** = 发件人 + 日期 + 一行预览；
  **展开卡** = 现有单封详情完整渲染（头/操作行/正文/附件，HTML 渲染管线
  原样复用）；默认展开最后一封（最新）；
- 展开态：单展开（点折叠卡切换展开位，原展开卡折叠）——v1 简化，避免
  多封正文同时常驻的内存/渲染压力；
- 操作（回复/全部回复/转发/星标/删除/附件/浏览器查看）作用于**当前展开
  封**——现有 handler 原样复用；
- 平铺档或非会话入口：现有单封详情形态不变；
- 划选/图片放行/缓存等正文能力随单封渲染自然继承（DetailStore 缓存键
  含 email_id + 内容 hash，多封共存无冲突）。

## 7. 设置与切换

- 列表头部图标切换（会话/平铺二态），写 `ui-state.json`；
- 切换不重启：store 重载对应列表模型；详情随之清空回占位。

## 8. 边界与错误处理

| 场景 | 行为 |
|---|---|
| 空 Message-ID 邮件（部分客户端不发） | 链路对其不可达；仅主题兜底可聚 |
| 空主题 | subject_key 空串恒不主题匹配 |
| 主题兜底误聚 | 可修数据（物化优势）：改 emails.thread_id 即可，UI 无需变 |
| 同步中断在 assign 后 | 归属随邮件行同事务，无中间态 |
| 回填遇无 message_id 存量 | 走主题路径；无主题自成一线 |
| 大会话（100+ 封） | 时间线折叠卡轻量；展开单封；列表聚合查询有索引 |
| 线程 participants 膨胀 | 追加去重，集合上限 64 邮箱（达上限后不再追加；交集判定仍可用既有集合） |

## 9. 测试策略

1. **引擎单测**（表驱动）：规范化（中/英/全半角/嵌套前缀/空白）、链解析
   （尖括号/多 ID/自引用）、三分支决策与短路序、参与者交集边界；
2. **thread_repo 集成测**（临时 sqlite）：assign 三分支落库、backfill 存量
   归组、list_conversations 聚合（未读/星标/封数/排序/分页）、
   conversation_emails 时间序；
3. **同步路径测**：fake 邮件带 In-Reply-To → 正确并线；无头邮件 → 主题
   兜底并线；全新 → 建线；
4. **写侧测**：Reply 草稿带 In-Reply-To/References/Re: 主题；
5. **迁移测**：旧 schema 库升级 + 回填结果断言；
6. **e2e**：两封同主题（第二封 Re:）fixture——会话聚合成一行、未读/封数
   角标、进详情时间线两卡且最新展开、点折叠卡切换、打开后未读清零、切
   平铺回到两行、切回会话态记忆保持。

## 10. 交付形态

- 实现 branch：`email-threading`（自 `html-renderer` 或 main 最新切出，
  kickoff 时定）；
- 沿用 SDD 流程：本 spec → writing-plans 实施计划 → 任务化执行；
- 变更面：postium-core（domain/infrastructure/service）、postium-ui
  （stores/emails、email_list、email_detail、ui_state）、迁移与回填。
