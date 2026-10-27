//! 侧栏（移植自 src/lib/components/layout/Sidebar.svelte）：
//! 邮件分类（收件箱~垃圾邮件）/ 日历、工作流快捷入口 / 各账号文件夹、标签 /
//! 底部停靠回收站与设置入口。
//! 导航行为自定义行（未读数右对齐徽标），回收站与设置固定底部。

use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::service::email_service::EmailCategory;

use crate::i18n::t;
use crate::nav::{NavTarget, category_label_key, folder_label_key};
use crate::stores::accounts::AccountsStore;

/// 侧栏对外事件：导航目标变化。
#[derive(Clone, Debug)]
pub enum SidebarEvent {
    Navigate(NavTarget),
    /// 请求打开独立写信窗口
    Compose,
    /// 请求打开设置窗口
    Settings,
}

pub struct Sidebar {
    pub accounts: Entity<AccountsStore>,
    pub current: NavTarget,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    fn nav_row(
        &self,
        id: SharedString,
        icon: IconName,
        label: SharedString,
        unread: Option<usize>,
        target: NavTarget,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.current == target;
        let row_id = id.to_string();
        let mut row = div()
            .h(px(30.))
            .px_2()
            .rounded(cx.theme().radius)
            .when(selected, |d| d.bg(cx.theme().accent))
            .when(!selected, |d| {
                d.hover(|s| s.bg(cx.theme().accent.opacity(0.12)))
            })
            .id(row_id)
            .cursor_pointer()
            .on_click({
                let target = target.clone();
                cx.listener(move |this: &mut Self, _, _, cx| {
                    this.current = target.clone();
                    cx.emit(SidebarEvent::Navigate(target.clone()));
                    cx.notify();
                })
            })
            .child(
                h_flex()
                    .size_full()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_kit::component::Icon::new(icon)
                            .size_4()
                            .map(|i| {
                                if selected {
                                    i.text_color(cx.theme().accent_foreground)
                                } else {
                                    i.text_color(cx.theme().muted_foreground)
                                }
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .when(selected, |d| {
                                d.text_color(cx.theme().accent_foreground)
                            })
                            .line_clamp(1)
                            .child(label),
                    ),
            );

        // 未读徽标：右对齐，选中态反色
        if let Some(n) = unread.filter(|n| *n > 0) {
            row = row.child(
                div()
                    .ml_auto()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(8.))
                    .text_size(px(10.5))
                    .when(selected, |d| {
                        d.bg(gpui_kit::white().opacity(0.25))
                            .text_color(cx.theme().accent_foreground)
                    })
                    .when(!selected, |d| {
                        d.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .child(n.to_string()),
            );
        }
        row
    }
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = self.accounts.read(cx).groups.clone();
        let theme = cx.theme().clone();
        let store_groups = &groups;

        // nav_row 以宏形式内联（避免闭包双可变借用）
        macro_rules! nav {
            ($id:expr, $icon:expr, $label:expr, $unread:expr, $target:expr) => {
                self.nav_row($id.into(), $icon, $label, $unread, $target, cx)
            };
        }

        let mut scroll = v_flex()
            .gap_0p5()
            .flex_1()
            .min_h_0()
            .id("sidebar-scroll")
            .overflow_y_scroll()
            // 邮件分类：收件箱 ~ 垃圾邮件（回收站移至底部停靠）
            .child(nav!("nav-allinbox", IconName::Inbox, t("sidebar.inbox"), Some(total_unread_of(&groups)), NavTarget::AllInbox));

        // 虚拟分类（不含回收站）
        for (idx, (category, icon)) in [
            (EmailCategory::Starred, IconName::Star),
            (EmailCategory::Sent, IconName::Send),
            (EmailCategory::Drafts, IconName::FileText),
            (EmailCategory::Spam, IconName::Ban),
        ]
        .into_iter()
        .enumerate()
        {
            let category = &category;
            let unread: usize = store_groups
                .iter()
                .flat_map(|g| g.stats.iter())
                .filter(|s| category_matches_stat(category.clone(), s))
                .map(|s| s.unread)
                .sum();
            let label = t(category_label_key(category));
            scroll = scroll.child(nav!(
                &format!("nav-cat-{idx}"),
                icon,
                label,
                if unread > 0 { Some(unread) } else { None },
                NavTarget::Category(category.clone())
            ));
        }

        // 分隔线 → 快捷入口：日历 / 工作流
        scroll = scroll
            .child(divider(&theme))
            .child(nav!("nav-calendar", IconName::Calendar, t("sidebar.calendar"), None, NavTarget::Calendar))
            .child(nav!("nav-workflow", IconName::ListTodo, t("sidebar.workflow"), None, NavTarget::Workflow))
            .child(divider(&theme));

        // 各账号：账号名标题 + 文件夹行 + 标签行
        // 底部回收站未读数（TRASH/DELETED 文件夹合计）
        let trash_unread: usize = store_groups
            .iter()
            .flat_map(|g| g.stats.iter())
            .filter(|s| category_matches_stat(EmailCategory::Trash, s))
            .map(|s| s.unread)
            .sum();
        let trash_unread = if trash_unread > 0 { Some(trash_unread) } else { None };
        for (gi, group) in store_groups.iter().enumerate() {
            scroll = scroll.child(
                div()
                    .px_2()
                    .pt_3()
                    .pb_1()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .line_clamp(1)
                    .child(group.account.email.clone()),
            );
            for (fi, stat) in group.stats.iter().enumerate() {
                let label = match folder_label_key(&stat.folder) {
                    Some(key) => t(key),
                    None => stat.folder.clone().into(),
                };
                let icon = match stat.folder.to_ascii_uppercase().as_str() {
                    "INBOX" => IconName::Inbox,
                    f if f.contains("SENT") => IconName::Send,
                    f if f.contains("DRAFT") => IconName::FileText,
                    "JUNK" | "SPAM" => IconName::Ban,
                    f if f.contains("TRASH") || f.contains("DELETED") => IconName::Trash,
                    _ => IconName::Folder,
                };
                scroll = scroll.child(nav!(
                    &format!("nav-acc-{gi}-{fi}"),
                    icon,
                    label,
                    Some(stat.unread),
                    NavTarget::AccountFolder {
                        account_id: group.account.id,
                        folder: stat.folder.clone(),
                    }
                ));
            }
            for (li, label_dto) in group.labels.iter().enumerate() {
                scroll = scroll.child(nav!(
                    &format!("nav-lbl-{gi}-{li}"),
                    IconName::Tag,
                    label_dto.name.clone().into(),
                    None,
                    NavTarget::Label(label_dto.id)
                ));
            }
        }

        // 侧栏骨架：标题 + 写信按钮 + 滚动导航 + 固定底部设置
        v_flex()
            .w(px(220.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .p_2()
            .gap_1()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .child(t("app.name")),
            )
            .child({
                let on_compose = cx.listener(|_: &mut Self, _, _, cx| {
                    cx.emit(SidebarEvent::Compose);
                });
                crate::theme::compose_button(t("sidebar.compose"), on_compose)
            })
            .child(scroll)
            // 固定底部：回收站（停靠） + 设置
            .child(
                div().pt_1().mt_1().border_t_1().border_color(theme.border).child(
                    nav!("nav-trash", IconName::Trash, t(category_label_key(&EmailCategory::Trash)), trash_unread, NavTarget::Category(EmailCategory::Trash)),
                )
                .child(
                    Button::new("settings")
                        .ghost()
                        .w_full()
                        .justify_start()
                        .icon(IconName::Settings2)
                        .label(t("settings.title"))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(SidebarEvent::Settings);
                        })),
                ),
            )
    }
}

/// 侧栏分隔线。
fn divider(theme: &gpui_kit::component::Theme) -> gpui_kit::Div {
    div().flex_none().mx_1().my_1().h(px(1.)).bg(theme.border.opacity(0.7))
}

fn category_matches_stat(category: EmailCategory, stat: &postium_core::domain::sync::FolderStat) -> bool {
    let folder = stat.folder.to_ascii_uppercase();
    match category {
        EmailCategory::Inbox => folder == "INBOX",
        EmailCategory::Sent => folder.contains("SENT"),
        EmailCategory::Drafts => folder.contains("DRAFT"),
        EmailCategory::Spam => folder == "JUNK" || folder == "SPAM",
        EmailCategory::Trash => folder.contains("TRASH") || folder.contains("DELETED"),
        _ => false,
    }
}


/// 全部账号收件箱未读总数（render 内免借用计算）。
fn total_unread_of(groups: &[crate::stores::accounts::AccountNode]) -> usize {
    groups
        .iter()
        .flat_map(|g| g.stats.iter())
        .filter(|s| s.folder.eq_ignore_ascii_case("INBOX"))
        .map(|s| s.unread)
        .sum()
}
