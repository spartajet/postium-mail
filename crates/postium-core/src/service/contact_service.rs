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
use crate::infrastructure::csv_contacts;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::contact_repo::{
    self, ContactDto, ContactGroupWithCount, ContactListResponse, ContactWrite,
};
use crate::infrastructure::vcard;
use crate::service::email_service::ImportReport;

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

    // ===== 导入 / 导出（vCard / CSV）=====

    /// 导入 vCard 文本（支持多张卡片）
    ///
    /// 每张卡片 `emails[0]` 为主邮箱（无邮箱的卡片跳过并记入 `errors`），
    /// 其余邮箱并入 `extra_emails`；姓名取 `FN`，缺省回退 `N` 原始值；
    /// `CATEGORIES` 映射为分组名，导入结束后统一建组并挂成员
    /// （重名分组复用已有 ID）。邮箱已存在（忽略大小写）的卡片跳过计数。
    pub async fn import_contacts_vcard(&self, text: String) -> Result<ImportReport, MailError> {
        let cards = split_vcard_texts(&text);
        if cards.is_empty() {
            return Err(MailError::InvalidParam(
                "vCard 文本无效：未找到 BEGIN:VCARD 卡片".to_string(),
            ));
        }

        let mut entries: Vec<ImportEntry> = Vec::with_capacity(cards.len());
        for card in cards {
            match vcard::parse(&card) {
                Ok(data) => {
                    let label = data
                        .fn_name
                        .clone()
                        .or_else(|| data.n_name.clone())
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| "无名称卡片".to_string());
                    let label = format!("vCard「{label}」");
                    let Some(primary) = data.emails.first().cloned() else {
                        entries.push(ImportEntry::skipped(label, "无邮箱，已跳过"));
                        continue;
                    };
                    // 其余邮箱并入 extra_emails（忽略大小写去重、排除主邮箱）
                    let primary_key = primary.to_lowercase();
                    let mut extra_emails: Vec<String> = Vec::new();
                    for email in &data.emails[1..] {
                        let key = email.trim().to_lowercase();
                        if key != primary_key
                            && !extra_emails.iter().any(|e| e.to_lowercase() == key)
                        {
                            extra_emails.push(email.trim().to_string());
                        }
                    }
                    let write = ContactWrite {
                        email: primary,
                        extra_emails: Some(extra_emails),
                        name: data
                            .fn_name
                            .clone()
                            .filter(|name| !name.trim().is_empty())
                            .or_else(|| data.n_name.clone()),
                        phones: Some(data.phones.clone()),
                        notes: data.note.clone(),
                        org: data.org.clone(),
                    };
                    entries.push(ImportEntry::importable(label, write, data.categories.clone()));
                }
                Err(err) => {
                    entries.push(ImportEntry::skipped(
                        "vCard 卡片".to_string(),
                        format!("解析失败: {err}"),
                    ));
                }
            }
        }
        self.import_entries(entries).await
    }

    /// 按过滤条件导出联系人为 vCard 文本（VERSION:3.0，多卡片拼接）
    ///
    /// 过滤条件与 [`Self::list`] 一致（关键词 / 分组 / 仅星标），导出全部
    /// 匹配项（自动翻页取全量）。`groups` 映射为 `CATEGORIES`。
    pub async fn export_contacts_vcard(
        &self,
        query: Option<String>,
        group_id: Option<i32>,
        starred_only: bool,
    ) -> Result<String, MailError> {
        let contacts = self
            .list_all(query.as_deref(), group_id, starred_only)
            .await?;
        let mut out = String::new();
        for contact in contacts {
        let mut data = vcard::VcardData {
            fn_name: contact.name.clone().filter(|name| !name.trim().is_empty()),
            ..Default::default()
        };
            let primary_key = contact.email.to_lowercase();
            data.emails.push(contact.email.clone());
            data.emails.extend(
                contact
                    .extra_emails
                    .iter()
                    .filter(|email| email.to_lowercase() != primary_key)
                    .cloned(),
            );
            data.phones = contact.phones.clone();
            data.org = contact.org.clone();
            data.note = contact.notes.clone();
            data.categories = contact.groups.iter().map(|g| g.name.clone()).collect();
            out.push_str(&vcard::emit(&data));
        }
        Ok(out)
    }

    /// 导入 CSV 文本（Gmail / Outlook / 通用方言自动嗅探）
    ///
    /// 无表头或无法定位邮箱列时报 `MailError::InvalidParam`；缺邮箱的数据行
    /// 跳过并记入 `errors`；`group` 列映射为分组名，导入结束后统一建组挂成员。
    pub async fn import_contacts_csv(&self, text: String) -> Result<ImportReport, MailError> {
        let outcome = csv_contacts::parse_contacts(&text)?;
        let entries: Vec<ImportEntry> = outcome
            .rows
            .into_iter()
            .map(|row| {
                let label = row.write.email.clone();
                ImportEntry::importable(label, row.write, row.groups)
            })
            .collect();
        let mut report = self.import_entries(entries).await?;
        // CSV 层的跳过（缺邮箱行）并入报告
        report.skipped += outcome.skipped;
        report.errors.extend(outcome.errors);
        Ok(report)
    }

    /// 按过滤条件导出联系人为 CSV 文本
    ///
    /// 固定表头 `name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency`，
    /// UTF-8 BOM 开头、CRLF 行结束、值含逗号/引号/换行时 RFC 4180 转义。
    /// 多值单元格（额外邮箱/额外电话/分组）以分号分隔。
    pub async fn export_contacts_csv(
        &self,
        query: Option<String>,
        group_id: Option<i32>,
        starred_only: bool,
    ) -> Result<String, MailError> {
        let contacts = self
            .list_all(query.as_deref(), group_id, starred_only)
            .await?;
        let mut rows: Vec<Vec<String>> = vec![csv_contacts::EXPORT_COLUMNS
            .iter()
            .map(|column| (*column).to_string())
            .collect()];
        for contact in &contacts {
            let phone = contact.phones.first().cloned().unwrap_or_default();
            let phones_extra = contact.phones.get(1..).map(|rest| rest.to_vec()).unwrap_or_default();
            rows.push(vec![
                contact.name.clone().unwrap_or_default(),
                contact.email.clone(),
                contact.extra_emails.join(";"),
                phone,
                phones_extra.join(";"),
                contact.org.clone().unwrap_or_default(),
                contact.notes.clone().unwrap_or_default(),
                contact
                    .groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect::<Vec<_>>()
                    .join(";"),
                if contact.is_starred { "true" } else { "false" }.to_string(),
                contact.frequency.to_string(),
            ]);
        }
        Ok(csv_contacts::render_csv(&rows))
    }

    /// 分页取全量联系人（导出用；500/页循环取满 `total`）
    async fn list_all(
        &self,
        query: Option<&str>,
        group_id: Option<i32>,
        starred_only: bool,
    ) -> Result<Vec<ContactDto>, MailError> {
        const PAGE_SIZE: usize = 500;
        let mut all: Vec<ContactDto> = Vec::new();
        let mut page = 1usize;
        loop {
            let response =
                contact_repo::list(&self.db, query, group_id, starred_only, page, PAGE_SIZE)
                    .await?;
            let total = response.total as usize;
            let page_contacts = response.contacts;
            let exhausted = page_contacts.len() < PAGE_SIZE;
            all.extend(page_contacts);
            if all.len() >= total || exhausted {
                break;
            }
            page += 1;
        }
        Ok(all)
    }

    /// 批量导入条目：逐条 `insert`（重复邮箱跳过计数），结束后统一建组挂成员
    ///
    /// 分组名通过 `group_insert` 自动建组；重名报"分组已存在"时用
    /// `group_list` 查回已有 ID 复用（结果按组名缓存）。
    async fn import_entries(&self, entries: Vec<ImportEntry>) -> Result<ImportReport, MailError> {
        let mut report = ImportReport::default();
        let mut memberships: Vec<(i32, Vec<String>)> = Vec::new();

        for entry in entries {
            match entry.item {
                Err(reason) => {
                    report.skipped += 1;
                    report.errors.push(format!("{}：{reason}", entry.label));
                }
                Ok((write, groups)) => match contact_repo::insert(&self.db, write).await {
                    Ok(id) => {
                        report.imported += 1;
                        if !groups.is_empty() {
                            memberships.push((id, groups));
                        }
                    }
                    Err(MailError::DatabaseError(message))
                        if message.contains("联系人已存在") =>
                    {
                        report.skipped += 1;
                        report.errors.push(format!("{}：{message}", entry.label));
                    }
                    Err(err) => return Err(err),
                },
            }
        }

        // 统一建组挂成员（仅对本次成功导入的联系人）
        if !memberships.is_empty() {
            let mut group_id_cache: std::collections::HashMap<String, i32> =
                std::collections::HashMap::new();
            for (contact_id, group_names) in memberships {
                let mut group_ids: Vec<i32> = Vec::new();
                for name in group_names {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let group_id = match group_id_cache.get(&name) {
                        Some(&id) => id,
                        None => {
                            let id = match contact_repo::group_insert(&self.db, &name).await {
                                Ok(id) => id,
                                Err(MailError::DatabaseError(message))
                                    if message.contains("分组已存在") =>
                                {
                                    // 组名重复：查回已有分组 ID 复用
                                    contact_repo::group_list(&self.db)
                                        .await?
                                        .into_iter()
                                        .find(|group| group.name == name)
                                        .ok_or(MailError::DatabaseError(message))?
                                        .id
                                }
                                Err(err) => return Err(err),
                            };
                            group_id_cache.insert(name, id);
                            id
                        }
                    };
                    if !group_ids.contains(&group_id) {
                        group_ids.push(group_id);
                    }
                }
                contact_repo::group_set_members(&self.db, contact_id, &group_ids).await?;
            }
        }
        Ok(report)
    }
}

