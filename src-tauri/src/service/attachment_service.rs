// ═════════════════════════════════════════════════════════════════════════
// 附件服务模块 (Attachment Service)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责邮件附件的缓存、解码、保存与展示，包括：
// 1. 附件的按需缓存（首次访问时从远端 IMAP 拉取并落盘）
// 2. 邮件正文的 Content-Transfer-Encoding 解码（base64 / quoted-printable 等）
// 3. 附件的「另存为」与「使用系统默认程序打开」
// 4. 邮件正文中内联（inline）CID 图片的解析与本地化
//
// 架构设计：
// - Service 层：编排缓存、解码、文件系统操作的业务流程
// - Repository 层：负责附件元数据的数据库持久化
// - MailRemoteOperator：负责从 IMAP 远端获取附件的原始 section
//
// 设计原则：
// - 大附件（> 10MB）不进入应用缓存，避免磁盘膨胀
// - 写盘采用「先写 .tmp 再 rename」的原子写入策略，防止半截文件
// - 文件名经过 safe_filename 清洗，防止路径穿越与非法字符
// ═════════════════════════════════════════════════════════════════════════

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::auth::AuthManager;
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::attachments;
use crate::infrastructure::storage::repository::attachment_repo;
use crate::service::mail_operation::{MailRemoteOperator, RealMailRemoteOperator};
use base64::Engine;
use serde::{Deserialize, Serialize};
use specta::Type;

// ─── 常量定义 ───

/// 进入应用缓存的附件大小上限（10 MB）。
///
/// 超过此大小的附件不会被缓存到本地磁盘，仅在用户显式
/// 「另存为」时从远端拉取并直接写入目标路径，
/// 以避免缓存目录无限膨胀。
pub const SMALL_ATTACHMENT_LIMIT_BYTES: i64 = 10 * 1024 * 1024;

// ─── 数据传输对象 (DTO) 部分 ───

/// 附件数据传输对象
///
/// 这是附件信息对外展示的标准格式，用于前后端数据交换。
/// 由数据库模型 `attachments::Model` 转换而来，
/// 自动补全缺失的文件名、Content-Type 等字段。
///
/// # 字段说明
///
/// - `id`: 附件在数据库中的唯一标识
/// - `email_id`: 所属邮件的数据库 ID
/// - `filename`: 文件名（若原始数据缺失则自动生成兜底名）
/// - `content_type`: MIME 类型（缺失时默认 `application/octet-stream`）
/// - `size`: 附件字节大小
/// - `disposition`: Content-Disposition 值（如 `attachment` / `inline`）
/// - `content_id`: 内联资源的 Content-ID（对应正文中的 `cid:` 引用）
/// - `is_inline`: 是否为内联资源（disposition 为 inline 或存在 content_id）
/// - `is_cached`: 是否已缓存到本地磁盘
/// - `cache_path`: 本地缓存文件的绝对路径（未缓存时为 None）
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct AttachmentDto {
    /// 数据库主键 ID
    pub id: i32,
    /// 所属邮件的数据库 ID
    pub email_id: i32,
    /// 文件名
    pub filename: String,
    /// MIME 类型
    pub content_type: String,
    /// 附件字节大小
    pub size: i64,
    /// Content-Disposition（可选）
    pub disposition: Option<String>,
    /// 内联资源 Content-ID（可选）
    pub content_id: Option<String>,
    /// 是否为内联资源
    pub is_inline: bool,
    /// 是否已缓存到本地
    pub is_cached: bool,
    /// 本地缓存文件路径（可选）
    pub cache_path: Option<String>,
}

/// 内联附件数据传输对象
///
/// 用于将邮件正文中引用的内联图片（`cid:` 引用）解析为
/// 可直接访问的本地文件 URL，供前端渲染正文时替换 `cid:` 占位符。
///
/// # 字段说明
///
/// - `content_id`: 内联资源的 Content-ID（不含尖括号）
/// - `url`: 本地缓存文件的路径，作为 `cid:` 的替换目标
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InlineAttachmentDto {
    /// 内联资源 Content-ID
    pub content_id: String,
    /// 本地文件路径（用于替换 cid: 引用）
    pub url: String,
}

/// 附件服务
///
/// 编排附件的缓存、解码、保存与打开等业务流程。
///
/// # 字段说明
///
/// - `db`: 数据库连接，用于读写附件元数据
/// - `remote`: 远端邮件操作器，用于从 IMAP 拉取附件原始 section
/// - `cache_root`: 附件本地缓存的根目录
#[derive(Clone)]
pub struct AttachmentService {
    /// 数据库连接
    db: DbConn,
    /// 远端邮件操作器（线程安全，trait object）
    remote: Arc<dyn MailRemoteOperator>,
    /// 附件缓存根目录
    cache_root: PathBuf,
}

