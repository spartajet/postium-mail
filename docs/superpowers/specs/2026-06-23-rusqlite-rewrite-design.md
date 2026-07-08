# rusqlite 数据库层全量替换设计

日期：2026-06-23

## 背景

当前后端数据库层基于 SeaORM 和 sea-orm-migration：

- `src-tauri/src/infrastructure/storage/database.rs` 负责创建 SeaORM `DatabaseConnection` 并运行迁移。
- `src-tauri/src/infrastructure/storage/entities/` 保存 SeaORM entity 和 ActiveModel。
- `src-tauri/src/infrastructure/storage/repository/` 使用 Entity、ActiveModel、QueryFilter、TransactionTrait 等 SeaORM API。
- `src-tauri/migration/` 是独立迁移 crate，维护历史迁移。
- 后端测试和 seed 数据也直接依赖 SeaORM。

这套结构对本地 SQLite 桌面应用偏重。近期账号删除和 FTS 问题也暴露出历史迁移、FTS 触发器、foreign key 开关之间的维护成本。新方向是一次性移除 SeaORM，改为 `tokio-rusqlite + rusqlite`。

用户明确约束：

1. 一次性全量替换，不做分阶段双栈。
2. 不迁移旧数据。
3. 不兼容旧版本数据库。
4. 应用启动时直接删除旧 `postium.sqlite`，再按新 schema 重建。
5. 维护一份详细建表 SQL 文件。

## 目标

本次重写完成后应满足：

1. `src-tauri` 不再依赖 `sea-orm` 和 `sea-orm-migration`。
2. 删除 `src-tauri/migration` crate 以及 SeaORM entity/ActiveModel 使用。
3. 数据库初始化使用 `tokio_rusqlite::Connection`。
4. 启动时删除已有 `postium.sqlite`、`postium.sqlite-wal`、`postium.sqlite-shm`，再创建新数据库。
5. `src-tauri/sql/schema.sql` 成为唯一 schema 来源，包含表、索引、FTS5 虚表和触发器。
6. Repository 使用手写 SQL，并返回稳定的 Rust model struct。
7. 事务使用 `Connection::call` 内部的 `rusqlite::Transaction` 完成，避免跨 `await` 持有同步事务。
8. 现有命令、service、前端类型绑定的行为尽量保持不变。
9. 后端测试全部切到新数据库层。

## 非目标

本次不做以下事情：

1. 不保留旧数据库文件中的账号、邮件或同步数据。
2. 不实现 schema version 表或历史 migration 系统。
3. 不支持从 SeaORM 版本数据库升级。
4. 不改变前端交互、命令名称或业务功能。
5. 不引入连接池；`tokio-rusqlite` 单连接后台线程足够支撑当前本地应用。
6. 不把 SQL 拆成多份 migration 文件。

## 总体方案

采用“单 schema 文件 + 手写 repository + tokio-rusqlite 异步边界”的全量替换方案。

启动流程：

1. 计算数据库路径 `data_dir/postium.sqlite`。
2. 删除 `postium.sqlite`、`postium.sqlite-wal`、`postium.sqlite-shm`。文件不存在时忽略。
3. 用 `tokio_rusqlite::Connection::open` 打开新数据库。
4. 通过 `include_str!("../../../sql/schema.sql")` 或等价路径加载 schema。
5. 在 `Connection::call` 中执行：
   - `PRAGMA foreign_keys = ON;`
   - `PRAGMA journal_mode = WAL;`
   - `PRAGMA busy_timeout = 5000;`
   - `schema.sql`
6. 返回应用共享的 `DbConn`。

数据库操作：

- `DbConn` 定义为可克隆的 wrapper，例如：

```rust
#[derive(Clone)]
pub struct DbConn {
    inner: tokio_rusqlite::Connection,
}
```

- Repository 接收 `&DbConn`。
- 简单读写通过 `db.call(|conn| { ... })` 执行。
- 跨表原子操作通过 `db.transaction(|tx| { ... })` 形式封装。这个 helper 内部进入 `Connection::call`，同步创建 `rusqlite::Transaction`，执行闭包，最后 commit/rollback。

