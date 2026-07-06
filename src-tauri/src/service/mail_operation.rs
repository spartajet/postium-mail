//!
//! # 邮件操作服务（Mail Operation Service）
//!
//! 本文件实现"远程优先（remote-first）"的邮件操作模型，是邮件读/写状态变更的核心入口。
//! 所有改变邮件状态的操作（已读、星标、移动、归档、删除、重载、附件下载）都先在 IMAP
//! 远端执行，成功后再同步更新本地 SQLite 数据库。这样保证了多端一致性：本应用的修改对
//! 其他邮件客户端可见，本应用的本地状态也与服务器保持一致。
//!
//! ## 模块组织
//!
//! - [`MailRemoteOperator`]（trait）：抽象远程 IMAP 操作，便于在单元测试中替换为 mock
//! - [`RealMailRemoteOperator`]：基于真实 IMAP 连接的默认实现
//! - [`MailOperationService`]：对外暴露的服务层，协调"远端 + 本地"两阶段写操作
//!
//! ## 设计原则
//!
//! 1. **远程优先**：先改远端，再改本地。远端失败直接向上抛错，不污染本地数据
//! 2. **依赖注入**：通过 `MailRemoteOperator` trait 注入远端实现，便于测试
//! 3. **两阶段写**：远端写成功后立即写本地，二者之间不做事务（IMAP 不支持分布式事务），
//!    极端情况下可能出现远端已改、本地未改的窗口，由下次同步兜底修正
//!
//! ## 错误处理
//!
//! 远端 IMAP 错误（连接失败、文件夹不存在、UID 失效）会被转换为 [`MailError`] 上抛；
//! 本地数据库写失败同样上抛。调用方需根据具体变体决定是否提示用户重试或触发同步。
//!

use crate::domain::auth::AuthManager;
use crate::domain::auth::manager::Credentials;
use crate::domain::folders::{FolderCategory, FolderRegistry};
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::protocols::types::{FetchedBodySection, WholeEmailDto};
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::infrastructure::storage::repository::{account_repo, email_repo};
use crate::service::account_connection::imap_config_from_account;
use crate::service::email_service::local_folder_registry_inputs;
use async_trait::async_trait;
use std::sync::Arc;

/// 远程邮件操作抽象接口
///
/// 该 trait 封装所有直接作用于 IMAP 远端的邮件操作，目的有二：
/// 1. 对上层屏蔽 IMAP 连接/选文件夹/登出等样板流程
/// 2. 在单元测试中可替换为 mock 实现（如 [`NoopMailRemoteOperator`]/手写桩），
///    避免 service 层测试依赖真实邮件服务器
///
/// 所有方法都要求 `Send + Sync`，以便通过 `Arc<dyn MailRemoteOperator>` 跨线程共享。
#[async_trait]
pub trait MailRemoteOperator: Send + Sync {
    /// 设置/取消邮件的 `\Seen` 标志（已读/未读）
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError>;

    /// 设置/取消邮件的 `\Flagged` 标志（星标）
    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError>;

    /// 把邮件从当前文件夹移动到目标文件夹（IMAP MOVE / COPY+\Deleted）
    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError>;

    /// 按 UID 重新拉取一封完整邮件（含正文与附件元信息），用于"刷新"功能
    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError>;

    /// 下载指定 MIME section 的附件原始字节（带 Content-Type 等 MIME 头信息）
    async fn fetch_attachment_section(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError>;
}

/// 基于真实 IMAP 连接的远程操作实现
///
/// 持有 [`AuthManager`] 用于按账号获取凭证（密码或 OAuth2 access_token），
/// 每次操作都新建一条 IMAP 连接并在结束时登出。无连接池，适合低频的"用户主动操作"场景；
/// 高频批量同步应走同步管线（[`crate::domain::sync`]），不复用本类型。
pub struct RealMailRemoteOperator {
    /// 凭证管理器，按邮箱地址解析出密码或刷新后的 access_token
    auth: Arc<AuthManager>,
}

impl RealMailRemoteOperator {
    /// 创建一个使用指定 [`AuthManager`] 的真实远程操作器
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self { auth }
    }

    /// 为指定账号建立一条新的 IMAP 连接
    ///
    /// # 参数
    /// - account: 账号模型（含 provider、email、IMAP 配置等字段）
    ///
    /// # 返回
    /// - Ok(client): 已就绪、尚未 select 任何文件夹的 IMAP 客户端
    /// - Err(MailError):
    ///   - ProviderNotSupported: provider pool 未初始化，或该 provider 未注册
    ///   - 凭证获取/IMAP 连接相关的错误
    async fn connect_for_account(
        &self,
        account: &accounts::Model,
    ) -> Result<ImapClient, MailError> {
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let credentials = self
            .auth
            .get_credentials(
                &account.email,
                &provider.provider_info().auth_type,
                Some(&account.provider),
            )
            .await?;
        let imap_config = imap_config_from_account(account)?;

        match credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &account.email, &password).await
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await
            }
        }
    }
}

