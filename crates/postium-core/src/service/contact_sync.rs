//! CardDAV 双向同步引擎（spec §3.5）。
//!
//! 单账号一次同步四个阶段：
//! 1. **拉取**：REPORT addressbook-query 全量 (href, etag)，与本地比对后对
//!    变化集 multiget 逐卡落库（UID 命中更新 / 同邮箱并入 / 新建）；
//!    服务器已消失的本地条目删除。本地已改动的条目（`updated_at > 游标`）
//!    拉取阶段跳过，留给推送阶段以 412 + REV 裁决（last-write-wins）。
//! 2. **推送**：脏条目（上次同步后变更）生成 vCard（dav 条目以服务器 raw
//!    为基底 `merge_into_raw`，本地条目 `emit` 新卡 + `UID:postium-<uuid>`），
//!    `PUT`（有 etag 带 If-Match）；412 → 单卡拉服务端 REV 比较：服务端新
//!    则覆盖本地（conflicts+1），本地新则无 If-Match 重推覆盖服务端。
//! 3. **删除**：`dav_deletes` 墓碑中上次同步后的条目逐个 DELETE（404 视为
//!    成功），确认后清除墓碑。
//! 4. **回写游标**：`binding_touch_synced` 更新 last_sync_at。
//!
//! 防回推：拉取/推送落库统一把 `updated_at` 写为本次同步游标（旧
//! last_sync_at），拉到的行不会在下一轮被当作脏条目推回服务器。

use std::collections::{HashMap, HashSet};

use crate::domain::auth::manager::AuthManager;
use crate::error::MailError;
use crate::infrastructure::carddav::{self, CardDavClient};
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::contact_repo::{self, ContactDto, ContactWrite};
use crate::infrastructure::vcard;

/// 一次同步的结果报告（UI toast 展示用）。
#[derive(Debug, Default, Clone)]
pub struct DavSyncReport {
    /// 从服务器拉取更新/新建的条目数
    pub pulled: usize,
    /// 推送到服务器的条目数（含 412 后本地胜重推成功）
    pub pushed: usize,
    /// 删除数（服务器消失的本地删除 + 本地墓碑的服务器删除）
    pub deleted: usize,
    /// 412 冲突且服务端胜（本地被覆盖）的条目数
    pub conflicts: usize,
    /// 逐条错误信息（单条失败不中断整轮同步）
    pub errors: Vec<String>,
}

