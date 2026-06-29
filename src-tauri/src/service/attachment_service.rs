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

pub const SMALL_ATTACHMENT_LIMIT_BYTES: i64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct AttachmentDto {
    pub id: i32,
    pub email_id: i32,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub disposition: Option<String>,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub is_cached: bool,
    pub cache_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InlineAttachmentDto {
    pub content_id: String,
    pub url: String,
}

#[derive(Clone)]
pub struct AttachmentService {
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
    cache_root: PathBuf,
}

impl AttachmentService {
    pub fn new(db: DbConn, auth: Arc<AuthManager>, data_dir: PathBuf) -> Self {
        let remote = Arc::new(RealMailRemoteOperator::new(auth));
        Self::new_with_remote(db, remote, data_dir.join("attachments-cache"))
    }

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

    pub async fn open(&self, attachment_id: i32) -> Result<(), MailError> {
        let dto = self.ensure_cached(attachment_id).await?;
        let path = dto
            .cache_path
            .ok_or_else(|| MailError::AttachmentUnavailable("附件尚未缓存".to_string()))?;
        tauri_plugin_opener::open_path(path, None::<&str>)
            .map_err(|err| MailError::FileSystemError(err.to_string()))
    }

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

fn fallback_filename(id: i32, content_id: Option<&str>) -> String {
    if let Some(content_id) = content_id {
        let trimmed = content_id.trim_matches(['<', '>']);
        if !trimmed.is_empty() {
            return format!("inline-{trimmed}");
        }
    }
    format!("attachment-{id}")
}

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