/// 导入条目：可导入的写入数据 + 目标分组；`Err(原因)` 为解析期已判定的跳过项
struct ImportEntry {
    /// 错误报告中的条目标识（卡片 FN / 邮箱）
    label: String,
    item: Result<(ContactWrite, Vec<String>), String>,
}

impl ImportEntry {
    fn importable(label: String, write: ContactWrite, groups: Vec<String>) -> Self {
        Self {
            label,
            item: Ok((write, groups)),
        }
    }

    fn skipped(label: String, reason: impl Into<String>) -> Self {
        Self {
            label,
            item: Err(reason.into()),
        }
    }
}

/// 把多卡片 vCard 文本按 `BEGIN:VCARD` / `END:VCARD` 拆成单卡文本
///
/// 只保留卡片边界内的行；未闭合的尾部卡片也保留（交给 `vcard::parse`
/// 报"缺少 END:VCARD"并计入导入错误）。
fn split_vcard_texts(text: &str) -> Vec<String> {
    let mut cards: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let upper = line.trim_start().to_ascii_uppercase();
        if !inside && upper.starts_with("BEGIN:VCARD") {
            inside = true;
            current.clear();
        }
        if inside {
            current.push_str(line);
            current.push('\n');
            if upper.starts_with("END:VCARD") {
                cards.push(std::mem::take(&mut current));
                inside = false;
            }
        }
    }
    if inside {
        cards.push(current);
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn service() -> ContactService {
        ContactService::new(DbConn::open_in_memory_for_test().await.unwrap())
    }

    /// 联系人可比较签名（导入导出等价性断言用；忽略顺序敏感字段）：
    /// (email, name, extra_emails, phones, notes, org, group_names)
    type ContactSignature = (
        String,
        Option<String>,
        Vec<String>,
        Vec<String>,
        Option<String>,
        Option<String>,
        Vec<String>,
    );

    fn signature(c: &ContactDto) -> ContactSignature {
        let mut extras = c.extra_emails.clone();
        extras.sort();
        let mut phones = c.phones.clone();
        phones.sort();
        let mut groups: Vec<String> = c.groups.iter().map(|g| g.name.clone()).collect();
        groups.sort();
        (
            c.email.clone(),
            c.name.clone(),
            extras,
            phones,
            c.notes.clone(),
            c.org.clone(),
            groups,
        )
    }

    fn signatures(contacts: &[ContactDto]) -> Vec<ContactSignature> {
        let mut out: Vec<ContactSignature> = contacts.iter().map(signature).collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    const TWO_CARDS_PLUS_NO_EMAIL: &str = concat!(
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:Alice\r\n",
        "EMAIL;TYPE=INTERNET:alice@postium.test\r\n",
        "EMAIL;TYPE=INTERNET:alice.alt@postium.test\r\n",
        "TEL:13800000000\r\n",
        "ORG:Postium\r\n",
        "NOTE:会议认识\r\n",
        "CATEGORIES:工作,朋友\r\n",
        "END:VCARD\r\n",
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:Bob\r\n",
        "EMAIL;TYPE=INTERNET:bob@postium.test\r\n",
        "END:VCARD\r\n",
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:NoEmail\r\n",
        "TEL:13811111111\r\n",
        "END:VCARD\r\n",
    );

    #[tokio::test]
    async fn import_vcard_two_cards_duplicate_skip_and_no_email_card() {
        let svc = service().await;

        // 首次导入：两张有效卡 imported=2，无邮箱卡 skipped=1
        let report = svc
            .import_contacts_vcard(TWO_CARDS_PLUS_NO_EMAIL.to_string())
            .await
            .unwrap();
        assert_eq!(report.imported, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].contains("无邮箱"), "错误信息应说明无邮箱: {}", report.errors[0]);

        // Alice 字段完整落库 + 分组挂载
        let alice = svc.get_by_email("alice@postium.test").await.unwrap().unwrap();
        assert_eq!(alice.name.as_deref(), Some("Alice"));
        assert_eq!(alice.extra_emails, vec!["alice.alt@postium.test"]);
        assert_eq!(alice.phones, vec!["13800000000"]);
        assert_eq!(alice.org.as_deref(), Some("Postium"));
        assert_eq!(alice.notes.as_deref(), Some("会议认识"));
        let mut group_names: Vec<&str> = alice.groups.iter().map(|g| g.name.as_str()).collect();
        group_names.sort_unstable();
        assert_eq!(group_names, vec!["工作", "朋友"], "CATEGORIES 应映射为分组");

        let bob = svc.get_by_email("bob@postium.test").await.unwrap().unwrap();
        assert_eq!(bob.name.as_deref(), Some("Bob"));
        assert!(bob.groups.is_empty());

        // 分组各建一次
        let groups = svc.group_list().await.unwrap();
        assert_eq!(groups.len(), 2);

        // 再次导入同一文本：三张卡全部跳过，分组不重复创建
        let report = svc
            .import_contacts_vcard(TWO_CARDS_PLUS_NO_EMAIL.to_string())
            .await
            .unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.skipped, 3);
        assert_eq!(report.errors.len(), 3);
        assert!(
            report.errors.iter().filter(|e| e.contains("联系人已存在")).count() == 2,
            "两张重复卡应记联系人已存在: {:?}",
            report.errors
        );
        assert_eq!(svc.group_list().await.unwrap().len(), 2, "分组不得重复创建");
    }

    /// M-1：组名复用分支——预建同名分组，导入 CATEGORIES 同名卡片应挂入既有组
    #[tokio::test]
    async fn import_vcard_reuses_preexisting_group_by_name() {
        let svc = service().await;
        let group_id = svc.group_insert("同事").await.unwrap();

        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:3.0\r\n",
            "FN:Carol\r\n",
            "EMAIL;TYPE=INTERNET:carol@postium.test\r\n",
            "CATEGORIES:同事\r\n",
            "END:VCARD\r\n",
        );
        let report = svc.import_contacts_vcard(text.to_string()).await.unwrap();
        assert_eq!(
            report,
            ImportReport { imported: 1, skipped: 0, errors: Vec::new() },
            "重名分组应静默复用，不产生任何错误"
        );

        let carol = svc.get_by_email("carol@postium.test").await.unwrap().unwrap();
        assert_eq!(carol.groups.len(), 1, "应挂入分组");
        assert_eq!(carol.groups[0].id, group_id, "应挂入既有分组（同 ID）");
        assert_eq!(carol.groups[0].name, "同事");

        let groups = svc.group_list().await.unwrap();
        assert_eq!(groups.len(), 1, "分组不得重复创建");
        assert_eq!(groups[0].id, group_id);
        assert_eq!(groups[0].count, 1, "既有分组成员计数应 +1");
    }

    #[tokio::test]
    async fn export_vcard_then_import_equivalent() {
        let svc = service().await;
        let work_id = svc.group_insert("工作").await.unwrap();
        let friend_id = svc.group_insert("朋友").await.unwrap();

        svc.create(ContactWrite {
            email: "Alice@Postium.Test".into(),
            extra_emails: Some(vec!["alice.alt@postium.test".into()]),
            name: Some("Alice".into()),
            phones: Some(vec!["13800000000".into()]),
            notes: Some("会议认识".into()),
            org: Some("Postium".into()),
        })
        .await
        .unwrap();
        let bob = svc
            .create(ContactWrite {
                email: "bob@postium.test".into(),
                name: Some("Bob".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        svc.set_starred(bob.id, true).await.unwrap();
        svc.group_set_members(bob.id, &[work_id, friend_id]).await.unwrap();

        // 导出（按邮箱关键词过滤 Alice）
        let text = svc
            .export_contacts_vcard(Some("alice".into()), None, false)
            .await
            .unwrap();
        assert!(text.contains("BEGIN:VCARD"));
        assert!(text.contains("alice@postium.test"));
        assert!(!text.contains("bob@postium.test"), "过滤条件应生效");

        // 全量导出 → 新库导入 → 联系人字段等价
        let full = svc.export_contacts_vcard(None, None, false).await.unwrap();
        let target = service().await;
        let report = target.import_contacts_vcard(full.clone()).await.unwrap();
        assert_eq!(report.imported, 2, "导出再导入应全部成功: {report:?}");

        let source_list = svc.list(None, None, false, 1, 100).await.unwrap();
        let target_list = target.list(None, None, false, 1, 100).await.unwrap();
        assert_eq!(signatures(&source_list.contacts), signatures(&target_list.contacts));

        // 对同一库重复导入：分组已存在路径（group_insert 失败 → group_list 查回）
        let report = target.import_contacts_vcard(full).await.unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.skipped, 2);
        assert_eq!(target.group_list().await.unwrap().len(), 2, "分组复用不重建");

        // 仅星标导出：只有 Bob（星标在源库；vCard 不携带星标，目标库无星标）
        let starred = svc.export_contacts_vcard(None, None, true).await.unwrap();
        assert!(starred.contains("bob@postium.test"));
        assert!(!starred.contains("alice@postium.test"));
        assert!(
            target.export_contacts_vcard(None, None, true).await.unwrap().is_empty(),
            "目标库无星标联系人，星标导出应为空"
        );
    }

    #[tokio::test]
    async fn import_csv_generic_and_export_csv_bom_roundtrip() {
        let svc = service().await;

        // 通用方言导入
        let text = concat!(
            "name,email,phone,group\r\n",
            "王五,wang@postium.test,13800000003,重要\r\n",
        );
        let report = svc.import_contacts_csv(text.to_string()).await.unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped, 0);
        let wang = svc.get_by_email("wang@postium.test").await.unwrap().unwrap();
        assert_eq!(wang.name.as_deref(), Some("王五"));
        assert_eq!(wang.phones, vec!["13800000003"]);
        assert_eq!(wang.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), vec!["重要"]);

        // 构造带危险字符的联系人后导出
        svc.create(ContactWrite {
            email: "alice@postium.test".into(),
            extra_emails: Some(vec!["alice.alt@postium.test".into()]),
            name: Some("Alice, the Admin".into()),
            phones: Some(vec!["13800000002".into(), "13900000002".into()]),
            notes: Some("备注, 含逗号与\"引号\"\n第二行".into()),
            org: Some("Postium".into()),
        })
        .await
        .unwrap();

        let csv_text = svc.export_contacts_csv(None, None, false).await.unwrap();
        assert!(csv_text.starts_with('\u{feff}'), "导出应带 UTF-8 BOM");
        assert!(csv_text.contains("name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency"));

        // BOM 往返：新库导入导出文本 → 联系人字段等价（starred/frequency 为导出附加列，导入不回填）
        let target = service().await;
        let report = target.import_contacts_csv(csv_text).await.unwrap();
        assert_eq!(report.imported, 2, "BOM 往返应全部导入: {report:?}");
        assert_eq!(report.skipped, 0);

        let source_list = svc.list(None, None, false, 1, 100).await.unwrap();
        let target_list = target.list(None, None, false, 1, 100).await.unwrap();
        assert_eq!(signatures(&source_list.contacts), signatures(&target_list.contacts));

        // 分组随 CSV 往返重建
        let wang = target.get_by_email("wang@postium.test").await.unwrap().unwrap();
        assert_eq!(wang.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), vec!["重要"]);
    }
}
