//! 文件夹存储层
//!
//! 提供文件夹的数据库查询操作和 UTF-7 解码功能

use sea_orm::{EntityTrait, QueryOrder, ColumnTrait, QueryFilter, DbConn, Condition, Set};
use sea_orm::ActiveModelTrait;

use crate::error::{Result, StorageError};
use crate::models::folder;

/// 文件夹仓库
///
/// 处理文件夹的数据库查询和 UTF-7 解码
pub struct FolderRepository;

impl FolderRepository {
    // ========== 查询操作 ==========

    /// 获取账号的所有文件夹
    pub async fn get_by_account(db: &DbConn, account_id: i32) -> Result<Vec<folder::Model>> {
        let folders = folder::Entity::find()
            .filter(folder::Column::AccountId.eq(account_id))
            .order_by_asc(folder::Column::Id)
            .all(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹列表失败: {}", e)))?;

        Ok(folders)
    }

    /// 按 ID 获取文件夹
    pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<folder::Model>> {
        let folder = folder::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹失败: {}", e)))?;

        Ok(folder)
    }

    /// 按 IMAP 名称获取文件夹
    pub async fn get_by_imap_name(
        db: &DbConn,
        account_id: i32,
        imap_name: &str,
    ) -> Result<Option<folder::Model>> {
        let folder = folder::Entity::find()
            .filter(
                Condition::all()
                    .add(folder::Column::AccountId.eq(account_id))
                    .add(folder::Column::ImapName.eq(imap_name))
            )
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹失败: {}", e)))?;

        Ok(folder)
    }

    /// 按标准名称获取文件夹
    pub async fn get_by_name(
        db: &DbConn,
        account_id: i32,
        name: &str,
    ) -> Result<Option<folder::Model>> {
        let folder = folder::Entity::find()
            .filter(
                Condition::all()
                    .add(folder::Column::AccountId.eq(account_id))
                    .add(folder::Column::Name.eq(name))
            )
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹失败: {}", e)))?;

        Ok(folder)
    }

    // ========== CRUD 操作 ==========

    /// 查找或创建文件夹
    pub async fn find_or_create(
        db: &DbConn,
        account_id: i32,
        imap_name: &str,
    ) -> Result<folder::Model> {
        // 先尝试查找
        if let Some(folder) = Self::get_by_imap_name(db, account_id, imap_name).await? {
            return Ok(folder);
        }

        // 解码并映射文件夹名
        let decoded_name = Self::decode_imap_utf7(imap_name)?;
        let standard_name = Self::map_folder_name(&decoded_name);

        // 创建新文件夹
        let now = chrono::Utc::now().timestamp();
        let active_folder = folder::ActiveModel {
            account_id: Set(account_id),
            name: Set(standard_name),
            imap_name: Set(imap_name.to_string()),
            email_count: Set(0),
            unread_count: Set(0),
            synced_at: Set(Some(now)),
            ..Default::default()
        };

        let folder = active_folder.insert(db)
            .await
            .map_err(|e| StorageError::Database(format!("创建文件夹失败: {}", e)))?;

        Ok(folder)
    }

    /// 查找或创建文件夹（含 IMAP 元数据）
    pub async fn find_or_create_with_metadata(
        db: &DbConn,
        account_id: i32,
        imap_name: &str,
        uidvalidity: Option<i64>,
        attributes: Option<String>,
    ) -> Result<folder::Model> {
        // 先尝试查找
        if let Some(mut folder) = Self::get_by_imap_name(db, account_id, imap_name).await? {
            // 更新元数据
            if let Some(validity) = uidvalidity {
                if folder.uidvalidity != Some(validity) {
                    folder.uidvalidity = Some(validity);
                    let active_folder: folder::ActiveModel = folder.into();
                    let updated = active_folder.update(db)
                        .await
                        .map_err(|e| StorageError::Database(format!("更新文件夹失败: {}", e)))?;
                    return Ok(updated);
                }
            }
            return Ok(folder);
        }

        // 解码并映射文件夹名
        let decoded_name = Self::decode_imap_utf7(imap_name)?;
        let standard_name = Self::map_folder_name(&decoded_name);

        // 创建新文件夹
        let now = chrono::Utc::now().timestamp();
        let active_folder = folder::ActiveModel {
            account_id: Set(account_id),
            name: Set(standard_name),
            imap_name: Set(imap_name.to_string()),
            attributes: Set(attributes),
            email_count: Set(0),
            unread_count: Set(0),
            uidvalidity: Set(uidvalidity),
            synced_at: Set(Some(now)),
            ..Default::default()
        };

        let folder = active_folder.insert(db)
            .await
            .map_err(|e| StorageError::Database(format!("创建文件夹失败: {}", e)))?;

        Ok(folder)
    }

    /// 更新文件夹统计
    pub async fn update_stats(
        db: &DbConn,
        id: i32,
        email_count: i32,
        unread_count: i32,
    ) -> Result<()> {
        let folder = folder::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("文件夹不存在".to_string()))?;

        let mut active_folder: folder::ActiveModel = folder.into();
        active_folder.email_count = Set(email_count);
        active_folder.unread_count = Set(unread_count);

        active_folder.update(db)
            .await
            .map_err(|e| StorageError::Database(format!("更新统计失败: {}", e)))?;

        Ok(())
    }

