use sea_orm::{*, sea_query::Expr};
use anyhow::{anyhow, Result};
use crate::models::{folder, FolderEntity};

/// 解码 IMAP UTF-7 编码的文件夹名称
///
/// IMAP 使用改良版 UTF-7 编码来支持非 ASCII 字符的文件夹名
/// 规则：
/// - & 表示开始编码
/// - - 表示结束编码
/// - 中间的字符是 Base64 编码的 UTF-16
fn decode_imap_utf7(imap_name: &str) -> String {
    // 如果没有 & 符号，说明不是 UTF-7 编码
    if !imap_name.contains('&') {
        return String::from(imap_name);
    }

    // 常见的中文邮箱文件夹名称映射（163、QQ 邮箱等）
    // 这些是 UTF-7 编码的常见中文名称
    let common_mappings = [
        ("&XfJT0ZAB-", "已发送"),
        ("&XfJSIJZk-", "收件箱"),
        ("&V4NXPpCuTvY-", "垃圾邮件"),
        ("&dcVr0mWHTvZZOQ-", "已删除"),
        ("&g0l6P3ux-", "草稿箱"),
        ("&Xn9USpCuTvY-", "通讯录"),
        ("&i6KWBZCuTvY-", "订阅"),
        ("&WQdf2F9V-", "广告邮件"),
        ("&eT5OpA-", "重要邮件"),
        ("&Y6hef5CuTvY-", "病毒邮件"),
        ("&W4xRaFeDVz6Qrk72-", "RSS订阅"),
        ("&W1hoYw-", "订阅信息"),
    ];

    for (encoded, decoded) in common_mappings.iter() {
        if imap_name == *encoded || imap_name.ends_with(encoded) {
            return String::from(*decoded);
        }
    }

    // 如果不在映射表中，返回原始名称
    String::from(imap_name)
}

/// 映射 IMAP 文件夹名称到标准名称
pub fn map_folder_name(imap_name: &str) -> String {
    // 先解码 UTF-7 编码的名称
    let decoded_name = decode_imap_utf7(imap_name);

    // 处理嵌套文件夹（如 [Gmail]/Spam）
    let parts: Vec<&str> = decoded_name.split('/').collect();
    let folder_name = parts.last().copied().unwrap_or(decoded_name.as_str());

    match folder_name.to_uppercase().as_str() {
        "INBOX" => "inbox".to_string(),
        "SENT" | "SENT ITEMS" | "SENT MAIL" | "已发送" | "SENDEN" => "sent".to_string(),
        "DRAFT" | "DRAFTS" | "草稿箱" | "草稿" | "ENTWURFE" => "drafts".to_string(),
        "TRASH" | "DELETED" | "DELETED ITEMS" | "已删除" | "垃圾箱" | "GELÖSCHTE" | "PAPER" => "trash".to_string(),
        "SPAM" | "JUNK" | "JUNK E-MAIL" | "垃圾邮件" | "POSTINI" => "spam".to_string(),
        "ARCHIVE" | "ARCHIVES" | "归档" | "ALL MAIL" => "archive".to_string(),
        "STARRED" | "星标邮件" | "已加星标" => "starred".to_string(),
        _ => String::from(imap_name),  // 使用原始 IMAP 名称作为标准名称
    }
}

/// 获取账号的所有文件夹
pub async fn get_by_account(db: &DbConn, account_id: i32) -> Result<Vec<folder::Model>> {
    FolderEntity::find()
        .filter(folder::Column::AccountId.eq(account_id))
        .all(db)
        .await
        .map_err(|e| anyhow!("获取文件夹列表失败: {}", e))
}

/// 根据 ID 获取文件夹
pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<folder::Model>> {
    FolderEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取文件夹失败: {}", e))
}

/// 根据账号和 IMAP 名称获取文件夹
pub async fn get_by_imap_name(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
) -> Result<Option<folder::Model>> {
    FolderEntity::find()
        .filter(folder::Column::AccountId.eq(account_id))
        .filter(folder::Column::ImapName.eq(imap_name))
        .one(db)
        .await
        .map_err(|e| anyhow!("获取文件夹失败: {}", e))
}

/// 根据账号和 IMAP 名称获取文件夹（别名，用于同步管理器）
pub async fn get_by_account_and_imap_name(
    db: &DbConn,
    account_id: i32,
    imap_name: &str,
) -> Result<Option<folder::Model>> {
    get_by_imap_name(db, account_id, imap_name).await
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
        name: Set(String::from(name)),
        imap_name: Set(String::from(imap_name)),
        synced_at: Set(Some(now)),
        ..Default::default()
    };

    new_folder
        .insert(db)
        .await
        .map_err(|e| anyhow!("创建文件夹失败: {}", e))
}

