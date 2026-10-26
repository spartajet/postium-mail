//! 侧栏（移植自 src/lib/components/layout/Sidebar.svelte）：
//! 快捷入口 / 全部收件箱 / 分类 / 各账号文件夹 / 标签 / 设置入口。

use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    v_flex,
};
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

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.accounts.read(cx);

        let nav = |target: NavTarget, icon: IconName, label: SharedString, unread: Option<usize>, this: &Self| {
            let selected = this.current == target;
            let display = match unread {
                Some(n) if n > 0 => format!("{label}  {n}").into(),
                _ => label,
            };
            let btn = Button::new(format!("nav-{}", display))
                .ghost()
                .w_full()
                .justify_start()
                .icon(icon)
                .label(display);
            let btn = if selected { btn.primary() } else { btn };
            btn.on_click({
                let target = target.clone();
                cx.listener(move |this: &mut Self, _, _, cx| {
                    this.current = target.clone();
                    cx.emit(SidebarEvent::Navigate(target.clone()));
                    cx.notify();
                })
            })
        };

        let mut content = v_flex()
            .w(px(220.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .p_2()
            .gap_1()
            .id("sidebar-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .px_2()
                    .py_2()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .child(t("app.name")),
            )
            // 快信入口
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
            // 快捷入口
            .child(nav(NavTarget::Calendar, IconName::Calendar, t("sidebar.calendar"), None, self))
            .child(nav(NavTarget::Workflow, IconName::Workflow, t("sidebar.workflow"), None, self))
            // 全部收件箱
            .child(nav(
                NavTarget::AllInbox,
                IconName::Inbox,
                t("sidebar.inbox"),
                Some(store.total_unread()),
                self,
            ));

        // 虚拟分类
        for (category, icon) in [
            (EmailCategory::Starred, IconName::Star),
            (EmailCategory::Sent, IconName::Send),
            (EmailCategory::Drafts, IconName::FileText),
            (EmailCategory::Spam, IconName::Ban),
            (EmailCategory::Trash, IconName::Trash),
        ] {
            let category = &category;
            let unread: usize = store
                .groups
                .iter()
                .flat_map(|g| g.stats.iter())
                .filter(|s| category_matches_stat(category.clone(), s))
                .map(|s| s.unread)
                .sum();
            content = content.child(nav(
                NavTarget::Category(category.clone()),
                icon,
                t(category_label_key(category)),
                if unread > 0 { Some(unread) } else { None },
                self,
            ));
        }

        // 各账号：账号名标题 + 文件夹行 + 标签行
        for group in &store.groups {
            content = content.child(
                div()
                    .px_2()
                    .pt_3()
                    .pb_1()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child(group.account.name.clone()),
            );
            for stat in &group.stats {
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
                content = content.child(nav(
                    NavTarget::AccountFolder {
                        account_id: group.account.id,
                        folder: stat.folder.clone(),
                    },
                    icon,
                    label,
                    Some(stat.unread),
                    self,
                ));
            }
            for label_dto in &group.labels {
                content = content.child(nav(
                    NavTarget::Label(label_dto.id),
                    IconName::Tag,
                    label_dto.name.clone().into(),
                    None,
                    self,
                ));
            }
        }

        // 底部：设置（Task 8 实装窗口，先导航占位）
        content = content.child(
            div().pt_2().child(
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
        );

        content
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
