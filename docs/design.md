# Postium 方案设计文档

> 技术选型与实现方案  
> 版本：v0.1

---

## 一、技术栈

| 层级 | 技术选型 | 说明 |
|------|---------|------|
| 桌面容器 | Tauri 2.x | 跨平台原生窗口，Rust 后端 |
| 前端框架 | Vue 3 + TypeScript | Composition API + 类型安全 |
| 构建工具 | Vite | 快速 HMR，原生 ESM |
| UI 组件库 | Naive UI | 深色主题 token 系统完善，与原型设计变量对应 |
| 状态管理 | Pinia | Vue 3 官方推荐，支持 DevTools |
| 富文本编辑 | Tiptap | 基于 ProseMirror，扩展性强 |
| 日期处理 | date-fns | 轻量、tree-shakeable |
| 本地数据库 | SQLite（via rusqlite） | 邮件元数据、正文、账号、日历等本地持久化 |
| ORM | SeaORM 2.0 | 主力数据库操作层，异步 + 类型安全 |
| 全文检索 | rusqlite + sqlite-jieba-tokenizer | 中文分词全文搜索，FTS5 虚拟表 |
| 向量检索 | Qdrant + qdrant-client | 语义搜索、AI 相似邮件检索 |
| Rust 邮件收发 | lettre（SMTP）、imap（IMAP） | Tauri invoke 调用 |

---

## 二、数据库设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────┐
│                    Tauri Rust 后端                    │
│                                                     │
│   ┌───────────────┐    ┌─────────────────────────┐  │
│   │   SeaORM 2.0  │    │  rusqlite（直接操作）    │  │
│   │  （主力 CRUD） │    │  + sqlite-jieba-        │  │
│   └──────┬────────┘    │    tokenizer（FTS5）     │  │
│          │             └──────────┬──────────────┘  │
│          └──────────┬─────────────┘                 │
│                     ▼                               │
│              SQLite（本地文件）                       │
│                                                     │
│   ┌───────────────────────────────────────────────┐ │
│   │         qdrant-client（向量检索）               │ │
│   │         → 本地 Qdrant 进程 / embedded         │ │
│   └───────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**分工原则：**
- **SeaORM**：所有常规 CRUD（邮件列表查询、账号管理、日历事务、工作流存储）
- **rusqlite 直接调用**：FTS5 全文检索查询（SeaORM 对 FTS 虚拟表支持有限）
- **Qdrant**：语义向量检索（"找和这封邮件主题相似的邮件"、AI 对话的 RAG 增强）

### 2.2 SQLite 表结构

#### accounts

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PRIMARY KEY | UUID |
| name | TEXT | 显示名 |
| email | TEXT | 邮箱地址 |
| provider | TEXT | gmail / outlook / icloud / yahoo / imap |
| color | TEXT | 颜色 hex 值 |
| imap_host | TEXT | IMAP 服务器 |
| imap_port | INTEGER | IMAP 端口 |
| smtp_host | TEXT | SMTP 服务器 |
| smtp_port | INTEGER | SMTP 端口 |
| created_at | INTEGER | Unix 时间戳 |

> 账号密码不存入 SQLite，使用 Tauri 的操作系统密钥链（keyring）单独存储。

#### emails

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| account_id | TEXT | 关联账号 |
| message_id | TEXT UNIQUE | 邮件服务器 Message-ID |
| sender | TEXT | 发件人姓名 |
| sender_email | TEXT | 发件人邮箱 |
| recipient | TEXT | 收件人（JSON 数组） |
| subject | TEXT | 主题 |
| preview | TEXT | 正文前 100 字 |
| body | TEXT | 完整 HTML 正文 |
| folder | TEXT | inbox / sent / drafts / spam / trash |
| unread | INTEGER | 0 / 1 |
| starred | INTEGER | 0 / 1 |
| labels | TEXT | JSON 数组 |
| has_attachments | INTEGER | 0 / 1 |
| date | INTEGER | Unix 时间戳 |
| uid | INTEGER | IMAP UID，用于服务器同步 |

#### attachments

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| email_id | INTEGER | 关联邮件 |
| name | TEXT | 文件名 |
| size | INTEGER | 字节数 |
| mime_type | TEXT | MIME 类型 |
| local_path | TEXT | 本地缓存路径（可为空） |