/// 对单个 CardDAV 绑定账号执行一次双向同步。
pub async fn sync_account(
    db: &DbConn,
    auth: &AuthManager,
    account_id: i32,
) -> Result<DavSyncReport, MailError> {
    let binding = contact_repo::binding_get(db, account_id)
        .await?
        .ok_or_else(|| MailError::CardDavError(format!("账号 {account_id} 未绑定 CardDAV")))?;
    let Some(ab_href) = binding
        .addressbook_href
        .clone()
        .filter(|href| !href.trim().is_empty())
    else {
        return Err(MailError::CardDavError(
            "绑定缺少通讯录地址，请先在配置中完成「发现通讯录」".into(),
        ));
    };
    let password = auth.get_password(&format!("dav:{account_id}"))?;
    let client = CardDavClient::new(&binding.url, &binding.username, &password)?;
    let mut report = DavSyncReport::default();

    // 同步游标：上次 last_sync_at（首次为 0）。本轮落库一律写回该值，
    // 保证「拉到的行 ≠ 脏行」，不会自推自拉。
    let since = binding.last_sync_at.unwrap_or(0);

    let entries = client.list_entries(&ab_href).await?;
    let etag_of: HashMap<&str, &str> = entries
        .iter()
        .map(|(href, etag)| (href.as_str(), etag.as_str()))
        .collect();

    let local = contact_repo::dav_contacts_for_account(db, account_id).await?;
    let dirty = contact_repo::list_dav_dirty(db, account_id, since).await?;
    let dirty_ids: HashSet<i32> = dirty.iter().map(|d| d.contact.id).collect();

    // ── 阶段 1：拉取变化集 ──────────────────────────────────────────────
    let by_href: HashMap<&str, &ContactDto> = local
        .iter()
        .filter_map(|c| c.dav_href.as_deref().map(|href| (href, c)))
        .collect();
    let mut fetch: Vec<String> = Vec::new();
    for (href, etag) in &entries {
        let stale = match by_href.get(href.as_str()) {
            Some(local_contact) => local_contact.dav_etag.as_deref() != Some(etag.as_str()),
            None => true,
        };
        if stale
            // 本地已改动的条目不拉取覆盖——交给推送阶段 412 裁决
            && !by_href
                .get(href.as_str())
                .is_some_and(|c| dirty_ids.contains(&c.id))
        {
            fetch.push(href.clone());
        }
    }
    if !fetch.is_empty() {
        for (href, text) in client.multiget(&ab_href, &fetch).await? {
            let etag = etag_of.get(href.as_str()).copied().unwrap_or("");
            match apply_pulled_card(db, account_id, &href, etag, &text, since).await {
                Ok(true) => report.pulled += 1,
                Ok(false) => {}
                Err(err) => report.errors.push(format!("拉取 {href}: {err}")),
            }
        }
    }

    // 服务器已消失 → 删本地（repo::delete 会写墓碑，删除阶段 404 视为成功）
    for contact in &local {
        if let Some(href) = &contact.dav_href {
            if !entries.iter().any(|(server_href, _)| server_href == href) {
                contact_repo::delete(db, contact.id).await?;
                report.deleted += 1;
            }
        }
    }

    // ── 阶段 2：推送脏条目 ──────────────────────────────────────────────
    for dirty_item in dirty {
        let contact = dirty_item.contact;
        let uid = contact
            .dav_uid
            .clone()
            .filter(|uid| !uid.trim().is_empty())
            .unwrap_or_else(|| format!("postium-{}", uuid::Uuid::new_v4()));
        let data = dto_to_vcard(&contact, Some(&uid));
        // dav 条目以服务器 raw 为基底合并（保留 PHOTO 等未建模属性）；
        // 本地条目生成全新卡片
        let text = match dirty_item.raw_vcard.as_deref() {
            Some(raw) if contact.dav_uid.is_some() => vcard::merge_into_raw(raw, &data),
            _ => vcard::emit(&data),
        };
        let sep = if ab_href.ends_with('/') { "" } else { "/" };
        let href = contact
            .dav_href
            .clone()
            .filter(|href| !href.is_empty())
            .unwrap_or_else(|| format!("{ab_href}{sep}{uid}.vcf"));

        match client.put(&href, &text, contact.dav_etag.as_deref()).await {
            Ok(new_etag) => {
                contact_repo::dav_push_mark(
                    db,
                    contact.id,
                    account_id,
                    &uid,
                    &href,
                    new_etag.as_deref().unwrap_or(""),
                    &text,
                    since,
                )
                .await?;
                report.pushed += 1;
            }
            Err(err) if carddav::is_conflict(&err) => {
                match resolve_conflict(
                    db,
                    &client,
                    &ab_href,
                    account_id,
                    &contact,
                    &uid,
                    &href,
                    etag_of.get(href.as_str()).copied(),
                    dirty_item.updated_at,
                    since,
                    &text,
                )
                .await
                {
                    // true = 服务端胜（本地被服务端版本覆盖）
                    Ok(true) => report.conflicts += 1,
                    Ok(false) => report.pushed += 1,
                    Err(err) => report.errors.push(format!("冲突 {}: {err}", contact.email)),
                }
            }
            Err(err) => report.errors.push(format!("推送 {}: {err}", contact.email)),
        }
    }

    // ── 阶段 3：推送本地删除（墓碑） ────────────────────────────────────
    for href in contact_repo::dav_tombstones(db, account_id, since).await? {
        match client.delete(&href).await {
            Ok(()) => {
                contact_repo::dav_tombstone_remove(db, account_id, &href).await?;
                report.deleted += 1;
            }
            Err(err) => report.errors.push(format!("删除 {href}: {err}")),
        }
    }

    // ── 阶段 4：回写游标 ────────────────────────────────────────────────
    contact_repo::binding_touch_synced(db, account_id, Some(chrono_now_string())).await?;

    Ok(report)
}

