# Phase 0: 项目基建

> 前置：空 Tauri 2 + SvelteKit 模板项目
> 完成标志：所有依赖安装完毕，模块骨架编译通过，前端 Tailwind 工作正常

---

### Task 0.1: 配置 Tauri 窗口

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**Step 1: 修改窗口配置为 1600x1000 无边框**

将 `windows` 配置改为：

```json
"windows": [
  {
    "title": "Postium Mail",
    "width": 1600,
    "height": 1000,
    "decorations": false
  }
]
```

**Step 2: 验证**

Run: `cd D:\Xuan\postium-mail && bun run tauri dev`
Expected: 窗口以无边框模式打开，无系统标题栏

**Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "chore: set window to 1600x1000 frameless"
```

---

### Task 0.2: 更新 Cargo.toml 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 替换 dependencies 为完整依赖列表**

```toml
[dependencies]
# Tauri
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-opener = "2"
tauri-plugin-keyring = "0.1"

# Specta (类型绑定)
specta = { version = "=2.0.0-rc.22", features = ["chrono"] }
specta-typescript = "0.0.9"
tauri-specta = { version = "=2.0.0-rc.20", features = ["derive", "typescript"] }

# Database
sea-orm = { version = "2.0.0-rc.35", features = ["macros", "runtime-tokio-rustls", "sqlx-sqlite"] }
sea-orm-migration = "2.0.0-rc.35"
postium-mail-migration = { path = "../migration" }

# Email Protocols
async-imap = { version = "0.11", default-features = false, features = ["runtime-tokio"] }
async-native-tls = "0.6"
lettre = { version = "0.11", features = ["tokio1-native-tls"] }
mail-parser = "0.11"

# OAuth
oauth2 = { version = "5", features = ["reqwest"] }

# Async
tokio = { version = "1", features = ["full"] }
tokio-native-tls = "0.3"
async-trait = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
reqwest = { version = "0.12", features = ["json"] }
url = "2"
uuid = { version = "1", features = ["v4"] }
once_cell = "1"
regex = "1"
```

**Step 2: 验证编译**

Run: `cd D:\Xuan\postium-mail\src-tauri && cargo check`
Expected: 编译成功（可能有 unused warnings，正常。注意 migration 模块在 Task 0.6 后才能通过）

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add all backend dependencies"
```

---

### Task 0.3: 安装前端依赖

**Files:**
- Modify: `package.json`

**Step 1: 安装核心依赖**

Run:
```bash
cd D:\Xuan\postium-mail
bun add -d tailwindcss @tailwindcss/vite
bun add -d bits-ui
bun add clsx tailwind-merge tailwind-variants
bun add lucide-svelte
bun add @tiptap/core @tiptap/starter-kit @tiptap/extension-placeholder @tiptap/pm
```

**Step 2: 初始化 shadcn-svelte**

Run: `npx shadcn-svelte@next init`

选择：
- Style: Default
- Base color: Slate
- CSS variables: Yes

**Step 3: 安装常用 shadcn-svelte 组件**

Run:
```bash
npx shadcn-svelte@next add button input dialog dropdown-menu avatar badge scroll-area separator tooltip popover tabs progress skeleton
```

**Step 4: 验证**

Run: `bun run dev`
Expected: Vite 开发服务器正常启动

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: setup frontend deps, shadcn-svelte, tailwind v4"
```

---

### Task 0.4: 配置 Tailwind CSS v4

**Files:**
- Modify: `vite.config.ts`
- Create/Modify: `src/app.css`

**Step 1: 在 vite.config.ts 添加 Tailwind 插件**

```typescript
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
});
```

**Step 2: 创建全局 CSS `src/app.css`**

```css
@import 'tailwindcss';

