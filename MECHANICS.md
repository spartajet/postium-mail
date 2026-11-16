# MECHANICS — 数值与规则清单（来源：网易邮箱大师 5.5.3.1009）

本清单记录从目标程序中提取（反编译/静态读取）的具体数值与行为规则，
postium-mail 依据本清单实现等价行为（等价换算允许，不复制任何代码）。

## 0. IMAP 服务器适配（按服务器软件分流）

反编译 mastercore.dll `imap_session.cpp` 的 Connect 流程（FUN_10932230 @
`0x10932230`）：登录前把服务器 greeting 与一张硬编码的服务器软件名单做
子串匹配，按命中结果设置会话级兼容开关（0xf6/0xf8/0xf9/0xfb/0xfc/0x106/0x10d
等布尔字段）。另有按域名的特殊名单（yahoo.com / qq.com / foxmail.com，
@ `0x11110fac`）。

| 规则 | 值 | 来源 |
| --- | --- | --- |
| greeting 指纹名单 | "Coremail System IMap Server Ready"（字符串表 @ `0x1119b2bc`）、"Courier-IMAP"（@ `0x1119b2e0`）、"Hermes"（@ `0x1119b2f0`）、"QIP IMAP server"（@ `0x1119b2f8`）、"WinWebMail"（@ `0x1119b308`）、"Microsoft Exchange IMAP4"（@ `0x1119b314`） | FUN_10932230 逐项 strstr |
| 位 ↔ 匹配串 | 0xf6=Hermes、0xf8=QIP、0xf9=域名 yahoo/qq/foxmail、0xfb=Coremail、0xfc=Exchange、0x106=Courier、0x10d=**反相**（非 WinWebMail 才置位） | FUN_10932230 打包指针 `(len<<32)\|addr` 逐位解码 |
| 读点（已证实用途） | 0xf6→FUN_10931bf0：**非 Hermes 且 capability 含 CONDSTORE(0x1000) 才启用 CONDSTORE**；0x106→FUN_109372a0：Courier 连接清理强制收尾挂起命令；0xfc→FUN_10acbd40/10acc380：Exchange 单独通知分支（0x2714 网络重试）；0xf9→FUN_10acbd40：清延迟旗标；f8 无消费者；0x10d 读点仅在 Qt 树模型（QStandardItem） | 各读点函数反编译 |
| 域名特殊名单 | yahoo.com / qq.com / foxmail.com | `0x11110fac` 字符串表 |
| 匹配方式 | greeting 内大小写不敏感子串匹配 | FUN_10932230 反汇编 |
| 未命中 | 会话按标准 RFC 行为走 | FUN_10932230 默认路径 |

postium 实现：`server_fingerprint.rs` 的 `ServerQuirks`（greeting+域名双输入
→ 7 个开关位，含 3 组新单测：quirks_detect_matches_master_table、
condstore_gating_follows_hermes_rule）；命中的服务器软件与开关组整体记入
日志（`software=… quirks=…`）。**行为差异落地**：Hermes 服务器禁用
CONDSTORE/QRESYNC（大师 FUN_10931bf0 能力协商门控：`f6==0 && caps &
0x1000`），postium 在 factory.rs 连接后用 `caps.without_condstore()` 摘除，
同步档位决策（decide_plan）随之退到 UID 增量。其余位的读点未在
imap_session 内证实有差异行为（f8 无消费者；0x10d 仅 Qt 树模型），暂不照搬。

## 1. IMAP IDLE 重连策略（mastercore.dll）

反编译 `mm::PushServiceIMAP::OnIdleDone`（mastercore.dll @ `0x106729e0`，
源码路径 `src/mailmaster/push_service/push_service_imap.cpp` 行 0x80–0x87）：

| 规则 | 值 | 来源 |
| --- | --- | --- |
| IDLE 失败后首次重试延迟 | **3 秒** | `*piVar6 = 3`（初始 else 分支） |
| 重试延迟增长 | **×2 指数退避** | `iVar4 = *piVar6 * 2` |
| 重试延迟上限 | **300 秒** | `if (299 < iVar4) *piVar6 = 300` |
| 成功（错误码 0）后 | 立即重启 IDLE，延迟复位 | `if (param_6 == 0) … FUN_106725a0` |
| 错误码 0x2d（'-'，服务器拒绝） | 停止该账号 IDLE 服务 | `else if (param_6 == 0x2d) … FUN_10670100(0)` |

日志格式（英文原文）：`IMAP: Account <id> try do idle failed with error <code>, retry in <delay> second(s).`

## 2. 推送降级轮询（mastercore.dll）

`mm::PushService::StartPoll`（mastercore.dll @ `0x106701b0`，
`src/mailmaster/push_service/push_service.cpp` 行 0x48–0x51）：IDLE 不可用时遍历全部账号轮询；
**轮询间隔由服务端配置下发，本地无字面量常量**（mastercore.dll 中无 `poll_interval` 字符串，已全文件搜索）。

