//! 联系人视图：三栏布局（对齐设置页骨架）。
//! 左 180px 分组栏（全部/星标 + 分组计数徽标 + 新建分组）｜中列联系人列表
//! （首字头像 + 名称/邮箱 + 频次徽标 + 星标乐观反转）｜右 360px 详情卡
//! （查看/编辑两态，编辑含分组勾选；新建为空白编辑态）。
//! 工具条：搜索输入（300ms 防抖，复用主壳顶栏搜索的代数模式）+ 新建按钮。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
    Disableable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::infrastructure::storage::repository::contact_repo::DavBindingDto;
use postium_core::service::DavSyncReport;
use postium_core::service::email_service::{ContactDto, ContactWrite};

use crate::i18n::{t, t_format};
use crate::runtime::spawn_tokio;
use crate::stores::contacts::{init_contacts_store, ContactsStore};

/// 详情卡编辑态的目标：新建（空白表单）或已有联系人。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditTarget {
    New,
    Existing(i32),
}

/// CardDAV 绑定行（附账号邮箱展示，列表用）。
#[derive(Clone, Debug)]
pub(crate) struct DavBindingRow {
    pub account_id: i32,
    pub email: String,
    pub url: String,
    pub addressbook_href: Option<String>,
    pub last_sync_at: Option<i64>,
}

/// CardDAV 配置弹窗状态：绑定列表（上半）+ 新建表单（下半）。
pub(crate) struct DavFlow {
    /// 已有绑定
    pub bindings: Vec<DavBindingRow>,
    /// 账号下拉数据 (id, email)
    pub accounts: Vec<(i32, String)>,
    /// 表单所选本地账号
    pub account_id: Option<i32>,
    pub url: Entity<InputState>,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
    /// 「发现通讯录」结果 (href, name)
    pub addressbooks: Vec<(String, String)>,
    /// 已选中的通讯录 href
    pub chosen_ab: Option<String>,
    pub discovering: bool,
    pub saving: bool,
    pub loading: bool,
    /// 表单/请求错误行
    pub error: SharedString,
}

impl DavFlow {
    fn new(window: &mut Window, cx: &mut Context<ContactsView>) -> Self {
        let input = |ph: &str, window: &mut Window, cx: &mut Context<ContactsView>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(ph.to_string()))
        };
        Self {
            bindings: Vec::new(),
            accounts: Vec::new(),
            account_id: None,
            url: input(&t("contacts.davUrl"), window, cx),
            username: input(&t("contacts.davUsername"), window, cx),
            password: input(&t("contacts.davPassword"), window, cx),
            addressbooks: Vec::new(),
            chosen_ab: None,
            discovering: false,
            saving: false,
            loading: true,
            error: " ".into(),
        }
    }
}

pub struct ContactsView {
    pub api: Arc<CoreApi>,
    pub(crate) store: Entity<ContactsStore>,
    /// 工具条搜索输入
    search_input: Entity<InputState>,
    /// 搜索防抖代数（每次变更 +1，超时后比对决定是否执行）
    search_gen: u64,
    /// 新建分组内联输入（Some = 展开）
    new_group_open: bool,
    new_group_input: Entity<InputState>,
    /// 详情卡编辑态（None = 查看态）
    pub(crate) editing: Option<EditTarget>,
    /// 合并多选（行首复选框）：按点选顺序记录 id，末位 = 合并主联系人
    merge_picks: Vec<i32>,
    // 编辑表单
    edit_name: Entity<InputState>,
    edit_email: Entity<InputState>,
    edit_extra: Entity<InputState>,
    edit_phones: Entity<InputState>,
    edit_org: Entity<InputState>,
    edit_notes: Entity<InputState>,
    /// 编辑表单中勾选的分组 id
    edit_groups: Vec<i32>,
    /// 状态/错误行（编辑卡底部）
    status: SharedString,
    /// status 是否为错误（错误红显；loading 类提示用中性色，终审缓期项 T10）
    status_error: bool,
    /// CardDAV 配置弹窗（Some = 打开）
    pub(crate) dav: Option<DavFlow>,
    /// CardDAV 弹窗焦点句柄（ESC 关闭 / 焦点圈定）
    dav_modal_focus: gpui_kit::FocusHandle,
    /// 「同步」进行中（防重入）
    syncing: bool,
}

