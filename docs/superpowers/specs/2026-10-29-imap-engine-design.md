# IMAP 引擎设计（feat/imap-engine）

- 日期：2026-10-29
- 状态：已确认（设计三节均经用户逐节确认）
- 分支：`feat/imap-engine`

## 1. 背景与目标

现有 IMAP 收信链路是 async-imap 0.11 的薄封装（3396 行，40+ 平铺方法）加一套基于 UID 范围对比的同步管线（1752 行）。核心痛点：

1. **每次操作新建连接**——发信、草稿、移动、同步各自 `ImapClient::connect`，无连接复用；
2. **收信同步原始**——全量枚举 UID 对比（O(n)），不感知 flag 变更与删除（EXPUNGE），无结构化 ENVELOPE 解析；
3. **新邮件感知靠 5 分钟轮询**——无 IDLE 推送；
4. **文件夹管理靠名字猜**——每 provider 硬编码中英文文件夹名映射，不使用服务器标准标注；
5. **服务器能力从不探测**——`capabilities()` 存在但从不调用，QRESYNC/CONDSTORE/IDLE/SPECIAL-USE 全部未用。

目标：以每账号 Actor 为核心构建完整 IMAP 运行时（同步引擎 + 连接池 + IDLE 推送 + 文件夹管理），一次到位取代现有同步层，收信体验从"分钟级轮询"升级为"秒级推送"。

已确认的设计决策：

| 决策点 | 结论 |
|---|---|
| 范围 | B：完整运行时（同步引擎 + 连接池 + IDLE），一次到位 |
| IDLE 覆盖 | B：收件箱恒定 + 用户浏览过的文件夹（LRU，上限 2 条 IDLE 连接） |
| 与旧代码关系 | A：完全取代，不留双轨 |
| 并发模型 | 方案 1：每账号一个 tokio Actor（命令通道 + 事件出口） |

## 2. 协议依据（检索结论）