postium 现状：`crates/postium-core/src/engine/idle.rs` 的 `DEGRADE_POLL_INTERVAL = 90s`，行为等价（本地常量替代服务端下发）。

## 3. 附件文件类型 → 图标颜色（webui.zip js/common.js `AttachUtils`）

| 类型 | 颜色 | 扩展名（节选完整列表见 mechanics.json） |
| --- | --- | --- |
| excel | #2EB257 | xls xlsx csv xlsm … |
| word | #3291FF | doc docx dot … |
| ppt | #FF7421 | ppt pptx pot … |
| zip | #2EB257 | 7z rar zip tar gz … |
| mail | #F59C00 | eml |
| html / js / css | #0DC4D1 | htm html js json css ini xml |
| font | #8F96A5 | ttf fon |
| pdf | #FF575C | pdf |
| ps | #3291FF | psd pdd ps |
| ai | #F59C00 | ai eps ait |
| flash | #FF575C | fla swf flv |
| txt | #3291FF | txt |
| video | #0DC4D1 | mp4 avi mkv mov … |
| music | #FF575C | mp3 flac wav ogg … |
| img | #617EFF | jpg png gif bmp svg … |
| dll | #8F96A5 | dll ocx |
| vcf | #3291FF | vcf |
| ics | #2EB257 | ics |
| wps | #3291FF | wps wpt |
| apk | #2EB257 | apk |
| exe | #8F96A5 | exe dmg |
| key | #F59C00 | key |
| unknown | #8F96A5 | 其余全部 |

## 4. 附件大小显示（webui.zip js/common.js `MailDataAccess`）

- `< 1024` 字节 → `NNN B`
- 否则 KB；`≥ 1024 KB` → MB；均 **1 位小数**（`toFixed(1)`）
  - 即 `12.0 KB`、`1.5 MB`（不显示 `0.5 KB`，小于 1 KB 时显示字节数）

## 5. 超大附件过期提示（同上）

服务端返回 `hugeExpiredDate`（Unix 秒）时：

| 剩余时间 | 显示 |
| --- | --- |
| `hugeExpiredDate == 0` | 长期有效 |
| `< 0` | 不显示 |
| 剩余 ≥ 1 天 | （N天后过期）/ N天有效 |
| 剩余 ≥ 1 小时 | （N小时后过期）/ N小时有效 |
| 剩余 ≥ 1 分钟 | （N分钟后过期）/ N分钟有效 |
| 其余 | （N秒后过期）/ N秒有效 |

> postium 实现：`postium-ui/src/views/attach_style.rs::format_expiry`（显示
> 框架 + 单测；None/0/负值 → 长期有效，天/时/分/秒逐级两档显示）。数据端
> 需要网易中转服务（`hugeExpiredDate` 为其私有字段），协议侧暂无来源，
> 待有等价数据源时在附件条直接调用。

## 7. 新邮件提示音（对照观察）

- 行为：收到新邮件时播放一声提示音，**独立于系统通知**（大师自带
  wav 音频文件，收信路径触发）。
- postium 实现：`postium-ui/src/sound.rs::play_new_mail_chime`
  （Windows winmm `PlaySoundA` 系统事件别名 `SystemExclamation`，异步播
  放不阻塞 UI；5s 节流防批量同步连响；首次全量拉历史不响）。触发点
  `stores/sync.rs`：Completed 事件 `new_emails > 0` 且非首同步。
- 等价说明：未内置音频文件（避免拷贝大师资源），用 Windows 系统事件
  音等价替代；声音选择可在将来换成自带 wav。

## 8. 降级轮询间隔可配置（fnd_4 对照）

- 大师：轮询间隔由服务端配置下发（`mm::PushService::StartPoll`
  @ mastercore.dll 0x106701b0；二进制内无 poll_interval 本地常量）。
- postium：本地配置文件等价 —— `~/.postium/settings.json` 的
  `poll_interval_secs`（默认 90，值域 15–3600，越界钳制；文件缺失/损坏
  回默认值）。实现 `postium-core/src/infrastructure/settings.rs`，
  读取链 CoreApi 装配 → RegistryEngineAccess → IdleSupervisor →
  run_idle_loop。

## 6. 引用折叠（webui.zip js/viewer/viewer.js `checkQuote`）

- 检测 BLOCKQUOTE（id `isReplyContent`、`gmail_quote`、`ntes-*-quote` 等 6 种特征）
- 折叠面板：`font-size: 14px; color: #2a83f3`，切换文字 `12px`
- **会话中后面还有邮件（`next().length > 0`）→ 默认折叠**；否则默认展开
- 文案：展开引用 / 收起引用

## 7. 未读邮件提醒（res/audio）

`res/audio/` 含独立新邮件提示音（`newmail.wav` 等多国别版本）——邮件到达独立于系统通知播放提示音。
