//! 邮件全文搜索模块
//!
//! 提供基于 SQLite FTS5 全文搜索扩展的邮件搜索功能。
//!
//! # 核心功能
//!
//! - **全文搜索**: 基于 FTS5 的高效全文搜索
//! - **多字段搜索**: 同时搜索主题、发件人、正文
//! - **简单搜索**: 使用 LIKE 的备用搜索方案
//! - **搜索历史**: 热门搜索关键词（TODO）
//!
//! # FTS5 全文搜索
//!
//! ## 什么是 FTS5
//!
//! FTS (Full-Text Search) 是 SQLite 的全文搜索扩展，FTS5 是最新版本：
//!
//! - 高效的倒排索引
//! - 支持布尔查询、短语查询、前缀查询
//! - 支持中文分词（需配置 simple tokenizer）
//!
//! ## 虚拟表结构
//!
//! ```sql
//! CREATE VIRTUAL TABLE emails_fts USING fts5(
//!     subject,
//!     sender_email,
//!     sender_name,
//!     body_text,
//!     content=emails,
//!     content_rowid=rowid
//! );
//! ```
//!
//! # 搜索语法
//!
//! ## 基本搜索
//!
//! ```text
//! hello           → 包含 "hello" 的邮件
//! "hello world"   → 精确短语 "hello world"
//! ```
//!
//! ## 布尔操作
//!
//! ```text
//! hello OR world  → 包含 "hello" 或 "world"
//! hello NOT world → 包含 "hello" 但不含 "world"
//! hello AND world → 同时包含 "hello" 和 "world"
//! ```
//!
//! ## 前缀查询
//!
//! ```text
//! hel*            → 以 "hel" 开头的单词
//! ```
//!
//! # 使用示例
//!
//! ## 基本搜索
//!
//! ```rust,no_run
//! # use crate::storage::SearchService;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let results = SearchService::search_emails(
//!     &db,
//!     Some(1),              // 账号 ID（None 表示所有账号）
//!     "重要通知",           // 搜索关键词
//!     Some(50),             // 最大结果数
//! ).await?;
//!
//! for result in results {
//!     println!("{} - {}", result.subject, result.sender_email);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## 简单搜索（备用）
//!
//! 当 FTS5 不可用时，使用 LIKE 搜索：
//!
//! ```rust,no_run
//! # use crate::storage::SearchService;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let results = SearchService::simple_search(
//!     &db,
//!     Some(1),
//!     "重要",
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # 查询处理
//!
//! ## 特殊字符转义
//!
//! FTS5 查询中的单引号需要转义为双引号：
//!
//! ```text
//! John's email → John''s email
//! ```
//!
//! ## 空格处理
//!
//! 包含空格的查询自动转为短语搜索：
//!
//! ```text
//! "hello world" → MATCH '"hello world"'
//! ```
//!
//! # 性能优化
//!
//! - FTS5 使用倒排索引，搜索速度快
//! - 限制返回结果数量（默认 50）
//! - 结果按时间倒序排列
//! - 考虑添加搜索结果缓存
//!
//! # 安全注意事项
//!
//! - FTS5 MATCH 不支持参数化查询
//! - 使用字符串拼接，需要手动转义特殊字符
//! - 单引号转义为双引号防止 SQL 注入
//! - 未来考虑使用更安全的查询方式

use crate::storage::models::email;
use crate::error::{MailError, Result, StorageError};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbConn, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};

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
        // 注意：FTS5 的 MATCH 子句不能使用参数化查询（SQLite 限制）
        // 我们对输入进行转义处理以缓解 SQL 注入风险
        // TODO: 未来考虑使用 sea-orm 的 from_raw_sql 配合 Entity 来提升安全性
        let limit_val = limit.unwrap_or(50);
        let sql = if let Some(acc_id) = account_id {
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
        };

        let results = db
            .query_all_raw(Statement::from_string(DbBackend::Sqlite, &sql))
            .await
            .map_err(|e| MailError::Storage(StorageError::Database(format!("FTS5 搜索失败: {}", e))))?;

        let mut search_results = Vec::new();
        for row in results {
            // 解析查询结果
            // 注意：这里使用 try_get 方法
            let id: i32 = row.try_get_by_index(0).unwrap_or(0);
            let subject: String = row.try_get_by_index(1).unwrap_or("".to_string());
            let sender_email: String = row.try_get_by_index(2).unwrap_or("".to_string());
            let sender_name: Option<String> = row.try_get_by_index(3).ok();
            let body_text: Option<String> = row.try_get_by_index(4).ok();
            let folder: String = row.try_get_by_index(5).unwrap_or("inbox".to_string());
            let sent_at: i64 = row.try_get_by_index(6).unwrap_or(0);
            let account_id: i32 = row.try_get_by_index(7).unwrap_or(0);

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
            .map_err(|e| MailError::Storage(StorageError::Database(format!("简单搜索失败: {}", e))))?;

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