impl ContactsView {
    pub fn new(api: Arc<CoreApi>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = init_contacts_store(&api, cx);
        let input = |ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(ph.to_string()))
        };
        let search_input = input(&t("contacts.search"), window, cx);
        let new_group_input = input(&t("contacts.newGroup"), window, cx);
        let edit_name = input(&t("contacts.name"), window, cx);
        let edit_email = input(&t("contacts.email"), window, cx);
        let edit_extra = input(&t("contacts.extraEmails"), window, cx);
        let edit_phones = input(&t("contacts.phones"), window, cx);
        let edit_org = input(&t("contacts.org"), window, cx);
        let edit_notes = input(&t("contacts.notes"), window, cx);

        // 搜索：回车立即搜；输入变更 300ms 防抖自动搜（对齐主壳顶栏模式）
        cx.subscribe(&search_input, |this: &mut Self, input, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => {
                    this.search_gen += 1;
                    let q = input.read(cx).value().to_string();
                    let api = this.api.clone();
                    this.store.update(cx, |store, cx| store.set_query(&q, &api, cx));
                }
                InputEvent::Change => {
                    this.search_gen += 1;
                    let debounce_gen = this.search_gen;
                    let input = input.clone();
                    let api = this.api.clone();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(300))
                            .await;
                        this.update(cx, |view, cx| {
                            if view.search_gen != debounce_gen {
                                return; // 后续输入已接管
                            }
                            let q = input.read(cx).value().to_string();
                            view.store.update(cx, |store, cx| store.set_query(&q, &api, cx));
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }
                _ => {}
            }
        })
        .detach();

        // ESC 关闭 CardDAV 配置弹窗（对齐设置页向导模式）
        let dav_modal_focus = cx.focus_handle();
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "escape",
            gpui_kit::base::actions::Cancel,
            Some("CardDavModal"),
        )]);

        Self {
            api,
            store,
            search_input,
            search_gen: 0,
            new_group_open: false,
            new_group_input,
            editing: None,
            merge_picks: Vec::new(),
            edit_name,
            edit_email,
            edit_extra,
            edit_phones,
            edit_org,
            edit_notes,
            edit_groups: Vec::new(),
            status: " ".into(),
            status_error: false,
            dav: None,
            dav_modal_focus,
            syncing: false,
        }
    }

    fn value(input: &Entity<InputState>, cx: &gpui_kit::App) -> String {
        input.read(cx).value().to_string()
    }

    // ── 列表/过滤交互（pub(crate) 供 e2e 直驱） ──

    /// 选中联系人（行点击同款路径）。
    pub(crate) fn pick_contact(&mut self, id: i32, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.select(id, cx));
        self.editing = None;
        cx.notify();
    }

    /// 星标反转（行 ★ 点击同款路径）。
    pub(crate) fn toggle_star(&mut self, id: i32, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| store.toggle_starred(id, &api, cx));
    }

    /// 左栏分组过滤（None = 全部）。
    pub(crate) fn pick_group(&mut self, group: Option<i32>, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| store.select_group(group, &api, cx));
        cx.notify();
    }

    /// 左栏「星标」过滤（独立入口：星标态 + 退出分组）。
    pub(crate) fn pick_starred(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| store.select_starred(&api, cx));
    }

    /// 合并多选复选框（行首勾选；确认弹层省略，末位选中 = 主联系人）。
    pub(crate) fn toggle_merge_pick(&mut self, id: i32, cx: &mut Context<Self>) {
        if let Some(pos) = self.merge_picks.iter().position(|&x| x == id) {
            self.merge_picks.remove(pos);
        } else {
            self.merge_picks.push(id);
        }
        cx.notify();
    }

    /// 工具条「合并」：≥2 枚选中时可用；末位选中为主联系人，其余并入。
    /// 写入与收敛由 store.merge 承担（write_gen 竞态防护），此处清空多选。
    pub(crate) fn merge_selected(&mut self, cx: &mut Context<Self>) {
        if self.merge_picks.len() < 2 {
            return;
        }
        let primary = *self.merge_picks.last().expect("长度已校验 >= 2");
        let absorbed = self.merge_picks[..self.merge_picks.len() - 1].to_vec();
        let api = self.api.clone();
        self.store.update(cx, |store, cx| {
            store.merge(primary, absorbed, &api, cx);
        });
        self.merge_picks.clear();
        cx.notify();
    }

    // ── 详情卡编辑态 ──

    /// 新建：详情卡切空白编辑态（点击时清空上次的残留输入）。
    fn start_new(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.edit_name,
            &self.edit_email,
            &self.edit_extra,
            &self.edit_phones,
            &self.edit_org,
            &self.edit_notes,
        ] {
            input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.edit_groups.clear();
        self.editing = Some(EditTarget::New);
        self.status = " ".into();
        cx.notify();
    }

    /// 编辑选中联系人：表单回填 + 分组勾选初始化。
    fn start_edit(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let Some(c) = self.store.read(cx).contacts.iter().find(|c| c.id == id).cloned() else {
            return;
        };
        self.set_input(&self.edit_name.clone(), c.name.as_deref().unwrap_or(""), window, cx);
        self.set_input(&self.edit_email.clone(), &c.email, window, cx);
        self.set_input(&self.edit_extra.clone(), &c.extra_emails.join(", "), window, cx);
        self.set_input(&self.edit_phones.clone(), &c.phones.join(", "), window, cx);
        self.set_input(&self.edit_org.clone(), c.org.as_deref().unwrap_or(""), window, cx);
        self.set_input(&self.edit_notes.clone(), c.notes.as_deref().unwrap_or(""), window, cx);
        self.edit_groups = c.groups.iter().map(|g| g.id).collect();
        self.editing = Some(EditTarget::Existing(id));
        self.status = " ".into();
        cx.notify();
    }

    /// set_value 的借用安全包装。
    fn set_input(
        &self,
        input: &Entity<InputState>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        input.update(cx, |s, cx| s.set_value(value.to_string(), window, cx));
    }

    /// 取消编辑（回到查看态，保留选中）。
    fn cancel_edit(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.status = " ".into();
        self.status_error = false;
        cx.notify();
    }

    /// 保存：新建走 create_contact，编辑走 update_contact；随后分组勾选全量
    /// 替换。两个写调用在同一异步链顺序执行，任一失败都中断保存流程
    /// （status 错误提示、不 toast 成功、保留编辑态供重试）。
    fn save_edit(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.editing else { return };
        let email = Self::value(&self.edit_email, cx).trim().to_string();
        if email.is_empty() {
            self.status = format!("{}: {}", t("common.error"), t("contacts.email")).into();
            self.status_error = true;
            cx.notify();
            return;
        }
        let write = ContactWrite {
            email,
            extra_emails: Some(split_list(&Self::value(&self.edit_extra, cx))),
            name: non_empty(&Self::value(&self.edit_name, cx)),
            phones: Some(split_list(&Self::value(&self.edit_phones, cx))),
            notes: non_empty(&Self::value(&self.edit_notes, cx)),
            org: non_empty(&Self::value(&self.edit_org, cx)),
        };
        let group_ids = self.edit_groups.clone();
        let api = self.api.clone();
        let api_keep = api.clone();
        let store = self.store.clone();
        let rx: smol::channel::Receiver<Result<i32, postium_core::error::MailError>> =
            spawn_tokio(async move {
                // 写主体 → 写分组：任一 Err 都短路为整链失败
                let id = match target {
                    EditTarget::New => api.create_contact(write).await.map(|c| c.id)?,
                    EditTarget::Existing(id) => api.update_contact(id, write).await.map(|_| id)?,
                };
                api.set_contact_group_members(id, group_ids).await?;
                Ok(id)
            });
        self.status = t("email.loading");
        self.status_error = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(id)) => {
                    store.update(cx, |store, cx| {
                        store.selected = Some(id);
                        store.load(&api_keep, cx);
                    });
                    this.update(cx, |view, cx| {
                        view.editing = None;
                        view.status = " ".into();
                        cx.notify();
                    })
                    .ok();
                    crate::notify::toast(if matches!(target, EditTarget::New) {
                        t("contacts.added")
                    } else {
                        t("contacts.saved")
                    });
                }
                Ok(Err(e)) => {
                    // 主体或分组设置失败：中断保存，保留编辑态与错误提示
                    this.update(cx, |view, cx| {
                        view.status = format!("{e}").into();
                        view.status_error = true;
                        cx.notify();
                    })
                    .ok();
                }
                Err(_) => {}
            }
        })
        .detach();
    }

    /// 删除选中联系人（直接删 + toast，确认弹层省略）。
    fn delete_selected(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let api = self.api.clone();
        let api_keep = api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.delete_contact(id).await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                store.update(cx, |store, cx| {
                    store.selected = None;
                    store.load(&api_keep, cx);
                });
                this.update(cx, |view, cx| {
                    view.editing = None;
                    cx.notify();
                })
                .ok();
                crate::notify::toast(t("contacts.deleted"));
            }
        })
        .detach();
    }

    /// 编辑表单中勾选/取消分组。
    fn toggle_edit_group(&mut self, group_id: i32, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(pos) = self.edit_groups.iter().position(|g| *g == group_id) {
            self.edit_groups.remove(pos);
        } else {
            self.edit_groups.push(group_id);
        }
        cx.notify();
    }

    // ── 导入/导出 ──

    /// 工具条「导入」：后台线程 rfd 选 `.vcf/.csv` 并读字节（UI 线程不做
    /// 阻塞 IO；`read_to_string` 只认 UTF-8，中文 Windows 上 GBK 编码 CSV
    /// 必失败——改 `read` 字节 + `from_utf8_lossy`：GBK 会乱码但可导入）→
    /// 读文件失败 toast 导入失败（不折叠成取消静默）→ 按扩展名分派
    ///（无扩展/未知按内容嗅探：BEGIN:VCARD 开头 → vCard，否则 CSV）→
    /// store 导入（api 调用 + 刷新 + toast 报告）。
    fn import_contacts(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (tx, rx) = smol::channel::bounded(1);
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Contacts", &["vcf", "csv"])
                .pick_file()
                .map(|path| {
                    std::fs::read(&path)
                        .map(|bytes| (path, String::from_utf8_lossy(&bytes).into_owned()))
                        .map_err(|e| format!("{e}"))
                });
            let _ = tx.try_send(picked);
        });
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Some(Ok((path, text)))) => {
                    this.update(cx, |view, cx| {
                        let api = view.api.clone();
                        let vcard = match path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_ascii_lowercase())
                            .as_deref()
                        {
                            Some("vcf") | Some("vcard") => true,
                            Some("csv") => false,
                            _ => sniff_is_vcard(&text),
                        };
                        view.store.update(cx, |store, cx| {
                            if vcard {
                                store.import_contacts_vcard_text(text, &api, cx);
                            } else {
                                store.import_contacts_csv_text(text, &api, cx);
                            }
                        });
                    })
                    .ok();
                }
                // 读文件失败（权限/占用/磁盘等）：toast 导入失败而非静默返回
                Ok(Some(Err(error))) => {
                    crate::notify::toast(t_format(
                        "contacts.importFailed",
                        &[("error", &error)],
                    ));
                }
                _ => {} // 取消对话框 / 通道关闭
            }
        })
        .detach();
    }

    /// 工具条「导出」：后台线程 rfd 保存对话框（默认名 postium-contacts.vcf）
    /// → 按所选扩展分派 api 导出（用 store 当前 query/分组/星标过滤，UI 线程
    /// 拍快照）→ 后台线程写文件 → toast「已导出到 <路径>」；失败 toast 错误。
    fn export_contacts(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        // 过滤条件快照（对话框期间列表过滤可能变化，导出以点击时为准）
        let (query, group, starred) = {
            let s = self.store.read(cx);
            (s.query.trim().to_string(), s.active_group, s.starred_only)
        };
        let api = self.api.clone();
        let (tx, rx) = smol::channel::bounded(1);
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("vCard", &["vcf"])
                .add_filter("CSV", &["csv"])
                .set_file_name("postium-contacts.vcf")
                .save_file();
            let _ = tx.try_send(picked);
        });
        cx.spawn(async move |_this, _cx| {
            if let Ok(Some(mut path)) = rx.recv().await {
                // 扩展分派：.csv → CSV 导出；其余（含无扩展，补 .vcf 后缀）→ vCard
                let as_csv = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
                if path.extension().is_none() {
                    path = path.with_extension("vcf");
                }
                let query = (!query.is_empty()).then(|| query.clone());
                let rx = spawn_tokio(async move {
                    if as_csv {
                        api.export_contacts_csv(query, group, starred).await
                    } else {
                        api.export_contacts_vcard(query, group, starred).await
                    }
                });
                match rx.recv().await {
                    Ok(Ok(text)) => {
                        // 写文件留在后台线程（文件 IO 不占 UI 线程）
                        let (wtx, wrx) = smol::channel::bounded(1);
                        std::thread::spawn(move || {
                            let written = std::fs::write(&path, text).map(|_| path);
                            let _ = wtx.try_send(written);
                        });
                        match wrx.recv().await {
                            Ok(Ok(path)) => crate::notify::toast(t_format(
                                "contacts.exportDone",
                                &[("path", &path.display().to_string())],
                            )),
                            Ok(Err(e)) => {
                                tracing::warn!("联系人导出写文件失败: {e}");
                                crate::notify::toast(t_format(
                                    "contacts.exportFailed",
                                    &[("error", &format!("{e}"))],
                                ));
                            }
                            Err(_) => {}
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("联系人导出失败: {e}");
                        crate::notify::toast(t_format(
                            "contacts.exportFailed",
                            &[("error", &format!("{e}"))],
                        ));
                    }
                    Err(_) => {}
                }
            }
        })
        .detach();
    }

    // ── 分组栏 ──

    /// 展开/收起新建分组内联输入（展开时清空残留）。
    fn toggle_new_group(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.new_group_open = !self.new_group_open;
        if self.new_group_open {
            self.new_group_input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        cx.notify();
    }

    /// 创建分组（内联输入 + 确认按钮；成功后收起并刷新分组）。
    fn create_group(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let name = Self::value(&self.new_group_input, cx).trim().to_string();
        if name.is_empty() {
            return;
        }
        let api = self.api.clone();
        let api_keep = api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.create_contact_group(name).await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(_)) = rx.recv().await {
                store.update(cx, |store, cx| store.load(&api_keep, cx));
                this.update(cx, |view, cx| {
                    view.new_group_open = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
    // ── CardDAV 配置与同步 ──────────────────────────────────────────────

    /// 工具条「同步」：顺序同步全部绑定账号，聚合报告 toast（防重入）。
    fn sync_contacts_now(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        self.syncing = true;
        let api = self.api.clone();
        let rx = spawn_tokio(async move {
            let mut report = DavSyncReport::default();
            let mut errors: Vec<String> = Vec::new();
            match api.list_dav_bindings().await {
                Ok(bindings) => {
                    for b in bindings {
                        match api.sync_contacts(b.account_id).await {
                            Ok(r) => {
                                report.pulled += r.pulled;
                                report.pushed += r.pushed;
                                report.deleted += r.deleted;
                                report.conflicts += r.conflicts;
                                errors.extend(r.errors);
                            }
                            Err(e) => errors.push(e.to_string()),
                        }
                    }
                }
                Err(e) => errors.push(e.to_string()),
            }
            (report, errors)
        });
        cx.spawn(async move |this, cx| {
            let Ok((report, errors)) = rx.recv().await else { return };
            this.update(cx, |view, cx| {
                view.syncing = false;
                if errors.is_empty() {
                    let pulled = report.pulled.to_string();
                    let pushed = report.pushed.to_string();
                    let deleted = report.deleted.to_string();
                    let conflicts = report.conflicts.to_string();
                    crate::notify::toast(t_format(
                        "contacts.syncDone",
                        &[
                            ("pulled", &pulled),
                            ("pushed", &pushed),
                            ("deleted", &deleted),
                            ("conflicts", &conflicts),
                        ],
                    ));
                } else {
                    let joined = errors.join("；");
                    crate::notify::toast(t_format("contacts.syncFailed", &[("error", &joined)]));
                }
                // 同步可能改动联系人：统一刷新收敛
                let api = view.api.clone();
                view.store.update(cx, |store, cx| store.refresh_after_write(&api, cx));
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// 打开 CardDAV 配置弹窗并装载绑定 + 账号数据。
    pub(crate) fn open_dav_modal(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dav = Some(DavFlow::new(window, cx));
        let focus = self.dav_modal_focus.clone();
        window.focus(&focus, cx);
        self.dav_reload_data(cx);
        cx.notify();
    }

    /// 关闭 CardDAV 配置弹窗（X / 背景点击 / ESC）。
    pub(crate) fn close_dav_modal(
        &mut self,
        _: &ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dav = None;
        cx.notify();
    }

    /// 装载绑定列表（附账号邮箱展示）+ 账号下拉数据。
    fn dav_reload_data(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let rx = spawn_tokio(async move {
            let bindings = api.list_dav_bindings().await;
            let accounts = api.list_accounts().await;
            (bindings, accounts)
        });
        cx.spawn(async move |this, cx| {
            let Ok((bindings, accounts)) = rx.recv().await else { return };
            this.update(cx, |view, cx| {
                let Some(dav) = view.dav.as_mut() else { return };
                let account_list = accounts.unwrap_or_default();
                let email_of: std::collections::HashMap<i32, String> = account_list
                    .iter()
                    .map(|a| (a.id, a.email.clone()))
                    .collect();
                dav.accounts = account_list.into_iter().map(|a| (a.id, a.email)).collect();
                dav.bindings = bindings
                    .unwrap_or_default()
                    .into_iter()
                    .map(|b: DavBindingDto| DavBindingRow {
                        account_id: b.account_id,
                        email: email_of
                            .get(&b.account_id)
                            .cloned()
                            .unwrap_or_else(|| b.account_id.to_string()),
                        url: b.url,
                        addressbook_href: b.addressbook_href,
                        last_sync_at: b.last_sync_at,
                    })
                    .collect();
                dav.loading = false;
                if dav.account_id.is_none() {
                    dav.account_id = dav.accounts.first().map(|(id, _)| *id);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 「发现通讯录」：读表单 → 临时 client 直连发现链（e2e 离线短路）。
    fn dav_discover(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(dav) = self.dav.as_ref() else { return };
        if dav.discovering {
            return;
        }
        let url = Self::value(&dav.url, cx).trim().to_string();
        let username = Self::value(&dav.username, cx).trim().to_string();
        let password = Self::value(&dav.password, cx);
        if url.is_empty() || username.is_empty() || password.is_empty() {
            return;
        }
        let api = self.api.clone();
        if let Some(dav) = self.dav.as_mut() {
            dav.discovering = true;
            dav.addressbooks.clear();
            dav.chosen_ab = None;
            dav.error = " ".into();
        }
        let rx = spawn_tokio(async move { api.discover_addressbooks(url, username, password).await });
        cx.spawn(async move |this, cx| {
            let Ok(result) = rx.recv().await else { return };
            this.update(cx, |view, cx| {
                let Some(dav) = view.dav.as_mut() else { return };
                dav.discovering = false;
                match result {
                    Ok(list) => dav.addressbooks = list,
                    Err(e) => {
                        dav.error =
                            t_format("contacts.davDiscoverFailed", &[("error", &format!("{e}"))]);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// 表单选择本地账号。
    pub(crate) fn dav_pick_account(&mut self, id: i32, cx: &mut Context<Self>) {
        if let Some(dav) = self.dav.as_mut() {
            dav.account_id = Some(id);
        }
        cx.notify();
    }

    /// 选择发现到的通讯录。
    pub(crate) fn dav_pick_addressbook(&mut self, href: String, cx: &mut Context<Self>) {
        if let Some(dav) = self.dav.as_mut() {
            dav.chosen_ab = Some(href);
        }
        cx.notify();
    }

    /// 保存绑定：密码经 AuthManager 托管 + binding 落库 → 刷新列表并清表单。
    fn dav_save(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(dav) = self.dav.as_ref() else { return };
        let Some(account_id) = dav.account_id else { return };
        let Some(ab) = dav.chosen_ab.clone() else { return };
        let url = Self::value(&dav.url, cx).trim().to_string();
        let username = Self::value(&dav.username, cx).trim().to_string();
        let password = Self::value(&dav.password, cx);
        if url.is_empty() || username.is_empty() || password.is_empty() || dav.saving {
            return;
        }
        if let Some(dav) = self.dav.as_mut() {
            dav.saving = true;
            dav.error = " ".into();
        }
        let api = self.api.clone();
        let rx = spawn_tokio(async move {
            api.save_dav_binding(account_id, url, username, password, Some(ab))
                .await
        });
        cx.spawn(async move |this, cx| {
            let Ok(result) = rx.recv().await else { return };
            this.update(cx, |view, cx| {
                match result {
                    Ok(()) => {
                        crate::notify::toast(t("contacts.davSaved"));
                        // 表单保留 URL/用户名便于连续添加；仅清发现结果
                        if let Some(dav) = view.dav.as_mut() {
                            dav.saving = false;
                            dav.addressbooks.clear();
                            dav.chosen_ab = None;
                        }
                        view.dav_reload_data(cx);
                    }
                    Err(e) => {
                        if let Some(dav) = view.dav.as_mut() {
                            dav.saving = false;
                            dav.error = t_format(
                                "contacts.davSaveFailed",
                                &[("error", &format!("{e}"))],
                            );
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// 删除绑定（密码一并清除）。
    pub(crate) fn dav_delete_binding(&mut self, account_id: i32, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.delete_dav_binding(account_id).await });
        cx.spawn(async move |this, cx| {
            let Ok(_) = rx.recv().await else { return };
            this.update(cx, |view, cx| {
                view.dav_reload_data(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 渲染
// ─────────────────────────────────────────────────────────────────────────────


impl Render for ContactsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (contacts, groups, selected_id, active_group, starred_only, loading, total) = {
            let s = self.store.read(cx);
            (
                s.contacts.clone(),
                s.groups.clone(),
                s.selected,
                s.active_group,
                s.starred_only,
                s.loading,
                s.total,
            )
        };

        // ── 工具条：标题 + 搜索 + 新建 ──
        let toolbar = h_flex()
            .flex_none()
            .items_center()
            .gap_2()
            .pb_2()
            .child(
                gpui_kit::component::Icon::new(IconName::Contact)
                    .size_5()
                    .text_color(theme.accent),
            )
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .child(t("contacts.title")),
            )
            .child(div().w(px(260.)).child(Input::new(&self.search_input)))
            .child(
                Button::new("contact-new")
                    .primary()
                    .xsmall()
                    .icon(IconName::UserPlus)
                    .label(t("contacts.new"))
                    .on_click(cx.listener(Self::start_new)),
            )
            // 合并入口：多选 ≥2 枚时出现（primary = 最后勾选）
            .when(self.merge_picks.len() >= 2, |this| {
                this.child(
                    Button::new("contact-merge")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Combine)
                        .label(t("contacts.merge"))
                        .on_click(cx.listener(|view: &mut Self, _, _, cx| {
                            view.merge_selected(cx);
                        })),
                )
            })
            // 导入入口：文件 IO 在后台线程，实际导入走 store（可测路径）
            .child(
                div()
                    .debug_selector(|| "contacts-import-btn".to_string())
                    .child(
                        Button::new("contact-import")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileUp)
                            .label(t("contacts.import"))
                            .on_click(cx.listener(Self::import_contacts)),
                    ),
            )
            // 导出入口：保存对话框 + 后台线程写文件 + toast 结果
            .child(
                div()
                    .debug_selector(|| "contacts-export-btn".to_string())
                    .child(
                        Button::new("contact-export")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileDown)
                            .label(t("contacts.export"))
                            .on_click(cx.listener(Self::export_contacts)),
                    ),
            )
            // CardDAV 同步：顺序同步全部绑定账号（进行中 loading 态防重入）
            .child(
                div()
                    .debug_selector(|| "contacts-sync-btn".to_string())
                    .child(
                        Button::new("contact-sync")
                            .ghost()
                            .xsmall()
                            .icon(IconName::RefreshCw)
                            .label(t("contacts.sync"))
                            .loading(self.syncing)
                            .on_click(cx.listener(Self::sync_contacts_now)),
                    ),
            )
            // CardDAV 账号配置弹窗入口
            .child(
                div()
                    .debug_selector(|| "contacts-dav-btn".to_string())
                    .child(
                        Button::new("contact-dav")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Globe)
                            .label(t("contacts.davTitle"))
                            .on_click(cx.listener(Self::open_dav_modal)),
                    ),
            )
            .child(
                div()
                    .ml_auto()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(format!("{total}")),
            )
            .when(loading, |this| {
                this.child(
                    gpui_kit::component::Icon::new(IconName::LoaderCircle)
                        .size_4()
                        .text_color(theme.muted_foreground),
                )
            });

        // ── 左 180px 分组栏：全部 / 星标 / 分组（计数徽标）/ 新建分组 ──
        let mut nav = v_flex()
            .id("contacts-group-nav")
            .w(px(180.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .p_3()
            .gap_1()
            .overflow_y_scroll();
        let all_active = !starred_only && active_group.is_none();
        nav = nav
            .child(nav_item(
                &theme,
                "nav-all",
                IconName::Users,
                t("contacts.all"),
                all_active,
                Some(total),
                cx.listener(move |view: &mut Self, _, _, cx| view.pick_group(None, cx)),
            ))
            .child(nav_item(
                &theme,
                "nav-starred",
                IconName::Star,
                t("contacts.starred"),
                starred_only,
                None,
                cx.listener(move |view: &mut Self, _, _, cx| view.pick_starred(cx)),
            ))
            .child(
                div()
                    .mt_2()
                    .mb_1()
                    .px_2()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(t("contacts.groups")),
            );
        for g in &groups {
            let id = g.id;
            nav = nav.child(nav_item(
                &theme,
                format!("nav-group-{id}"),
                IconName::Tag,
                g.name.clone().into(),
                active_group == Some(id),
                Some(g.count),
                cx.listener(move |view: &mut Self, _, _, cx| view.pick_group(Some(id), cx)),
            ));
        }
        // 新建分组：内联小输入（展开）或 ghost 按钮（收起）
        if self.new_group_open {
            nav = nav.child(
                v_flex()
                    .id("new-group-row")
                    .gap_1()
                    .pt_2()
                    .child(Input::new(&self.new_group_input))
                    .child(
                        h_flex().gap_1().child(
                            Button::new("new-group-confirm")
                                .primary()
                                .xsmall()
                                .icon(IconName::Check)
                                .label(t("common.confirm"))
                                .on_click(cx.listener(Self::create_group)),
                        ),
                    ),
            );
        } else {
            nav = nav.child(
                Button::new("new-group-toggle")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .label(t("contacts.newGroup"))
                    .on_click(cx.listener(Self::toggle_new_group)),
            );
        }

        // ── 中列列表 ──
        let mut list_col = v_flex().flex_1().min_w_0().h_full().p_2().gap_1();
        if loading && contacts.is_empty() {
            list_col = list_col.child(
                h_flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .py_8()
                    .child(
                        gpui_kit::component::Icon::new(IconName::LoaderCircle)
                            .size_5()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(t("email.loading")),
                    ),
            );
        } else if contacts.is_empty() {
            // 空状态：MailOpen 图标 + 收集提示
            list_col = list_col.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        gpui_kit::component::Icon::new(IconName::MailOpen)
                            .size_8()
                            .text_color(theme.muted_foreground.opacity(0.5)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child(t("contacts.empty")),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(theme.muted_foreground.opacity(0.8))
                            .child(t("contacts.emptyHint")),
                    ),
            );
        } else {
            let mut rows = v_flex()
                .gap_1()
                .id("contacts-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll();
            for c in &contacts {
                rows = rows.child(render_contact_row(
                    c,
                    selected_id == Some(c.id),
                    self.merge_picks.contains(&c.id),
                    cx,
                ));
            }
            list_col = list_col.child(rows);
        }

        // ── 右 360px 详情卡 ──
        let detail: AnyElement = match self.editing {
            Some(_) => render_edit_form(self, &groups, cx).into_any_element(),
            None => {
                let selected = contacts.iter().find(|c| Some(c.id) == selected_id);
                match selected {
                    Some(c) => render_detail_card(c, cx).into_any_element(),
                    None => v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(
                            gpui_kit::component::Icon::new(IconName::Contact)
                                .size_8()
                                .text_color(theme.muted_foreground.opacity(0.5)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child(t("contacts.selectHint")),
                        )
                        .into_any_element(),
                }
            }
        };
        let detail_col = v_flex()
            .w(px(360.))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(theme.border)
            .p_3()
            .gap_2()
            .child(detail)
            .when(
                !self.status.clone().trim().is_empty() && self.status != SharedString::from(" "),
                |this| {
                    this.child(
                        div()
                            .text_size(px(11.5))
                            .text_color(if self.status_error {
                                gpui_kit::rgb(0xf43f5e).into()
                            } else {
                                theme.muted_foreground
                            })
                            .line_clamp(2)
                            .child(self.status.clone()),
                    )
                },
            );

        v_flex()
            .size_full()
            .relative()
            .p_2()
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(nav)
                    .child(list_col)
                    .child(detail_col),
            )
            .when(self.dav.is_some(), |this| {
                this.child(render_dav_modal(self, window, cx))
            })
    }
}

/// 左栏过滤项（图标 + 名称 + 计数徽标；选中 accent 底，对齐设置页 nav）。
fn nav_item(
    theme: &gpui_kit::component::Theme,
    key: impl Into<ElementId>,
    icon: IconName,
    label: SharedString,
    selected: bool,
    count: Option<u64>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(key)
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py(px(7.))
        .rounded(px(8.))
        .cursor_pointer()
        .text_size(px(13.))
        .min_w_0()
        .when(selected, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
        .when(!selected, |d| {
            d.text_color(theme.muted_foreground)
                .hover(|s| s.bg(theme.accent.opacity(0.10)).text_color(theme.foreground))
        })
        .child(gpui_kit::component::Icon::new(icon).size_4().flex_none())
        .child(div().flex_1().min_w_0().line_clamp(1).child(label))
        .when_some(
            count.filter(|c| *c > 0),
            |d, c| {
                d.child(
                    div()
                        .flex_none()
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(8.))
                        .bg(if selected {
                            theme.accent_foreground.opacity(0.25)
                        } else {
                            theme.muted.opacity(0.6)
                        })
                        .text_size(px(10.5))
                        .child(c.to_string()),
                )
            },
        )
        .on_click(on_click)
}

/// 联系人行：合并复选框 + 首字头像 + 名称/邮箱 + 频次徽标 + 星标。
fn render_contact_row(
    c: &ContactDto,
    selected: bool,
    merge_checked: bool,
    cx: &mut Context<ContactsView>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let id = c.id;
    let display_name = c.name.clone().unwrap_or_else(|| c.email.clone());
    let initial = display_name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().to_string())
        .unwrap_or_default();
    let starred = c.is_starred;
    let frequency = c.frequency;

    div()
        .id(format!("contact-row-{id}"))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py(px(7.))
        .rounded(theme.radius)
        .cursor_pointer()
        .when(selected, |d| {
            d.bg(theme.accent.opacity(0.14))
                .border_1()
                .border_color(theme.accent.opacity(0.6))
        })
        .when(!selected, |d| {
            d.border_1()
                .border_color(gpui_kit::transparent_black())
                .hover(|s| s.bg(theme.accent.opacity(0.08)))
        })
        // 行首合并复选框（吞掉点击避免触发行选中）
        .child(
            div()
                .id(format!("contact-merge-{id}"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(15.))
                .h(px(15.))
                .rounded(px(4.))
                .border_1()
                .border_color(if merge_checked { theme.accent } else { theme.border.opacity(0.7) })
                .when(merge_checked, |d| {
                    d.bg(theme.accent).child(
                        gpui_kit::component::Icon::new(IconName::Check)
                            .size_3()
                            .text_color(theme.accent_foreground),
                    )
                })
                .on_click(cx.listener(move |view: &mut ContactsView, _, _, cx| {
                    cx.stop_propagation();
                    view.toggle_merge_pick(id, cx);
                })),
        )
        // 首字圆形头像（accent 淡底，对齐设置页账号卡）
        .child(
            div()
                .w(px(30.))
                .h(px(30.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.accent.opacity(0.15))
                .text_color(theme.accent)
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .child(initial),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(1)
                        .child(display_name),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.muted_foreground)
                        .line_clamp(1)
                        .child(c.email.clone()),
                ),
        )
        // 频次徽标
        .when(frequency > 0, |d| {
            d.child(
                div()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(8.))
                    .bg(theme.muted.opacity(0.6))
                    .text_size(px(10.5))
                    .text_color(theme.muted_foreground)
                    .child(frequency.to_string()),
            )
        })
        // 星标 ★（乐观反转；吞掉点击避免触发行选中）
        .child(
            div()
                .id(format!("contact-star-{id}"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(26.))
                .h(px(26.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(theme.accent.opacity(0.12)))
                .child(
                    gpui_kit::component::Icon::new(IconName::Star)
                        .size_4()
                        .text_color(if starred {
                            gpui_kit::Hsla::from(gpui_kit::rgb(0xf59e0b))
                        } else {
                            theme.muted_foreground.opacity(0.5)
                        }),
                )
                .on_click(cx.listener(move |view: &mut ContactsView, _, _, cx| {
                    cx.stop_propagation();
                    view.toggle_star(id, cx);
                })),
        )
        .on_click(cx.listener(move |view: &mut ContactsView, _, _, cx| {
            view.pick_contact(id, cx);
        }))
}

/// 查看态详情卡：头像/名称/邮箱 + 字段行（额外邮箱/电话/组织/备注/分组/来源/频次）+ 编辑/删除。
fn render_detail_card(c: &ContactDto, cx: &mut Context<ContactsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let display_name = c.name.clone().unwrap_or_else(|| c.email.clone());
    let initial = display_name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().to_string())
        .unwrap_or_default();

    let field_row = |label: SharedString, value: String| -> AnyElement {
        v_flex()
            .gap_0p5()
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .line_clamp(2)
                    .child(if value.is_empty() { "—".to_string() } else { value }),
            )
            .into_any_element()
    };

    let source_label = match c.source.as_str() {
        "local" => t("contacts.sourceLocal"),
        "collected" => t("contacts.sourceCollected"),
        "dav" => t("contacts.sourceDav"),
        other => other.to_string().into(),
    };

    v_flex()
        .id("contact-detail")
        .gap_3()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(38.))
                        .h(px(38.))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(theme.accent.opacity(0.15))
                        .text_color(theme.accent)
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .child(initial),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .line_clamp(1)
                                .child(display_name),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .line_clamp(1)
                                .child(c.email.clone()),
                        ),
                )
                .child(
                    gpui_kit::component::Icon::new(IconName::Star)
                        .size_4()
                        .text_color(if c.is_starred {
                            gpui_kit::Hsla::from(gpui_kit::rgb(0xf59e0b))
                        } else {
                            theme.muted_foreground.opacity(0.4)
                        }),
                ),
        )
        .child(field_row(t("contacts.email"), c.email.clone()))
        .child(field_row(t("contacts.extraEmails"), c.extra_emails.join(", ")))
        .child(field_row(t("contacts.phones"), c.phones.join(", ")))
        .child(field_row(t("contacts.org"), c.org.clone().unwrap_or_default()))
        .child(field_row(t("contacts.notes"), c.notes.clone().unwrap_or_default()))
        .child(field_row(
            t("contacts.groups"),
            c.groups.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(", "),
        ))
        .child(field_row(t("contacts.source"), source_label.to_string()))
        .child(field_row(
            t("contacts.frequency"),
            c.frequency.to_string(),
        ))
        .child(
            h_flex()
                .gap_2()
                .pt_1()
                .child(
                    Button::new("detail-edit")
                        .outline()
                        .xsmall()
                        .icon(IconName::Pencil)
                        .label(t("contacts.edit"))
                        .on_click(cx.listener(ContactsView::start_edit)),
                )
                .child(
                    Button::new("detail-delete")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Trash)
                        .label(t("common.delete"))
                        .on_click(cx.listener(ContactsView::delete_selected)),
                ),
        )
}

/// 编辑态详情卡：字段表单 + 分组勾选 + 保存/取消。
fn render_edit_form(
    view: &mut ContactsView,
    groups: &[postium_core::infrastructure::storage::repository::contact_repo::ContactGroupWithCount],
    cx: &mut Context<ContactsView>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let is_new = view.editing == Some(EditTarget::New);

    let field = |key: &str, label: SharedString, input: &Entity<InputState>, hint: bool| -> AnyElement {
        v_flex()
            .id(format!("edit-field-{key}"))
            .gap_1()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .when(hint, |this| {
                        this.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.muted_foreground.opacity(0.8))
                                .child(t("contacts.commaHint")),
                        )
                    }),
            )
            .child(Input::new(input))
            .into_any_element()
    };

    let mut form = v_flex().gap_3().id("contact-edit-form");
    form = form
        .child(
            div()
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(if is_new { t("contacts.new") } else { t("contacts.edit") }),
        )
        .child(field("name", t("contacts.name"), &view.edit_name.clone(), false))
        .child(field("email", t("contacts.email"), &view.edit_email.clone(), false))
        .child(field("extra", t("contacts.extraEmails"), &view.edit_extra.clone(), true))
        .child(field("phones", t("contacts.phones"), &view.edit_phones.clone(), true))
        .child(field("org", t("contacts.org"), &view.edit_org.clone(), false))
        .child(field("notes", t("contacts.notes"), &view.edit_notes.clone(), false));

    // 分组勾选
    if !groups.is_empty() {
        let mut checks = v_flex().gap_1();
        for g in groups {
            let id = g.id;
            let checked = view.edit_groups.contains(&id);
            checks = checks.child(
                div()
                    .id(format!("edit-group-{id}"))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py(px(4.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.accent.opacity(0.08)))
                    .on_click(cx.listener(move |v: &mut ContactsView, e, w, cx| {
                        cx.stop_propagation();
                        v.toggle_edit_group(id, e, w, cx);
                    }))
                    // 勾选框
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(14.))
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(if checked { theme.accent } else { theme.border })
                            .when(checked, |d| {
                                d.bg(theme.accent).child(
                                    gpui_kit::component::Icon::new(IconName::Check)
                                        .size_3()
                                        .text_color(theme.accent_foreground),
                                )
                            }),
                    )
                    .child(div().text_size(px(12.)).child(g.name.clone())),
            );
        }
        form = form.child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t("contacts.groups")),
                )
                .child(checks),
        );
    }

    form.child(
        h_flex()
            .gap_2()
            .child(
                Button::new("edit-save")
                    .primary()
                    .xsmall()
                    .icon(IconName::Check)
                    .label(t("common.save"))
                    .on_click(cx.listener(ContactsView::save_edit)),
            )
            .child(
                Button::new("edit-cancel")
                    .ghost()
                    .xsmall()
                    .label(t("common.cancel"))
                    .on_click(cx.listener(ContactsView::cancel_edit)),
            ),
    )
    .id("contact-edit-scroll")
    .overflow_y_scroll()
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具
// ─────────────────────────────────────────────────────────────────────────────

/// 空串 → None（ContactWrite 的 Option 字段）。
fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// 逗号/分号/中文逗号分隔的列表输入 → 去空白项的 Vec。
fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';', '，', '；'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// 内容嗅探是否 vCard 文本：去 BOM（`\u{feff}`，CSV 导出带 BOM）与首部空白后，
/// 以 BEGIN:VCARD 开头（ASCII 大小写不敏感；字节前缀零拷贝比对，
/// 不做 `to_ascii_uppercase` 的整串 Unicode 拷贝）。
fn sniff_is_vcard(text: &str) -> bool {
    const PREFIX: &[u8] = b"BEGIN:VCARD";
    let head = text.trim_start_matches('\u{feff}').trim_start();
    head.len() >= PREFIX.len() && head.as_bytes()[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
}

// ── CardDAV 配置弹窗（复用设置页向导的 backdrop/ESC/焦点圈定模式）──────────

fn render_dav_modal(
    view: &mut ContactsView,
    _window: &mut Window,
    cx: &mut Context<ContactsView>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let dav = view.dav.as_ref().expect("render_dav_modal 无弹窗状态");

    // 上半：绑定列表
    let mut binding_rows = v_flex().gap_1();
    if dav.loading {
        binding_rows = binding_rows.child(
            h_flex()
                .items_center()
                .gap_2()
                .py_2()
                .child(
                    gpui_kit::component::Icon::new(IconName::LoaderCircle)
                        .size_4()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(t("email.loading")),
                ),
        );
    } else if dav.bindings.is_empty() {
        binding_rows = binding_rows.child(
            div()
                .py_2()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child(t("contacts.davNone")),
        );
    } else {
        for b in &dav.bindings {
            let last_sync = match b.last_sync_at {
                None => t("contacts.davNever").to_string(),
                Some(ts) => chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| ts.to_string()),
            };
            let last_sync_line = t_format("contacts.davLastSync", &[("time", &last_sync)]);
            let mut subtitle = b.url.clone();
            if let Some(href) = &b.addressbook_href {
                subtitle.push_str(&format!(" · {href}"));
            }
            subtitle.push_str(&format!(" · {last_sync_line}"));
            let account_id = b.account_id;
            binding_rows = binding_rows.child(
                h_flex()
                    .id(ElementId::Name(format!("dav-binding-{account_id}").into()))
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .px_2()
                    .py_1()
                    .bg(theme.muted)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(b.email.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        Button::new(ElementId::Name(format!("dav-del-{account_id}").into()))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Trash)
                            .on_click(cx.listener(move |v: &mut ContactsView, _, _, cx| {
                                v.dav_delete_binding(account_id, cx);
                            })),
                    ),
            );
        }
    }

    // 下半：新建表单（账号选择 + 地址/凭证 + 发现 + 通讯录单选 + 保存）
    let account_rows = dav.accounts.iter().map(|(id, email)| {
        let id = *id;
        let chosen = dav.account_id == Some(id);
        h_flex()
            .id(ElementId::Name(format!("dav-account-{id}").into()))
            .items_center()
            .gap_2()
            .rounded(px(6.))
            .px_2()
            .py_1()
            .cursor_pointer()
            .hover(|s| s.bg(theme.muted))
            .when(chosen, |d| d.bg(theme.accent.opacity(0.12)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |v: &mut ContactsView, _, _, cx| {
                v.dav_pick_account(id, cx);
            }))
            .child(
                gpui_kit::component::Icon::new(if chosen { IconName::CircleCheck } else { IconName::UserRound })
                    .size_3p5()
                    .text_color(if chosen { theme.accent } else { theme.muted_foreground }),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(if chosen { theme.accent } else { theme.foreground })
                    .truncate()
                    .child(email.clone()),
            )
    });

    let ab_rows = dav.addressbooks.iter().map(|(href, name)| {
        let href = href.clone();
        let chosen = dav.chosen_ab.as_deref() == Some(href.as_str());
        let label = if name.trim().is_empty() { href.clone() } else { format!("{name} ({href})") };
        h_flex()
            .id(ElementId::Name(format!("dav-ab-{}", href.replace('/', "_")).into()))
            .items_center()
            .gap_2()
            .rounded(px(6.))
            .px_2()
            .py_1()
            .cursor_pointer()
            .hover(|s| s.bg(theme.muted))
            .when(chosen, |d| d.bg(theme.accent.opacity(0.12)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |v: &mut ContactsView, _, _, cx| {
                v.dav_pick_addressbook(href.clone(), cx);
            }))
            .child(
                gpui_kit::component::Icon::new(if chosen { IconName::CircleCheck } else { IconName::Folder })
                    .size_3p5()
                    .text_color(if chosen { theme.accent } else { theme.muted_foreground }),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(if chosen { theme.accent } else { theme.foreground })
                    .truncate()
                    .child(label),
            )
    });

    let can_save = dav.account_id.is_some()
        && dav.chosen_ab.is_some()
        && !dav.url.read(cx).value().trim().is_empty()
        && !dav.username.read(cx).value().trim().is_empty()
        && !dav.password.read(cx).value().is_empty();

    let header = h_flex()
        .items_center()
        .px_5()
        .py_3()
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex_1()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .child(t("contacts.davTitle")),
        )
        .child(
            Button::new("dav-close")
                .ghost()
                .icon(IconName::X)
                .on_click(cx.listener(ContactsView::close_dav_modal)),
        );

    let card = v_flex()
        .id("dav-modal")
        .w(px(520.))
        .max_h(px(560.))
        .overflow_y_scroll()
        .rounded(px(12.))
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .child(header)
        .child(v_flex().px_5().py_3().gap_2().child(binding_rows))
        .child(div().border_t_1().border_color(theme.border))
        .child(
            v_flex()
                .px_5()
                .py_3()
                .gap_2()
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.muted_foreground)
                        .child(t("contacts.davAccount")),
                )
                .children(account_rows)
                .child(Input::new(&dav.url))
                .child(Input::new(&dav.username))
                .child(Input::new(&dav.password))
                .child(
                    Button::new("dav-discover")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Search)
                        .label(if dav.discovering {
                            t("contacts.davDiscovering")
                        } else {
                            t("contacts.davDiscover")
                        })
                        .loading(dav.discovering)
                        .on_click(cx.listener(ContactsView::dav_discover)),
                )
                .when(!dav.addressbooks.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child(t("contacts.davChoose")),
                    )
                    .children(ab_rows)
                })
                .when(!dav.error.trim().is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(11.5))
                            .text_color(gpui_kit::rgb(0xf43f5e))
                            .line_clamp(3)
                            .child(dav.error.clone()),
                    )
                })
                .child(
                    Button::new("dav-save")
                        .primary()
                        .xsmall()
                        .icon(IconName::Check)
                        .label(t("common.confirm"))
                        .loading(dav.saving)
                        .disabled(!can_save)
                        .on_click(cx.listener(ContactsView::dav_save)),
                ),
        );

    let modal_focus = view.dav_modal_focus.clone();
    gpui_kit::base::FocusTrapElement::focus_trap(
        div()
            .id("dav-backdrop")
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .track_focus(&modal_focus)
            .key_context("CardDavModal")
            .on_action(
                cx.listener(|v: &mut ContactsView, _: &gpui_kit::base::actions::Cancel, w, cx| {
                    if v.dav.is_some() {
                        v.close_dav_modal(&gpui_kit::ClickEvent::default(), w, cx);
                    }
                }),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .child(card),
        "dav-modal-trap",
        &modal_focus,
    )
}

#[cfg(test)]
mod tests {
    use super::sniff_is_vcard;

    #[test]
    fn sniff_detects_vcard_head_with_bom_and_case_insensitive() {
        assert!(sniff_is_vcard("BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD\r\n"));
        // 小写 begin:vcard（core 解析层同样按大小写不敏感识别）
        assert!(sniff_is_vcard("begin:vcard\r\nVERSION:3.0\r\n"));
        // UTF-8 BOM 前缀（导出产物带 BOM）不影响嗅探
        assert!(sniff_is_vcard("\u{feff}BEGIN:VCARD\r\n"));
        // 首部空白/换行后才是卡片头
        assert!(sniff_is_vcard("  \r\nBEGIN:VCARD\r\n"));
        // CSV / 空文本按 CSV 路径处理
        assert!(!sniff_is_vcard("name,email\r\nCarol,c@postium.test\r\n"));
        assert!(!sniff_is_vcard(""));
        // 前缀不完整不算 vCard
        assert!(!sniff_is_vcard("BEGIN:VCAR"));
    }
}