#[async_trait]
impl MailRemoteOperator for RealMailRemoteOperator {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if seen {
            client.add_flags(uid, "\\Seen").await?;
        } else {
            client.remove_flags(uid, "\\Seen").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if flagged {
            client.add_flags(uid, "\\Flagged").await?;
        } else {
            client.remove_flags(uid, "\\Flagged").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        client.move_uid_to_folder(uid, target_folder).await?;
        client.logout().await.ok();
        Ok(())
    }

    async fn reload_email(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
    ) -> Result<Option<WholeEmailDto>, MailError> {
        let mut client = self.connect_for_account(account).await?;
        let result = client.fetch_email_by_uid(folder, uid).await;
        client.logout().await.ok();
        result
    }

    async fn fetch_attachment_section(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        section_path: &str,
    ) -> Result<Option<FetchedBodySection>, MailError> {
        let mut client = self.connect_for_account(account).await?;
        let result = client
            .fetch_body_section_with_mime(folder, uid, section_path)
            .await;
        client.logout().await.ok();
        result
    }
}

pub struct MailOperationService {
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
}

impl MailOperationService {
    /// 创建邮件操作服务
    ///
    /// # 参数
    /// - db: 数据库连接（用于本地状态同步）
    /// - _auth: 历史遗留参数，当前实现在远端操作器内部持有 auth，此处保留以兼容调用方签名
    /// - remote: 注入的远程操作实现（生产用 [`RealMailRemoteOperator`]，测试用 mock）
    pub fn new(db: DbConn, _auth: Arc<AuthManager>, remote: Arc<dyn MailRemoteOperator>) -> Self {
        Self { db, remote }
    }

    /// 暴露远程操作器引用，供需要直接走 IMAP 的上层服务（如附件下载）复用
    pub(crate) fn remote(&self) -> Arc<dyn MailRemoteOperator> {
        self.remote.clone()
    }

    /// 按 ID 读取单封邮件模型；不存在则返回 `EmailNotFound`
    async fn get_email(&self, email_id: i32) -> Result<emails::Model, MailError> {
        email_repo::get_by_id(&self.db, email_id)
            .await?
            .ok_or(MailError::EmailNotFound(email_id))
    }

