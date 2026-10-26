//! 侧栏（移植自 src/lib/components/layout/Sidebar.svelte）：
//! 快捷入口 / 全部收件箱 / 分类 / 各账号文件夹 / 标签 / 设置入口。
//! 导航行为自定义行（未读数右对齐徽标），设置固定底部。

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
            // 快捷入口
            .child(nav!("nav-calendar", IconName::Calendar, t("sidebar.calendar"), None, NavTarget::Calendar))
            .child(nav!("nav-workflow", IconName::ListTodo, t("sidebar.workflow"), None, NavTarget::Workflow))
            // 全部收件箱
            .child(nav!("nav-allinbox", IconName::Inbox, t("sidebar.inbox"), Some(total_unread_of(&groups)), NavTarget::AllInbox));

        // 虚拟分类
        for (idx, (category, icon)) in [
            (EmailCategory::Starred, IconName::Star),
            (EmailCategory::Sent, IconName::Send),
            (EmailCategory::Drafts, IconName::FileText),
            (EmailCategory::Spam, IconName::Ban),
            (EmailCategory::Trash, IconName::Trash),
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

        // 各账号：账号名标题 + 文件夹行 + 标签行
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
            .child(
                Button::new("compose")
                    .primary()
                    .w_full()
                    .icon(IconName::MailPlus)
                    .label(t("sidebar.compose"))
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(SidebarEvent::Compose);
                    })),
            )
            .child(scroll)
            // 固定底部：设置
            .child(
                div().pt_1().mt_1().border_t_1().border_color(theme.border).child(
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