@theme {
  --color-background: hsl(0 0% 100%);
  --color-foreground: hsl(222.2 84% 4.9%);
  --color-card: hsl(0 0% 100%);
  --color-card-foreground: hsl(222.2 84% 4.9%);
  --color-popover: hsl(0 0% 100%);
  --color-popover-foreground: hsl(222.2 84% 4.9%);
  --color-primary: hsl(222.2 47.4% 11.2%);
  --color-primary-foreground: hsl(210 40% 98%);
  --color-secondary: hsl(210 40% 96.1%);
  --color-secondary-foreground: hsl(222.2 47.4% 11.2%);
  --color-muted: hsl(210 40% 96.1%);
  --color-muted-foreground: hsl(215.4 16.3% 46.9%);
  --color-accent: hsl(210 40% 96.1%);
  --color-accent-foreground: hsl(222.2 47.4% 11.2%);
  --color-destructive: hsl(0 84.2% 60.2%);
  --color-destructive-foreground: hsl(210 40% 98%);
  --color-border: hsl(214.3 31.8% 91.4%);
  --color-input: hsl(214.3 31.8% 91.4%);
  --color-ring: hsl(222.2 84% 4.9%);
  --radius: 0.5rem;
}

.dark {
  --color-background: hsl(222.2 84% 4.9%);
  --color-foreground: hsl(210 40% 98%);
  --color-card: hsl(222.2 84% 4.9%);
  --color-card-foreground: hsl(210 40% 98%);
  --color-popover: hsl(222.2 84% 4.9%);
  --color-popover-foreground: hsl(210 40% 98%);
  --color-primary: hsl(210 40% 98%);
  --color-primary-foreground: hsl(222.2 47.4% 11.2%);
  --color-secondary: hsl(217.2 32.6% 17.5%);
  --color-secondary-foreground: hsl(210 40% 98%);
  --color-muted: hsl(217.2 32.6% 17.5%);
  --color-muted-foreground: hsl(215 20.2% 65.1%);
  --color-accent: hsl(217.2 32.6% 17.5%);
  --color-accent-foreground: hsl(210 40% 98%);
  --color-destructive: hsl(0 62.8% 30.6%);
  --color-destructive-foreground: hsl(210 40% 98%);
  --color-border: hsl(217.2 32.6% 17.5%);
  --color-input: hsl(217.2 32.6% 17.5%);
  --color-ring: hsl(212.7 26.8% 83.9%);
}

@layer base {
  * {
    @apply border-border;
  }
  body {
    @apply bg-background text-foreground;
    font-family: 'Inter', system-ui, sans-serif;
  }
}
```

确保 `+layout.svelte` 中 import 了 `../app.css`。

**Step 3: 验证**

在 `+page.svelte` 中临时测试：`<div class="p-4 text-primary bg-secondary">Tailwind v4 works!</div>`
Run: `bun run dev` → 页面显示带样式文字

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: configure tailwind css v4 with theme"
```

---

### Task 0.5: 创建后端模块骨架

**Files:**
- Create: `src-tauri/src/error/mod.rs` + `types.rs`
- Create: `src-tauri/src/command/mod.rs`
- Create: `src-tauri/src/service/mod.rs`
- Create: `src-tauri/src/domain/mod.rs` + `auth/mod.rs` + `providers/mod.rs` + `providers/traits.rs` + `providers/personal/mod.rs` + `providers/enterprise/mod.rs` + `sync/mod.rs`
- Create: `src-tauri/src/infrastructure/mod.rs` + `storage/mod.rs` + `protocols/mod.rs` + `sys/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建模块文件**

`error/mod.rs`:
```rust
pub mod types;
```

其余所有 `mod.rs` 内容为空注释（如 `// Command 层`）。

`domain/mod.rs`:
```rust
pub mod auth;
pub mod providers;
pub mod sync;
```

`domain/providers/mod.rs`:
```rust
pub mod traits;
pub mod personal;
pub mod enterprise;
```

`infrastructure/mod.rs`:
```rust
pub mod storage;
pub mod protocols;
pub mod sys;
```

**Step 2: 更新 lib.rs**

```rust
pub mod error;
pub mod command;
pub mod service;
pub mod domain;
pub mod infrastructure;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: 验证编译**

Run: `cd D:\Xuan\postium-mail\src-tauri && cargo check`
Expected: 成功，只有 unused warnings

**Step 4: Commit**

```bash
git add src-tauri/src/
git commit -m "chore: create backend module skeleton"
```

---

### Task 0.6: 创建 SeaORM Migration 项目

**Files:**
- Create: `src-tauri/migration/Cargo.toml`
- Create: `src-tauri/migration/src/lib.rs`
- Create: `src-tauri/migration/src/m20260331_000001_create_accounts.rs`
- Create: `src-tauri/migration/src/m20260331_000002_create_emails.rs`
- Create: `src-tauri/migration/src/m20260331_000003_create_attachments.rs`
- Create: `src-tauri/migration/src/m20260331_000004_create_sync_state.rs`
- Create: `src-tauri/migration/src/m20260331_000005_create_sync_errors.rs`
- Create: `src-tauri/migration/src/m20260331_000006_create_folders.rs`
- Create: `src-tauri/migration/src/m20260331_000007_create_fts.rs`

**Step 1: 创建 migration/Cargo.toml**

```toml
[package]
name = "postium-mail-migration"
version = "0.1.0"
edition = "2021"