#### calendar_events

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| title | TEXT | 事务标题 |
| date | TEXT | ISO 日期（YYYY-MM-DD） |
| start_time | TEXT | HH:mm |
| end_time | TEXT | HH:mm |
| repeat | TEXT | none / daily / weekly / monthly / yearly |
| repeat_end | TEXT | ISO 日期，可为空 |
| color | TEXT | hex 颜色值 |
| notes | TEXT | 备注 |

#### workflows

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PRIMARY KEY | UUID |
| name | TEXT | 工作流名称 |
| nodes | TEXT | JSON（节点数组） |
| edges | TEXT | JSON（连线数组） |
| enabled | INTEGER | 0 / 1 |
| created_at | INTEGER | Unix 时间戳 |

#### contacts

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| name | TEXT NOT NULL | 联系人姓名 |
| emails | TEXT NOT NULL | JSON 数组，邮箱地址列表 |
| phones | TEXT | JSON 数组，电话号码列表 |
| company | TEXT | 公司名称 |
| title | TEXT | 职位/头衔 |
| notes | TEXT | 备注信息 |
| avatar | TEXT | 头像图片路径 |
| starred | INTEGER | 0 / 1，是否星标 |
| created_at | INTEGER | Unix 时间戳 |
| updated_at | INTEGER | Unix 时间戳 |

#### contact_groups

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| name | TEXT NOT NULL | 分组名称 |
| color | TEXT | hex 颜色值 |
| created_at | INTEGER | Unix 时间戳 |

#### contact_group_members (联系人-分组关联表)

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PRIMARY KEY | 自增 |
| contact_id | INTEGER | 联系人 ID，外键到 contacts(id) |
| group_id | INTEGER | 分组 ID，外键到 contact_groups(id) |
| created_at | INTEGER | Unix 时间戳 |

> 联系人与分组是多对多关系，一个联系人可以属于多个分组。

### 2.3 数据库迁移（SeaORM Migrate）

所有关系型表的创建与变更均通过 **SeaORM Migration** 管理，禁止手动执行 DDL。

**Crate 结构：**

```
src-tauri/
├── Cargo.toml
├── src/
│   └── main.rs          # 启动时调用 Migrator::up()
└── migration/
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs        # MigratorTrait 实现，注册所有 Migration
    │   ├── m20240001_000001_create_accounts.rs
    │   ├── m20240001_000002_create_emails.rs
    │   ├── m20240001_000003_create_attachments.rs
    │   ├── m20240001_000004_create_calendar_events.rs
    │   ├── m20240001_000005_create_workflows.rs
    │   ├── m20240001_000006_create_emails_fts.rs   # FTS5 虚拟表（raw SQL）
    │   ├── m20240001_000007_create_contacts.rs
    │   ├── m20240001_000008_create_contact_groups.rs
    │   └── m20240001_000009_create_contact_group_members.rs
    └── ...
```

**启动时自动迁移：**

```rust
// src-tauri/src/main.rs
use migration::{Migrator, MigratorTrait};

let db = Database::connect(db_url).await?;
Migrator::up(&db, None).await?;   // 自动执行未运行的迁移
```

**迁移原则：**

| 场景 | 做法 |
|------|------|
| 新增表 | 创建新的 `mYYYYMMDD_NNNNNN_create_xxx.rs` 文件 |
| 新增字段 | 创建新的 `mYYYYMMDD_NNNNNN_alter_xxx_add_yyy.rs`，使用 `ALTER TABLE ADD COLUMN` |
| 删除/重命名字段 | 创建新迁移，SQLite 限制下通过"新建表→复制数据→删旧表→重命名"完成 |
| FTS5 虚拟表 | 在独立迁移文件中使用 `Statement::from_string` 执行 raw SQL |
| 回滚 | 实现 `down()` 方法，开发期可用 `Migrator::down(&db, None)` 全量回滚 |

> FTS5 虚拟表和 jieba tokenizer 触发器不由 SeaORM Schema Builder 生成，在对应迁移文件的 `up()` 中直接执行 raw SQL。

在 SQLite 中建立 FTS5 虚拟表，使用 `sqlite-jieba-tokenizer` 注册中文分词器：

