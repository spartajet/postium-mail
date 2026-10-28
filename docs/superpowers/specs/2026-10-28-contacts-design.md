# 联系人管理模块设计（Contacts）

- 状态：已批准（设计阶段）
- 日期：2026-10-28
- 轨道：GPUI（postium-core + postium-ui）
- 决策记录：范围 = 自动通讯录（A）+ 完整管理（C，分组/vCard/CSV 导入导出）；存储 = 本地 SQLite（core 一等公民）；历史回填 = 是；服务端同步 = 标准 CardDAV（iCloud/Nextcloud/Fastmail/Radicale）；同步方向 = 双向（last-write-wins）

## 1. 目标与非目标

### 目标

1. **自动收集**：收发邮件自动把发件人/收件人/抄送入库，frequency 累计、last_contacted_at 更新、名字补齐；首次启用对存量邮件做一次性回填（纯 SQL，不走网络）。
2. **手动管理**：新建/编辑/删除/星标/合并（N→1）；分组（多对多）。
3. **写信自动补全**：RecipientField 输入时按 名字/邮箱前缀 匹配，按 频率+星标 排序的建议浮层。
4. **导入导出**：vCard 3.0/4.0（接受 2.1 尽力而为，导出固定 3.0）与 CSV（导入识别 Gmail/Outlook/通用方言，导出通用方言）。
5. **CardDAV 双向同步**：标准 CardDAV 服务器（iCloud 应用专用密码、Nextcloud、Fastmail、Radicale）；拉取服务端变更、推送本地编辑；ETag 变更检测、412 冲突 last-write-wins（REV vs updated_at）、删除双向传播。

### 非目标

- Google Contacts OAuth2 CardDAV（可后续增量）
- 国内服务商（QQ/163 等）服务端同步——无公开联系人 API，仅本地收集
- 联系人照片/地址等未建模属性的 UI 编辑（vCard 往返保真，但不提供编辑界面）
- CardDAV 自动发现（well-known URI）——v1 手动输入服务器地址，发现链到 addressbook 为止

## 2. 数据模型

新迁移（postium-core storage，随现有 migrate 机制）：

```sql
CREATE TABLE contacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL,             -- 主邮箱；写入前小写化
    extra_emails TEXT,               -- JSON array[string]；全部小写化
    name TEXT,
    phones TEXT,                     -- JSON array[string]
    notes TEXT,
    org TEXT,
    is_starred INTEGER NOT NULL DEFAULT 0,
    frequency INTEGER NOT NULL DEFAULT 0,
    last_contacted_at INTEGER,
    source TEXT NOT NULL DEFAULT 'local',  -- local | collected | dav
    dav_account_id INTEGER,          -- REFERENCES accounts(id) ON DELETE SET NULL
    dav_uid TEXT,
    dav_href TEXT,
    dav_etag TEXT,
    raw_vcard TEXT,                  -- 服务端最近一次 vCard 原文
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_contacts_email ON contacts(lower(email));

CREATE TABLE contact_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT
);

CREATE TABLE contact_group_members (
    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (contact_id, group_id)
);

CREATE TABLE dav_bindings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    url TEXT NOT NULL,               -- 用户输入的 CardDAV 基地址
    username TEXT NOT NULL,
    addressbook_href TEXT,           -- 发现后选定
    sync_token TEXT,
    last_sync_at INTEGER
);
```

### 同步策略按 source

| source | 拉取 | 推送 | 删除传播 |
|---|---|---|---|
| `collected`（自动收集） | 否（服务端同邮箱条目存在时并入并升级为 dav） | **否**（避免污染服务端） | 否 |
| `local`（手动创建） | 否 | 是（首次推送后升级为 dav） | 本地删 → 不发 DELETE（服务端从未有过） |
| `dav`（服务端来源） | 是 | 是 | 双向 |

## 3. core 服务层（ContactService）

文件：`crates/postium-core/src/service/contact_service.rs`，同构 `label_service`（构造于 CoreApi::init，方法上 CoreApi 门面）。

### 3.1 收集与回填

- `upsert_from_emails(items: Vec<IncomingContact>)`：按 email（小写）查找——命中则 `frequency += n`、`last_contacted_at = max(...)`、name 为空时补齐；未命中则插入 `source='collected'`。批量单事务。
- 挂接点 1（收）：`sync_service` 每个文件夹同步落库完成后，把本批邮件的 sender/recipients/cc 交给 upsert。
- 挂接点 2（发）：`mail_send::send` 成功后，收件人+抄送给 upsert。
- `backfill_from_emails()`：一条聚合 SQL 扫 emails 表（sender_email、recipient_emails、cc_emails 拆分聚合），批量 upsert；幂等（重复执行 frequency 不重复累计——回填以全量重算值覆盖 collected 条目的 frequency，或以 max 现值防重）。**实现取：回填只在 collected 条目 frequency 为初始 0 语义下写入一次性值；再次执行时以 (email→count) 全量覆盖 collected 条目的 frequency。** 手动/服务端条目只更新 last_contacted_at，不覆盖用户数据。

