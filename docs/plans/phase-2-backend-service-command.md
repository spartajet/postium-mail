# Phase 2: 后端 Service + Command 层

> 前置：Phase 1 完成
> 完成标志：所有 Tauri Command 注册完毕，tauri-specta 自动生成 TypeScript 绑定，`cargo check` 通过

---

### Task 2.1: SeaORM Entity 生成 + Database 连接

**Files:**
- Create: `src-tauri/src/infrastructure/storage/database.rs`
- Create: `src-tauri/src/infrastructure/storage/entities/` (目录，由 sea-orm-cli 生成)
- Modify: `src-tauri/src/infrastructure/storage/mod.rs`

**Step 1: 用 sea-orm-cli 生成 Entity**

先运行迁移创建 SQLite 数据库，然后生成 entity：

```bash
cd D:\Xuan\postium-mail\src-tauri
# 创建临时数据库用于生成 entity
mkdir -p .temp
echo "sqlite://.temp/postium.db" > .env
sea-orm-cli generate entity -o src/infrastructure/storage/entities --with-serde both --date-time-crate chrono
```

如果 `sea-orm-cli` 不可用，手动写 entity。创建以下文件：

**Step 2: 手动写 entity 文件**

`src-tauri/src/infrastructure/storage/entities/mod.rs`:
```rust
pub mod accounts;
pub mod emails;
pub mod attachments;
pub mod sync_state;
pub mod sync_errors;
pub mod folders;
```

每个 entity 文件用 `sea_orm::entity::prelude::*` 宏。示例 `accounts.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, specta::Type)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
    pub last_sync_at: Option<i64>,
    pub auth_type: Option<String>,
    pub oauth_provider: Option<String>,
    pub oauth_expires_at: Option<i64>,
    pub account_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::emails::Entity")]
    Emails,
    #[sea_orm(has_many = "super::sync_state::Entity")]
    SyncState,
    #[sea_orm(has_many = "super::sync_errors::Entity")]
    SyncErrors,
    #[sea_orm(has_many = "super::folders::Entity")]
    Folders,
}

impl Related<super::emails::Entity> for Entity {
    fn to() -> RelationDef { Relation::Emails.def() }
}

impl ActiveModelBehavior for ActiveModel {}
```

对其余 entity (`emails`, `attachments`, `sync_state`, `sync_errors`, `folders`) 按同样模式编写，字段参考 Phase 0 的 migration。

**Step 3: 写 database.rs**

```rust
use sea_orm::{Database, DatabaseConnection, ConnectOptions};
use std::time::Duration;

pub type DbConn = DatabaseConnection;

/// 初始化数据库连接并运行迁移
pub async fn init_database(data_dir: &std::path::Path) -> Result<DbConn, sea_orm::DbErr> {
    let db_path = data_dir.join("postium.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let mut opt = ConnectOptions::new(&db_url);
    opt.max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    let db = Database::connect(opt).await?;

    // 运行迁移
    postium_mail_migration::Migrator::up(&db, None).await?;

    Ok(db)
}
```

**Step 4: 更新 storage/mod.rs**

```rust
pub mod database;
pub mod entities;
pub mod repository;

pub use database::DbConn;
```

**Step 5: 验证**

Run: `cargo check`

**Step 6: Commit**

```bash
git add src-tauri/src/infrastructure/storage/
git commit -m "feat: add SeaORM entities and database initialization"
```

---

### Task 2.2: 实现 Repository 层

**Files:**
- Create: `src-tauri/src/infrastructure/storage/repository/mod.rs`
- Create: `src-tauri/src/infrastructure/storage/repository/account_repo.rs`
- Create: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
- Create: `src-tauri/src/infrastructure/storage/repository/sync_repo.rs`
- Create: `src-tauri/src/infrastructure/storage/repository/folder_repo.rs`
- Create: `src-tauri/src/infrastructure/storage/search.rs`

**Step 1: 写 account_repo.rs**

