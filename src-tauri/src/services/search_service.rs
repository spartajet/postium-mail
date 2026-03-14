use sea_orm::{ConnectionTrait, Statement, DbConn, EntityTrait, QueryFilter, ColumnTrait, QuerySelect, QueryOrder, Condition};
use anyhow::{Result, Context};
use crate::models::email;

/// 搜索结果项
#[derive(Debug, serde::Serialize)]
pub struct SearchResult {
    pub id: i32,
    pub subject: String,
    pub sender_email: String,
    pub sender_name: Option<String>,
    pub body_text: Option<String>,
    pub folder: String,
    pub sent_at: i64,
    pub account_id: i32,
}

/// 搜索服务
pub struct SearchService;

impl SearchService {
    /// 全文搜索邮件
    pub async fn search_emails(
        db: &DbConn,
        account_id: Option<i32>,
        query: &str,
        limit: Option<u64>,
    ) -> Result<Vec<SearchResult>> {
        // 处理搜索查询：转义特殊字符
        let processed_query = Self::process_fts_query(query);

        // 执行查询
        // 注意：这里简化实现，实际应该使用参数化查询
        // 由于 SeaORM 的限制，我们使用原始 SQL
        let limit_val = limit.unwrap_or(50);
        let results = db
            .query_all(Statement::from_string(
                db.get_database_backend(),
                if let Some(acc_id) = account_id {
                    format!(
                        "SELECT e.id, e.subject, e.sender_email, e.sender_name, e.body_text, e.folder, e.sent_at, e.account_id \
                         FROM emails e \
                         INNER JOIN emails_fts fts ON e.id = fts.rowid \
                         WHERE e.account_id = {} \
                         AND emails_fts MATCH '{}' \
                         ORDER BY e.sent_at DESC \
                         LIMIT {}",
                        acc_id,
                        processed_query.replace('\'', "''"),
                        limit_val
                    )
                } else {
                    format!(
                        "SELECT e.id, e.subject, e.sender_email, e.sender_name, e.body_text, e.folder, e.sent_at, e.account_id \
                         FROM emails e \
                         INNER JOIN emails_fts fts ON e.id = fts.rowid \
                         WHERE emails_fts MATCH '{}' \
                         ORDER BY e.sent_at DESC \
                         LIMIT {}",
                        processed_query.replace('\'', "''"),
                        limit_val
                    )
                },
            ))
            .await
            .context("FTS5 搜索失败")?;

        let mut search_results = Vec::new();
        for row in results {
            // 解析查询结果
            // 注意：这里使用 try_get 方法
            let id: i32 = row.try_get_by_index(0)
                .unwrap_or(0);
            let subject: String = row.try_get_by_index(1)
                .unwrap_or("".to_string());
            let sender_email: String = row.try_get_by_index(2)
                .unwrap_or("".to_string());
            let sender_name: Option<String> = row.try_get_by_index(3).ok();
            let body_text: Option<String> = row.try_get_by_index(4).ok();
            let folder: String = row.try_get_by_index(5)
                .unwrap_or("inbox".to_string());
            let sent_at: i64 = row.try_get_by_index(6)
                .unwrap_or(0);
            let account_id: i32 = row.try_get_by_index(7)
                .unwrap_or(0);

            search_results.push(SearchResult {
                id,
                subject,
                sender_email,
                sender_name,
                body_text,
                folder,
                sent_at,
                account_id,
            });
        }

        tracing::info!("搜索完成，找到 {} 个结果", search_results.len());
        Ok(search_results)
    }

    /// 简单的搜索（使用 LIKE，作为 Fallback）
    pub async fn simple_search(
        db: &DbConn,
        account_id: Option<i32>,
        query: &str,
    ) -> Result<Vec<email::Model>> {
        // 使用 Condition::any() 组合多个 OR 条件
        let mut condition = Condition::any()
            .add(email::Column::Subject.contains(query))
            .add(email::Column::SenderEmail.contains(query))
            .add(email::Column::BodyText.contains(query));

        // 添加账号过滤
        if let Some(acc_id) = account_id {
            condition = condition.add(email::Column::AccountId.eq(acc_id));
        }

        let results = email::Entity::find()
            .filter(condition)
            .order_by_desc(email::Column::SentAt)
            .limit(50)
            .all(db)
            .await
            .context("简单搜索失败")?;

        Ok(results)
    }

    /// 处理 FTS5 查询字符串
    /// 处理特殊字符和短语搜索
    fn process_fts_query(query: &str) -> String {
        let query = query.trim();

        // 如果查询包含空格，默认为短语搜索
        if query.contains(' ') {
            format!("\"{}\"", query.replace('"', "\"\""))
        } else {
            query.to_string()
        }
    }

    /// 获取热门搜索关键词
    /// TODO: 实现搜索历史记录功能
    pub async fn get_recent_searches(
        _db: &DbConn,
        _account_id: i32,
        _limit: u64,
    ) -> Result<Vec<String>> {
        // 暂时返回空向量
        // 未来可以添加 search_history 表来记录搜索历史
        Ok(vec![])
    }
}