## schema.sql 设计

新增文件：

`src-tauri/sql/schema.sql`

该文件包含完整 schema，而不是增量迁移。建议结构如下：

1. `PRAGMA foreign_keys = ON;`
2. `CREATE TABLE accounts ...`
3. `CREATE TABLE emails ...`
4. `CREATE TABLE attachments ...`
5. `CREATE TABLE labels ...`
6. `CREATE TABLE email_labels ...`
7. `CREATE TABLE sync_state ...`
8. `CREATE TABLE sync_errors ...`
9. 普通索引
10. `emails_fts` FTS5 虚表
11. `emails_fts_ai`、`emails_fts_ad`、`emails_fts_au` 触发器

核心表保持当前最终模型字段：

- `accounts`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `name TEXT NOT NULL`
  - `email TEXT NOT NULL UNIQUE`
  - `display_name TEXT`
  - `provider TEXT NOT NULL`
  - `imap_host TEXT`
  - `imap_port INTEGER`
  - `imap_ssl INTEGER DEFAULT 1`
  - `imap_ssl_mode TEXT`
  - `smtp_host TEXT`
  - `smtp_port INTEGER`
  - `smtp_ssl INTEGER DEFAULT 1`
  - `smtp_ssl_mode TEXT`
  - `color TEXT`
  - `sync_enabled INTEGER DEFAULT 1`
  - `last_sync_at INTEGER`
  - `auth_type TEXT DEFAULT 'password'`
  - `account_type TEXT NOT NULL DEFAULT 'personal'`
  - `created_at INTEGER NOT NULL`
  - `updated_at INTEGER NOT NULL`

- `emails`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE`
  - `folder TEXT NOT NULL`
  - `uid INTEGER NOT NULL`
  - `message_id TEXT UNIQUE`
  - `subject TEXT`
  - `sender_name TEXT`
  - `sender_email TEXT NOT NULL`
  - `recipient_emails TEXT NOT NULL`
  - `cc_emails TEXT`
  - `bcc_emails TEXT`
  - `preview TEXT`
  - `body_text TEXT`
  - `body_html TEXT`
  - `is_read INTEGER DEFAULT 0`
  - `is_starred INTEGER DEFAULT 0`
  - `is_draft INTEGER DEFAULT 0`
  - `is_answered INTEGER DEFAULT 0`
  - `is_deleted INTEGER DEFAULT 0`
  - `sent_at INTEGER NOT NULL`
  - `received_at INTEGER NOT NULL`
  - `created_at INTEGER NOT NULL`
  - `updated_at INTEGER NOT NULL`

- `attachments`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE`
  - `filename TEXT`
  - `content_type TEXT`
  - `size INTEGER NOT NULL`
  - `section_path TEXT NOT NULL`
  - `disposition TEXT`
  - `content_id TEXT`
  - `path TEXT`
  - `created_at INTEGER NOT NULL`

- `labels`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE`
  - `name TEXT NOT NULL`
  - `color TEXT NOT NULL`
  - `created_at INTEGER NOT NULL`

- `email_labels`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE`
  - `label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE`
  - `created_at INTEGER NOT NULL`
  - `UNIQUE(email_id, label_id)`