[lib]
name = "postium_mail_migration"
crate-type = ["lib"]

[dependencies]
sea-orm-migration = { version = "2.0.0-rc.35", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
```

**Step 2: 创建 migration/src/lib.rs**

```rust
pub use sea_orm_migration::prelude::*;

mod m20260331_000001_create_accounts;
mod m20260331_000002_create_emails;
mod m20260331_000003_create_attachments;
mod m20260331_000004_create_sync_state;
mod m20260331_000005_create_sync_errors;
mod m20260331_000006_create_folders;
mod m20260331_000007_create_fts;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260331_000001_create_accounts::Migration),
            Box::new(m20260331_000002_create_emails::Migration),
            Box::new(m20260331_000003_create_attachments::Migration),
            Box::new(m20260331_000004_create_sync_state::Migration),
            Box::new(m20260331_000005_create_sync_errors::Migration),
            Box::new(m20260331_000006_create_folders::Migration),
            Box::new(m20260331_000007_create_fts::Migration),
        ]
    }
}
```

**Step 3: 写 accounts migration**

`m20260331_000001_create_accounts.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Accounts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Accounts::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Accounts::Name).text().not_null())
                    .col(ColumnDef::new(Accounts::Email).text().not_null().unique())
                    .col(ColumnDef::new(Accounts::DisplayName).text())
                    .col(ColumnDef::new(Accounts::Provider).text().not_null())
                    .col(ColumnDef::new(Accounts::ImapHost).text())
                    .col(ColumnDef::new(Accounts::ImapPort).integer())
                    .col(ColumnDef::new(Accounts::ImapSsl).boolean().default(true))
                    .col(ColumnDef::new(Accounts::SmtpHost).text())
                    .col(ColumnDef::new(Accounts::SmtpPort).integer())
                    .col(ColumnDef::new(Accounts::SmtpSsl).boolean().default(true))
                    .col(ColumnDef::new(Accounts::Color).text())
                    .col(ColumnDef::new(Accounts::SyncEnabled).boolean().default(true))
                    .col(ColumnDef::new(Accounts::LastSyncAt).big_integer())
                    .col(ColumnDef::new(Accounts::AuthType).text().default("password"))
                    .col(ColumnDef::new(Accounts::OauthProvider).text())
                    .col(ColumnDef::new(Accounts::OauthExpiresAt).big_integer())
                    .col(ColumnDef::new(Accounts::AccountType).text().not_null().default("personal"))
                    .col(ColumnDef::new(Accounts::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Accounts::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(Index::create().if_not_exists().name("idx_accounts_email").table(Accounts::Table).col(Accounts::Email).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Accounts::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table, Id, Name, Email, DisplayName, Provider,
    ImapHost, ImapPort, ImapSsl, SmtpHost, SmtpPort, SmtpSsl,
    Color, SyncEnabled, LastSyncAt, AuthType, OauthProvider,
    OauthExpiresAt, AccountType, CreatedAt, UpdatedAt,
}
```

**Step 4: 写 emails migration**

`m20260331_000002_create_emails.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Emails::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Emails::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Emails::AccountId).integer().not_null())
                    .col(ColumnDef::new(Emails::Folder).text().not_null())
                    .col(ColumnDef::new(Emails::Uid).integer())
                    .col(ColumnDef::new(Emails::MessageId).text().unique())
                    .col(ColumnDef::new(Emails::Subject).text())
                    .col(ColumnDef::new(Emails::SenderName).text())
                    .col(ColumnDef::new(Emails::SenderEmail).text().not_null())
                    .col(ColumnDef::new(Emails::RecipientEmails).text().not_null())
                    .col(ColumnDef::new(Emails::CcEmails).text())
                    .col(ColumnDef::new(Emails::BccEmails).text())
                    .col(ColumnDef::new(Emails::Preview).text())
                    .col(ColumnDef::new(Emails::BodyText).text())
                    .col(ColumnDef::new(Emails::BodyHtml).text())
                    .col(ColumnDef::new(Emails::IsRead).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsStarred).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsDraft).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsAnswered).boolean().default(false))
                    .col(ColumnDef::new(Emails::IsDeleted).boolean().default(false))
                    .col(ColumnDef::new(Emails::SentAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::ReceivedAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Emails::UpdatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_emails_account")
                            .from(Emails::Table, Emails::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_emails_account ON emails(account_id);").await?;
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder);").await?;
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_emails_sent_at ON emails(sent_at DESC);").await?;
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read);").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Emails::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Accounts { Table, Id }

#[derive(DeriveIden)]
enum Emails {
    Table, Id, AccountId, Folder, Uid, MessageId, Subject,
    SenderName, SenderEmail, RecipientEmails, CcEmails, BccEmails,
    Preview, BodyText, BodyHtml, IsRead, IsStarred, IsDraft,
    IsAnswered, IsDeleted, SentAt, ReceivedAt, CreatedAt, UpdatedAt,
}
```

**Step 5: 写 attachments migration**

`m20260331_000003_create_attachments.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Attachments::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Attachments::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Attachments::EmailId).integer().not_null())
                    .col(ColumnDef::new(Attachments::Filename).text().not_null())
                    .col(ColumnDef::new(Attachments::ContentType).text())
                    .col(ColumnDef::new(Attachments::Size).integer().not_null())
                    .col(ColumnDef::new(Attachments::Path).text())
                    .col(ColumnDef::new(Attachments::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create().name("fk_attachments_email")
                            .from(Attachments::Table, Attachments::EmailId)
                            .to(Emails::Table, Emails::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            ).await?;

        manager.create_index(Index::create().if_not_exists().name("idx_attachments_email").table(Attachments::Table).col(Attachments::EmailId).to_owned()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Attachments::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Emails { Table, Id }

#[derive(DeriveIden)]
enum Attachments { Table, Id, EmailId, Filename, ContentType, Size, Path, CreatedAt }
```

**Step 6: 写 sync_state migration**

`m20260331_000004_create_sync_state.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncState::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SyncState::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(SyncState::AccountId).integer().not_null())
                    .col(ColumnDef::new(SyncState::Folder).text().not_null())
                    .col(ColumnDef::new(SyncState::FolderNickName).text())
                    .col(ColumnDef::new(SyncState::Uidvalidity).big_integer())
                    .col(ColumnDef::new(SyncState::Uidnext).big_integer())
                    .col(ColumnDef::new(SyncState::SyncedAt).big_integer())
                    .col(ColumnDef::new(SyncState::LastSyncUid).big_integer())
                    .col(ColumnDef::new(SyncState::CreatedAt).big_integer())
                    .col(ColumnDef::new(SyncState::UpdatedAt).big_integer())
                    .foreign_key(
                        ForeignKey::create().name("fk_sync_state_account")
                            .from(SyncState::Table, SyncState::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(Index::create().unique().col(SyncState::AccountId).col(SyncState::Folder))
                    .to_owned(),
            ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(SyncState::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Accounts { Table, Id }

#[derive(DeriveIden)]
enum SyncState {
    Table, Id, AccountId, Folder, FolderNickName,
    Uidvalidity, Uidnext, SyncedAt, LastSyncUid, CreatedAt, UpdatedAt,
}
```

**Step 7: 写 sync_errors migration**

`m20260331_000005_create_sync_errors.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncErrors::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SyncErrors::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(SyncErrors::AccountId).integer().not_null())
                    .col(ColumnDef::new(SyncErrors::Folder).text())
                    .col(ColumnDef::new(SyncErrors::ErrorType).text().not_null())
                    .col(ColumnDef::new(SyncErrors::ErrorMessage).text().not_null())
                    .col(ColumnDef::new(SyncErrors::Uid).integer())
                    .col(ColumnDef::new(SyncErrors::StackTrace).text())
                    .col(ColumnDef::new(SyncErrors::Resolved).boolean().default(false))
                    .col(ColumnDef::new(SyncErrors::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create().name("fk_sync_errors_account")
                            .from(SyncErrors::Table, SyncErrors::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            ).await?;

        let db = manager.get_connection();
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_sync_errors_account ON sync_errors(account_id);").await?;
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_sync_errors_resolved ON sync_errors(resolved);").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(SyncErrors::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Accounts { Table, Id }

#[derive(DeriveIden)]
enum SyncErrors {
    Table, Id, AccountId, Folder, ErrorType, ErrorMessage,
    Uid, StackTrace, Resolved, CreatedAt,
}
```

**Step 8: 写 folders migration**

`m20260331_000006_create_folders.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Folders::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Folders::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Folders::AccountId).integer().not_null())
                    .col(ColumnDef::new(Folders::Name).text().not_null())
                    .col(ColumnDef::new(Folders::Delimiter).text())
                    .col(ColumnDef::new(Folders::ParentId).integer())
                    .col(ColumnDef::new(Folders::StandardFolder).text())
                    .col(ColumnDef::new(Folders::UnreadCount).integer().default(0))
                    .col(ColumnDef::new(Folders::TotalCount).integer().default(0))
                    .col(ColumnDef::new(Folders::SortOrder).integer().default(0))
                    .col(ColumnDef::new(Folders::CreatedAt).big_integer())
                    .col(ColumnDef::new(Folders::UpdatedAt).big_integer())
                    .foreign_key(
                        ForeignKey::create().name("fk_folders_account")
                            .from(Folders::Table, Folders::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Folders::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Accounts { Table, Id }

#[derive(DeriveIden)]
enum Folders {
    Table, Id, AccountId, Name, Delimiter, ParentId,
    StandardFolder, UnreadCount, TotalCount, SortOrder, CreatedAt, UpdatedAt,
}
```

**Step 9: 写 FTS5 migration**

`m20260331_000007_create_fts.rs`:
```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
                subject, sender_email, preview,
                content=emails, content_rowid=id
            );"
        ).await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_ai AFTER INSERT ON emails BEGIN
                INSERT INTO emails_fts(rowid, subject, sender_email, preview)
                VALUES (new.id, new.subject, new.sender_email, new.preview);
            END;"
        ).await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_ad AFTER DELETE ON emails BEGIN
                DELETE FROM emails_fts WHERE rowid = old.id;
            END;"
        ).await?;

        db.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS emails_fts_au AFTER UPDATE ON emails BEGIN
                UPDATE emails_fts SET subject = new.subject,
                    sender_email = new.sender_email, preview = new.preview
                WHERE rowid = new.id;
            END;"
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_au;").await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ad;").await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS emails_fts_ai;").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS emails_fts;").await
    }
}
```

**Step 10: 验证编译**

Run: `cd D:\Xuan\postium-mail\src-tauri && cargo check`
Expected: 编译成功

**Step 11: Commit**

```bash
git add -A
git commit -m "feat: add SeaORM migrations for all 7 tables"
```

---

### Task 0.7: 创建前端目录结构

**Files:**
- Create: `src/lib/bindings.ts`
- Create: `src/lib/utils.ts`
- Create: `src/lib/stores/*.svelte.ts` (5 个骨架)
- Create: `src/lib/components/layout/`
- Create: `src/lib/components/email/`
- Create: `src/lib/components/settings/`
- Create: `src/lib/components/calendar/`
- Create: `src/lib/components/workflow/`
- Create: `src/lib/i18n/`

**Step 1: 创建工具文件**

`src/lib/utils.ts`:
```typescript
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

`src/lib/bindings.ts`:
```typescript
// Auto-generated by tauri-specta — do not edit
export {};
```

**Step 2: 创建 store 骨架**

创建以下文件（内容为空注释）：
- `src/lib/stores/account.svelte.ts`
- `src/lib/stores/email.svelte.ts`
- `src/lib/stores/sync.svelte.ts`
- `src/lib/stores/theme.svelte.ts`
- `src/lib/stores/i18n.svelte.ts`

**Step 3: 创建组件目录**

```bash
mkdir -p src/lib/components/{layout,email,settings,calendar,workflow} src/lib/i18n
```

每个目录放一个 `.gitkeep` 文件。

**Step 4: 验证**

Run: `bun run build`
Expected: 构建成功

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: create frontend directory structure and store skeletons"
```