```sql
CREATE VIRTUAL TABLE emails_fts USING fts5(
    subject,
    sender,
    preview,
    body,
    content='emails',
    content_rowid='id',
    tokenize='jieba'
);

-- 触发器保持 FTS 与主表同步
CREATE TRIGGER emails_ai AFTER INSERT ON emails BEGIN
    INSERT INTO emails_fts(rowid, subject, sender, preview, body)
    VALUES (new.id, new.subject, new.sender, new.preview, new.body);
END;
```

查询示例：

```sql
-- 普通关键词检索
SELECT e.* FROM emails e
JOIN emails_fts f ON e.id = f.rowid
WHERE emails_fts MATCH '项目报告'
ORDER BY rank;

-- 前缀匹配 + 多字段
SELECT e.* FROM emails e
JOIN emails_fts f ON e.id = f.rowid
WHERE emails_fts MATCH 'subject: 发票 OR sender: 财务部'
ORDER BY rank;
```

### 2.4 全文检索（FTS5 + jieba 分词）

在 SQLite 中建立 FTS5 虚拟表，使用 `sqlite-jieba-tokenizer` 注册中文分词器。该表通过独立迁移文件（`m..._create_emails_fts.rs`）以 raw SQL 创建。

```sql
CREATE VIRTUAL TABLE emails_fts USING fts5(
    subject,
    sender,
    preview,
    body,
    content='emails',
    content_rowid='id',
    tokenize='jieba'
);

-- 触发器保持 FTS 与主表同步
CREATE TRIGGER emails_ai AFTER INSERT ON emails BEGIN
    INSERT INTO emails_fts(rowid, subject, sender, preview, body)
    VALUES (new.id, new.subject, new.sender, new.preview, new.body);
END;
```

查询示例：

```sql
-- 普通关键词检索
SELECT e.* FROM emails e
JOIN emails_fts f ON e.id = f.rowid
WHERE emails_fts MATCH '项目报告'
ORDER BY rank;

-- 前缀匹配 + 多字段
SELECT e.* FROM emails e
JOIN emails_fts f ON e.id = f.rowid
WHERE emails_fts MATCH 'subject: 发票 OR sender: 财务部'
ORDER BY rank;
```

### 2.5 向量检索（Qdrant）

用于语义搜索和 AI 对话的 RAG 增强。

**Collection 设计：**

```
Collection: emails_vectors
  - vector size: 1536（OpenAI text-embedding-3-small）或 768（本地模型）
  - payload:
      email_id: u64
      account_id: string
      subject: string
      sender_email: string
      date: i64（Unix 时间戳）
      folder: string
      labels: string[]
```

**使用场景：**

| 场景 | 说明 |
|------|------|
| 语义搜索 | "找和项目预算相关的邮件"（关键词匹配不到但语义相关） |
| AI 对话 RAG | 将语义相近的邮件作为上下文注入 AI 对话，提升回答准确性 |
| 相似邮件推荐 | 查看某封邮件时，推荐主题相近的历史邮件 |
| 智能分类辅助 | 新邮件与已标签邮件向量相似度，辅助自动打标签 |

**嵌入策略：**
- 对 `subject + preview`（非完整正文）做嵌入，控制 token 成本
- 新邮件同步后异步生成向量，不阻塞主流程
- 支持切换嵌入模型（OpenAI / 本地 Ollama nomic-embed-text）

---

## 三、数据模型（TypeScript 类型定义）

### 3.1 邮件（Email）

```typescript
interface Email {
  id: number
  sender: string
  senderEmail: string
  recipient: string
  subject: string
  preview: string        // 正文前100字
  body: string           // HTML 格式正文
  date: Date
  unread: boolean
  starred: boolean
  labels: string[]       // ['work', 'urgent', ...]
  folder: 'inbox' | 'starred' | 'sent' | 'drafts' | 'spam' | 'trash'
  attachments: Attachment[]
  accountId: string      // 关联账号
}

interface Attachment {
  name: string
  size: string
  path?: string          // 本地缓存路径
}
```

### 3.2 账号（Account）

```typescript
interface Account {
  id: string
  name: string
  email: string
  provider: 'gmail' | 'outlook' | 'icloud' | 'yahoo' | 'imap'
  color: string
  unreadCount: number
  // 后端存储（Tauri 安全存储）
  imapHost?: string
  imapPort?: number
  smtpHost?: string
  smtpPort?: number
  password?: string      // 加密存储，不在前端明文持久化
}
```

