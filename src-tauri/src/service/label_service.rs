// ═════════════════════════════════════════════════════════════════════════
// 标签服务模块 (Label Service)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责邮件标签的管理，包括：
// 1. 标签的创建、查询、更新和删除（CRUD 操作）
// 2. 标签与邮件的关联管理（添加、移除、查询）
// 3. 按标签查询邮件
//
// 设计特点：
// - 标签是用户自定义的分类系统，与 IMAP 文件夹独立
// - 一封邮件可以有多个标签
// - 标签带有颜色，用于视觉区分
// - 标签作用域限定在账号内（account_id）
//
// 使用场景：
// - 用户为重要邮件添加"重要"标签
// - 使用"工作"、"个人"等标签分类邮件
// - 使用颜色标签快速识别邮件类型
// ═════════════════════════════════════════════════════════════════════════

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::entities::labels;
use crate::infrastructure::storage::repository::label_repo;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use specta::Type;

// ═════════════════════════════════════════════════════════════════════════
// 数据传输对象 (DTO)
// ═════════════════════════════════════════════════════════════════════════

/// 标签数据传输对象
///
/// 这是标签信息对外展示的标准格式，用于前后端数据交换。
///
/// # 字段说明
///
/// - `id`: 标签在数据库中的唯一标识
/// - `account_id`: 所属账号 ID，标签是账号级别的资源
/// - `name`: 标签名称（用户自定义，如"工作"、"重要"）
/// - `color`: 标签颜色（十六进制颜色值，如 "#ff5722"）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LabelDto {
    /// 数据库主键 ID
    pub id: i32,
    /// 所属账号 ID
    pub account_id: i32,
    /// 标签名称
    pub name: String,
    /// 标签颜色（十六进制）
    pub color: String,
}

/// 创建标签请求
///
/// 前端调用创建标签 API 时传递的参数。
///
/// # 必填字段
///
/// - `account_id`: 所属账号 ID
/// - `name`: 标签名称
/// - `color`: 标签颜色
///
/// # 使用示例
///
/// ```typescript,ignore
/// const req: CreateLabelRequest = {
///     account_id: 1,
///     name: '重要',
///     color: '#ff5722'
/// };
/// const label = await createLabel(req);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateLabelRequest {
    /// 所属账号 ID
    pub account_id: i32,
    /// 标签名称
    pub name: String,
    /// 标签颜色
    pub color: String,
}

/// 更新标签请求
///
/// 用于更新标签的部分信息，所有字段都是可选的。
/// 只更新提供的字段，未提供的字段保持不变。
///
/// # 可更新字段
///
/// - `name`: 标签名称
/// - `color`: 标签颜色
///
/// # 注意事项
///
/// - 标签 ID 不能更新，需要删除后重新创建
/// - 所属账号 ID 不能更新
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLabelRequest {
    /// 新的标签名称
    pub name: Option<String>,
    /// 新的标签颜色
    pub color: Option<String>,
}

// ═════════════════════════════════════════════════════════════════════════
// 标签服务实现
// ═════════════════════════════════════════════════════════════════════════

/// 标签服务
///
/// 负责所有标签相关的业务逻辑。
///
/// # 字段
///
/// - `db`: 数据库连接，用于持久化操作
///
/// # 主要功能
///
/// - 标签的 CRUD 操作
/// - 标签与邮件的关联管理
/// - 按标签查询邮件
pub struct LabelService {
    /// 数据库连接
    db: DbConn,
}

