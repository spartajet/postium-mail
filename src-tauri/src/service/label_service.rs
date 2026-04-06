use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::entities::labels;
use crate::infrastructure::storage::repository::label_repo;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LabelDto {
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateLabelRequest {
    pub account_id: i32,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLabelRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

pub struct LabelService {
    db: DbConn,
}

impl LabelService {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn list_labels(&self, account_id: i32) -> Result<Vec<LabelDto>, MailError> {
        let labels = label_repo::list_by_account(&self.db, account_id).await?;
        Ok(labels
            .into_iter()
            .map(|l| LabelDto {
                id: l.id,
                account_id: l.account_id,
                name: l.name,
                color: l.color,
            })
            .collect())
    }

    pub async fn create_label(&self, req: CreateLabelRequest) -> Result<LabelDto, MailError> {
        tracing::info!(account_id = req.account_id, name = %req.name, "创建标签");
        let now = chrono::Utc::now().timestamp();
        let model = labels::ActiveModel {
            account_id: Set(req.account_id),
            name: Set(req.name),
            color: Set(req.color),
            created_at: Set(now),
            ..Default::default()
        };
        let created = label_repo::create(&self.db, model).await?;
        Ok(LabelDto {
            id: created.id,
            account_id: created.account_id,
            name: created.name,
            color: created.color,
        })
    }

    pub async fn update_label(
        &self,
        id: i32,
        req: UpdateLabelRequest,
    ) -> Result<LabelDto, MailError> {
        let existing = label_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::LabelNotFound(id))?;
        let model = labels::ActiveModel {
            id: Set(existing.id),
            account_id: Set(existing.account_id),
            name: Set(req.name.unwrap_or(existing.name)),
            color: Set(req.color.unwrap_or(existing.color)),
            created_at: Set(existing.created_at),
        };
        let updated = label_repo::update(&self.db, id, model).await?;
        Ok(LabelDto {
            id: updated.id,
            account_id: updated.account_id,
            name: updated.name,
            color: updated.color,
        })
    }

    pub async fn delete_label(&self, id: i32) -> Result<(), MailError> {
        tracing::info!(id, "删除标签");
        label_repo::delete(&self.db, id).await
    }

    pub async fn add_label_to_email(&self, email_id: i32, label_id: i32) -> Result<(), MailError> {
        label_repo::add_label_to_email(&self.db, email_id, label_id).await
    }

    pub async fn remove_label_from_email(
        &self,
        email_id: i32,
        label_id: i32,
    ) -> Result<(), MailError> {
        label_repo::remove_label_from_email(&self.db, email_id, label_id).await
    }

    pub async fn get_labels_for_email(&self, email_id: i32) -> Result<Vec<LabelDto>, MailError> {
        let labels = label_repo::get_labels_for_email(&self.db, email_id).await?;
        Ok(labels
            .into_iter()
            .map(|l| LabelDto {
                id: l.id,
                account_id: l.account_id,
                name: l.name,
                color: l.color,
            })
            .collect())
    }

    pub async fn list_emails_by_label(&self, label_id: i32) -> Result<Vec<i32>, MailError> {
        label_repo::list_emails_by_label(&self.db, label_id).await
    }
}