### 3.2 CRUD / 搜索 / 分组 / 合并

- `list(query, group_id, starred_only, page, limit)`：query 匹配 name/email LIKE；排序 `is_starred DESC, frequency DESC, name ASC`；返回总数。
- `create/update/delete/get`；`set_starred`。
- 分组：`group_create/rename/delete(color 可选)`、`group_set_members`、`groups_list(含计数)`。
- `merge(primary_id, absorbed_ids)`：extra_emails 并集（去重去主邮箱）、frequency 求和、last_contacted_at 取 max、分组并集、notes 拼接；absorbed 中 dav 条目需对其服务端发 DELETE（同步会话内执行；离线时记入待办重试——v1 简化为：合并时如有 dav 条目，要求在线并在同一次同步里完成，失败则中止合并并报错）。

### 3.3 vCard 解析/生成

自研轻量实现（`infrastructure/vcard.rs`）：

- 解析：接受 3.0/4.0（2.1 尽力：quoted-printable 折行处理）；抽取 FN、N（拼接为 name 后备）、EMAIL（多值，pref 优先为 email 主键）、TEL（多值）、ORG、NOTE、CATEGORIES（多值空格/逗号分隔）、UID、REV；属性参数（TYPE=...）忽略。
- 生成：固定 3.0；字段 = BEGIN/VCARD、VERSION、N、FN、EMAIL;TYPE=INTERNET（主+extra）、TEL*、ORG、NOTE、CATEGORIES（分组名并集）、UID（本地无则生成 `postium-<uuid>`）、REV（updated_at ISO8601）、END/VCARD；CRLF 行尾、折行 75 字节。
- **属性级合并（推送保真）**：推送 dav 条目时若 `raw_vcard` 存在——解析 raw，仅替换上述建模属性，其余属性（PHOTO/ADR/IMPP/自定义 X-）原样保留后整体序列化；raw 不存在（local 首推）则纯生成。

### 3.4 CSV 导入/导出

- 导入：嗅探表头——`First Name[,Last Name],E-mail`（Gmail）、`姓名,电子邮件` / `Name,E-Mail Address`（Outlook 中英）、通用 `name,email[,phone,group]`；逐行建联系人（source=local）；行级错误跳过并计数。
- 导出：UTF-8 BOM + `name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency` 通用方言。

### 3.5 CardDAV 客户端（`infrastructure/carddav.rs`，reqwest）

- Basic Auth（用户名 + 应用专用密码，密码经 AuthManager/keyring 存取，键 `dav:<account_id>`）。
- 发现链：`PROPFIND Depth:0` 请求 URL → `current-user-principal` → `PROPFIND principal` → `addressbook-home-set` → `PROPFIND Depth:1` 列出 addressbook（displayName）供选择，写回 `dav_bindings.addressbook_href`。
- 拉取：`REPORT addressbook-query`（Depth:1）取全量 (href, etag)；etag ≠ 本地 dav_etag 的用 `REPORT addressbook-multiget` 批量取 vCard 正文；解析后按 dav_uid 匹配更新（email 变更时仍按 uid 跟随；本地同 email 的 collected/local 条目并入该 dav 条目——分组成员保并集）。
- 推送：`updated_at > last_sync_at` 且 source ∈ {local, dav} 的条目——新（无 href）`PUT href=<addressbook>/<uid>.vcf` 无条件；已有 `PUT If-Match: <etag>`；412 → 拉服务端版本，REV 较新者胜：服务端新则本地被覆盖（提示），本地新则重推。成功后更新 etag/raw_vcard，source 升级 dav。
- 删除：本地删 dav 条目 → `DELETE`（成功或 404 均算完成）；服务端 410/条目消失 → 删本地。
- 空响应/解析失败跳过计数；整个同步返回 `DavSyncReport { pulled, pushed, deleted, conflicts, errors }`。

## 4. UI（postium-ui）

### 4.1 导航

侧栏快捷入口组新增"联系人"（日历/工作流同级，`NavTarget::Contacts`）；app.rs 宿主按 calendar_view 模式挂 `contacts_view`。

### 4.2 ContactsView（`views/contacts.rs`）

