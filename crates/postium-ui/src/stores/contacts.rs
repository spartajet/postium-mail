//! 联系人状态（模式同 accounts.rs）：过滤列表（搜索/分组/星标）+ 分组
//! （含成员计数）+ 选中项；星标走局部乐观更新，失败回滚。
//! 竞态防护：写操作推进 `write_gen`，在途 load 的完成回调比对代数，
//! 陈旧快照（装载期间发生过写）不覆盖本地列表；星标写完成后自行补跑
//! load 收敛到数据库最新状态。

use std::sync::Arc;

use gpui_kit::{App, AppContext as _, Context, Entity};
use postium_core::api::CoreApi;
use postium_core::infrastructure::storage::repository::contact_repo::ContactGroupWithCount;
use postium_core::service::email_service::ContactDto;

use crate::runtime::spawn_tokio;

/// 单页上限（联系人量级小，一次拉全）。
const PAGE_LIMIT: usize = 200;

pub struct ContactsStore {
    /// 当前过滤条件下的联系人页
    pub contacts: Vec<ContactDto>,
    /// 过滤命中总数
    pub total: u64,
    pub loading: bool,
    /// 分组（含成员计数）
    pub groups: Vec<ContactGroupWithCount>,
    /// 选中的联系人 id（详情卡数据源）
    pub selected: Option<i32>,
    /// 搜索关键词（空串 = 不过滤）
    pub query: String,
    /// 仅星标
    pub starred_only: bool,
    /// 当前分组过滤（None = 不限）
    pub active_group: Option<i32>,
    /// 数据版本：每次列表装载完成 +1（视图/e2e 等待用）
    pub list_version: u64,
    /// 写操作代数：乐观星标等写路径 +1；在途 load 完成时比对，
    /// 不一致则丢弃陈旧快照（参照 emails.rs 的 load_gen 模式）
    write_gen: u64,
    /// 在途 load 期间又有 load 请求：完成后补跑
    pending_reload: bool,
}

impl ContactsStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            contacts: Vec::new(),
            total: 0,
            loading: false,
            groups: Vec::new(),
            selected: None,
            query: String::new(),
            starred_only: false,
            active_group: None,
            list_version: 0,
            write_gen: 0,
            pending_reload: false,
        }
    }

    /// 拉取联系人列表 + 分组列表（按当前 query / active_group / starred_only）。
    pub fn load(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        if self.loading {
            self.pending_reload = true; // 在途期间的新请求：完成后补跑
            return;
        }
        self.loading = true;
        let write_g = self.write_gen;
        cx.notify();

        let query = self.query.trim().to_string();
        let group = self.active_group;
        let starred_only = self.starred_only;
        let api = api.clone();
        let api_keep = api.clone();
        let rx = spawn_tokio(async move {
            let list = api
                .list_contacts(
                    (!query.is_empty()).then(|| query.clone()),
                    group,
                    starred_only,
                    1,
                    PAGE_LIMIT,
                )
                .await;
            let groups = api.list_contact_groups().await;
            (list, groups)
        });

        cx.spawn(async move |this, cx| {
            let result = rx.recv().await.ok();
            this.update(cx, |store, cx| {
                store.loading = false;
                if store.write_gen != write_g {
                    // 装载期间发生过写操作（乐观星标）：陈旧快照不覆盖本地列表；
                    // 写路径完成后会自行补跑 load 收敛（见 toggle_starred）
                    cx.notify();
                    return;
                }
                if let Some((list, groups)) = result {
                    match list {
                        Ok(l) => {
                            store.contacts = l.contacts;
                            store.total = l.total;
                            store.list_version += 1;
                        }
                        Err(e) => tracing::warn!("联系人列表加载失败: {e}"),
                    }
                    if let Ok(g) = groups {
                        store.groups = g;
                    }
                }
                cx.notify();
            })
            .ok();
            // 补跑挂起的 load（若有）
            let _ = this.update(cx, |store, cx| {
                if store.pending_reload {
                    store.pending_reload = false;
                    store.load(&api_keep, cx);
                }
            });
        })
        .detach();
    }

    /// 更新搜索关键词并重新装载（视图搜索输入防抖后的直达入口）。
    pub fn set_query(&mut self, query: &str, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.query = query.trim().to_string();
        self.load(api, cx);
    }

    /// 切换分组过滤（Some(id) = 该分组成员；None = 「全部」）。
    /// 任何调用都退出星标模式（左栏三态互斥：全部 / 星标 / 某分组）。
    pub fn select_group(&mut self, group: Option<i32>, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.active_group = group;
        self.starred_only = false;
        self.load(api, cx);
    }

    /// 「星标」过滤（独立入口：进入星标态并退出分组过滤）。
    pub fn select_starred(&mut self, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        self.starred_only = true;
        self.active_group = None;
        self.load(api, cx);
    }

    /// 选中联系人（中列行点击 → 右侧详情卡）。
    pub fn select(&mut self, id: i32, cx: &mut Context<Self>) {
        self.selected = Some(id);
        cx.notify();
    }

    /// 乐观星标反转（列表行 ★ 点击）：本地立即翻转 + write_gen 失效在途快照，
    /// 落库失败回滚；成功后补跑一次 load（读后于写提交，保证最终一致）。
    pub fn toggle_starred(&mut self, id: i32, api: &Arc<CoreApi>, cx: &mut Context<Self>) {
        let Some(contact) = self.contacts.iter_mut().find(|c| c.id == id) else {
            return;
        };
        contact.is_starred = !contact.is_starred;
        let starred = contact.is_starred;
        self.write_gen += 1; // 在途 load 的快照已过期：完成时不得覆盖本地列表
        cx.notify();

        let api = api.clone();
        let api_late = api.clone();
        let rx = spawn_tokio(async move { api.set_contact_starred(id, starred).await });
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(())) => {
                    // 写已落库：补一次装载收敛（防止被更早发起的陈旧快照覆盖过）
                    let api = api_late;
                    this.update(cx, |store, cx| store.load(&api, cx)).ok();
                }
                Ok(Err(e)) => {
                    tracing::warn!("联系人星标切换失败: {e}");
                    this.update(cx, |store, cx| {
                        if let Some(contact) = store.contacts.iter_mut().find(|c| c.id == id) {
                            contact.is_starred = !contact.is_starred;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(_) => {}
            }
        })
        .detach();
    }
}

/// 在 App 上下文中创建并立即装载一次。
pub fn init_contacts_store(api: &Arc<CoreApi>, cx: &mut App) -> Entity<ContactsStore> {
    cx.new(|cx| {
        let mut store = ContactsStore::new(cx);
        store.load(api, cx);
        store
    })
}