```rust
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::entities::{accounts, emails, sync_state, folders};
use crate::error::MailError;
use sea_orm::*;

pub struct AccountRepo(pub DbConn);

impl AccountRepo {
    pub async fn list(&self) -> Result<Vec<accounts::Model>, MailError> {
        Ok(accounts::Entity::find()
            .order_by_asc(accounts::Column::Id)
            .all(&self.0)
            .await?)
    }

    pub async fn get_by_id(&self, id: i32) -> Result<Option<accounts::Model>, MailError> {
        Ok(accounts::Entity::find_by_id(id).one(&self.0).await?)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<accounts::Model>, MailError> {
        Ok(accounts::Entity::find()
            .filter(accounts::Column::Email.eq(email))
            .one(&self.0)
            .await?)
    }

    pub async fn create(&self, model: accounts::ActiveModel) -> Result<accounts::Model, MailError> {
        Ok(model.insert(&self.0).await?)
    }

    pub async fn update(&self, id: i32, model: accounts::ActiveModel) -> Result<accounts::Model, MailError> {
        let mut model = model;
        model.id = Set(id);
        Ok(model.update(&self.0).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<(), MailError> {
        accounts::Entity::delete_by_id(id).exec(&self.0).await?;
        Ok(())
    }

    pub async fn update_last_sync(&self, id: i32) -> Result<(), MailError> {
        accounts::Entity::update_many()
            .col_expr(accounts::Column::LastSyncAt, Expr::value(chrono::Utc::now().timestamp()))
            .col_expr(accounts::Column::UpdatedAt, Expr::value(chrono::Utc::now().timestamp()))
            .filter(accounts::Column::Id.eq(id))
            .exec(&self.0)
            .await?;
        Ok(())
    }
}
```

**Step 2: 写 email_repo.rs**

```rust
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::entities::emails;
use crate::error::MailError;
use sea_orm::*;

pub struct EmailRepo(pub DbConn);

impl EmailRepo {
    /// 分页获取邮件列表（不含 body）
    pub async fn list_by_folder(
        &self,
        account_id: i32,
        folder: &str,
        page: usize,
        limit: usize,
    ) -> Result<(Vec<emails::Model>, u64), MailError> {
        let query = emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_id))
            .filter(emails::Column::Folder.eq(folder))
            .filter(emails::Column::IsDeleted.eq(false));

        let total = query.clone().count(&self.0).await?;

        let items = query
            .order_by_desc(emails::Column::SentAt)
            .offset(Some(((page - 1) * limit) as u64))
            .limit(Some(limit as u64))
            .all(&self.0)
            .await?;

        Ok((items, total))
    }

    /// 获取单个邮件（含 body）
    pub async fn get_by_id(&self, id: i32) -> Result<Option<emails::Model>, MailError> {
        Ok(emails::Entity::find_by_id(id).one(&self.0).await?)
    }

    /// 按 UID 查找
    pub async fn get_by_uid(&self, account_id: i32, folder: &str, uid: u32) -> Result<Option<emails::Model>, MailError> {
        Ok(emails::Entity::find()
            .filter(emails::Column::AccountId.eq(account_id))
            .filter(emails::Column::Folder.eq(folder))
            .filter(emails::Column::Uid.eq(uid as i32))
            .one(&self.0)
            .await?)
    }

    /// 批量插入
    pub async fn bulk_insert(&self, models: Vec<emails::ActiveModel>) -> Result<(), MailError> {
        if models.is_empty() { return Ok(()); }
        emails::Entity::insert_many(models).exec(&self.0).await?;
        Ok(())
    }

    /// 更新邮件
    pub async fn update(&self, id: i32, model: emails::ActiveModel) -> Result<emails::Model, MailError> {
        let mut model = model;
        model.id = Set(id);
        Ok(model.update(&self.0).await?)
    }

    /// 标记已读
    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), MailError> {
        emails::Entity::update_many()
            .col_expr(emails::Column::IsRead, Expr::value(is_read))
            .filter(emails::Column::Id.eq(id))
            .exec(&self.0).await?;
        Ok(())
    }

    /// 切换星标
    pub async fn toggle_star(&self, id: i32) -> Result<bool, MailError> {
        let email = emails::Entity::find_by_id(id).one(&self.0).await?
            .ok_or(MailError::EmailNotFound(id))?;
        let new_state = !email.is_starred.unwrap_or(false);
        emails::Entity::update_many()
            .col_expr(emails::Column::IsStarred, Expr::value(new_state))
            .filter(emails::Column::Id.eq(id))
            .exec(&self.0).await?;
        Ok(new_state)
    }

    /// 删除邮件（软删除）
    pub async fn soft_delete(&self, ids: Vec<i32>) -> Result<usize, MailError> {
        let result = emails::Entity::update_many()
            .col_expr(emails::Column::IsDeleted, Expr::value(true))
            .filter(emails::Column::Id.is_in(ids))
            .exec(&self.0).await?;
        Ok(result.rows_affected as usize)
    }

    /// 移动邮件到其他文件夹
    pub async fn move_to_folder(&self, id: i32, folder: &str) -> Result<(), MailError> {
        emails::Entity::update_many()
            .col_expr(emails::Column::Folder, Expr::value(folder.to_string()))
            .filter(emails::Column::Id.eq(id))
            .exec(&self.0).await?;
        Ok(())
    }
}
```

