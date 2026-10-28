// ═════════════════════════════════════════════════════════════════════════
// 联系人服务模块 (Contact Service)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责联系人的业务入口，包括：
// 1. 联系人的查询（分页列表 / 按 ID / 按邮箱）与增删改
// 2. 星标状态切换、联系人合并（N→1）
// 3. 联系人分组的增删改与成员关系管理
// 4. 存量邮件的联系人回填
//
// 设计特点：
// - 本阶段为纯本地薄封装：方法逐一直转 repository 层，无额外业务规则
// - CardDAV 网络同步方法在后续任务（阶段四）扩展到本服务
// - DTO 与写入结构复用 contact_repo 中的定义，服务层不重复建模
// ═════════════════════════════════════════════════════════════════════════

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::contact_repo::{
    self, ContactDto, ContactGroupWithCount, ContactListResponse, ContactWrite,
};

/// 联系人服务
///
/// 联系人与联系人分组相关操作的统一入口。
///
/// # 字段
///
/// - `db`: 数据库连接，用于持久化操作
pub struct ContactService {
    /// 数据库连接
    db: DbConn,
}

impl ContactService {
    /// 创建新的联系人服务实例
    ///
    /// # 参数
    ///
    /// - `db`: 数据库连接
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// 分页查询联系人
    ///
    /// 支持按关键词（姓名 / 邮箱）、所属分组、仅星标三个过滤条件，可任意组合。
    ///
    /// # 参数
    ///
    /// - `query`: 关键词（`None` 或空串表示不过滤）
    /// - `group_id`: 分组 ID（仅返回该分组成员）
    /// - `starred_only`: 仅返回星标联系人
    /// - `page`: 页码（从 1 开始，0 视为第 1 页）
    /// - `limit`: 每页数量
    pub async fn list(
        &self,
        query: Option<String>,
        group_id: Option<i32>,
        starred_only: bool,
        page: usize,
        limit: usize,
    ) -> Result<ContactListResponse, MailError> {
        contact_repo::list(
            &self.db,
            query.as_deref(),
            group_id,
            starred_only,
            page,
            limit,
        )
        .await
    }

    /// 按 ID 查询联系人（含所属分组）
    ///
    /// 找到返回 `Some(dto)`，不存在返回 `None`。
    pub async fn get(&self, id: i32) -> Result<Option<ContactDto>, MailError> {
        contact_repo::get(&self.db, id).await
    }

    /// 按邮箱查询联系人（大小写不敏感，含所属分组）
    ///
    /// 邮件详情页「查看联系人」等按发件人邮箱联动的入口使用。
    pub async fn get_by_email(&self, email: &str) -> Result<Option<ContactDto>, MailError> {
        contact_repo::get_by_email(&self.db, email).await
    }

    /// 创建联系人，返回创建后的完整 DTO
    ///
    /// 邮箱同条目（忽略大小写）已存在时报
    /// `MailError::DatabaseError("联系人已存在: ...")`。
    pub async fn create(&self, w: ContactWrite) -> Result<ContactDto, MailError> {
        let id = contact_repo::insert(&self.db, w).await?;
        contact_repo::get(&self.db, id)
            .await?
            .ok_or_else(|| MailError::DatabaseError(format!("联系人创建后不可读: {id}")))
    }

    /// 按 ID 更新联系人的可编辑字段
    ///
    /// 星标、频次、来源与 CardDAV 元数据保持不变。
    pub async fn update(&self, id: i32, w: ContactWrite) -> Result<(), MailError> {
        contact_repo::update(&self.db, id, w).await
    }

    /// 按 ID 删除联系人
    ///
    /// 所属分组成员关系由外键级联清理。
    pub async fn delete(&self, id: i32) -> Result<(), MailError> {
        contact_repo::delete(&self.db, id).await
    }

    /// 设置联系人的星标状态
    pub async fn set_starred(&self, id: i32, starred: bool) -> Result<(), MailError> {
        contact_repo::set_starred(&self.db, id, starred).await
    }

    /// 合并联系人（N→1），返回合并后的完整 DTO
    ///
    /// 合并规则与防御校验见 `contact_repo::merge`。
    pub async fn merge(&self, primary_id: i32, absorbed_ids: &[i32]) -> Result<ContactDto, MailError> {
        contact_repo::merge(&self.db, primary_id, absorbed_ids).await
    }

    /// 存量邮件回填：聚合 `emails` 表的发件人 / 收件人 / 抄送并入库
    ///
    /// 幂等，可重复执行。返回聚合并处理的地址数量。
    pub async fn backfill(&self) -> Result<usize, MailError> {
        contact_repo::backfill_from_emails(&self.db).await
    }

    /// 查询全部分组（含各分组成员计数）
    pub async fn group_list(&self) -> Result<Vec<ContactGroupWithCount>, MailError> {
        contact_repo::group_list(&self.db).await
    }

    /// 创建联系人分组，返回自增 ID
    ///
    /// 与现有分组重名时报 `MailError::DatabaseError("分组已存在: ...")`。
    pub async fn group_insert(&self, name: &str) -> Result<i32, MailError> {
        contact_repo::group_insert(&self.db, name).await
    }

    /// 重命名分组
    pub async fn group_rename(&self, id: i32, name: &str) -> Result<(), MailError> {
        contact_repo::group_rename(&self.db, id, name).await
    }

    /// 删除分组
    ///
    /// 成员关系由外键级联清理，成员联系人本身不受影响。
    pub async fn group_delete(&self, id: i32) -> Result<(), MailError> {
        contact_repo::group_delete(&self.db, id).await
    }

    /// 全量替换联系人的分组成员关系
    ///
    /// 单事务内执行；传空切片等效于把联系人移出所有分组。
    pub async fn group_set_members(
        &self,
        contact_id: i32,
        group_ids: &[i32],
    ) -> Result<(), MailError> {
        contact_repo::group_set_members(&self.db, contact_id, group_ids).await
    }
}