/// 412 冲突裁决：服务端 REV 较新 → 服务端胜（整卡覆盖本地，返回 true）；
/// 否则本地胜（无 If-Match 重推覆盖服务端，返回 false）。
#[allow(clippy::too_many_arguments)]
async fn resolve_conflict(
    db: &DbConn,
    client: &CardDavClient,
    ab_href: &str,
    account_id: i32,
    contact: &ContactDto,
    uid: &str,
    href: &str,
    server_etag: Option<&str>,
    local_updated_at: i64,
    sync_ts: i64,
    local_text: &str,
) -> Result<bool, MailError> {
    let cards = client.multiget(ab_href, &[href.to_string()]).await?;
    let Some((_, server_text)) = cards.first() else {
        return Err(MailError::CardDavError(format!(
            "服务器未返回 {href} 的版本，无法裁决"
        )));
    };
    let server_rev = vcard::parse(server_text)?.rev.unwrap_or(0);
    if server_rev > local_updated_at {
        let data = vcard::parse(server_text)?;
        if let Some((write, groups)) = card_to_parts(&data) {
            contact_repo::dav_pull_apply(
                db,
                contact.id,
                &write,
                account_id,
                uid,
                href,
                server_etag.unwrap_or(""),
                server_text,
                sync_ts,
            )
            .await?;
            let existing_groups: Vec<String> =
                contact.groups.iter().map(|g| g.name.clone()).collect();
            set_groups_union(db, contact.id, existing_groups, groups).await?;
        }
        Ok(true)
    } else {
        let new_etag = client.put(href, local_text, None).await?;
        contact_repo::dav_push_mark(
            db,
            contact.id,
            account_id,
            uid,
            href,
            new_etag.as_deref().unwrap_or(""),
            local_text,
            sync_ts,
        )
        .await?;
        Ok(false)
    }
}

/// 拉取单卡落库：UID 命中 → 更新（分组并集）；同邮箱未绑定条目 → 并入；
/// 否则新建。返回 false 表示该卡无邮箱、不入库。
async fn apply_pulled_card(
    db: &DbConn,
    account_id: i32,
    href: &str,
    etag: &str,
    text: &str,
    sync_ts: i64,
) -> Result<bool, MailError> {
    let data = vcard::parse(text)?;
    let Some((write, groups)) = card_to_parts(&data) else {
        return Ok(false);
    };
    let uid = data
        .uid
        .clone()
        .filter(|uid| !uid.trim().is_empty())
        .unwrap_or_else(|| uid_from_href(href));

    if let Some(existing) = contact_repo::find_by_uid(db, account_id, &uid).await? {
        contact_repo::dav_pull_apply(db, existing.id, &write, account_id, &uid, href, etag, text, sync_ts)
            .await?;
        let existing_groups: Vec<String> = existing.groups.iter().map(|g| g.name.clone()).collect();
        set_groups_union(db, existing.id, existing_groups, groups).await?;
        return Ok(true);
    }

    // UID 未命中：同邮箱的本地/collected 条目并入（分组保并集）
    if let Some(existing) = contact_repo::get_by_email(db, &write.email).await? {
        let absorbable = (existing.dav_account_id.is_none() && existing.dav_uid.is_none())
            || existing.dav_account_id == Some(account_id);
        if !absorbable {
            return Err(MailError::CardDavError(format!(
                "邮箱 {} 已归属其他 CardDAV 条目，跳过并入",
                write.email
            )));
        }
        contact_repo::dav_pull_apply(db, existing.id, &write, account_id, &uid, href, etag, text, sync_ts)
            .await?;
        let existing_groups: Vec<String> = existing.groups.iter().map(|g| g.name.clone()).collect();
        set_groups_union(db, existing.id, existing_groups, groups).await?;
        return Ok(true);
    }

    let id = contact_repo::dav_pull_insert(db, &write, account_id, &uid, href, etag, text, sync_ts).await?;
    set_groups_union(db, id, Vec::new(), groups).await?;
    Ok(true)
}

