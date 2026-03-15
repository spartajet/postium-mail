// 测试附件数据
#[test]
fn test_check_attachments() {
    // 连接数据库
    let db_path = "postium-mail.db";
    println!("检查数据库: {}", db_path);

    // 尝试使用 rusqlite 直接查询
    if let Ok(conn) = rusqlite::Connection::open(db_path) {
        // 查询附件表中的数据
        let mut stmt = conn.prepare("SELECT id, email_id, filename, size FROM attachments LIMIT 10").unwrap();

        let attachment_count = stmt
            .query_map([], |row| {
                let id: i32 = row.get(0)?;
                let email_id: i32 = row.get(1)?;
                let filename: String = row.get(2)?;
                let size: i32 = row.get(3)?;
                println!("附件: id={}, email_id={}, filename='{}', size={} bytes", id, email_id, filename, size);
                Ok(())
            })
            .unwrap()
            .count();

        println!("总共找到 {} 个附件记录", attachment_count);
    } else {
        println!("无法打开数据库");
    }
}