    /// 按 ID 读取单个账号模型；不存在则返回 `AccountNotFound`
    async fn get_account(&self, account_id: i32) -> Result<accounts::Model, MailError> {
        account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))
    }

    /// 同时取出一封邮件及其所属账号，所有公开操作方法的公共前置步骤
    async fn email_and_account(
        &self,
        email_id: i32,
    ) -> Result<(emails::Model, accounts::Model), MailError> {
        let email = self.get_email(email_id).await?;
        let account = self.get_account(email.account_id).await?;
        Ok((email, account))
    }

    /// 标记邮件已读 / 未读（远端 `\Seen` 标志 + 本地 `is_read` 字段）
    ///
    /// # 参数
    /// - email_id: 本地邮件 ID
    /// - is_read: true=标记已读，false=标记未读
    pub async fn mark_as_read(&self, email_id: i32, is_read: bool) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        self.remote
            .mark_seen(&account, &email.folder, email.uid, is_read)
            .await?;
        email_repo::mark_as_read(&self.db, email_id, is_read).await
    }

    /// 切换邮件星标状态（远端 `\Flagged` 标志 + 本地 `is_starred` 翻转）
    ///
    /// # 参数
    /// - email_id: 本地邮件 ID
    ///
    /// # 返回
    /// - Ok(true/false): 切换后最新的星标状态
    pub async fn toggle_star(&self, email_id: i32) -> Result<bool, MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let new_state = !email.is_starred.unwrap_or(false);
        self.remote
            .set_flagged(&account, &email.folder, email.uid, new_state)
            .await?;
        email_repo::toggle_star(&self.db, email_id).await
    }

    /// 把邮件移动到指定文件夹（远端 IMAP MOVE + 本地 `folder` 字段更新）
    ///
    /// 若邮件已在目标文件夹，直接返回 Ok（幂等）。
    ///
    /// # 参数
    /// - email_id: 本地邮件 ID
    /// - target_folder: 目标文件夹的 IMAP 名称（如 "INBOX"、"Archive"）
    pub async fn move_to_folder(
        &self,
        email_id: i32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        if email.folder == target_folder {
            return Ok(());
        }
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, target_folder).await
    }

    /// 删除邮件（第一阶段：移动到"已删除/Trash"文件夹）
    ///
    /// 注意：本阶段**不执行永久删除**（即不再对 Trash 内邮件做 IMAP EXPUNGE）。
    /// 这样可避免批量操作产生"部分成功"的不可恢复状态——
    /// 用户可在邮件客户端的 Trash 文件夹中再次确认后彻底清空。
    ///
    /// # 第一阶段限制
    /// - 当前实现仅支持**单封**删除（`email_ids.len() > 1` 时返回 `InvalidParam`），
    ///   后续扩展为批量时需重新评估远端顺序失败的处理策略
    /// - 若邮件**已在 Trash 文件夹**则拒绝执行（避免误永久删除），返回 `InvalidParam`
    ///
    /// # 参数
    /// - email_ids: 待删除邮件 ID 列表（当前仅允许 1 个）
    ///
    /// # 返回
    /// - Ok(n): 实际成功移动的封数
    pub async fn delete(&self, email_ids: Vec<i32>) -> Result<usize, MailError> {
        if email_ids.len() > 1 {
            return Err(MailError::InvalidParam(
                "第一阶段删除仅支持单封邮件，避免批量远端移动产生部分成功状态".to_string(),
            ));
        }

        let mut pending = Vec::with_capacity(email_ids.len());
        for email_id in email_ids.iter().copied() {
            let (email, account) = self.email_and_account(email_id).await?;
            let target_folder = self.resolve_special_folder(&account, "trash").await?;
            let trash_folders = self.resolve_special_folders(&account, "trash").await?;
            if trash_folders.iter().any(|folder| folder == &email.folder) {
                return Err(MailError::InvalidParam(format!(
                    "邮件 {email_id} 已在 Trash 文件夹，第一阶段不执行永久删除"
                )));
            }
            pending.push((email_id, email, account, target_folder));
        }

        let mut moved = 0;
        for (email_id, email, account, target_folder) in pending {
            self.remote
                .move_to_folder(&account, &email.folder, email.uid, &target_folder)
                .await?;
            email_repo::move_to_folder(&self.db, email_id, &target_folder).await?;
            moved += 1;
        }
        Ok(moved)
    }

    /// 归档邮件（移动到"归档/Archive"文件夹）
    ///
    /// 归档目标文件夹通过 [`Self::resolve_special_folder`] 按账号解析。
    ///
    /// # 参数
    /// - email_id: 本地邮件 ID
    pub async fn archive(&self, email_id: i32) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let target_folder = self.resolve_special_folder(&account, "archive").await?;
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, &target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, &target_folder).await
    }

    /// 解析账号的某个特殊文件夹（取 [`Self::resolve_special_folders`] 结果的第一个）
    ///
    /// # 参数
    /// - account: 账号模型
    /// - kind: 文件夹类别标识，当前支持 "trash"、"archive"
    ///
    /// # 返回
    /// - Ok(name): 该类别下首个匹配的 IMAP 文件夹名
    /// - Err(FolderNotFound): 账号未配置该类别文件夹
    async fn resolve_special_folder(
        &self,
        account: &accounts::Model,
        kind: &str,
    ) -> Result<String, MailError> {
        self.resolve_special_folders(account, kind)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                MailError::FolderNotFound(format!("账号 {} 未配置 {kind} 文件夹", account.email))
            })
    }

    /// 解析账号某类别下的全部候选特殊文件夹（如 Trash 的多个别名）
    ///
    /// 由于本方法**不连接 IMAP**（仅在本地已存文件夹名上做降级解析），
    /// [`FolderRegistry`] 的 SPECIAL-USE 层置空、`no_select` 默认 false，
    /// 仅依赖"跨语言关键词 + provider 候选名"两层兜底匹配。
    /// 若需要更精确的 SPECIAL-USE 解析，应走"带 IMAP LIST 的"完整同步流程。
    ///
    /// # 参数
    /// - account: 账号模型
    /// - kind: "trash" / "archive"，其他值返回空 Vec
    async fn resolve_special_folders(
        &self,
        account: &accounts::Model,
        kind: &str,
    ) -> Result<Vec<String>, MailError> {
        let cat = match kind {
            "trash" => FolderCategory::Trash,
            "archive" => FolderCategory::Archive,
            _ => return Ok(Vec::new()),
        };
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        // resolve_special_folders 不连接 IMAP，只能用本地已存文件夹名做降级解析：
        // SPECIAL-USE 置空、no_select=false，仅靠跨语言关键词 + provider 候选名兜底。
        let (remote, known_categories) = local_folder_registry_inputs(&self.db, account.id).await?;
        let registry = FolderRegistry::builder()
            .remote_folders(remote)
            .known_categories(known_categories)
            .provider_mapping(provider.folder_mapping())
            .allow_unverified_provider_fallback(true)
            .build();
        Ok(registry.resolve(cat))
    }
}