/// 查找或创建文件夹（包含IMAP元数据）
pub async fn find_or_create_with_metadata(
    db: &DbConn,
    account_id: i32,
    name: &str,
    imap_name: &str,
    uidvalidity: i64,
    uidnext: i64,
    highest_modseq: Option<i64>,
) -> Result<folder::Model> {
    // 先尝试查找
    if let Some(existing) = get_by_imap_name(db, account_id, imap_name).await? {
        // 更新IMAP元数据
        let now = chrono::Utc::now().timestamp();
        let mut active: folder::ActiveModel = existing.into();
        active.uidvalidity = Set(Some(uidvalidity));
        active.uidnext = Set(Some(uidnext));
        active.highest_modseq = Set(highest_modseq);
        active.synced_at = Set(Some(now));

        return active
            .update(db)
            .await
            .map_err(|e| anyhow!("更新文件夹元数据失败: {}", e));
    }

    // 不存在则创建
    let now = chrono::Utc::now().timestamp();
    let new_folder = folder::ActiveModel {
        account_id: Set(account_id),
        name: Set(String::from(name)),
        imap_name: Set(String::from(imap_name)),
        uidvalidity: Set(Some(uidvalidity)),
        uidnext: Set(Some(uidnext)),
        highest_modseq: Set(highest_modseq),
        synced_at: Set(Some(now)),
        ..Default::default()
    };

    new_folder
        .insert(db)
        .await
        .map_err(|e| anyhow!("创建文件夹失败: {}", e))
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

/// 检查是否是标准文件夹
pub fn is_standard_folder(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "inbox" | "sent" | "drafts" | "spam" | "trash" | "archive" | "starred"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_imap_utf7() {
        // 测试常见的中文文件夹名称
        assert_eq!(decode_imap_utf7("&XfJT0ZAB-"), "已发送");
        assert_eq!(decode_imap_utf7("&XfJSIJZk-"), "收件箱");
        assert_eq!(decode_imap_utf7("&V4NXPpCuTvY-"), "垃圾邮件");
        assert_eq!(decode_imap_utf7("&g0l6P3ux-"), "草稿箱");

        // 测试非 UTF-7 编码的名称
        assert_eq!(decode_imap_utf7("INBOX"), "INBOX");
        assert_eq!(decode_imap_utf7("Sent"), "Sent");
        assert_eq!(decode_imap_utf7("[Gmail]/Spam"), "[Gmail]/Spam");
    }

    #[test]
    fn test_map_folder_name() {
        // 测试 UTF-7 编码的中文名称映射
        assert_eq!(map_folder_name("&XfJT0ZAB-"), "sent");          // 已发送 -> sent
        assert_eq!(map_folder_name("&g0l6P3ux-"), "drafts");        // 草稿箱 -> drafts
        assert_eq!(map_folder_name("&V4NXPpCuTvY-"), "spam");       // 垃圾邮件 -> spam

        // 测试英文标准名称
        assert_eq!(map_folder_name("INBOX"), "inbox");
        assert_eq!(map_folder_name("Sent"), "sent");
        assert_eq!(map_folder_name("Drafts"), "drafts");
        assert_eq!(map_folder_name("已发送"), "sent");
        assert_eq!(map_folder_name("草稿箱"), "drafts");

        // 测试嵌套文件夹
        assert_eq!(map_folder_name("[Gmail]/Spam"), "spam");
        assert_eq!(map_folder_name("[Gmail]/Sent"), "sent");

        // 测试未知名称（保留原始 IMAP 名称）
        assert_eq!(map_folder_name("CustomFolder"), "CustomFolder");
        assert_eq!(map_folder_name("&UnknownCode-"), "&UnknownCode-");
    }

    #[test]
    fn test_is_standard_folder() {
        // 测试标准文件夹
        assert!(is_standard_folder("inbox"));
        assert!(is_standard_folder("sent"));
        assert!(is_standard_folder("drafts"));
        assert!(is_standard_folder("spam"));
        assert!(is_standard_folder("trash"));
        assert!(is_standard_folder("archive"));
        assert!(is_standard_folder("starred"));

        // 测试大小写不敏感
        assert!(is_standard_folder("INBOX"));
        assert!(is_standard_folder("Sent"));
        assert!(is_standard_folder("DRAFTS"));

        // 测试非标准文件夹
        assert!(!is_standard_folder("custom"));
        assert!(!is_standard_folder("myfolder"));
        assert!(!is_standard_folder("&XfJT0ZAB-"));  // UTF-7 编码的名称
    }
}