    /// 更新同步时间
    pub async fn update_sync_time(db: &DbConn, id: i32) -> Result<()> {
        let folder = folder::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取文件夹失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("文件夹不存在".to_string()))?;

        let mut active_folder: folder::ActiveModel = folder.into();
        active_folder.synced_at = Set(Some(chrono::Utc::now().timestamp()));

        active_folder.update(db)
            .await
            .map_err(|e| StorageError::Database(format!("更新同步时间失败: {}", e)))?;

        Ok(())
    }

    /// 删除文件夹
    pub async fn delete(db: &DbConn, id: i32) -> Result<()> {
        folder::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除文件夹失败: {}", e)))?;

        Ok(())
    }

    // ========== UTF-7 解码和名称映射 ==========

    /// 解码 IMAP UTF-7 编码的文件夹名
    ///
    /// IMAP UTF-7 是 Modified UTF-7 编码，用于处理非 ASCII 字符（如中文）
    ///
    /// # 示例
    ///
    /// - `&XfJT0ZAB-` → `已发送`
    /// - `&XfJSIJZk-` → `收件箱`
    pub fn decode_imap_utf7(encoded: &str) -> Result<String> {
        // 如果不含 & 符号，不是 UTF-7 编码，直接返回
        if !encoded.contains('&') {
            return Ok(encoded.to_string());
        }

        // IMAP UTF-7 解码逻辑
        let mut result = String::new();
        let mut chars = encoded.chars().peekable();
        let mut base64_buffer = String::new();
        let mut in_base64 = false;

        while let Some(c) = chars.next() {
            if c == '&' {
                if in_base64 {
                    // 结束 base64 模式
                    if base64_buffer.ends_with('-') {
                        base64_buffer.pop();
                    }
                    if !base64_buffer.is_empty() {
                        // 解码 base64
                        match Self::decode_imap_utf7_base64(&base64_buffer) {
                            Ok(decoded) => result.push_str(&decoded),
                            Err(_) => result.push('&'),
                        }
                    }
                    base64_buffer.clear();
                }
                in_base64 = true;
            } else if in_base64 {
                if c == '-' {
                    // &- 表示 & 字符
                    if base64_buffer.is_empty() {
                        result.push('&');
                    }
                    in_base64 = false;
                } else {
                    base64_buffer.push(c);
                }
            } else {
                result.push(c);
            }
        }

        // 处理结尾的 base64
        if in_base64 && !base64_buffer.is_empty() {
            match Self::decode_imap_utf7_base64(&base64_buffer) {
                Ok(decoded) => result.push_str(&decoded),
                Err(_) => result.push_str(&format!("&{}-", base64_buffer)),
            }
        }

        Ok(result)
    }

    /// 解码 IMAP UTF-7 Base64 部分
    fn decode_imap_utf7_base64(encoded: &str) -> Result<String> {
        // IMAP UTF-7 使用修改后的 Base64 字母表
        // 将 , 替换为 / 后用标准 Base64 解码
        let standard_base64 = encoded.replace(',', "/");

        let decoded_bytes = base64::decode_engine(&standard_base64, &base64::engine::general_purpose::STANDARD)
            .map_err(|_| StorageError::Database("Base64 解码失败".to_string()))?;

        // 解码为 UTF-16 字符串
        let utf16_chars: Vec<u16> = decoded_bytes
            .chunks(2)
            .map(|chunk: &[u8]| {
                if chunk.len() == 2 {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                } else {
                    chunk[0] as u16
                }
            })
            .collect();

        Ok(String::from_utf16(&utf16_chars)
            .map_err(|_| StorageError::Database("UTF-16 解码失败".to_string()))?)
    }

    /// 将 IMAP 文件夹名映射到标准名称
    ///
    /// # 示例
    ///
    /// - `INBOX` → `inbox`
    /// - `已发送` → `sent`
    /// - `Sent Items` → `sent`
    /// - `Drafts` → `drafts`
    pub fn map_folder_name(imap_name: &str) -> String {
        // 标准化名称（小写、去除空格）
        let normalized = imap_name.to_lowercase().replace(' ', "");

        // 先检查中文映射
        if let Some(standard) = Self::get_chinese_mapping(imap_name) {
            return standard.to_string();
        }

        // 检查英文标准名称
        match normalized.as_str() {
            "inbox" => "inbox".to_string(),
            "sent" | "sentitems" => "sent".to_string(),
            "drafts" => "drafts".to_string(),
            "junk" | "spam" | "bulkmail" => "spam".to_string(),
            "trash" | "deleted" | "deleteditems" => "trash".to_string(),
            "archive" | "archives" => "archive".to_string(),
            _ => imap_name.to_string(), // 保持原始名称
        }
    }

    /// 获取中文文件夹名映射
    fn get_chinese_mapping(name: &str) -> Option<&'static str> {
        // 常见的中文邮箱文件夹映射
        let common_mappings = [
            ("已发送", "sent"),
            ("已删除", "trash"),
            ("垃圾邮件", "spam"),
            ("草稿箱", "drafts"),
            ("收件箱", "inbox"),
            ("归档", "archive"),
            ("&XfJT0ZAB-", "sent"),      // 已发送的 UTF-7 编码
            ("&XfJSIJZk-", "inbox"),      // 收件箱的 UTF-7 编码
            ("&V4NXPpCuTvY-", "spam"),    // 垃圾邮件的 UTF-7 编码
            ("&Xn9USpCuTvY-", "trash"),   // 已删除的 UTF-7 编码
        ];

        for (key, value) in common_mappings.iter() {
            if name == *key {
                return Some(value);
            }
        }

        None
    }

    /// 检查是否是标准文件夹
    pub fn is_standard_folder(name: &str) -> bool {
        matches!(
            name,
            "inbox" | "sent" | "drafts" | "spam" | "trash" | "archive" | "starred"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_imap_utf7_simple() {
        // 非 UTF-7 编码的字符串应原样返回
        assert_eq!(FolderRepository::decode_imap_utf7("INBOX").unwrap(), "INBOX");
        assert_eq!(FolderRepository::decode_imap_utf7("Sent Items").unwrap(), "Sent Items");
    }

    #[test]
    fn test_decode_imap_utf7_ampersand() {
        // &- 表示 & 字符
        assert_eq!(FolderRepository::decode_imap_utf7("Test-&-Item").unwrap(), "Test-&Item");
    }

    #[test]
    fn test_map_folder_name_inbox() {
        assert_eq!(FolderRepository::map_folder_name("INBOX"), "inbox");
        assert_eq!(FolderRepository::map_folder_name("inbox"), "inbox");
    }

    #[test]
    fn test_map_folder_name_sent() {
        assert_eq!(FolderRepository::map_folder_name("Sent Items"), "sent");
        assert_eq!(FolderRepository::map_folder_name("已发送"), "sent");
        assert_eq!(FolderRepository::map_folder_name("sent"), "sent");
    }

    #[test]
    fn test_map_folder_name_standard() {
        assert_eq!(FolderRepository::map_folder_name("Drafts"), "drafts");
        assert_eq!(FolderRepository::map_folder_name("Junk"), "spam");
        assert_eq!(FolderRepository::map_folder_name("Trash"), "trash");
        assert_eq!(FolderRepository::map_folder_name("Archive"), "archive");
    }

    #[test]
    fn test_is_standard_folder() {
        assert!(FolderRepository::is_standard_folder("inbox"));
        assert!(FolderRepository::is_standard_folder("sent"));
        assert!(FolderRepository::is_standard_folder("drafts"));
        assert!(FolderRepository::is_standard_folder("spam"));
        assert!(FolderRepository::is_standard_folder("trash"));
        assert!(FolderRepository::is_standard_folder("archive"));
        assert!(FolderRepository::is_standard_folder("starred"));
        assert!(!FolderRepository::is_standard_folder("custom"));
    }
}