/// vCard → (联系人写入数据, 分组名)；无邮箱卡片返回 None（不入库）。
/// 映射规则与导入路径一致：首邮箱为主、其余去重并入 extra_emails。
fn card_to_parts(data: &vcard::VcardData) -> Option<(ContactWrite, Vec<String>)> {
    let primary = data.emails.first()?.trim().to_string();
    if primary.is_empty() {
        return None;
    }
    let primary_key = primary.to_lowercase();
    let mut extra_emails: Vec<String> = Vec::new();
    for email in &data.emails[1..] {
        let key = email.trim().to_lowercase();
        if key != primary_key && !extra_emails.iter().any(|e| e.to_lowercase() == key) {
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
    Some((write, data.categories.clone()))
}

/// 联系人 DTO → vCard 结构（推送生成用；映射与导出路径一致）。
/// REV 固定为当前时间——412 冲突时以 REV vs 本地 updated_at 裁决。
fn dto_to_vcard(contact: &ContactDto, uid: Option<&str>) -> vcard::VcardData {
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
    data.uid = uid.map(str::to_string);
    data.rev = Some(chrono::Utc::now().timestamp());
    data
}

/// 建组挂成员：`existing ∪ card_groups` 按名建组/复用后整体重设成员关系。
async fn set_groups_union(
    db: &DbConn,
    contact_id: i32,
    existing: Vec<String>,
    card_groups: Vec<String>,
) -> Result<(), MailError> {
    let mut ids: Vec<i32> = Vec::new();
    for name in existing.into_iter().chain(card_groups) {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let id = contact_repo::group_ensure(db, name).await?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    contact_repo::group_set_members(db, contact_id, &ids).await
}

/// 服务器未给 UID 的卡片：从 href 派生稳定 UID（`/ab/x.vcf` → `x`）。
fn uid_from_href(href: &str) -> String {
    href.rsplit('/')
        .next()
        .unwrap_or(href)
        .trim_end_matches(".vcf")
        .to_string()
}

fn chrono_now_string() -> String {
    chrono::Utc::now().timestamp().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// 本地 updated_at 基准（2026-01-21，与测试卡片 REV 比较用）
    const LOCAL_TS: i64 = 1_769_000_000;
    /// 比它新的 REV → 服务端胜；比它旧 → 本地胜
    const REV_NEWER: &str = "REV:20330101T000000Z";
    const REV_OLDER: &str = "REV:20200101T000000Z";

    async fn seed_fixture(server: &MockServer) -> (DbConn, AuthManager) {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        db.call(|conn| {
            conn.execute(
                "INSERT INTO accounts (name, email, provider, account_type, created_at, updated_at)
                 VALUES ('测试', 't@x.test', 'custom', 'personal', 0, 0)",
                [],
            )
        })
        .await
        .unwrap();
        let auth = AuthManager::in_memory();
        auth.save_password("dav:1", "pass").unwrap();
        contact_repo::binding_upsert(&db, 1, &server.uri(), "user", Some("/ab/"))
            .await
            .unwrap();
        (db, auth)
    }

    fn list_response(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(207).set_body_string(body)
    }

    fn multistatus(responses: &str) -> String {
        format!(
            r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">{responses}</D:multistatus>"#
        )
    }

    fn entry(href: &str, etag: &str) -> String {
        format!(
            r#"<D:response><D:href>{href}</D:href><D:propstat><D:prop><D:getetag>{etag}</D:getetag></D:prop></D:propstat></D:response>"#
        )
    }

    fn card_entry(href: &str, etag: &str, vcard_text: &str) -> String {
        format!(
            r#"<D:response><D:href>{href}</D:href><D:propstat><D:prop><D:getetag>{etag}</D:getetag><C:address-data>{vcard_text}</C:address-data></D:prop></D:propstat></D:response>"#
        )
    }

    fn vcard_text(uid: &str, fn_name: &str, email: &str) -> String {
        format!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{fn_name}\r\nEMAIL:{email}\r\nTEL:+8613800000000\r\nCATEGORIES:work\r\nEND:VCARD\r\n"
        )
    }

    fn vcard_text_rev(uid: &str, fn_name: &str, email: &str, rev: &str) -> String {
        format!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{fn_name}\r\nEMAIL:{email}\r\n{rev}\r\nEND:VCARD\r\n"
        )
    }

    async fn query_mock(server: &MockServer, body: String) {
        Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-query"))
            .respond_with(list_response(&body))
            .mount(server)
            .await;
    }

    async fn multiget_mock(server: &MockServer, must_contain: Option<&str>, body: String) {
        let mock = Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-multiget"));
        let mock = match must_contain {
            Some(frag) => mock.and(body_string_contains(frag)),
            None => mock,
        };
        mock.respond_with(list_response(&body)).mount(server).await;
    }

    #[tokio::test]
    async fn first_sync_pulls_two_cards_with_groups() {
        let server = MockServer::start().await;
        let (db, auth) = seed_fixture(&server).await;

        query_mock(
            &server,
            multistatus(&format!(
                "{}{}",
                entry("/ab/a.vcf", "\"e1\""),
                entry("/ab/b.vcf", "\"e2\"")
            )),
        )
        .await;
        multiget_mock(
            &server,
            None,
            multistatus(&format!(
                "{}{}",
                card_entry("/ab/a.vcf", "\"e1\"", &vcard_text("u-a", "张三", "a@x.test")),
                card_entry("/ab/b.vcf", "\"e2\"", &vcard_text("u-b", "李四", "b@x.test"))
            )),
        )
        .await;

        let report = sync_account(&db, &auth, 1).await.unwrap();
        assert_eq!(report.pulled, 2, "{:?}", report.errors);
        assert!(report.errors.is_empty(), "{:?}", report.errors);

        let a = contact_repo::get_by_email(&db, "a@x.test")
            .await
            .unwrap()
            .expect("a 应入库");
        assert_eq!(a.source, "dav");
        assert_eq!(a.dav_uid.as_deref(), Some("u-a"));
        assert_eq!(a.dav_etag.as_deref(), Some("\"e1\""));
        assert_eq!(a.dav_href.as_deref(), Some("/ab/a.vcf"));
        assert_eq!(a.name.as_deref(), Some("张三"));
        assert_eq!(a.phones, vec!["+8613800000000"]);
        assert!(a.groups.iter().any(|g| g.name == "work"), "{:?}", a.groups);
        let groups = contact_repo::group_list(&db).await.unwrap();
        assert!(groups.iter().any(|g| g.name == "work"), "{groups:?}");
    }

    #[tokio::test]
    async fn incremental_sync_fetches_only_changed_etags() {
        let server = MockServer::start().await;
        let (db, auth) = seed_fixture(&server).await;

        // 第一轮全量（mock 仅消费一次；wiremock 同优先级按挂载顺序匹配，
        // 耗尽后第二轮请求落到后挂载的 mock）
        Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-query"))
            .respond_with(list_response(&multistatus(&format!(
                "{}{}",
                entry("/ab/a.vcf", "\"e1\""),
                entry("/ab/b.vcf", "\"e2\"")
            ))))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-multiget"))
            .respond_with(list_response(&multistatus(&format!(
                "{}{}",
                card_entry("/ab/a.vcf", "\"e1\"", &vcard_text("u-a", "张三", "a@x.test")),
                card_entry("/ab/b.vcf", "\"e2\"", &vcard_text("u-b", "李四", "b@x.test"))
            ))))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        let first = sync_account(&db, &auth, 1).await.unwrap();
        assert_eq!(first.pulled, 2);

        // 第二轮：仅 a.vcf etag 变化；multiget 只应包含 a.vcf
        query_mock(
            &server,
            multistatus(&format!(
                "{}{}",
                entry("/ab/a.vcf", "\"e1x\""),
                entry("/ab/b.vcf", "\"e2\"")
            )),
        )
        .await;
        multiget_mock(
            &server,
            Some("/ab/a.vcf"),
            multistatus(&card_entry(
                "/ab/a.vcf",
                "\"e1x\"",
                &vcard_text("u-a", "张三改", "a@x.test"),
            )),
        )
        .await;

        let second = sync_account(&db, &auth, 1).await.unwrap();
        assert!(second.errors.is_empty(), "未变卡片不应触发推送/删除: {:?}", second.errors);
        assert_eq!(second.pulled, 1, "只有变化的卡片应被拉取");
        let a = contact_repo::get_by_email(&db, "a@x.test").await.unwrap().unwrap();
        assert_eq!(a.name.as_deref(), Some("张三改"));
        assert_eq!(a.dav_etag.as_deref(), Some("\"e1x\""));
        let b = contact_repo::get_by_email(&db, "b@x.test").await.unwrap().unwrap();
        assert_eq!(b.name.as_deref(), Some("李四"), "未变卡片不应被重拉");
        assert_eq!(b.dav_etag.as_deref(), Some("\"e2\""));
    }

    #[tokio::test]
    async fn push_local_new_contact_upgrades_to_dav() {
        let server = MockServer::start().await;
        let (db, auth) = seed_fixture(&server).await;

        // 服务器为空
        query_mock(&server, multistatus("")).await;
        Mock::given(method("PUT"))
            .and(body_string_contains("UID:postium-"))
            .respond_with(ResponseTemplate::new(201).insert_header("ETag", "\"srv-1\""))
            .expect(1)
            .mount(&server)
            .await;

        contact_repo::insert(
            &db,
            ContactWrite {
                email: "local@x.test".into(),
                name: Some("本地人".into()),
                phones: Some(vec!["123".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let report = sync_account(&db, &auth, 1).await.unwrap();
        assert_eq!(report.pushed, 1, "{:?}", report.errors);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.pulled, 0);

        let c = contact_repo::get_by_email(&db, "local@x.test")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.source, "dav", "推送成功后应升级为 dav 条目");
        let uid = c.dav_uid.clone().unwrap_or_default();
        assert!(uid.starts_with("postium-"), "{uid}");
        assert_eq!(c.dav_etag.as_deref(), Some("\"srv-1\""));
        let href = c.dav_href.clone().unwrap_or_default();
        assert!(href.starts_with("/ab/") && href.ends_with(".vcf"), "{href}");
        assert!(href.contains(&uid), "href 应含 UID 文件名: {href}");
    }

    #[tokio::test]
    async fn conflict_server_wins_when_rev_newer() {
        let server = MockServer::start().await;
        let (db, auth) = seed_fixture(&server).await;

        // 本地 dav 条目：etag "old"、updated_at = LOCAL_TS（本地已改动 → 跳过拉取）
        let raw = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:u-x\r\nFN:本地版\r\nEMAIL:x@x.test\r\nEND:VCARD\r\n";
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO contacts (email, name, source, dav_account_id, dav_uid, dav_href, dav_etag, raw_vcard, created_at, updated_at)
                 VALUES ('x@x.test', '本地版', 'dav', 1, 'u-x', '/ab/x.vcf', '\"old\"', ?1, ?2, ?2)",
                rusqlite::params![raw, LOCAL_TS],
            )
        })
        .await
        .unwrap();

        query_mock(&server, multistatus(&entry("/ab/x.vcf", "\"srv-new\""))).await;
        Mock::given(method("PUT"))
            .and(header("If-Match", "\"old\""))
            .respond_with(ResponseTemplate::new(412))
            .mount(&server)
            .await;
        multiget_mock(
            &server,
            None,
            multistatus(&card_entry(
                "/ab/x.vcf",
                "\"srv-new\"",
                &vcard_text_rev("u-x", "服务器版", "x@x.test", REV_NEWER),
            )),
        )
        .await;

        let report = sync_account(&db, &auth, 1).await.unwrap();
        assert_eq!(report.conflicts, 1, "{:?}", report.errors);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.pushed, 0);

        let c = contact_repo::get_by_email(&db, "x@x.test").await.unwrap().unwrap();
        assert_eq!(c.name.as_deref(), Some("服务器版"), "服务端胜应覆盖本地内容");
        assert_eq!(c.dav_etag.as_deref(), Some("\"srv-new\""));
    }

    #[tokio::test]
    async fn conflict_local_wins_when_rev_older() {
        let server = MockServer::start().await;
        let (db, auth) = seed_fixture(&server).await;

        let raw = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:u-y\r\nFN:本地新版\r\nEMAIL:y@x.test\r\nEND:VCARD\r\n";
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO contacts (email, name, source, dav_account_id, dav_uid, dav_href, dav_etag, raw_vcard, created_at, updated_at)
                 VALUES ('y@x.test', '本地新版', 'dav', 1, 'u-y', '/ab/y.vcf', '\"old\"', ?1, ?2, ?2)",
                rusqlite::params![raw, LOCAL_TS],
            )
        })
        .await
        .unwrap();

        query_mock(&server, multistatus(&entry("/ab/y.vcf", "\"srv-new\""))).await;
        // 挂载顺序即匹配优先级（wiremock 同优先级先挂载先匹配）：
        // 带 If-Match 的首推命中 412；不带头的重推落到后面的 204
        Mock::given(method("PUT"))
            .and(header("If-Match", "\"old\""))
            .respond_with(ResponseTemplate::new(412))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(204).insert_header("ETag", "\"srv-2\""))
            .mount(&server)
            .await;
        multiget_mock(
            &server,
            None,
            multistatus(&card_entry(
                "/ab/y.vcf",
                "\"srv-new\"",
                &vcard_text_rev("u-y", "服务器旧版", "y@x.test", REV_OLDER),
            )),
        )
        .await;

        let report = sync_account(&db, &auth, 1).await.unwrap();
        assert_eq!(report.pushed, 1, "本地胜应重推成功: {:?}", report.errors);
        assert_eq!(report.conflicts, 0);
        assert!(report.errors.is_empty(), "{:?}", report.errors);

        let c = contact_repo::get_by_email(&db, "y@x.test").await.unwrap().unwrap();
        assert_eq!(c.name.as_deref(), Some("本地新版"), "本地内容应保留");
        assert_eq!(c.dav_etag.as_deref(), Some("\"srv-2\""));
    }
}