### 3.3 日历事务（CalendarEvent）

```typescript
interface CalendarEvent {
  id: number
  title: string
  date: Date
  startTime: string      // 'HH:mm'
  endTime: string        // 'HH:mm'
  repeat: 'none' | 'daily' | 'weekly' | 'monthly' | 'yearly'
  repeatEnd?: string     // ISO 日期字符串
  color: string          // hex 颜色值
  notes: string
}
```

### 3.4 工作流节点（WorkflowNode）

```typescript
interface WorkflowNode {
  id: string
  type: 'trigger' | 'condition' | 'action' | 'email' | 'delay' | 'ai'
  position: { x: number; y: number }
  config: Record<string, unknown>
}
```

### 3.5 AI 对话消息（ChatMessage）

```typescript
interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  // 操作类消息附加字段
  pendingAction?: {
    type: string
    description: string
    affectedCount: number
    emailIds: number[]
  }
  actionResult?: {
    success: number
    failed: number
  }
}
```

### 3.6 联系人（Contact）

```typescript
interface Contact {
  id: number
  name: string
  emails: ContactEmail[]
  phones: string[]
  company?: string
  title?: string
  groupIds: number[]      // 分组 ID 列表
  notes?: string
  avatar?: string         // 头像路径
  starred: boolean
  createdAt: Date
  updatedAt: Date
}

interface ContactEmail {
  email: string
  label?: 'personal' | 'work' | 'other'  // 邮箱类型标签
  isPrimary: boolean      // 是否为主要邮箱
}

interface ContactGroup {
  id: number
  name: string
  color: string           // hex 颜色值
  createdAt: Date
  contactCount?: number   // 该分组下的联系人数量（计算属性）
}
```

### 3.7 联系人表单数据（ContactFormData）