**Step 3: 写 sync_repo.rs + folder_repo.rs**

类似模式，包含 `upsert_sync_state`、`log_error`、`list_folders`、`update_counts` 等方法。

**Step 4: 写 search.rs (FTS5)**

```rust
use crate::infrastructure::storage::DbConn;
use crate::error::MailError;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchResult {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub subject: Option<String>,
    pub sender_email: String,
    pub sent_at: i64,
    pub preview: Option<String>,
    pub rank: f64,
}

pub async fn search_fts(
    db: &DbConn,
    query: &str,
    account_id: Option<i32>,
    limit: u64,
) -> Result<Vec<SearchResult>, MailError> {
    let sql = if account_id.is_some() {
        "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank \
         FROM emails_fts f JOIN emails e ON f.rowid = e.id \
         WHERE emails_fts MATCH ? AND e.account_id = ? AND e.is_deleted = 0 \
         ORDER BY f.rank DESC LIMIT ?"
    } else {
        "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank \
         FROM emails_fts f JOIN emails e ON f.rowid = e.id \
         WHERE emails_fts MATCH ? AND e.is_deleted = 0 \
         ORDER BY f.rank DESC LIMIT ?"
    };

    // 使用 sea_orm 的 raw SQL 查询
    let results = if let Some(aid) = account_id {
        sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            sql,
            [query.into(), aid.into(), limit.into()],
        )
    } else {
        sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            sql,
            [query.into(), limit.into()],
        )
    };

    let rows = db.query_all(results).await?;
    let mut out = Vec::new();
    for row in rows {
        let r = SearchResult {
            id: row.try_get_by_index(0)?,
            account_id: row.try_get_by_index(1)?,
            folder: row.try_get_by_index(2)?,
            subject: row.try_get_by_index(3)?,
            sender_email: row.try_get_by_index(4)?,
            sent_at: row.try_get_by_index(5)?,
            preview: row.try_get_by_index(6)?,
            rank: row.try_get_by_index(7)?,
        };
        out.push(r);
    }
    Ok(out)
}
```

**Step 5: 验证**

Run: `cargo check`

**Step 6: Commit**

```bash
git add src-tauri/src/infrastructure/storage/
git commit -m "feat: implement repository layer and FTS5 search"
```

---

### Task 2.3: 实现 AccountService

**Files:**
- Create: `src-tauri/src/service/account_service.rs`
- Modify: `src-tauri/src/service/mod.rs`

**Step 1: 写 DTO 类型**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::infrastructure::storage::entities::accounts;

// ─── DTO ───

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AccountDto {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub color: Option<String>,
    pub sync_enabled: bool,
    pub auth_type: String,
    pub account_type: String,
    pub last_sync_at: Option<i64>,
    pub created_at: i64,
}

