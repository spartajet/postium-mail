//! 联系人 CoreApi 门面链路测试（tempdir 真实 init）。
//!
//! 覆盖门面全链路：create → get/get_by_email → list → star →
//! group CRUD + 成员替换 → merge → update → delete，以及 backfill 回填。

use postium_core::api::CoreApi;
use postium_core::error::MailError;
use postium_core::infrastructure::storage::repository::contact_repo::ContactWrite;

/// 在 `emails` 表插入一行最小夹具（与 contact_repo 单测同构）。
fn insert_email_row(
    conn: &rusqlite::Connection,
    account_id: i32,
    uid: i64,
    sender_name: Option<&str>,
    sender_email: &str,
    recipient_emails: &str,
    cc_emails: Option<&str>,
    sent_at: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO emails (
            account_id, folder, uid, sender_name, sender_email,
            recipient_emails, cc_emails,
            sent_at, received_at, created_at, updated_at
        ) VALUES (?1, 'INBOX', ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7, ?7)",
        rusqlite::params![
            account_id,
            uid,
            sender_name,
            sender_email,
            recipient_emails,
            cc_emails,
            sent_at
        ],
    )?;
    Ok(())
}

#[tokio::test]
async fn contact_facade_chain_create_list_star_merge_groups() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();

    // ===== create =====
    let alice = api
        .create_contact(ContactWrite {
            email: "Alice@Postium.Test".into(),
            extra_emails: Some(vec!["alice.alt@postium.test".into()]),
            name: Some("Alice".into()),
            phones: Some(vec!["13800000000".into()]),
            notes: Some("主备注".into()),
            org: Some("Postium".into()),
        })
        .await
        .unwrap();
    assert!(alice.id > 0);
    assert_eq!(alice.email, "alice@postium.test", "邮箱应小写化");
    assert_eq!(alice.name.as_deref(), Some("Alice"));
    assert_eq!(alice.source, "local");
    assert!(alice.groups.is_empty(), "新联系人无分组");

    // ===== get / get_by_email =====
    let fetched = api.get_contact(alice.id).await.unwrap().expect("应能按 id 查到");
    assert_eq!(fetched.id, alice.id);
    assert!(api.get_contact(9999).await.unwrap().is_none(), "不存在 id 返回 None");
    let by_email = api
        .get_contact_by_email("ALICE@Postium.Test".into())
        .await
        .unwrap()
        .expect("按邮箱应能查到（大小写不敏感）");
    assert_eq!(by_email.id, alice.id);

    let bob = api
        .create_contact(ContactWrite {
            email: "bob@postium.test".into(),
            name: Some("Bob".into()),
            ..Default::default()
        })
        .await
        .unwrap();

    // ===== list（关键词过滤 / 分页字段回显）=====
    let list = api.list_contacts(None, None, false, 1, 20).await.unwrap();
    assert_eq!(list.total, 2);
    assert_eq!(list.contacts.len(), 2);
    assert_eq!(list.page, 1);
    assert_eq!(list.limit, 20);

    let list = api.list_contacts(Some("alice".into()), None, false, 1, 20).await.unwrap();
    assert_eq!(list.total, 1, "关键词应命中 alice");
    assert_eq!(list.contacts[0].id, alice.id);

    // ===== star =====
    api.set_contact_starred(bob.id, true).await.unwrap();
    let starred = api.list_contacts(None, None, true, 1, 20).await.unwrap();
    assert_eq!(starred.total, 1, "仅星标的 Bob");
    assert_eq!(starred.contacts[0].id, bob.id);

    // ===== group CRUD + 成员 =====
    let family = api.create_contact_group("家人".into()).await.unwrap();
    let work = api.create_contact_group("工作".into()).await.unwrap();
    assert!(family != work);

    // 重名建组 → DatabaseError("分组已存在: ...")
    let dup = api.create_contact_group("家人".into()).await.unwrap_err();
    assert!(
        matches!(dup, MailError::DatabaseError(ref msg) if msg.contains("分组已存在")),
        "重名建组应报分组已存在，实际: {dup:?}"
    );

    // 设置成员（含重复 id，应去重）
    api.set_contact_group_members(alice.id, vec![work, family, work])
        .await
        .unwrap();
    let dto = api.get_contact(alice.id).await.unwrap().unwrap();
    let group_ids: Vec<i32> = dto.groups.iter().map(|group| group.id).collect();
    let mut expected = vec![family, work];
    expected.sort_unstable();
    assert_eq!(group_ids, expected, "成员去重后按分组 id 升序");

    // 全量替换：只剩工作分组
    api.set_contact_group_members(alice.id, vec![work]).await.unwrap();
    let dto = api.get_contact(alice.id).await.unwrap().unwrap();
    let group_ids: Vec<i32> = dto.groups.iter().map(|group| group.id).collect();
    assert_eq!(group_ids, vec![work], "set_members 为全量替换语义");

    // 计数
    let groups = api.list_contact_groups().await.unwrap();
    assert_eq!(groups.iter().find(|g| g.id == work).unwrap().count, 1);
    assert_eq!(groups.iter().find(|g| g.id == family).unwrap().count, 0);

    // 改名
    api.rename_contact_group(work, "同事".into()).await.unwrap();
    let groups = api.list_contact_groups().await.unwrap();
    assert_eq!(groups.iter().find(|g| g.id == work).unwrap().name, "同事");

    // ===== merge =====
    // bob 在家人分组 → 并入 alice 后分组并集 [family, work]，bob 消失
    api.set_contact_group_members(bob.id, vec![family]).await.unwrap();
    let merged = api.merge_contacts(alice.id, vec![bob.id]).await.unwrap();
    assert_eq!(merged.id, alice.id);
    let mut merged_groups: Vec<i32> = merged.groups.iter().map(|group| group.id).collect();
    merged_groups.sort_unstable();
    assert_eq!(merged_groups, vec![family, work], "分组应并集");
    assert!(
        api.get_contact(bob.id).await.unwrap().is_none(),
        "absorbed 应被物理删除"
    );
    assert_eq!(api.list_contacts(None, None, false, 1, 20).await.unwrap().total, 1);
    let groups = api.list_contact_groups().await.unwrap();
    assert_eq!(groups.iter().find(|g| g.id == family).unwrap().count, 1, "成员转移到 primary");

    // ===== update =====
    api.update_contact(
        alice.id,
        ContactWrite {
            email: "alice@postium.test".into(),
            name: Some("Alice Prime".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        api.get_contact(alice.id).await.unwrap().unwrap().name.as_deref(),
        Some("Alice Prime"),
        "更新后姓名应生效"
    );

    // ===== delete =====
    api.delete_contact(alice.id).await.unwrap();
    assert!(api.get_contact(alice.id).await.unwrap().is_none());
    assert_eq!(api.list_contacts(None, None, false, 1, 20).await.unwrap().total, 0);

    api.delete_contact_group(family).await.unwrap();
    let groups = api.list_contact_groups().await.unwrap();
    assert!(groups.iter().all(|group| group.id != family), "删除的分组不再出现");
}

#[tokio::test]
async fn contact_facade_backfill_and_email_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();

    // 空库回填：0 个地址
    assert_eq!(api.backfill_contacts().await.unwrap(), 0);

    // 夹具：一个账号 + 两封邮件（a@x 发件人两次、收件人 b@x）
    api.db
        .call(|conn| {
            conn.execute(
                "INSERT INTO accounts (name, email, provider, account_type, created_at, updated_at)
                 VALUES ('联系人测试账号', 'contacts-test@postium.test', 'custom', 'personal', 0, 0)",
                [],
            )?;
            let account_id = conn.last_insert_rowid() as i32;
            insert_email_row(conn, account_id, 1, Some("Ann"), "a@x", "", None, 100)?;
            insert_email_row(conn, account_id, 2, Some("Ann"), "A@X", "b@x,c@x", None, 200)?;
            Ok(())
        })
        .await
        .unwrap();

    // 回填：聚合出 a@x / b@x / c@x 三个地址
    let processed = api.backfill_contacts().await.unwrap();
    assert_eq!(processed, 3);

    let ann = api
        .get_contact_by_email("a@x".into())
        .await
        .unwrap()
        .expect("回填后应能按邮箱查到");
    assert_eq!(ann.source, "collected");
    assert_eq!(ann.frequency, 2, "a@x 作为两封邮件的 sender（大小写归一）");
    assert_eq!(ann.name.as_deref(), Some("Ann"));

    assert_eq!(api.list_contacts(None, None, false, 1, 20).await.unwrap().total, 3);
}