impl LabelService {
    /// 创建新的标签服务实例
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    ///
    /// # 返回
    ///
    /// 返回初始化好的 LabelService 实例
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// 获取账号的所有标签列表
    ///
    /// 查询指定账号下的所有标签，按创建时间倒序排列。
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    ///
    /// # 返回
    ///
    /// 成功时返回标签 DTO 向量，失败时返回错误。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let labels = label_service.list_labels(1).await?;
    /// for label in labels {
    ///     println!("标签: {} ({})", label.name, label.color);
    /// }
    /// ```
    pub async fn list_labels(&self, account_id: i32) -> Result<Vec<LabelDto>, MailError> {
        // 从数据库查询该账号的所有标签
        let labels = label_repo::list_by_account(&self.db, account_id).await?;

        // 转换为 DTO 格式
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

    /// 创建新标签
    ///
    /// 为指定账号创建一个新的自定义标签。
    ///
    /// # 参数
    ///
    /// - `req`: 创建标签请求对象
    ///
    /// # 返回
    ///
    /// 成功时返回创建的标签 DTO，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户创建新的标签分类
    /// - 系统预设标签的初始化
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let req = CreateLabelRequest {
    ///     account_id: 1,
    ///     name: "工作".to_string(),
    ///     color: "#2196f3".to_string(),
    /// };
    /// let label = label_service.create_label(req).await?;
    /// ```
    pub async fn create_label(&self, req: CreateLabelRequest) -> Result<LabelDto, MailError> {
        // 记录日志，便于追踪
        tracing::info!(account_id = req.account_id, name = %req.name, "创建标签");

        // 获取当前时间戳
        let now = chrono::Utc::now().timestamp();

        // 构建数据库模型
        let model = labels::ActiveModel {
            account_id: Set(req.account_id),
            name: Set(req.name),
            color: Set(req.color),
            created_at: Set(now),
            ..Default::default()
        };

        // 持久化到数据库
        let created = label_repo::create(&self.db, model).await?;

        // 转换为 DTO 格式返回
        Ok(LabelDto {
            id: created.id,
            account_id: created.account_id,
            name: created.name,
            color: created.color,
        })
    }

    /// 更新标签信息
    ///
    /// 更新标签的可编辑字段：名称、颜色等。
    ///
    /// # 参数
    ///
    /// - `id`: 标签 ID
    /// - `req`: 更新标签请求对象
    ///
    /// # 返回
    ///
    /// 成功时返回更新后的标签 DTO，失败时返回错误。
    ///
    /// # 注意事项
    ///
    /// - 标签 ID 不能更新
    /// - 所属账号 ID 不能更新
    /// - 如果只更新部分字段，其他字段保持不变
    pub async fn update_label(
        &self,
        id: i32,
        req: UpdateLabelRequest,
    ) -> Result<LabelDto, MailError> {
        // 检查标签是否存在
        let existing = label_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::LabelNotFound(id))?;

        // 构建更新模型，保留未修改的字段
        let model = labels::ActiveModel {
            id: Set(existing.id),
            account_id: Set(existing.account_id),
            // 如果提供了新名称则更新，否则保持原值
            name: Set(req.name.unwrap_or(existing.name)),
            // 如果提供了新颜色则更新，否则保持原值
            color: Set(req.color.unwrap_or(existing.color)),
            created_at: Set(existing.created_at),
        };

        // 保存到数据库
        let updated = label_repo::update(&self.db, id, model).await?;

        // 转换为 DTO 格式返回
        Ok(LabelDto {
            id: updated.id,
            account_id: updated.account_id,
            name: updated.name,
            color: updated.color,
        })
    }

    /// 删除标签
    ///
    /// 删除指定的标签及其与邮件的关联关系。
    ///
    /// # 参数
    ///
    /// - `id`: 要删除的标签 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 安全提示
    ///
    /// - 此操作不可逆
    /// - 删除标签会自动移除所有邮件与该标签的关联
    /// - 邮件本身不会被删除
    /// - 建议在前端显示确认对话框
    ///
    /// # 示例（前端确认对话框）
    ///
    /// ```typescript,ignore
    /// if (confirm(`确定要删除标签 "${label.name}" 吗？`)) {
    ///     await deleteLabel(label.id);
    /// }
    /// ```
    pub async fn delete_label(&self, id: i32) -> Result<(), MailError> {
        tracing::info!(id, "删除标签");

        // 从数据库删除标签
        // 关联表会自动级联删除（根据外键配置）
        label_repo::delete(&self.db, id).await
    }

    /// 为邮件添加标签
    ///
    /// 建立邮件与标签的关联关系。
    /// 如果关联已存在，不会重复添加。
    ///
    /// # 参数
    ///
    /// - `email_id`: 邮件 ID
    /// - `label_id`: 标签 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户手动为邮件添加标签
    /// - 邮件分类的自动化规则
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 为重要邮件添加"重要"标签
    /// label_service.add_label_to_email(123, 1).await?;
    /// ```
    pub async fn add_label_to_email(&self, email_id: i32, label_id: i32) -> Result<(), MailError> {
        label_repo::add_label_to_email(&self.db, email_id, label_id).await
    }

    /// 从邮件移除标签
    ///
    /// 移除邮件与标签的关联关系。
    ///
    /// # 参数
    ///
    /// - `email_id`: 邮件 ID
    /// - `label_id`: 标签 ID
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户手动移除邮件标签
    /// - 标签分类的调整
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 移除邮件的"重要"标签
    /// label_service.remove_label_from_email(123, 1).await?;
    /// ```
    pub async fn remove_label_from_email(
        &self,
        email_id: i32,
        label_id: i32,
    ) -> Result<(), MailError> {
        label_repo::remove_label_from_email(&self.db, email_id, label_id).await
    }

    /// 获取邮件的所有标签
    ///
    /// 查询指定邮件关联的所有标签。
    ///
    /// # 参数
    ///
    /// - `email_id`: 邮件 ID
    ///
    /// # 返回
    ///
    /// 成功时返回标签 DTO 向量，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 在邮件详情页显示标签
    /// - 在邮件列表显示标签徽章
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let labels = label_service.get_labels_for_email(123).await?;
    /// for label in labels {
    ///     println!("标签: {}", label.name);
    /// }
    /// ```
    pub async fn get_labels_for_email(&self, email_id: i32) -> Result<Vec<LabelDto>, MailError> {
        // 从数据库查询该邮件的所有标签
        let labels = label_repo::get_labels_for_email(&self.db, email_id).await?;

        // 转换为 DTO 格式
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

    /// 获取标签下的所有邮件 ID
    ///
    /// 查询关联了指定标签的所有邮件 ID。
    ///
    /// # 参数
    ///
    /// - `label_id`: 标签 ID
    ///
    /// # 返回
    ///
    /// 成功时返回邮件 ID 向量，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 点击标签显示相关邮件
    /// - 标签筛选功能
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let email_ids = label_service.list_emails_by_label(1).await?;
    /// println!("该标签下有 {} 封邮件", email_ids.len());
    /// ```
    pub async fn list_emails_by_label(&self, label_id: i32) -> Result<Vec<i32>, MailError> {
        label_repo::list_emails_by_label(&self.db, label_id).await
    }
}