- `sync_state`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE`
  - `folder TEXT NOT NULL`
  - `folder_nick_name TEXT`
  - `uidvalidity INTEGER`
  - `uidnext INTEGER`
  - `synced_at INTEGER`
  - `last_sync_uid INTEGER`
  - `created_at INTEGER`
  - `updated_at INTEGER`

- `sync_errors`
  - `id INTEGER PRIMARY KEY AUTOINCREMENT`
  - `account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE`
  - `folder TEXT`
  - `error_type TEXT NOT NULL`
  - `error_message TEXT NOT NULL`
  - `uid INTEGER`
  - `stack_trace TEXT`
  - `resolved INTEGER DEFAULT 0`
  - `created_at INTEGER NOT NULL`

索引至少包含：

- `idx_emails_account`
- `idx_emails_folder`
- `idx_emails_sent_at`
- `idx_emails_is_read`
- `idx_attachments_email`
- `idx_labels_account`
- `idx_email_labels_email`
- `idx_email_labels_label`
- `idx_sync_state_account_folder`
- `idx_sync_errors_account`

FTS 设计：

```sql
CREATE VIRTUAL TABLE emails_fts USING fts5(
    subject,
    sender_email,
    preview,
    content='emails',
    content_rowid='id'
);
```

触发器使用当前已验证过的 FTS5 delete command 形式：

- insert：插入 `new.id/new.subject/new.sender_email/new.preview`
- delete：`INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview) VALUES ('delete', old.id, old.subject, old.sender_email, old.preview)`
- update：先 delete old，再 insert new

## Rust 模型设计

删除 SeaORM entity 后，在 storage model 层定义普通 struct。建议路径：

`src-tauri/src/infrastructure/storage/models/`

初始文件：

- `accounts.rs`
- `emails.rs`
- `attachments.rs`
- `labels.rs`
- `email_labels.rs`
- `sync_state.rs`
- `sync_errors.rs`
- `mod.rs`

这些 model 保持当前 `entities::*::Model` 对外字段名和类型，继续 derive 需要的 trait：

```rust
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Account { ... }
```

为了降低 service 改动量，可以有两种命名策略：

1. 直接将新 model 命名为 `accounts::Model`、`emails::Model`，路径从 `entities` 改为 `models`。
2. 改成更普通的 `Account`、`Email`，同时批量更新 service/repository 类型。

推荐策略 1，减少一次性替换的业务层改动面。后续可以再做命名清理。

## Repository 改造

Repository 保持现有模块名：

- `account_repo`
- `email_repo`
- `attachment_repo`
- `label_repo`
- `sync_repo`

但实现从 SeaORM API 改为 SQL：

- 查询使用 `prepare`、`query_map`、`query_row`。
- 单行可空查询统一使用 `OptionalExtension`。
- 插入后用 `last_insert_rowid()` 查询主键。
- 批量插入使用事务和 prepared statement。
- 动态 `IN (...)` 查询必须使用参数占位符生成 helper，禁止字符串拼接用户输入。
- 布尔值在 SQLite 存为 `INTEGER`，row mapping 时转换为 `Option<bool>` 或 `bool`。

账号删除保留当前 service 编排语义，但事务实现改为 rusqlite：

1. 删除 `email_labels` 中目标账号邮件或标签产生的关联。
2. 删除 `labels`。
3. 删除 `attachments`。
4. 删除 `emails`。
5. 删除 `sync_errors`。
6. 删除 `sync_state`。
7. 删除 `accounts`。
8. 事务 commit 后再删除 keyring 密码。

即使 `schema.sql` 开启 foreign key，本业务仍显式清理依赖表，保持当前已验证的行为。

## Search 改造

`search.rs` 中的 `FromQueryResult` 改为手写 row mapping。

`SearchResult` 类型继续保留当前字段和 `Serialize`、`Deserialize`、`specta::Type` derive。

查询 SQL 可以沿用当前语义：

- account scoped search：
  - `emails_fts MATCH ?`
  - `e.account_id = ?`
  - `e.is_deleted = 0`
  - `ORDER BY f.rank DESC LIMIT ?`
- 全局 search：
  - 不加 account filter

## 错误处理

`MailError` 删除 `From<sea_orm::DbErr>`，新增：

```rust
impl From<rusqlite::Error> for MailError
impl From<tokio_rusqlite::Error> for MailError
```

转换目标仍是 `MailError::DatabaseError(err.to_string())`。

`tokio-rusqlite` 的 `Connection::call` 闭包通常要求错误类型满足 Send 边界。Repository 内部应避免返回不可发送的自定义借用错误，必要时把 rusqlite 错误转换为 owned error message。

## 测试策略

测试 helper 改为使用同一份 `schema.sql`：

- 内存库测试：`Connection::open_in_memory()` 后执行 schema。
- 文件库初始化测试：用 tempdir，创建旧 `postium.sqlite` 文件，然后调用 `init_database`，断言旧内容被删除，新 schema 可用。

需要改造的测试范围：

- `account_commands.rs`
- `email_commands.rs`
- `label_commands.rs`
- `e2e_seed.rs`
- `migrations.rs`
- `common/mod.rs`

`migrations.rs` 不再测试历史迁移，改名或重写为 `schema.rs` 更准确，测试内容变为：

1. `schema.sql` 可在空库执行。
2. FTS insert/update/delete 触发器正常。
3. foreign key cascade 在新 schema 下开启并工作。
4. 账号删除 service 仍显式清理目标账号数据且不影响其他账号。

## SQLite MCP

SQLite MCP 对这次实现不是前置依赖。推荐策略：

1. 本次实现不依赖 SQLite MCP，避免工具安装影响核心迁移。
2. 完成 rusqlite 替换后，如果需要经常检查本地 `postium.sqlite`，再安装 SQLite MCP 用于开发调试。
3. 即使安装 MCP，也只作为观察工具，不作为测试或构建门禁。

理由：

- 当前任务主要是代码重写和测试验证，`rusqlite` 集成测试已经能覆盖正确性。
- MCP 更适合人工查看表数据、跑临时查询。
- 把 MCP 放进本次实施路径会增加额外不确定性。

## 备选方案

### 方案 A：一次性全量替换并启动重建数据库

优点：

- 完全移除 SeaORM 和历史 migration 复杂度。
- schema 来源单一，审查和调试更直接。
- 符合“不迁移、不兼容旧版”的约束。

缺点：

- 一次性改动大。
- Repository、测试、seed 都要同步重写。
- 需要严格依赖现有测试防止业务行为退化。

结论：采用该方案。

### 方案 B：保留 SeaORM migration，只替换 repository

优点：

- 初期改动少。
- 旧 schema 创建逻辑可复用。

缺点：

- 不能真正移除 SeaORM。
- 仍保留历史迁移和 FTS 迁移维护成本。
- 与用户要求“一次性全量替换”冲突。

结论：不采用。

### 方案 C：引入 sqlx 而不是 rusqlite

优点：

- 异步生态更完整。
- 编译期 SQL 检查能力更强。

缺点：

- 对本地 SQLite 桌面应用偏重。
- 仍会引入连接池、宏、离线元数据等额外复杂度。
- 用户已倾向 `tokio-rusqlite + rusqlite`。

结论：不采用。

## 实施风险与缓解

风险：一次性替换导致 service 层类型大面积破坏。

缓解：新 model 尽量保持 `accounts::Model`、`emails::Model` 等路径和字段名，先完成行为等价替换。

风险：手写 SQL 出现字段遗漏。

缓解：`schema.sql`、model、row mapper 三者按表逐一实现；每迁移一个 repository 运行对应测试。

风险：动态 IN 查询参数错误或 SQL 注入。

缓解：只拼接占位符，不拼接用户输入；值全部通过 rusqlite 参数绑定。

风险：事务跨 await 错误使用。

缓解：事务只在 `Connection::call` 闭包内同步创建和提交，闭包内部不允许 await。

风险：FTS 触发器再次出错。

缓解：`schema.sql` 使用当前已验证的 FTS5 delete command 形式，并保留 schema 测试覆盖 insert/update/delete。

风险：启动删除数据库导致开发误删数据。

缓解：这是用户明确选择的行为；代码中用清晰函数名表达 `reset_database_file`，日志明确输出删除和重建动作。

## 完成标准

实现完成后应满足：

1. `src-tauri/Cargo.toml` 不再包含 `sea-orm`、`sea-orm-migration`。
2. `src-tauri/migration` crate 被移除，workspace/build 不再引用它。
3. `src-tauri/src/infrastructure/storage/entities` 被普通 model 替代。
4. `src-tauri/sql/schema.sql` 存在并被生产初始化和测试共同使用。
5. `rtk bun run test:rust` 通过。
6. `rtk bun run test:frontend` 通过。
7. `rtk bun run check` 通过。
8. 本次触及 Rust 文件通过 `rtk rustfmt --edition 2024 --check`。
9. 全仓库搜索不到业务代码对 `sea_orm`、`sea-orm`、`sea_orm_migration` 的依赖。

