# 文件夹识别引擎设计 (Folder Detection Engine)

**日期**: 2026-06-30
**状态**: 设计中
**分支**: feature/mail_operate

## 1. 背景与问题

### 1.1 现状

当前系统识别"标准邮件文件夹"（收件箱/已发送/草稿/垃圾/已删除/归档）的方式非常原始，
存在三套互相不一致的解析逻辑，全部建立在"每个 provider 手写一个候选名 `Vec<String>`"之上：

| 逻辑 | 位置 | 行为 |
|------|------|------|
| `resolve_folders` | `email_service.rs:112` | 返回**所有**候选别名 |
| `resolve_sync_folders` | `folder_sync_dispatcher.rs:269` | 只挑**一个**（第一个远程存在的） |
| `resolve_history_candidate_folders` | `sync_service.rs:508` | 返回**所有**真实存在的 |

另外 `find_standard_type`（反向映射，`providers/mod.rs:350`）做的是**双向子串包含**匹配。

### 1.2 三个核心痛点

1. **三套解析逻辑不一致** — 同一个"已发送"分类，初始同步只挑一个文件夹、历史回填又要全部文件夹、
   移动操作直接读 `mapping.trash`。行为互相打架，前面已经因此出过 bug（发件箱无法加载历史）。

2. **子串匹配会误判** — `find_standard_type` 用 `imap.contains(name) || name.contains(imap)` 双向包含，
   一个叫 `"Sent Archive"` 的自定义文件夹会被误判成 `sent`。

3. **provider 手写候选名脆弱** — 候选名是唯一依据，写错即废。网易全靠 IMAP-UTF-7 编码字符串
   （如 `&XfJT0ZAB-` 表示"已发送"），Gmail 个人版与企业版映射不一致。

### 1.3 关于 SPECIAL-USE 的调研结论

我们调研了 RFC 6154（IMAP LIST SPECIAL-USE）在主流服务商的支持情况：

| 服务商 | SPECIAL-USE 支持 | 备注 |
|--------|-----------------|------|
| Gmail / Google Workspace | ✅ 完善 | 返回 `[Gmail]/Sent Mail` 带 `\Sent` 等 |
| Outlook / Microsoft 365 | ✅ 完善 | 微软服务器规范宣告 |
| iCloud / Fastmail | ✅ 完善 | 标准实现 |
| **QQ邮箱 / 腾讯企业邮** | ❌ **不宣告** | 返回文件夹名但不带 SPECIAL-USE 属性 |
| **网易 163/126/Yeah** | ❌ **不可靠** | 系统文件夹是中文名，SPECIAL-USE 支持有限 |
| **139邮箱 / 各类国内服务商** | （不宣告） | 无公开文档证实支持 |

**关键结论**：本项目核心用户是中文用户，国内主流服务商基本不支持 SPECIAL-USE。
若把 SPECIAL-USE 作为唯一或第一优先手段，国内用户几乎永远命中不了，直接退化成候选名匹配。
SPECIAL-USE 只对国际服务商（候选名本来就写得准）有效，治不了国内的病。

## 2. 业界方案调研：FairEmail 三层识别

### 2.1 FairEmail 的做法