- **工具条**：搜索框（输入即过滤）、新建、导入（rfd 选 .vcf/.csv 按扩展分派）、导出（当前过滤结果 → .vcf/.csv 对话框保存）、"账号同步"按钮（含配置入口）。
- **左侧分组栏**（~180px）：全部 / 星标 / 各分组（计数徽标）+ 新建分组。
- **中列列表**：行 = 首字圆形头像（品牌紫淡底）、名字（粗）、邮箱、frequency 徽标、星标（点击切换）；多选（Ctrl+点）后工具条出现"合并"。
- **右侧详情卡**（~360px）：查看/编辑两态——名字、主邮箱、额外邮箱（chips 式增删）、电话（同）、组织、备注、分组勾选、星标；`source=dav` 显示来源账号徽标；删除按钮（dav 条目提示将同步删除服务端）。
- 空状态：图标 + "从收发邮件自动收集，或点击新建"。

### 4.3 写信自动补全（RecipientField 扩展）

- 输入变化（≥1 字符）→ 查询 `list(query, limit 8)` → 输入框下方 PopupMenu 浮层（gpui-kit popup）：
- 行 = 头像首字 + 名字 + 邮箱（匹配段高亮）+ 星标；键盘 ↑↓ 移动、Enter 提交（= 解析为 chip 并清输入）、Esc 关闭；鼠标点击同 Enter；无匹配时浮层隐藏。
- 仅补全主邮箱地址。

### 4.4 邮件详情联动

发件人头像/邮箱旁按钮"查看联系人"（存在 → 跳转 ContactsView 并选中；不存在 → "添加联系人"快捷建卡）。

### 4.5 CardDAV 配置弹窗（ContactsView 内模态，复用 AddFlow 模式）

- 账号选择（来自现有账号列表）→ CardDAV 服务器地址 + 用户名 + 应用专用密码 → "发现通讯录"（发现链，列出 addressbook 单选）→ 保存绑定（密码入 keyring）→ "立即同步"。
- 绑定列表：每条显示 账号/地址/上次同步时间/状态，支持删除绑定（不删本地联系人）与手动同步。

## 5. CoreApi 门面（新增方法一览）

```
backfill_contacts() / collect_status()
list_contacts(query, group_id, starred_only, page, limit) / get_contact / create_contact /
update_contact / delete_contact / set_contact_starred / merge_contacts
list_contact_groups / create_contact_group / rename_contact_group / delete_contact_group /
set_contact_group_members
import_vcard(text) -> ImportReport / export_vcard(filter) -> String
import_csv(text) -> ImportReport / export_csv(filter) -> String
save_dav_binding(account_id, url, username, password, addressbook_href) / delete_dav_binding /
discover_addressbooks(url, username, password) -> Vec<(href, name)> / sync_contacts(account_id) -> DavSyncReport
```

## 6. 错误处理

- CardDAV 网络/HTTP 错误：同步继续处理其余条目，汇总进 `DavSyncReport.errors`，UI Toast 展示"同步完成：拉 N 推 M，错误 K"；不阻塞本地任何功能。
- vCard 单条解析失败：跳过 + 计数（导入与同步共用报告结构）。
- 合并含 dav 条目而服务端 DELETE 失败：中止合并、还原本地（单事务），报错提示稍后在线重试。
- e2e 离线模式：所有 CardDAV 方法短路返回空结果/Ok（保持测试确定性，同 sync 的 e2e_offline 守卫）。

## 7. 测试策略

- **core 单测**：vCard 解析→生成→再解析等价（含多邮箱/分组/quoted-printable 折行）；合并并集逻辑；回填聚合与幂等；upsert 大小写去重；CSV 方言嗅探。
- **CardDAV 集成**：wiremock 模拟服务器覆盖——发现链、REPORT 列表、multiget、PUT 新建/If-Match、412 冲突两分支（服务端胜/本地胜）、DELETE、410 清理。
- **UI e2e**（headless）：种子联系人渲染/搜索过滤/星标切换/分组成员；写信自动补全（键入→浮层项→Enter 成 chip）；合并后条目消失。
- CI：现有 workspace 测试矩阵全绿。

## 8. 实施分期（单一 spec 内四阶段，各自可验证可提交）

1. **阶段一**：迁移 + ContactService（收集挂接、回填、CRUD、搜索、分组、合并）+ CoreApi 门面 + core 单测 + UI e2e 骨架（种子渲染）。
2. **阶段二**：ContactsView 完整交互 + 侧栏导航 + 邮件详情联动 + e2e。
3. **阶段三**：vCard/CSV 导入导出（含报告）+ ContactsView 导入导出入口 + 单测/e2e。
4. **阶段四**：CardDAV 客户端 + 同步引擎 + 绑定/发现/配置 UI + wiremock 集成测试。

## 9. 已知限制（v1 明示）

- 未建模 vCard 属性（照片/地址/IM 等）仅往返保真，不可在 UI 编辑。
- 合并/删除 dav 条目需在线（离线时报错而非静默排队）。
- 自动收集的条目不回推服务端（策略见 §2 表）。
- CardDAV 服务器地址需手动输入（无 well-known 自动发现）。
