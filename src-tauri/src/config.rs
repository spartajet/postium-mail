use std::path::PathBuf;
use anyhow::Result;

/// 获取用户数据目录
/// 默认为 ~/.postium
pub fn get_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("无法找到用户主目录"))?;

    let postium_dir = home.join(".postium");

    // 确保目录存在
    std::fs::create_dir_all(&postium_dir)
        .map_err(|e| anyhow::anyhow!("无法创建 .postium 目录: {}", e))?;

    Ok(postium_dir)
}

/// 获取数据库文件路径
/// 返回 ~/.postium/postium.db
pub fn get_db_path() -> Result<String> {
    let db_path = get_data_dir()?
        .join("postium.db");

    Ok(db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("路径转换失败"))?
        .to_string())
}

/// 获取附件存储目录
/// 返回 ~/.postium/attachments/
pub fn get_attachments_dir() -> Result<PathBuf> {
    let attachments_dir = get_data_dir()?.join("attachments");

    // 确保目录存在
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| anyhow::anyhow!("无法创建附件目录: {}", e))?;

    Ok(attachments_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_dir() {
        let dir = get_data_dir();
        assert!(dir.is_ok());
        assert!(dir.unwrap().ends_with(".postium"));
    }

    #[test]
    fn test_get_db_path() {
        let path = get_db_path();
        assert!(path.is_ok());
        assert!(path.unwrap().contains(".postium"));
        assert!(path.unwrap().contains("postium.db"));
    }
}