// ─── 服务实现部分 ───

impl AttachmentService {
    /// 创建新的附件服务实例
    ///
    /// 使用真实的远端邮件操作器，并将缓存目录设为
    /// `data_dir/attachments-cache`。
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `auth`: 认证管理器（使用 Arc 包装以便共享）
    /// - `data_dir`: 应用数据目录，附件缓存目录将作为其子目录
    ///
    /// # 返回
    ///
    /// 返回初始化好的 AttachmentService 实例
    pub fn new(db: DbConn, auth: Arc<AuthManager>, data_dir: PathBuf) -> Self {
        let remote = Arc::new(RealMailRemoteOperator::new(auth));
        Self::new_with_remote(db, remote, data_dir.join("attachments-cache"))
    }

    /// 使用指定的远端操作器和缓存根目录创建实例
    ///
    /// 该构造函数主要用于测试场景，允许注入自定义的
    /// `MailRemoteOperator`（如 mock）和独立的缓存目录。
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    /// - `remote`: 远端邮件操作器
    /// - `cache_root`: 附件缓存根目录
    ///
    /// # 返回
    ///
    /// 返回初始化好的 AttachmentService 实例
    pub fn new_with_remote(
        db: DbConn,
        remote: Arc<dyn MailRemoteOperator>,
        cache_root: PathBuf,
    ) -> Self {
        Self {
            db,
            remote,
            cache_root,
        }
    }