impl From<accounts::Model> for AccountDto {
    fn from(m: accounts::Model) -> Self {
        Self {
            id: m.id, name: m.name, email: m.email,
            display_name: m.display_name, provider: m.provider,
            color: m.color, sync_enabled: m.sync_enabled.unwrap_or(true),
            auth_type: m.auth_type.unwrap_or_else(|| "password".into()),
            account_type: m.account_type, last_sync_at: m.last_sync_at,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateAccountRequest {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub auth_type: String,
    pub password: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub color: Option<String>,
    pub account_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAccountRequest {
    pub id: i32,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
}
```

**Step 2: 写 AccountService**

```rust
use crate::domain::auth::AuthManager;
use crate::domain::providers::detect;
use crate::error::MailError;
use crate::infrastructure::storage::repository::account_repo::AccountRepo;
use crate::infrastructure::storage::entities::accounts;
use sea_orm::Set;
use std::sync::Arc;

pub struct AccountService {
    repo: AccountRepo,
    auth: Arc<AuthManager>,
}

impl AccountService {
    pub fn new(repo: AccountRepo, auth: Arc<AuthManager>) -> Self {
        Self { repo, auth }
    }

    pub async fn list(&self) -> Result<Vec<super::account_service::AccountDto>, MailError> {
        let accounts = self.repo.list().await?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    pub async fn get(&self, id: i32) -> Result<super::account_service::AccountDto, MailError> {
        let account = self.repo.get_by_id(id).await?
            .ok_or(MailError::AccountNotFound(id))?;
        Ok(account.into())
    }

    pub async fn create(&self, req: super::account_service::CreateAccountRequest) -> Result<super::account_service::AccountDto, MailError> {
        let now = chrono::Utc::now().timestamp();

        // 自动检测服务商（如果未指定 imap/smtp）
        let detected = detect::detect_provider(&req.email);

        let model = accounts::ActiveModel {
            name: Set(req.name),
            email: Set(req.email),
            display_name: Set(req.display_name),
            provider: Set(req.provider),
            imap_host: Set(req.imap_host),
            imap_port: Set(req.imap_port),
            smtp_host: Set(req.smtp_host),
            smtp_port: Set(req.smtp_port),
            color: Set(req.color),
            auth_type: Set(Some(req.auth_type)),
            account_type: Set(req.account_type.unwrap_or_else(|| "personal".into())),
            ..Default::default()
        };

        let account = self.repo.create(model).await?;

        // 保存密码到 Keyring
        self.auth.save_password(&account.email, &req.password).await?;

        Ok(account.into())
    }

    pub async fn update(&self, req: super::account_service::UpdateAccountRequest) -> Result<super::account_service::AccountDto, MailError> {
        let existing = self.repo.get_by_id(req.id).await?
            .ok_or(MailError::AccountNotFound(req.id))?;

        let mut model: accounts::ActiveModel = existing.into();
        if let Some(name) = req.name { model.name = Set(name); }
        if let Some(display_name) = req.display_name { model.display_name = Set(Some(display_name)); }
        if let Some(color) = req.color { model.color = Set(Some(color)); }
        if let Some(sync_enabled) = req.sync_enabled { model.sync_enabled = Set(Some(sync_enabled)); }
        model.updated_at = Set(chrono::Utc::now().timestamp());

        let account = self.repo.update(req.id, model).await?;
        Ok(account.into())
    }

    pub async fn delete(&self, id: i32) -> Result<(), MailError> {
        let account = self.repo.get_by_id(id).await?
            .ok_or(MailError::AccountNotFound(id))?;

        self.repo.delete(id).await?;

        // 从 Keyring 删除密码
        let _ = self.auth.delete_password(&account.email).await;

        Ok(())
    }
}
```

**Step 3: 更新 service/mod.rs**

```rust
pub mod account_service;
pub mod email_service;
pub mod sync_service;

pub use account_service::{AccountDto, CreateAccountRequest, UpdateAccountRequest, AccountService};
```

**Step 4: 验证**

Run: `cargo check`

**Step 5: Commit**

```bash
git add src-tauri/src/service/
git commit -m "feat: implement AccountService with DTOs"
```

---

### Task 2.4: 实现 EmailService

**Files:**
- Create: `src-tauri/src/service/email_service.rs`

**Step 1: 写 DTO + Service**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDto {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: Option<i32>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub preview: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub sent_at: i64,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDetail {
    #[serde(flatten)]
    pub email: EmailDto,
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailListResponse {
    pub emails: Vec<EmailDto>,
    pub total: u64,
    pub page: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

pub struct EmailService {
    email_repo: crate::infrastructure::storage::repository::email_repo::EmailRepo,
    account_repo: crate::infrastructure::storage::repository::account_repo::AccountRepo,
    auth: std::sync::Arc<crate::domain::auth::AuthManager>,
    db: crate::infrastructure::storage::DbConn,
}

impl EmailService {
    pub fn new(
        email_repo: crate::infrastructure::storage::repository::email_repo::EmailRepo,
        account_repo: crate::infrastructure::storage::repository::account_repo::AccountRepo,
        auth: std::sync::Arc<crate::domain::auth::AuthManager>,
        db: crate::infrastructure::storage::DbConn,
    ) -> Self {
        Self { email_repo, account_repo, auth, db }
    }

    pub async fn list(&self, account_id: i32, folder: &str, page: usize, limit: usize) -> Result<EmailListResponse, crate::error::MailError> {
        let (emails, total) = self.email_repo.list_by_folder(account_id, folder, page, limit).await?;
        Ok(EmailListResponse {
            emails: emails.into_iter().map(|e| EmailDto {
                id: e.id, account_id: e.account_id, folder: e.folder,
                uid: e.uid, subject: e.subject, sender_name: e.sender_name,
                sender_email: e.sender_email, preview: e.preview,
                is_read: e.is_read.unwrap_or(false), is_starred: e.is_starred.unwrap_or(false),
                sent_at: e.sent_at, has_attachments: false,
            }).collect(),
            total, page, limit,
        })
    }

    pub async fn get(&self, id: i32) -> Result<EmailDetail, crate::error::MailError> {
        let email = self.email_repo.get_by_id(id).await?
            .ok_or(crate::error::MailError::EmailNotFound(id))?;
        Ok(EmailDetail {
            email: EmailDto {
                id: email.id, account_id: email.account_id, folder: email.folder,
                uid: email.uid, subject: email.subject, sender_name: email.sender_name,
                sender_email: email.sender_email, preview: email.preview,
                is_read: email.is_read.unwrap_or(false), is_starred: email.is_starred.unwrap_or(false),
                sent_at: email.sent_at, has_attachments: false,
            },
            recipient_emails: email.recipient_emails,
            cc_emails: email.cc_emails,
            body_text: email.body_text,
            body_html: email.body_html,
        })
    }

    pub async fn search(&self, query: &str, account_id: Option<i32>, limit: Option<u64>) -> Result<Vec<crate::infrastructure::storage::search::SearchResult>, crate::error::MailError> {
        crate::infrastructure::storage::search::search_fts(&self.db, query, account_id, limit.unwrap_or(50)).await
    }

    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), crate::error::MailError> {
        self.email_repo.mark_as_read(id, is_read).await
    }

    pub async fn toggle_star(&self, id: i32) -> Result<bool, crate::error::MailError> {
        self.email_repo.toggle_star(id).await
    }

    pub async fn delete(&self, ids: Vec<i32>) -> Result<usize, crate::error::MailError> {
        self.email_repo.soft_delete(ids).await
    }

    pub async fn move_to_folder(&self, id: i32, folder: &str) -> Result<(), crate::error::MailError> {
        self.email_repo.move_to_folder(id, folder).await
    }

    pub async fn send(&self, req: SendEmailRequest) -> Result<String, crate::error::MailError> {
        let account = self.account_repo.get_by_id(req.account_id).await?
            .ok_or(crate::error::MailError::AccountNotFound(req.account_id))?;

        let password = self.auth.get_password(&account.email).await?;

        let provider = self.auth.get_provider(&account.provider)
            .ok_or(crate::error::MailError::ProviderNotSupported(account.provider.clone()))?;

        let smtp_config = provider.smtp_config(&account.email);
        let from = account.display_name
            .map(|n| format!("{} <{}>", n, account.email))
            .unwrap_or_else(|| account.email.clone());

        crate::infrastructure::protocols::smtp::send_email(
            &smtp_config, &account.email, &password,
            &from, &req.to, &req.cc, &req.bcc,
            &req.subject, &req.body_html, &req.body_text,
        ).await
    }
}
```

**Step 2: 验证**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src-tauri/src/service/email_service.rs
git commit -m "feat: implement EmailService with CRUD and SMTP send"
```

---

### Task 2.5: 实现 SyncService

**Files:**
- Create: `src-tauri/src/service/sync_service.rs`

**Step 1: 写 SyncService**

```rust
use crate::domain::sync::types::{SyncProgress, SyncResult, FolderStat, SyncStage};
use crate::domain::sync::progress::SyncProgressEmitter;
use crate::error::MailError;
use std::sync::Arc;
use tauri::AppHandle;

pub struct SyncService {
    db: crate::infrastructure::storage::DbConn,
    auth: Arc<crate::domain::auth::AuthManager>,
}

impl SyncService {
    pub fn new(db: crate::infrastructure::storage::DbConn, auth: Arc<crate::domain::auth::AuthManager>) -> Self {
        Self { db, auth }
    }

    /// 同步账号 — 带 Tauri 事件进度通知
    pub async fn sync_account_with_progress(&self, app_handle: AppHandle, account_id: i32) -> Result<(), MailError> {
        let emitter = SyncProgressEmitter::new(app_handle);

        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::Connecting,
            folder: None, current: 0, total: 0,
            message: "正在连接...".into(),
        });

        // TODO: Phase 2 后续实现完整同步逻辑
        // 目前只做骨架，返回 stub

        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::Completed,
            folder: None, current: 0, total: 0,
            message: "同步完成".into(),
        });

        Ok(())
    }

    /// 同步账号 — 静默模式
    pub async fn sync_account(&self, account_id: i32) -> Result<SyncResult, MailError> {
        // TODO: 完整同步逻辑
        Ok(SyncResult {
            new_emails: 0, updated_emails: 0, deleted_emails: 0, duration_ms: 0,
        })
    }

    /// 获取文件夹统计
    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>, MailError> {
        // TODO: 从 DB 查询
        Ok(vec![])
    }
}
```

**Step 2: 验证**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src-tauri/src/service/sync_service.rs
git commit -m "feat: implement SyncService skeleton with progress emission"
```

---

### Task 2.6: 实现 Command 层 (Tauri Commands)

**Files:**
- Create: `src-tauri/src/command/account.rs`
- Create: `src-tauri/src/command/email.rs`
- Create: `src-tauri/src/command/sync.rs`
- Create: `src-tauri/src/command/auth.rs`
- Modify: `src-tauri/src/command/mod.rs`

**Step 1: 写 command/account.rs**

```rust
use crate::error::MailError;
use crate::service::{AccountDto, CreateAccountRequest, UpdateAccountRequest};

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    service: tauri::State<'_, crate::service::account_service::AccountService>,
) -> Result<Vec<AccountDto>, MailError> {
    service.list().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_account(
    service: tauri::State<'_, crate::service::account_service::AccountService>,
    id: i32,
) -> Result<AccountDto, MailError> {
    service.get(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    service: tauri::State<'_, crate::service::account_service::AccountService>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    service.create(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    service: tauri::State<'_, crate::service::account_service::AccountService>,
    request: UpdateAccountRequest,
) -> Result<AccountDto, MailError> {
    service.update(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_account(
    service: tauri::State<'_, crate::service::account_service::AccountService>,
    id: i32,
) -> Result<(), MailError> {
    service.delete(id).await
}
```

**Step 2: 写 command/email.rs**

```rust
use crate::error::MailError;
use crate::service::email_service::{EmailListResponse, EmailDetail, SendEmailRequest};
use crate::infrastructure::storage::search::SearchResult;

#[tauri::command]
#[specta::specta]
pub async fn list_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<EmailListResponse, MailError> {
    service.list(account_id, &folder, page, limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    id: i32,
) -> Result<EmailDetail, MailError> {
    service.get(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn search_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<SearchResult>, MailError> {
    service.search(&query, account_id, limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn mark_as_read(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    is_read: bool,
) -> Result<(), MailError> {
    service.mark_as_read(email_id, is_read).await
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_star(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<bool, MailError> {
    service.toggle_star(email_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_ids: Vec<i32>,
) -> Result<usize, MailError> {
    service.delete(email_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn move_email_to_folder(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    folder: String,
) -> Result<(), MailError> {
    service.move_to_folder(email_id, &folder).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    request: SendEmailRequest,
) -> Result<String, MailError> {
    service.send(request).await
}
```

**Step 3: 写 command/sync.rs**

```rust
use crate::error::MailError;
use crate::domain::sync::types::FolderStat;

#[tauri::command]
#[specta::specta]
pub async fn sync_account(
    service: tauri::State<'_, crate::service::sync_service::SyncService>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), MailError> {
    service.sync_account_with_progress(app_handle, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_folder_stats(
    service: tauri::State<'_, crate::service::sync_service::SyncService>,
    account_id: i32,
) -> Result<Vec<FolderStat>, MailError> {
    service.get_folder_stats(account_id).await
}
```

**Step 4: 写 command/auth.rs**

```rust
use crate::domain::providers::detect::ProviderDetectionResult;
use crate::domain::providers::traits::ProviderInfo;

#[tauri::command]
#[specta::specta]
pub async fn detect_provider(
    email: String,
) -> Result<ProviderDetectionResult, crate::error::MailError> {
    Ok(crate::domain::providers::detect::detect_provider(&email))
}

#[tauri::command]
#[specta::specta]
pub async fn list_providers() -> Result<Vec<ProviderInfo>, crate::error::MailError> {
    Ok(crate::domain::providers::detect::list_providers())
}
```

**Step 5: 更新 command/mod.rs**

```rust
pub mod account;
pub mod email;
pub mod sync;
pub mod auth;
```

**Step 6: 验证**

Run: `cargo check`

**Step 7: Commit**

```bash
git add src-tauri/src/command/
git commit -m "feat: implement all Tauri command handlers"
```

---

### Task 2.7: 配置 tauri-specta Builder + 更新 lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 重写 lib.rs**

```rust
pub mod error;
pub mod command;
pub mod service;
pub mod domain;
pub mod infrastructure;

use error::MailError;

#[cfg(debug_assertions)]
const EXPORT_DIR: &str = "../src/lib/bindings.ts";

pub fn run() {
    let db = tauri::async_runtime::block_on(async {
        let data_dir = dirs::data_local_dir()
            .expect("无法获取数据目录")
            .join("postium-mail");
        std::fs::create_dir_all(&data_dir).expect("无法创建数据目录");
        infrastructure::storage::database::init_database(&data_dir)
            .await
            .expect("数据库初始化失败")
    });

    let auth = std::sync::Arc::new(domain::auth::AuthManager::new());

    let account_service = service::account_service::AccountService::new(
        infrastructure::storage::repository::account_repo::AccountRepo(db.clone()),
        auth.clone(),
    );

    let email_service = service::email_service::EmailService::new(
        infrastructure::storage::repository::email_repo::EmailRepo(db.clone()),
        infrastructure::storage::repository::account_repo::AccountRepo(db.clone()),
        auth.clone(),
        db.clone(),
    );

    let sync_service = service::sync_service::SyncService::new(db, auth);

    // tauri-specta Builder
    let builder = tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            command::account::list_accounts,
            command::account::get_account,
            command::account::create_account,
            command::account::update_account,
            command::account::delete_account,
            command::email::list_emails,
            command::email::get_email,
            command::email::search_emails,
            command::email::mark_as_read,
            command::email::toggle_star,
            command::email::delete_emails,
            command::email::move_email_to_folder,
            command::email::send_email,
            command::sync::sync_account,
            command::sync::get_folder_stats,
            command::auth::detect_provider,
            command::auth::list_providers,
        ])
        .events(tauri_specta::collect_events![
            domain::sync::progress::SyncProgressEvent,
        ]);

    // Debug 模式导出 TypeScript 绑定
    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), EXPORT_DIR)
        .expect("导出 TypeScript 绑定失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(account_service)
        .manage(email_service)
        .manage(sync_service)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
```

**Step 2: 添加 dirs 依赖到 Cargo.toml**

```toml
dirs = "6"
```

**Step 3: 验证**

Run: `cargo check`

**Step 4: 验证 TypeScript 绑定生成**

Run: `cargo build` (debug mode)
Expected: `src/lib/bindings.ts` 被自动生成，包含所有 Command 函数签名和类型

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: wire up tauri-specta builder with all commands and state management"
```

---

### Task 2.8: 验证完整后端编译 + 绑定生成

**Step 1: 完整编译**

Run: `cd D:\Xuan\postium-mail\src-tauri && cargo build`

**Step 2: 检查生成的 bindings.ts**

Read: `src/lib/bindings.ts`
Expected: 包含 `listAccounts`, `getEmail`, `sendEmail` 等所有函数

**Step 3: 修复编译问题**

根据错误信息逐一修复。

**Step 4: 最终 Commit**

```bash
git add -A
git commit -m "feat: Phase 2 complete - backend Service + Command + specta bindings"
```
