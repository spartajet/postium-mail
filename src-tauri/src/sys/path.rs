//! 系统路径工具模块
//!
//! 提供获取应用数据目录、数据库文件路径、附件存储目录等功能。

use std::path::PathBuf;

use crate::error::{MailError, Result, StorageError};

/// 获取用户数据目录
/// 默认为 ~/.postium
pub fn get_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| MailError::Storage(StorageError::Io("无法找到用户主目录".to_string())))?;

    let postium_dir = home.join(".postium");

    // 确保目录存在
    std::fs::create_dir_all(&postium_dir)
        .map_err(|e| MailError::Storage(StorageError::Io(format!("无法创建 .postium 目录: {}", e))))?;

    Ok(postium_dir)
}

/// 获取数据库文件路径
/// 返回 ~/.postium/postium.sqlite
pub fn get_db_path() -> Result<String> {
    let db_path = get_data_dir()?.join("postium.sqlite");

    Ok(db_path
        .to_str()
        .ok_or_else(|| MailError::Storage(StorageError::Io("路径转换失败".to_string())))?
        .to_string())
}

/// 获取附件存储目录
/// 返回 ~/.postium/attachments/
pub fn get_attachments_dir() -> Result<PathBuf> {
    let attachments_dir = get_data_dir()?.join("attachments");

    // 确保目录存在
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| MailError::Storage(StorageError::Io(format!("无法创建附件目录: {}", e))))?;

    Ok(attachments_dir)
}