#[tokio::test]
async fn contact_facade_vcard_and_csv_import_export() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();

    // ===== vCard 导入 / 重复跳过 =====
    let vcard_text = concat!(
        "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:门面测试\r\n",
        "EMAIL;TYPE=INTERNET:facade@postium.test\r\n",
        "TEL:13800000004\r\n",
        "CATEGORIES:门面组\r\n",
        "END:VCARD\r\n",
    );
    let report = api.import_contacts_vcard(vcard_text.to_string()).await.unwrap();
    assert_eq!((report.imported, report.skipped), (1, 0));
    let dup = api.import_contacts_vcard(vcard_text.to_string()).await.unwrap();
    assert_eq!((dup.imported, dup.skipped), (0, 1), "重复邮箱应跳过");

    let contact = api
        .get_contact_by_email("facade@postium.test".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(contact.name.as_deref(), Some("门面测试"));
    assert_eq!(contact.phones, vec!["13800000004"]);
    assert_eq!(
        contact.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
        vec!["门面组"],
        "CATEGORIES 应建组并挂成员"
    );

    // ===== vCard 导出（含过滤）=====
    let exported = api.export_contacts_vcard(None, None, false).await.unwrap();
    assert!(exported.contains("BEGIN:VCARD"));
    assert!(exported.contains("facade@postium.test"));
    assert!(api.export_contacts_vcard(None, None, true).await.unwrap().is_empty(), "无星标时导出为空");

    // ===== CSV 导入 / BOM 导出 =====
    let csv_text = "name,email,phone,group\r\n王五,wang@postium.test,13800000005,重要\r\n";
    let report = api.import_contacts_csv(csv_text.to_string()).await.unwrap();
    assert_eq!((report.imported, report.skipped), (1, 0));

    let csv_out = api.export_contacts_csv(None, None, false).await.unwrap();
    assert!(csv_out.starts_with('\u{feff}'), "CSV 导出应带 UTF-8 BOM");
    assert!(csv_out.contains("name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency"));
    assert!(csv_out.contains("wang@postium.test"));
    assert!(csv_out.contains("facade@postium.test"));
}