- 基线 [RFC 9051 (IMAP4rev2)](https://datatracker.ietf.org/doc/html/rfc9051)：核心已吸收 `ENABLE`、`IDLE`、`LITERAL+`、`UIDPLUS`、`SASL-IR`。
- 增量同步 [RFC 7162](https://www.rfc-editor.org/info/rfc7162)：CONDSTORE（每文件夹 `HIGHESTMODSEQ` + 每消息 mod-sequence）与 QRESYNC（SELECT 参数化快速重同步：一次往返返回 flag 变更 + vanished UID）。
- 实时推送：IDLE（RFC 2177）——单连接单文件夹，约 29 分钟需重发，需低频轮询兜底。
- 文件夹角色 [RFC 6154](https://datatracker.ietf.org/doc/html/rfc6154)：`LIST (SPECIAL-USE)` 服务器直接标注 `\Sent`/`\Drafts`/`\Trash`/`\Junk`/`\Archive`。
- 行业实践参考：[ImapGoose](https://whynothugo.nl/journal/2025/10/15/introducing-imapgoose)、[MailKit 连接管理](https://github.com/jstedfast/MailKit/issues/805)、[Nylas IMAP 集成深潜](https://www.nylas.com/blog/the-intricacies-of-integrating-with-imap)、[IANA 能力注册表](https://www.iana.org/assignments/imap-capabilities)。

## 3. 总体架构

```
UI / 服务层（AppShell / mail_send / mail_operation / 状态栏）
     │ 命令（SyncNow / FetchBody / Append / Move / SetFlags / WatchFolder …）
     ▼
ImapEngineRegistry（全局单例，账号 id → 引擎句柄）
     │
     └── AccountEngine ×N（每账号一个 tokio Actor 任务）
           ├── CommandRouter     mpsc 命令队列，账号内串行化、账号间并行
           ├── ConnectionPool    控制连接 ×1（复用）+ 临时连接（忙时）+ IDLE 连接 ≤3（LRU）
           ├── CapabilitySet     连接建立即 CAPABILITY → ENABLE QRESYNC/CONDSTORE/UTF8=ACCEPT
           ├── SyncEngine        档位决策：QRESYNC → CONDSTORE → UID 增量
           ├── IdleManager       watch set = 收件箱 + 浏览过的文件夹（LRU，上限 2）
           └── FolderRegistry    SPECIAL-USE 角色发现 + 启发式回退 + 用户覆盖

     事件出口：EngineEvent { Progress / NewMail / FlagsChanged / Expunged /
               FolderListChanged / Disconnected / CredentialsNeeded }
     → 桥接现有 SyncStore / EmailsStore / 状态栏（UI 改动面小）
```

### 3.1 AccountEngine（Actor）

- 每账号一个 tokio 任务，`mpsc<EngineCommand>` 接收命令；账号内命令串行化处理，账号之间天然并行。
- 命令集：`SyncNow(folder)` / `FetchBody(uid, section)` / `Append` / `Move` / `Delete` / `SetFlags` / `WatchFolder` / `Shutdown`。
- 事件集：`EngineEvent`——`Progress`（沿用 `SyncProgressEvent` 格式）/ `NewMail` / `FlagsChanged` / `Expunged` / `FolderListChanged` / `Disconnected` / `CredentialsNeeded`。
- `ImapEngineRegistry` 为全局单例（gpui Global），按账号惰性启动引擎；账号删除时发 `Shutdown` 停机。

### 3.2 ConnectionPool

- 控制连接 ×1 常驻复用；忙时按需开临时连接，用完归还或关闭。
- IDLE 连接：watch set（§5，上限 2 个文件夹）每文件夹一条；池容量按 3 预留。
- 总连接数封顶 4/账号（163 每账号限 5 并发连接）。
- 失效检测：命令超时（30s）/ IO 错误 → 连接标记可疑销毁 → 重建流程固定 `AUTH → CAPABILITY → ENABLE → SELECT 恢复`。
- 重连退避：指数 1s→2s→…→30s 封顶；期间发 `Disconnected` 事件（状态栏可见）。

### 3.3 CapabilitySet

- 连接建立即 `CAPABILITY`，缓存进 `CapabilitySet`；支持的扩展发 `ENABLE`。
- 决策同步档位（见 §4）与功能开关（IDLE 可用性、SPECIAL-USE、MOVE 扩展等）。
- 服务器怪癖层：163/Outlook/Gmail 已知行为差异按 provider 归档为策略表（如 163 的 ID 命令要求、BODYSTRUCTURE 非标准语法回退原文解析）。

## 4. 同步引擎（三档策略）

每文件夹持久化同步状态：`{uidvalidity, highestmodseq, last_seen_uid}`——这是增量同步的全部状态。

| 档位 | 条件 | SELECT 后一次往返拿到 | 额外开销 |
|---|---|---|---|
| QRESYNC | 服务器宣告 QRESYNC | flag 变更集 + vanished（已删除）UID 列表 | 零（SELECT 参数化） |
| CONDSTORE | 仅有 CONDSTORE | flag 变更（`FETCH 1:* (FLAGS) (CHANGEDSINCE modseq)`） | 删除靠 UID 集合对比 |
| UID 增量 | 都没有（163 部分文件夹、老服务器） | 新 UID（`last_seen_uid` 之后） | 删除靠本地/服务器 UID 集合对比（现有逻辑引擎化） |

统一规则：

- `UIDVALIDITY` 变化 → 该文件夹本地邮件清空 + 全量重同步（服务器语义即"UID 全部作废"）。
- 收信抓取升级：新邮件 FETCH `ENVELOPE + BODYSTRUCTURE + FLAGS + INTERNALDATE + BODY.PEEK[HEADER]` 结构化解析；BODYSTRUCTURE 解析失败自动回退原文 mail_parser 路径（沿用 163 修复经验）。
- 正文仍按需懒加载（点开才拉 `BODY.PEEK[]`）。
- 批次进度事件沿用 `SyncProgressEvent` 格式，状态栏/节流刷新链路直接复用。

## 5. IDLE 推送

- watch set：INBOX 恒定 + 用户浏览过的文件夹（LRU 上限 2），每条 IDLE 连接一个文件夹。
- 收到 `EXISTS` → 立即触发该文件夹增量同步（秒级收信）；`EXPUNGE`/`FLAGS` → 本地即时修正。
- 约 25 分钟主动 `DONE + 重发`（29 分钟服务器超时前）。
- IDLE 不被支持或连续失败 → 该文件夹自动降级 90 秒短轮询。
- IDLE 期间命令流量走控制连接，互不干扰。

## 6. 文件夹管理（FolderRegistry）

三层发现策略（高→低）：

1. `LIST "" "*" RETURN (SPECIAL-USE)`——服务器直接标注角色；
2. Gmail 旧式 `XLIST` 回退；
3. provider 启发式（现有 `folder_mapping` 中文名映射保留为最终兜底）。

- **用户手动指定 > 一切**：设置页手动指定角色存 DB 覆盖服务器标注。
- 文件夹新建/重命名/删除经引擎同步到本地 `folders` 表，侧栏实时反映。
- 存储升级：新表 `folder_sync_state(account_id, folder, uidvalidity, highestmodseq, last_seen_uid, special_use)`；`folders` 表加 `special_use` 列；schema 迁移走现有机制。

## 7. 错误恢复

| 场景 | 行为 |
|---|---|
| 连接断开 | 指数退避重连（1s→30s 封顶），重连后 `ENABLE+SELECT` 恢复，`Disconnected` 事件通知状态栏 |
| 凭据失效（AUTH 失败） | 停止重连，发 `CredentialsNeeded` → UI 复用改密码弹窗，更新后自动恢复 |
| 单命令超时 | 30s；连接标记可疑重建，命令返回错误 |
| UIDVALIDITY 变化 | 本地清空 + 全量重同步，进度照常上报 |

## 8. 旧代码处置（完全取代，不留双轨）

- 删除：`folder_sync_full` / `folder_sync_increment` / `folder_sync_dispatcher` / `scheduler`（1752 行），逻辑收编进 SyncEngine。
- `ImapClient` 降级为 `engine::transport` 纯传输层（连接 + 单命令），方法收敛为引擎所需子集。
- 接入点改造：`mail_send` / `mail_draft` / `mail_operation` 的散落连接调用改为引擎命令（连接池复用）。
- 公共 API 签名不变：`sync_account_with_range` / `sync_all_accounts` / `SyncProgressEvent` 保持，上层 UI 无感知。

## 9. 测试策略

- 传输层抽象：`trait ImapSession`（connect/select/fetch/idle…），引擎面向 trait 编写，mock 会话可脚本化模拟服务器行为（QRESYNC 响应、断连、怪癖）。
- 单测矩阵：三档同步策略 ×（正常 / UIDVALIDITY 变化 / 断连恢复 / 空文件夹）；文件夹三层发现回退链；IDLE LRU 替换。
- 集成：现有 e2e seed 离线模式跑引擎全流程；163/Outlook 真机回归（ID 命令、授权码、BODYSTRUCTURE 怪癖）。
- 不变量断言：同步完成后本地 UID 集合 == 服务器集合；modseq 单调；任何路径不重复抓取已存在邮件。

## 10. 交付切分（分支内三阶段，每阶段独立可验证）

1. **骨架**：传输层抽象 + 连接池 + 能力发现；操作类命令（发信/移动/删除/打标）先走引擎。
2. **同步引擎**：三档策略 + `folder_sync_state` 存储 + 替换旧 sync 层 + 删除旧代码。
3. **收尾**：IDLE + 文件夹 registry + UI 事件接线（秒级收信体验）。

## 11. 非目标

- 不做 SMTP 引擎（SMTP 保持现状，仅复用凭据）。
- 不做 JMAP。
- 不改邮件详情渲染管线（html-renderer 独立演进）。
- 多账号并发策略维持现状（每账号独立引擎，账号间天然并行，不做全局协调器）。