    /// 确保附件已缓存到本地，并返回最新 DTO
    ///
    /// 该方法是附件访问的核心入口，负责「按需缓存」流程：
    ///
    /// # 流程说明
    ///
    /// 1. 查询附件及其所属邮件/账号上下文
    /// 2. 若附件体积超过 `SMALL_ATTACHMENT_LIMIT_BYTES`，拒绝缓存
    /// 3. 若本地缓存文件已存在且可访问，直接返回
    /// 4. 否则从远端 IMAP 拉取附件 section
    /// 5. 按 Content-Transfer-Encoding 解码正文
    /// 6. 采用「先写 .tmp 再 rename」的原子策略写入缓存目录
    /// 7. 将缓存路径回写到数据库
    ///
    /// # 参数
    ///
    /// - `attachment_id`: 附件的数据库 ID
    ///
    /// # 返回
    ///
    /// 成功时返回更新后的附件 DTO（含 `cache_path`），
    /// 失败时返回 `MailError`（如附件不存在、超限、远端拉取失败等）。
    pub async fn ensure_cached(&self, attachment_id: i32) -> Result<AttachmentDto, MailError> {
        let context = attachment_repo::get_with_email_context(&self.db, attachment_id)
            .await?
            .ok_or(MailError::AttachmentNotFound(attachment_id))?;

        if context.attachment.size > SMALL_ATTACHMENT_LIMIT_BYTES {
            return Err(MailError::AttachmentUnavailable(
                "大于 10MB 的附件不进入应用缓存".to_string(),
            ));
        }

        if let Some(path) = context.attachment.path.as_deref()
            && tokio::fs::metadata(path).await.is_ok()
        {
            return Ok(attachment_model_to_dto(context.attachment));
        }

        let section = self
            .remote
            .fetch_attachment_section(
                &context.account,
                &context.email.folder,
                context.email.uid,
                &context.attachment.section_path,
            )
            .await?
            .ok_or_else(|| {
                MailError::AttachmentUnavailable("远端附件 section 不存在".to_string())
            })?;
        let bytes = decode_body(&section.body, section.transfer_encoding.as_deref())?;
        let dto = attachment_model_to_dto(context.attachment.clone());
        let path = cache_path_for(
            &self.cache_root,
            context.account.id,
            context.email.id,
            context.attachment.id,
            &dto.filename,
        );
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| MailError::FileSystemError(err.to_string()))?;
        }
        let tmp_path = path.with_extension("tmp");
        tokio::fs::write(&tmp_path, bytes)
            .await
            .map_err(|err| MailError::FileSystemError(err.to_string()))?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .map_err(|err| MailError::FileSystemError(err.to_string()))?;

        let updated = attachment_repo::update_path(
            &self.db,
            attachment_id,
            Some(path.to_string_lossy().to_string()),
        )
        .await?
        .ok_or(MailError::AttachmentNotFound(attachment_id))?;
        Ok(attachment_model_to_dto(updated))
    }

    /// 将附件另存为到指定路径
    ///
    /// # 流程说明
    ///
    /// 1. 对于小附件（≤ 上限）：优先复用本地缓存，直接拷贝到目标路径
    /// 2. 对于大附件或缓存缺失：从远端 IMAP 拉取并解码后直接写入目标路径
    ///
    /// 与 `ensure_cached` 不同，本方法不会将文件写入应用缓存目录，
    /// 大附件也可以通过此方法保存（绕过缓存大小限制）。
    ///
    /// # 参数
    ///
    /// - `attachment_id`: 附件的数据库 ID
    /// - `target_path`: 用户指定的目标文件路径
    ///
    /// # 返回
    ///
    /// 成功时返回 `Ok(())`，失败时返回 `MailError`。
    pub async fn save_as(&self, attachment_id: i32, target_path: String) -> Result<(), MailError> {
        let target = PathBuf::from(target_path);
        let context = attachment_repo::get_with_email_context(&self.db, attachment_id)
            .await?
            .ok_or(MailError::AttachmentNotFound(attachment_id))?;

        if context.attachment.size <= SMALL_ATTACHMENT_LIMIT_BYTES {
            let cached = self.ensure_cached(attachment_id).await?;
            if let Some(cache_path) = cached.cache_path {
                tokio::fs::copy(cache_path, &target)
                    .await
                    .map_err(|err| MailError::FileSystemError(err.to_string()))?;
                return Ok(());
            }
        }

        let section = self
            .remote
            .fetch_attachment_section(
                &context.account,
                &context.email.folder,
                context.email.uid,
                &context.attachment.section_path,
            )
            .await?
            .ok_or_else(|| {
                MailError::AttachmentUnavailable("远端附件 section 不存在".to_string())
            })?;
        let bytes = decode_body(&section.body, section.transfer_encoding.as_deref())?;
        tokio::fs::write(&target, bytes)
            .await
            .map_err(|err| MailError::FileSystemError(err.to_string()))?;
        Ok(())
    }

    /// 使用系统默认程序打开附件
    ///
    /// 先确保附件已缓存到本地，然后调用 `tauri_plugin_opener`
    /// 以系统默认应用程序打开该文件。
    ///
    /// # 参数
    ///
    /// - `attachment_id`: 附件的数据库 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 `Ok(())`，失败时返回 `MailError`
    /// （如附件未缓存、系统调用失败等）。
    pub async fn open(&self, attachment_id: i32) -> Result<(), MailError> {
        let dto = self.ensure_cached(attachment_id).await?;
        let path = dto
            .cache_path
            .ok_or_else(|| MailError::AttachmentUnavailable("附件尚未缓存".to_string()))?;
        tauri_plugin_opener::open_path(path, None::<&str>)
            .map_err(|err| MailError::FileSystemError(err.to_string()))
    }

    /// 解析邮件中的内联图片，返回 CID 到本地路径的映射
    ///
    /// 遍历指定邮件的所有附件，筛选出内联图片资源（Content-Type
    /// 以 `image/` 开头且存在 Content-ID，且体积不超过缓存上限），
    /// 确保它们已缓存到本地，并返回 `InlineAttachmentDto` 列表。
    ///
    /// 前端可据此将正文中的 `cid:xxx` 引用替换为本地文件路径。
    ///
    /// # 参数
    ///
    /// - `email_id`: 邮件的数据库 ID
    ///
    /// # 返回
    ///
    /// 成功时返回内联附件 DTO 列表（每项含 `content_id` 与本地 `url`），
    /// 失败时返回 `MailError`。
    pub async fn resolve_inline_images(
        &self,
        email_id: i32,
    ) -> Result<Vec<InlineAttachmentDto>, MailError> {
        let attachments = attachment_repo::list_by_email(&self.db, email_id).await?;
        let mut resolved = Vec::new();
        for attachment in attachments {
            let Some(content_id) = attachment.content_id.clone() else {
                continue;
            };
            let is_image = attachment
                .content_type
                .as_deref()
                .is_some_and(|content_type| content_type.starts_with("image/"));
            if !is_image || attachment.size > SMALL_ATTACHMENT_LIMIT_BYTES {
                continue;
            }

            let dto = self.ensure_cached(attachment.id).await?;
            if let Some(path) = dto.cache_path {
                resolved.push(InlineAttachmentDto {
                    content_id,
                    url: path,
                });
            }
        }
        Ok(resolved)
    }
}

// ─── 转换与工具函数部分 ───

/// 将数据库附件模型转换为 DTO
///
/// 执行字段映射并补全缺失的默认值：
/// - 文件名缺失时，基于 `content_id` 或 ID 生成兜底名
/// - Content-Type 缺失时，默认为 `application/octet-stream`
/// - 根据 disposition / content_id 推断 `is_inline`
/// - 根据是否存在本地路径推断 `is_cached`
///
/// # 参数
///
/// - `model`: 数据库附件模型
///
/// # 返回
///
/// 返回补全字段后的 `AttachmentDto`
pub fn attachment_model_to_dto(model: attachments::Model) -> AttachmentDto {
    let attachments::Model {
        id,
        email_id,
        filename,
        content_type,
        size,
        disposition,
        content_id,
        path,
        ..
    } = model;
    let filename = filename.unwrap_or_else(|| fallback_filename(id, content_id.as_deref()));
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let is_inline = disposition
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("inline"))
        || content_id.is_some();
    let is_cached = path.is_some();

    AttachmentDto {
        id,
        email_id,
        filename,
        content_type,
        size,
        disposition,
        content_id,
        is_inline,
        is_cached,
        cache_path: path,
    }
}

