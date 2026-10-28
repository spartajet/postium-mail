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
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{ContactDto, ContactWrite};

use crate::i18n::t;
use crate::runtime::spawn_tokio;
use crate::stores::contacts::{init_contacts_store, ContactsStore};

/// 详情卡编辑态的目标：新建（空白表单）或已有联系人。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditTarget {
    New,
    Existing(i32),
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

        Self {
            api,
            store,
            search_input,
            search_gen: 0,
            new_group_open: false,
            new_group_input,
            editing: None,
            edit_name,
            edit_email,
            edit_extra,
            edit_phones,
            edit_org,
            edit_notes,
            edit_groups: Vec::new(),
            status: " ".into(),
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
}

// ─────────────────────────────────────────────────────────────────────────────
// 渲染
// ─────────────────────────────────────────────────────────────────────────────

impl Render for ContactsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                rows = rows.child(render_contact_row(c, selected_id == Some(c.id), cx));
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
                            .text_color(gpui_kit::rgb(0xf43f5e))
                            .line_clamp(2)
                            .child(self.status.clone()),
                    )
                },
            );

        v_flex()
            .size_full()
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

/// 联系人行：首字头像 + 名称/邮箱 + 频次徽标 + 星标。
fn render_contact_row(
    c: &ContactDto,
    selected: bool,
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