```typescript
interface ContactFormData {
  name: string
  emails: ContactEmail[]
  phones: string[]
  company?: string
  title?: string
  groupIds: number[]
  notes?: string
  starred?: boolean
}
```
```

---

## 四、Pinia Store 划分

| Store | 职责 |
|-------|------|
| `useEmailStore` | 邮件列表、当前选中邮件、搜索/过滤状态 |
| `useAccountStore` | 账号列表、当前激活账号 |
| `useCalendarStore` | 日历事务、当前月份 |
| `useUIStore` | 主题、当前视图、模态框开关、Toast 队列 |
| `useWorkflowStore` | 工作流节点和连线 |
| `useAIChatStore` | 对话消息历史、会话上下文、流式输出状态 |
| `useContactStore` | 联系人列表、分组管理、搜索/过滤状态 |

---

## 五、Vue 组件结构

```
src/
├── components/
│   ├── layout/
│   │   ├── AppSidebar.vue
│   │   ├── EmailList.vue
│   │   ├── EmailDetail.vue
│   │   └── StatusBar.vue
│   ├── email/
│   │   ├── EmailItem.vue
│   │   ├── EmailViewer.vue
│   │   ├── AISummaryCard.vue
│   │   └── AIActionsBar.vue
│   ├── compose/
│   │   ├── ComposeModal.vue
│   │   ├── RichEditor.vue
│   │   └── AIComposePanel.vue
│   ├── calendar/
│   │   ├── CalendarView.vue
│   │   ├── CalendarGrid.vue
│   │   └── EventModal.vue
│   ├── workflow/
│   │   ├── WorkflowEditor.vue
│   │   ├── NodePalette.vue
│   │   └── WorkflowCanvas.vue
│   ├── ai-chat/
│   │   ├── AIChatView.vue         # 全宽对话视图
│   │   ├── AIChatDrawer.vue       # 侧边抽屉形式
│   │   ├── ChatMessage.vue        # 单条消息（含 Markdown 渲染）
│   │   ├── EmailResultCard.vue    # 查询结果邮件卡片
│   │   ├── ActionConfirm.vue      # 操作确认组件
│   │   └── QuickPrompts.vue       # 快捷指令按钮组
│   ├── contacts/
│   │   ├── ContactView.vue        # 通讯录主视图（双栏布局）
│   │   ├── ContactList.vue        # 联系人列表
│   │   ├── ContactCard.vue        # 单个联系人卡片
│   │   ├── ContactDetail.vue      # 联系人详情面板
│   │   ├── ContactModal.vue       # 添加/编辑联系人模态框
│   │   ├── ContactGroupList.vue   # 分组列表
│   │   ├── ContactGroupModal.vue  # 分组管理模态框
│   │   └── ContactSearch.vue      # 搜索组件
│   ├── settings/
│   │   └── SettingsModal.vue
│   └── common/
│       ├── ToastContainer.vue
│       └── AccountSelector.vue
├── stores/
│   ├── email.ts
│   ├── account.ts
│   ├── calendar.ts
│   ├── workflow.ts
│   ├── ui.ts
│   ├── aiChat.ts
│   └── contact.ts
├── composables/
│   ├── useAI.ts            # AI 基础调用封装（流式输出）
│   ├── useAIChat.ts        # 对话逻辑、意图解析、操作执行
│   ├── useEmailActions.ts  # 批量邮件操作（供 AI 和手动操作共用）
│   ├── useIMAP.ts          # 收邮件（Tauri invoke）
│   └── useSMTP.ts          # 发邮件（Tauri invoke）
└── App.vue
```

---

## 六、AI 对话框技术实现

| 方面 | 方案 |
|------|------|
| AI 模型调用 | 支持配置 OpenAI / Anthropic / 本地 Ollama |
| 意图解析 | Function Calling / Tool Use（结构化输出操作指令） |
| 邮件上下文注入 | 将邮件元数据（非完整正文）作为 context 传入，控制 token 消耗 |
| 流式输出 | AI 回复逐字流式显示（SSE / ReadableStream），提升响应感知速度 |
| 操作执行 | 前端调用 Pinia action 同步更新状态；后端通过 IMAP STORE 命令同步服务器 |
| 会话上下文 | 消息历史存入 `useAIChatStore`，每次请求携带最近 N 轮对话 |

---

## 七、Naive UI 主题映射

原型 CSS 变量与 Naive UI `themeOverrides` 的对应关系：

```typescript
const themeOverrides = {
  common: {
    primaryColor: '#7C3AED',
    primaryColorHover: '#6D28D9',
    bodyColor: '#0F0F23',
    cardColor: 'rgba(255,255,255,0.05)',
    borderRadius: '8px',
  }
}
```

---

## 八、开发优先级

### P0 - MVP 核心功能

- [ ] 基础三栏布局
- [ ] 邮件列表展示与选中
- [ ] 邮件详情查看
- [ ] 写信（Tiptap 编辑器）+ 发送
- [ ] 多账号添加（IMAP/SMTP 配置）
- [ ] 暗色/亮色主题切换
- [ ] SQLite 初始化 + SeaORM 实体定义（accounts / emails / attachments）
- [ ] FTS5 全文检索表及 jieba 分词器注册

### P1 - AI 核心特性

- [ ] AI 摘要卡片（接入 LLM API）
- [ ] 智能回复建议
- [ ] AI 写作助手（生成草稿/改进/精简/正式化）
- [ ] 邮件翻译
- [ ] 任务提取
- [ ] AI 对话框 —— 查询类（邮件搜索、统计、摘要）
- [ ] AI 对话框 —— 操作类（批量标记/移动/删除，含确认流程）
- [ ] AI 对话框 —— 上下文感知（多轮对话、代词指代）

### P2 - 效率功能

- [ ] 个人日程（日历视图 + 事务 CRUD）
- [ ] **通讯录管理（联系人 CRUD + 分组 + 搜索）**
- [ ] AI 智能分类（自动打标签）
- [ ] 搜索与过滤（FTS5 关键词 + 筛选条件）
- [ ] 键盘快捷键
- [ ] Qdrant 向量索引初始化 + 嵌入生成管线

### P3 - 进阶功能

- [ ] 工作流编辑器
- [ ] 本地 AI 模型支持（Ollama）
- [ ] 邮件通知（系统通知）
- [ ] 附件预览
- [ ] 虚拟滚动优化
- [ ] AI 对话框 —— 日程操作（"帮我把这封邮件的会议加到日历"）
- [ ] AI 对话框 —— 跨账号批量操作
- [ ] **通讯录导入/导出（vCard 格式）**
- [ ] **写信时联系人自动补全**
- [ ] **邮件发件人显示联系人姓名**
- [ ] **通讯录与系统通讯录同步（可选）**

---

*详细功能需求见 [requirements.md](./requirements.md)。*