/// 为缺失文件名的附件生成兜底文件名
///
/// 若存在 Content-ID，则去除尖括号后生成 `inline-<cid>`；
/// 否则使用 `attachment-<id>` 作为文件名。
///
/// # 参数
///
/// - `id`: 附件的数据库 ID
/// - `content_id`: 内联资源 Content-ID（可选）
///
/// # 返回
///
/// 返回生成的兜底文件名字符串
fn fallback_filename(id: i32, content_id: Option<&str>) -> String {
    if let Some(content_id) = content_id {
        let trimmed = content_id.trim_matches(['<', '>']);
        if !trimmed.is_empty() {
            return format!("inline-{trimmed}");
        }
    }
    format!("attachment-{id}")
}

/// 查询指定邮件的所有附件并转换为 DTO 列表
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件的数据库 ID
///
/// # 返回
///
/// 成功时返回该邮件的附件 DTO 列表，失败时返回 `MailError`。
pub async fn list_dtos_by_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<AttachmentDto>, MailError> {
    let attachments = attachment_repo::list_by_email(db, email_id).await?;
    Ok(attachments
        .into_iter()
        .map(attachment_model_to_dto)
        .collect())
}

/// 根据 Content-Transfer-Encoding 解码邮件正文
///
/// 支持以下编码（不区分大小写）：
/// - `base64`：过滤所有空白字符后使用标准 base64 解码
/// - `quoted-printable`：使用 Robust 模式解码 QP 编码
/// - `7bit` / `8bit` / `binary`：原样返回（无需解码）
/// - 缺省时按 `7bit` 处理
///
/// 其他不支持的编码会返回 `AttachmentDecodeFailed` 错误。
///
/// # 参数
///
/// - `body`: 原始正文字节切片
/// - `transfer_encoding`: Content-Transfer-Encoding 取值（可选）
///
/// # 返回
///
/// 成功时返回解码后的字节向量，失败时返回 `MailError`。
fn decode_body(body: &[u8], transfer_encoding: Option<&str>) -> Result<Vec<u8>, MailError> {
    match transfer_encoding
        .unwrap_or("7bit")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "base64" => {
            let normalized = body
                .iter()
                .copied()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>();
            base64::engine::general_purpose::STANDARD
                .decode(normalized)
                .map_err(|err| MailError::AttachmentDecodeFailed(err.to_string()))
        }
        "quoted-printable" => quoted_printable::decode(body, quoted_printable::ParseMode::Robust)
            .map_err(|err| MailError::AttachmentDecodeFailed(err.to_string())),
        "7bit" | "8bit" | "binary" => Ok(body.to_vec()),
        other => Err(MailError::AttachmentDecodeFailed(format!(
            "不支持的 Content-Transfer-Encoding: {other}"
        ))),
    }
}

/// 清洗文件名，确保其可安全用作磁盘文件名
///
/// 将控制字符及路径分隔符（`/`、`\`、`:`）替换为下划线，
/// 并去除首尾空白与首尾的点号（防止隐藏文件或意外遍历）。
/// 清洗后若结果为空，则返回 `attachment` 作为兜底名。
///
/// # 参数
///
/// - `filename`: 原始文件名
///
/// # 返回
///
/// 返回清洗后的安全文件名
fn safe_filename(filename: &str) -> String {
    let cleaned = filename
        .chars()
        .map(|ch| {
            if ch.is_control() || ch == '/' || ch == '\\' || ch == ':' {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if cleaned.is_empty() {
        "attachment".to_string()
    } else {
        cleaned
    }
}

/// 为附件生成本地缓存的完整路径
///
/// 目录结构按 `缓存根/账号ID/邮件ID/附件ID-文件名` 分层组织，
/// 便于按账号或邮件维度批量清理缓存。
///
/// # 参数
///
/// - `cache_root`: 缓存根目录
/// - `account_id`: 账号 ID
/// - `email_id`: 邮件 ID
/// - `attachment_id`: 附件 ID
/// - `filename`: 原始文件名（会经 `safe_filename` 清洗）
///
/// # 返回
///
/// 返回组装好的完整缓存路径
fn cache_path_for(
    cache_root: &Path,
    account_id: i32,
    email_id: i32,
    attachment_id: i32,
    filename: &str,
) -> PathBuf {
    cache_root
        .join(account_id.to_string())
        .join(email_id.to_string())
        .join(format!("{}-{}", attachment_id, safe_filename(filename)))
}