[FairEmail](https://github.com/M66B/FairEmail) 是开源邮件客户端里文件夹识别最严谨的实现之一。
它的 `EntityFolder.java` 采用**三层递进**识别策略：

```
第一层 getType()   — 协议层：读 SPECIAL-USE 属性（\Sent \Drafts \Junk \Trash \Archive）
                     和 \NoSelect/\NonExistent 过滤；INBOX 名字直接判定
第二层 guessTypes() — 名称猜测：一张跨语言关键词大表，folder.name.toLowerCase().contains(guess)
                     每个关键词带分数(score)，同分按文件夹层级深度排序，每类选一个最高分
第三层 USER         — 归为普通用户文件夹
```

### 2.2 关键洞察

1. **SPECIAL-USE 只是锦上添花** — FairEmail 先试 SPECIAL-USE，不行才用名称猜测。
   名称猜测才是覆盖所有服务商的主力。这印证了国内 SPECIAL-USE 命中不了但名称猜测照样能工作。

2. **单向子串包含比双向安全** — FairEmail 只做 `folder.name.contains(guess)`（单向），
   `guess` 是完整词（如 `Gesendet`），误判面远小于我们的双向 `a.contains(b) || b.contains(a)`。

3. **FairEmail 没有中文关键词** — 它是国际项目，`GUESS_FOLDER_TYPE` 表只含英德法俄意波兰等语言。
   这恰恰是我们的机会：核心用户是中文用户，补充中文关键词即可覆盖国内所有服务商。

### 2.3 FairEmail 名称猜测表（发件箱示例，摘录）

```java
put("sent",     new TypeScore(SENT, 100));      // 英文
put("gesendet", new TypeScore(SENT, 100));      // 德语
put("envoyés",  new TypeScore(SENT, 100));      // 法语
put("отправленные", new TypeScore(SENT, 100));  // 俄语
```

## 3. 设计目标

1. **统一解析逻辑** — 用一个引擎收敛现有三套 `resolve_*` 逻辑和 `find_standard_type`。
2. **消除子串误判** — 反向分类只做精确匹配 + 单向关键词包含，删除双向子串逻辑。
3. **跨语言名称识别** — 建立含**中文**关键词的猜测表，覆盖国内服务商（QQ/网易/139 等）。
4. **向后兼容** — 现有 provider `folder_mapping()` 候选名配置全部保留，作为兜底层。
5. **单连接内构建** — 引擎在每次 IMAP 连接后用一次 LIST 构建，账号级、连接内有效，不持久化。

## 4. 架构设计

### 4.1 核心类型

新模块 `src-tauri/src/domain/folders/`：

```
domain/folders/
  ├─ mod.rs              模块导出
  ├─ category.rs         FolderCategory 枚举（替代散落的字符串 "trash"/"archive"）
  ├─ keyword_table.rs    跨语言关键词猜测表（含中文）
  ├─ special_use.rs      SPECIAL-USE 属性解析（从 NameAttribute 提取）
  └─ registry.rs         FolderRegistry 引擎主体
```

#### 4.1.1 `FolderCategory` 枚举

统一现有散落的字符串字面量（`"trash"`、`"archive"`、`EmailCategory::Sent` 等）：

```rust
pub enum FolderCategory {
    Inbox,
    Sent,
    Drafts,
    Junk,    // 垃圾邮件（spam）
    Trash,   // 已删除
    Archive,
    // Starred 不在此列，它是跨文件夹查询，不映射单一文件夹
}
```

#### 4.1.2 `FolderRegistry` 引擎

```rust
/// 账号文件夹识别引擎。连接 IMAP 后用一次 LIST 构建，连接内有效。
///
/// 构建来源（三层信息）：
/// 1. 远程文件夹列表 + SPECIAL-USE 属性（从 list_folders() 带 attributes 得到）
/// 2. provider 候选名（provider.folder_mapping()）
/// 3. 跨语言关键词猜测表
pub struct FolderRegistry {
    /// 反向索引：真实文件夹名 -> 推断出的类别（构建时一次性算好）
    classified: HashMap<String, FolderCategory>,
    /// 正向索引：类别 -> 所有归属该类别的真实文件夹名
    by_category: HashMap<FolderCategory, Vec<String>>,
    /// 全部远程文件夹名（去重，用于存在性判断）
    remote_folders: HashSet<String>,
}
```

### 4.2 三层解析算法

构建 Registry 时，对每个远程文件夹依次尝试三层识别，**命中即停止**。

**重要：解码时机** — `list_folders()` 返回的 `name` 是原始 IMAP-UTF-7 编码
（如 `&XfJT0ZAB-`）。Registry 内部维护两个名字：**编码名**（原始，作为 IMAP 操作的 key，
用于后续 select/fetch）和**解码名**（`decode_utf7_imap` 后的可读名，用于关键词匹配）。
`classified` / `by_category` 的 key 统一用编码名，关键词匹配用解码名。

```
对每个远程文件夹 folder（带 attributes，name 为编码名）:

  前置过滤：
    遍历 attributes，含 \NoSelect / \Nonexistent → 直接跳过，不参与任何识别

  第一层 — INBOX 名字保留判定（先于 SPECIAL-USE，因为 INBOX 是 RFC 3501 保留名）
    folder.name 不区分大小写等于 "INBOX" → Inbox，记录，结束

  第二层 — SPECIAL-USE 属性（ getType ）
    遍历 attributes，匹配 \\Sent \\Drafts \\Junk \\Trash \\Archive \\Flagged
    命中 → 记录 编码名 → category，结束

  第三层 — 跨语言关键词猜测（ guessType ）
    name_lower = decode_utf7_imap(folder.name).to_lowercase()
    遍历关键词表，找 name_lower.contains(keyword) 的项
    多个命中取分数最高的关键词对应的类别（同分按类别优先级 Inbox>Sent>Drafts>Junk>Trash>Archive）
    命中 → 记录 编码名 → category，结束

  未命中任何层 → 该文件夹不进入 classified / by_category（归为普通文件夹）
```

#### 4.2.1 关键词猜测表（含中文）

`keyword_table.rs` 维护一张 `[(keyword, category, score)]` 表。
中文是重点补充（FairEmail 缺失的部分）。**关键词需先经 IMAP-UTF-7 解码后再匹配**。

| 类别 | 英文 | 中文 | 其他语言（精选） |
|------|------|------|------------------|
| Sent | `sent`(100), `sent items`(90), `sent messages`(90) | `已发送`(100), `已发邮件`(100), `发件箱`(80) | `gesendet`, `envoyés`, `отправленные` |
| Drafts | `drafts`(100), `draft`(90) | `草稿`(100), `草稿箱`(90) | `entwürfe`, `brouillons`, `черновики` |
| Junk | `junk`(100), `spam`(100), `bulk mail`(90) | `垃圾邮件`(100) | `spam`, `courrier indésirable` |
| Trash | `trash`(100), `deleted`(100), `deleted messages`(90) | `已删除`(100), `废件箱`(80) | `papierkorb`, `corbeille`, `корзина` |
| Archive | `archive`(100), `archived`(90), `all mail`(80) | `归档`(100), `已归档`(90), `所有邮件`(80) | `archiv`, `archives` |

**分数规则**：
- 完整标准名（如 `已发送`、`sent`）给 100 分
- 变体/别名（如 `发件箱`、`sent items`）给 80-90 分
- 同一文件夹可能命中多个关键词，取**最高分**对应的类别
- 关键词匹配是**单向** `name.contains(keyword)`，杜绝双向误判

#### 4.2.2 分数冲突处理

当一个文件夹名同时含多个类别的关键词（如 `"Junk E-mail"` 含 `junk`），
按分数高的优先；分数相同则按固定类别优先级 `Inbox > Sent > Drafts > Junk > Trash > Archive`。
（FairEmail 也是固定优先级 + 分数）

### 4.3 provider 候选名的角色

现有 `provider.folder_mapping()` 返回的 `StandardFolder`（6 个 `Vec<String>`）**全部保留**，
但角色从"唯一依据"降级为**兜底补充**：

```
Registry 三层识别全部完成后，补一轮 provider 候选名匹配：
  对每个 category：
    如果 by_category[category] 为空（三层都没识别出该类别的任何文件夹）：
      取 provider 候选名 candidates
      在 remote_folders 中找候选名里精确等值(==)存在的
      找到 → 加入 by_category[category] 和 classified
```

这样保证了：即使某服务商的关键词表和 SPECIAL-USE 都没命中，
只要 provider 配了正确的候选名（现有逻辑），识别仍然成立。
注意候选名匹配只在"前三层完全没结果"时才触发，避免和关键词猜测冲突。

### 4.4 查询接口

`FolderRegistry` 提供统一接口，取代现有三套逻辑：

```rust
impl FolderRegistry {
    /// 多选：返回该类别所有归属的真实文件夹（历史回填、初始同步用）
    pub fn resolve(&self, category: FolderCategory) -> Vec<String>

    /// 单选：返回首选那一个（移动到垃圾箱/归档用）
    /// 优先返回 by_category[category] 的第一个，没有则回退候选名第一个
    pub fn resolve_one(&self, category: FolderCategory) -> Option<String>

    /// 反向：真实文件夹名 -> 类别（替代 find_standard_type）
    /// 纯查 classified 反向索引，不做任何子串推断
    pub fn classify(&self, folder_name: &str) -> Option<FolderCategory>

    /// 判断文件夹是否真实存在于远程
    pub fn exists(&self, folder_name: &str) -> bool
}
```

- **`resolve`（多选）**：返回该类别所有真实文件夹。例如网易 `sent` 若 `Sent` 和
  `&XfJT0ZAB-`（解码后"已发送"）都被识别，两个都返回。
- **`resolve_one`（单选）**：给 move-to-trash 这类"只需一个目标"的操作用。
- **`classify`（反向）**：精确查表，**彻底删除双向子串逻辑**。

## 5. 迁移方案

### 5.1 现有调用点 → 引擎接口

| 现有逻辑 | 位置 | 改为 |
|---------|------|------|
| `resolve_sync_folders` | `folder_sync_dispatcher.rs:269` | `registry.resolve(cat)`（初始同步用多选，同步该类别所有真实文件夹） |
| `resolve_history_candidate_folders` | `sync_service.rs:508` | `registry.resolve(cat)` |
| `category.resolve_folders(mapping)` | `email_service.rs:112` | `registry.resolve(cat)` |
| `find_standard_type(name)` | `providers/mod.rs:350` | `registry.classify(name)` |
| `resolve_special_folders(acct,"trash")` | `mail_operation.rs:297` | `registry.resolve_one(Trash)` |

### 5.2 关键约束：不连接服务器的场景

`get_history_state`（`sync_service.rs:263`）不连接 IMAP，拿不到远程文件夹。
对此场景，Registry 提供**降级构建**：只用本地已存文件夹名（`sync_repo` + `email_repo` 的
`distinct_folders_by_account`）+ provider 候选名 + 关键词表构建，跳过 SPECIAL-USE 层。
这种降级 Registry 只用于查询本地状态，不用于实际同步。

### 5.3 `list_folders()` 的增强

当前 `list_folders()` 把 `attributes()` 用 `format!("{f:?}")` 拍成字符串丢弃（`imap/mod.rs:147`）。
需改为：把 `NameAttribute` 解析成结构化的 `SpecialUse` 枚举，供 Registry 第一层使用。
`FolderInfo` 增加 `special_use: Vec<SpecialUseFlag>` 字段。

### 5.4 保留与删除

- **保留**：所有 provider 的 `folder_mapping()` 实现、`StandardFolder` 类型（作为兜底配置源）
- **删除**：`find_standard_type` 的双向子串逻辑（改为 Registry.classify 精确查表）
- **删除**：三套 `resolve_*` 函数（合并为 Registry 接口调用）

## 6. 各服务商验证矩阵

文档需附验证矩阵，列出每个 provider 每个类别预期能被哪一层识别：

| Provider | Inbox | Sent | Drafts | Junk | Trash | Archive |
|----------|-------|------|--------|------|-------|---------|
| QQ | 名字 | 关键词"Sent Messages" | 关键词"Drafts" | 关键词"Junk" | 关键词"Deleted Messages" | 无（provider未配） |
| 网易163 | 名字 | 关键词"已发送"（UTF-7解码后） | 关键词"草稿箱" | 关键词"垃圾邮件" | 关键词"已删除" | 关键词"归档" |
| Gmail | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE("All Mail") |
| Outlook | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | SPECIAL-USE | 无 |
| 默认(英) | 名字 | 关键词 | 关键词 | 关键词 | 关键词 | 关键词 |

这张表既是设计验证，也是未来回归测试的依据。

## 7. 测试策略

1. **关键词表单测** — 给定解码后的文件夹名，断言被识别为正确类别（含中文 case）。
2. **Registry 构建单测** — 模拟远程文件夹列表 + 属性，断言三层识别结果。
3. **误判回归测试** — `"Sent Archive"` 应只识别为一个类别（按分数/优先级），不被双向误判。
4. **多文件夹归属测试** — 网易 `Sent` + `&XfJT0ZAB-` 都应归到 Sent 类别。
5. **provider 兜底测试** — 关键词表和 SPECIAL-USE 都不命中时，候选名能兜住。
6. **降级构建测试** — 不连接服务器时，基于本地文件夹名 + 候选名构建 Registry 仍能工作。

## 8. 范围与非目标

**本次范围**：
- 实现 FolderRegistry 引擎（三层识别 + 四个查询接口）
- 迁移现有五处调用点
- 增强list_folders 暴露 SPECIAL-USE 属性
- 建立含中文的关键词表

**非目标（明确排除）**：
- 不做 SPECIAL-USE 的 `LIST ... RETURN (SPECIAL-USE)` 主动请求（被动读取现有 LIST 返回即可）
- 不持久化 Registry（每次连接重新构建）
- 不做用户手动指定文件夹的 UI（FairEmail 有，本次不做，留给后续）
- 不重构 provider 配置文件格式（候选名 Vec 保留）
- 不引入 Gmail label 体系或多文件夹归属建模（如 Sent 同时进 All Mail 的复杂场景）

## 9. 风险

1. **IMAP-UTF-7 解码时机** — `list_folders()` 返回的 name 是原始 UTF-7 编码（如 `&XfJT0ZAB-`），
   关键词匹配前必须先 `decode_utf7_imap`。需确保解码在 Registry 构建时统一进行，
   且原始编码名作为 IMAP 操作的标识符保留（select/fetch 仍用编码名）。

2. **关键词误伤** — 中文关键词如"垃圾"可能误伤含该字的自定义文件夹。缓解：用完整词
   （"垃圾邮件"而非"垃圾"），且分数制让更具体的词优先。

3. **迁移期间兼容** — 五处调用点迁移需逐一进行，每处迁移后单独验证，避免一次性大改引入回归。
