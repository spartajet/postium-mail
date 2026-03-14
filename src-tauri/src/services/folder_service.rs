use sea_orm::{*, sea_query::Expr};
use anyhow::{anyhow, Result};
use crate::models::{folder, FolderEntity};

/// 获取账号的所有文件夹
pub async fn get_by_account(db: &DbConn, account_id: i32) -> Result<Vec<folder::Model>> {
    Ok(FolderEntity::find()
        .filter(folder::Column::AccountId.eq(account_id))
        .all(db)
        .await
        .map_err(|e| anyhow!("获取文件夹列表失败: {}", e))?)
}

/// 根据 ID 获取文件夹
pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<folder::Model>> {
    Ok(FolderEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取文件夹失败: {}", e))?)
}

/// 根据账号和 IMAP 名称获取文件夹
pub async fn get_by_imap_name(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
) -> Result<Option<folder::Model>> {
    Ok(FolderEntity::find()
        .filter(folder::Column::AccountId.eq(account_id))
        .filter(folder::Column::ImapName.eq(imap_name))
        .one(db)
        .await
        .map_err(|e| anyhow!("获取文件夹失败: {}", e))?)
}

/// 查找或创建文件夹
pub async fn find_or_create(
    db: &DbConn,
    account_id: i32,
    name: &str,
    imap_name: &str,
) -> Result<folder::Model> {
    // 先尝试查找
    if let Some(existing) = get_by_imap_name(db, account_id, imap_name).await? {
        return Ok(existing);
    }

    // 不存在则创建
    let now = chrono::Utc::now().timestamp();
    let new_folder = folder::ActiveModel {
        account_id: Set(account_id),
        name: Set(name.to_string()),
        imap_name: Set(imap_name.to_string()),
        synced_at: Set(Some(now)),
        ..Default::default()
    };

    Ok(new_folder
        .insert(db)
        .await
        .map_err(|e| anyhow!("创建文件夹失败: {}", e))?)
}

/// 更新文件夹统计数据
pub async fn update_stats(
    db: &DbConn,
    folder_id: i32,
    email_count: i32,
    unread_count: i32,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    FolderEntity::update_many()
        .filter(folder::Column::Id.eq(folder_id))
        .col_expr(
            folder::Column::EmailCount,
            Expr::value(email_count),
        )
        .col_expr(
            folder::Column::UnreadCount,
            Expr::value(unread_count),
        )
        .col_expr(
            folder::Column::SyncedAt,
            Expr::value(now),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("更新文件夹统计失败: {}", e))?;

    Ok(())
}

/// 删除文件夹
pub async fn delete(db: &DbConn, id: i32) -> Result<()> {
    FolderEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除文件夹失败: {}", e))?;

    Ok(())
}

/// 映射 IMAP 文件夹名称到标准名称
pub fn map_folder_name(imap_name: &str) -> String {
    // 处理嵌套文件夹（如 [Gmail]/Spam）
    let parts: Vec<&str> = imap_name.split('/').collect();
    let folder_name = parts.last().unwrap_or(&imap_name);

    match folder_name.to_uppercase().as_str() {
        "INBOX" => "inbox".to_string(),
        "SENT" | "SENT ITEMS" | "SENT MAIL" | "已发送" | "SENDEN" => "sent".to_string(),
        "DRAFT" | "DRAFTS" | "草稿箱" | "草稿" | "ENTWURFE" => "drafts".to_string(),
        "TRASH" | "DELETED" | "DELETED ITEMS" | "已删除" | "垃圾箱" | "GELÖSCHTE" | "PAPER" => "trash".to_string(),
        "SPAM" | "JUNK" | "JUNK E-MAIL" | "垃圾邮件" | "POSTINI" => "spam".to_string(),
        "ARCHIVE" | "ARCHIVES" | "归档" | "ALL MAIL" => "archive".to_string(),
        _ => imap_name.to_string(),
    }
}

/// 检查是否是标准文件夹
pub fn is_standard_folder(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "inbox" | "sent" | "drafts" | "spam" | "trash" | "archive"
    )
}
